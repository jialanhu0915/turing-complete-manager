//! `DllMain` for the cdylib build of `tc-mod-hook`.
//!
//! When the resulting `tc-mod_hook.dll` is loaded into a target process
//! (via `tc-mod-inject.exe` → `LoadLibraryW`), Windows calls this function.
//!
//! # Behavior (PoC)
//!
//! On `DLL_PROCESS_ATTACH`:
//! 1. Write a marker file at `%TEMP%\tc-mod-hook-<pid>.attached` so the
//!    injector / operator can confirm the DLL loaded successfully.
//! 2. Try to find `compile.dll` in this process via `GetModuleHandleW`.
//!    If found, resolve the `compile()` export via `GetProcAddress` and
//!    install an inline trampoline hook. The hook function logs every
//!    `compile()` invocation to a per-process log file.
//! 3. Log all of this to the marker file (so the operator sees the result
//!    without attaching a debugger).
//!
//! On `DLL_PROCESS_DETACH`: emit a debug log line. No uninstall — by the
//! time the DLL unloads, the target process is likely exiting anyway.
//!
//! # Limitations
//!
//! - The hook function does **not** call the original `compile()`. Real
//!   game behavior would break if `compile()` is invoked through the hook.
//!   This is acceptable for the PoC (we're proving the technique, not
//!   preserving game behavior). Real mod use will need a "trampoline-back"
//!   to the original bytes.
//! - File I/O happens under loader lock. `CreateFileW` is technically
//!   discouraged here; in practice it works for TEMP files. Real mod SDK
//!   would defer this work to a spawned thread.
//! - No mod callback registration yet — the hook is hardcoded to the local
//!   `compile_hook` function.
//!
//! # Safety
//!
//! `DllMain` runs under loader lock — only a subset of Win32 APIs are safe.
//! We call `OutputDebugStringW`, `GetModuleHandleW`, `GetProcAddress`,
//! `VirtualProtect`, and file APIs. The first three are explicitly safe;
//! the latter two are tolerated for the PoC but should be reviewed before
//! production.

#![cfg(windows)]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{HANDLE, HINSTANCE};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

use crate::trampoline::install_inline_hook;

/// Hook function installed on `compile.dll::compile`.
///
/// Matches the ABI reverse-engineered in
/// [`compile-signature.md`](../../../docs/10-investigation/compile-signature.md):
///
/// ```c
/// void compile(void* out_buf, void* src_str, int32_t mode, int32_t flags);
/// ```
///
/// Currently: logs each invocation to a per-process log file. Does NOT
/// call the original `compile()` — game behavior is broken by this hook,
/// which is acceptable for the PoC.
unsafe extern "system" fn compile_hook(
    out_buf: *mut core::ffi::c_void,
    src_str: *const core::ffi::c_void,
    mode: i32,
    flags: i32,
) {
    let pid = unsafe { GetCurrentProcessId() };
    let log_path = hook_log_path(pid);

    // src_str is a Nim string { int64 length, ptr char_data }.
    // Read length + pointer for a more useful log line.
    let (src_len, src_ptr) = if !src_str.is_null() {
        let len = unsafe { *(src_str as *const i64) };
        let ptr = unsafe { *((src_str as *const u8).add(8) as *const *const u8) };
        (len, ptr as usize)
    } else {
        (-1, 0)
    };

    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| {
            writeln!(
                f,
                "[ts={}] compile() hooked: out_buf={:p} src_str.len={} src_str.ptr=0x{:x} mode={} flags={}",
                unix_now(),
                out_buf,
                src_len,
                src_ptr,
                mode,
                flags
            )
        });
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            let pid = unsafe { GetCurrentProcessId() };
            let marker_path = marker_path(pid);

            // 1. Marker — proves injection succeeded.
            let _ = std::fs::write(
                &marker_path,
                format!(
                    "tc-mod-hook attached to PID {}\nloaded_at={}\n",
                    pid,
                    unix_now()
                ),
            );

            // 2. Try to find compile.dll in this process.
            let compile_dll_name = wide_null("compile.dll");
            let compile_handle = unsafe {
                GetModuleHandleW(windows::core::PCWSTR(compile_dll_name.as_ptr()))
            };
            let mut status_lines: Vec<String> = Vec::new();

            match compile_handle {
                Ok(handle) => {
                    let export_name = b"compile\0";
                    let compile_addr = unsafe {
                        GetProcAddress(handle, windows::core::PCSTR(export_name.as_ptr()))
                    };
                    match compile_addr {
                        Some(addr) => {
                            // 3. Install trampoline hook.
                            let install_result = unsafe {
                                install_inline_hook(
                                    addr as usize,
                                    compile_hook as *const () as usize,
                                )
                            };
                            match install_result {
                                Ok(_original) => {
                                    status_lines.push(format!(
                                        "hooked compile.dll::compile at 0x{:x} (original bytes saved for potential uninstall)",
                                        addr as usize
                                    ));
                                }
                                Err(e) => {
                                    status_lines.push(format!(
                                        "failed to install trampoline on compile() at 0x{:x}: {}",
                                        addr as usize, e
                                    ));
                                }
                            }
                        }
                        None => {
                            status_lines.push(
                                "compile.dll loaded but 'compile' export not found".into(),
                            );
                        }
                    }
                }
                Err(_) => {
                    status_lines.push(
                        "compile.dll NOT loaded in this process — no hook installed".into(),
                    );
                }
            }

            // 4. Append status to marker file.
            let status = format!(
                "status:\n{}\nlog_file: {}\n",
                status_lines.join("\n"),
                hook_log_path(pid).display()
            );
            let _ = OpenOptions::new()
                .append(true)
                .open(&marker_path)
                .and_then(|mut f| f.write_all(status.as_bytes()));

            debug_log(&format!(
                "tc-mod-hook: attached to PID {}, {}",
                pid,
                status_lines.join("; ")
            ));
        }
        DLL_PROCESS_DETACH => {
            debug_log("tc-mod-hook: DLL_PROCESS_DETACH (no uninstall performed)");
        }
        DLL_THREAD_ATTACH | DLL_THREAD_DETACH => {}
        _ => return 0,
    }
    1 // TRUE
}

// ---- helpers ---------------------------------------------------------------

/// Per-process marker file path. Proves DLL was loaded.
fn marker_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}.attached", pid))
}

/// Per-process compile-hook log path. One line per `compile()` invocation.
fn hook_log_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}-compile.log", pid))
}

fn temp_dir() -> PathBuf {
    std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows\\Temp"))
}

/// Encode `s` as UTF-16 with a NUL terminator.
fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Unix timestamp in seconds. Returns 0 if the clock is somehow before 1970.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Emit a UTF-8 debug string to the Windows debugger. Safe under loader lock.
#[cfg(windows)]
fn debug_log(s: &str) {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    unsafe { OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr())) };
}

// HANDLE kept in scope so the `use` isn't pruned by the compiler.
#[allow(dead_code)]
fn _typecheck(_: HANDLE) {}