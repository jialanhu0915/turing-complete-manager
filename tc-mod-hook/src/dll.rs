//! `DllMain` for the cdylib build of `tc-mod-hook`.
//!
//! When the resulting `tc_mod_hook.dll` is loaded into a target process
//! (via `tc-mod-inject.exe` → `LoadLibraryW`), Windows calls this function.
//!
//! # Behavior
//!
//! On `DLL_PROCESS_ATTACH`:
//! 1. Write a marker file at `%TEMP%\tc-mod-hook-<pid>.attached` so the
//!    injector / operator can confirm the DLL loaded successfully.
//! 2. Try to find `compile.dll` in this process via `GetModuleHandleW`.
//!    If found, resolve the `compile()` export via `GetProcAddress`.
//! 3. Install an inline trampoline hook on `compile()` so calls redirect
//!    to our `compile_hook`.
//! 4. Build a **trampoline-back** buffer: original 14 prologue bytes
//!    followed by an absolute JMP back to `compile() + 14`. This lets our
//!    hook call the *original* `compile()` so the game keeps working —
//!    the only difference is that we observe (and could modify) the call.
//! 5. Log everything to the marker file and an `OutputDebugStringW`.
//!
//! On every `compile()` call (via the hook):
//! 1. Log args (`out_buf` pointer, `src_str` length/pointer, mode, flags).
//! 2. Call original `compile()` via trampoline-back (game keeps working).
//! 3. Read out_buf's 5 fields (machine code length/pointer, entry offset,
//!    status, error pointer) and log them.
//!
//! On `DLL_PROCESS_DETACH`: just announce. No uninstall — by the time the
//! DLL unloads, the target process is likely exiting anyway.
//!
//! # Memory layout
//!
//! ```text
//! Address         │ Content
//! ────────────────┼───────────────────────────────────────────────────
//! compile.dll+0   │ [patched: JMP [RIP+0]] <hook_fn_addr>    (14 bytes)
//! trampoline_back │ [original 14 bytes] [JMP [RIP+0]] <compile+14>
//! ────────────────┴───────────────────────────────────────────────────
//! ```
//!
//! # Safety
//!
//! `DllMain` runs under loader lock. We call `OutputDebugStringW`,
//! `GetModuleHandleW`, `GetProcAddress`, `VirtualProtect`, `VirtualAlloc`,
//! and (during the marker file write) `OpenOptions::create().append()`.
//! All are tolerated under loader lock for this PoC; production mod SDK
//! would defer filesystem I/O to a spawned thread.

#![cfg(windows)]

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{HANDLE, HINSTANCE};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::VirtualAlloc;
use windows::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

use crate::trampoline::{install_inline_hook_with_size, OriginalBytes};

/// Address of the trampoline-back buffer (allocated in DllMain).
/// Set after hook install + trampoline-back setup. 0 = not set.
static TRAMPOLINE_BACK_ADDR: AtomicUsize = AtomicUsize::new(0);

/// Sequence number for dump files. Incremented per compile() invocation so
/// each dump has a unique filename even when the same level is recompiled.
static DUMP_SEQ: AtomicU32 = AtomicU32::new(0);

/// Hook function installed on `compile.dll::compile`. Receives the same
/// args as the original ABI:
///
/// ```c
/// void compile(void* out_buf, void* src_str, int32_t mode, int32_t flags);
/// ```
///
/// Calls the original via trampoline-back, then reads and logs the
/// output. Game behavior is preserved because the real compile() runs.
unsafe extern "system" fn compile_hook(
    out_buf: *mut c_void,
    src_str: *const c_void,
    mode: i32,
    flags: i32,
) {
    let pid = GetCurrentProcessId();
    let log_path = hook_log_path(pid);

    // ---- Pre: log args -----------------------------------------------------
    // src_str is NimStringV2: { i64 length, *mut [u8; 8..] payload_cap, payload[8..] }
    // We log the length + pointer but do NOT dump DSL bytes to disk — the
    // game ships its full DSL in `<install>/replay.nim` (79 MB, contains the
    // stdlib prefix + every level's SimulatorRequest). Dumping it again
    // would just duplicate that file.
    let (src_len, src_ptr) = if !src_str.is_null() {
        let len = *(src_str as *const i64);
        let ptr = *((src_str as *const u8).add(8) as *const *const u8);
        (len, ptr as usize)
    } else {
        (-1, 0)
    };

    let seq = DUMP_SEQ.fetch_add(1, Ordering::SeqCst);

    let _ = writeln_to_log(
        &log_path,
        format_args!(
            "[pre ] compile() called: out_buf={:p} src_str.len={} src_str.ptr=0x{:x} mode={} flags={}",
            out_buf, src_len, src_ptr, mode, flags
        ),
    );

    // ---- Mid: call original via trampoline-back ----------------------------
    let trampoline_addr = TRAMPOLINE_BACK_ADDR.load(Ordering::SeqCst);
    if trampoline_addr != 0 {
        let trampoline: unsafe extern "system" fn(*mut c_void, *const c_void, i32, i32) =
            std::mem::transmute(trampoline_addr);
        trampoline(out_buf, src_str, mode, flags);
    } else {
        // No trampoline-back — install failed earlier. Don't touch rax so
        // the caller at least sees *something* (best-effort: return out_buf
        // which is what compile() is documented to put in rax).
        // Note: this path means hook install succeeded but trampoline-back
        // setup didn't. The game will likely crash on out_buf use. Real
        // production code should uninstall the hook in this case.
        std::arch::asm!("mov rax, rcx", out("rax") _, in("rcx") out_buf);
    }

    // ---- Post: log out_buf state + dump machine code ----------------------
    // out_buf layout (per compile-signature.md):
    //   0..8   u64  machine_code_length
    //   8..16  u64  machine_code_ptr
    //   16..20 u32  entry_offset
    //   20..24 pad
    //   24..28 u32  status (0 = success, 13 = compiler error)
    //   28..32 pad
    //   32..40 u64  error_msg_ptr (valid when status != 0)
    unsafe {
        let base = out_buf as *const u8;
        let mc_len = *(base as *const u64);
        let mc_ptr = *((base as *const u64).add(1));
        let entry_off = *((base as *const u32).add(4));
        let status = *((base as *const u32).add(6));
        let err_ptr = *((base as *const u64).add(4));

        let _ = writeln_to_log(
            &log_path,
            format_args!(
                "[post] compile() returned: status={} mc_len={} mc_ptr=0x{:x} entry_off={} err_msg_ptr=0x{:x}",
                status, mc_len, mc_ptr, entry_off, err_ptr
            ),
        );

        // Dump JIT machine code to disk. Same 8 MiB cap as DSL.
        if status == 0 && mc_len > 0 && mc_len <= 8 * 1024 * 1024 && mc_ptr != 0 {
            let mc_slice = std::slice::from_raw_parts(mc_ptr as *const u8, mc_len as usize);
            let _ = std::fs::write(mc_dump_path(pid, seq), mc_slice);
        }

        // If status indicates failure and err_msg_ptr is set, dump first 200
        // bytes of the error message (Nim string layout: { i64 len, ptr chars }).
        if status != 0 && err_ptr != 0 {
            let err_len = *(err_ptr as *const i64);
            let err_data_ptr = *((err_ptr as *const u8).add(8) as *const *const u8);
            if err_data_ptr != std::ptr::null() && err_len > 0 && err_len < 4096 {
                let err_slice = std::slice::from_raw_parts(err_data_ptr, err_len.min(200) as usize);
                let err_str = String::from_utf8_lossy(err_slice);
                let _ = writeln_to_log(
                    &log_path,
                    format_args!("[err ] compile error ({} bytes): {}", err_len, err_str),
                );
            }
        }
    }
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

            // 1b. Pre-create dump dirs (std::fs::write doesn't create parents).
            let _ = std::fs::create_dir_all(temp_dir().join("tc-mod-hook-mc-dump"));

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
                            // 3. Install trampoline hook. compile.dll::compile has a
                            //    31-byte prologue (8 callee-saved pushes + sub $0x118 +
                            //    movaps xmm6 + pxor xmm0). We must patch all 31 bytes
                            //    so the trampoline-back runs the full prologue cleanly.
                            let patch_size = COMPILE_DLL_PROLOGUE_SIZE;
                            let install_result = unsafe {
                                install_inline_hook_with_size(
                                    addr as usize,
                                    compile_hook as *const () as usize,
                                    patch_size,
                                )
                            };
                            match install_result {
                                Ok(original_bytes) => {
                                    // 4. Build trampoline-back: original prologue + JMP back.
                                    let trampoline_addr = unsafe {
                                        build_trampoline_back(
                                            addr as usize,
                                            &original_bytes,
                                        )
                                    };
                                    if let Some(taddr) = trampoline_addr {
                                        TRAMPOLINE_BACK_ADDR.store(taddr, Ordering::SeqCst);
                                        status_lines.push(format!(
                                            "hooked compile.dll::compile at 0x{:x} (patched {} bytes, trampoline-back at 0x{:x})",
                                            addr as usize, patch_size, taddr
                                        ));
                                    } else {
                                        status_lines.push(format!(
                                            "hooked compile.dll::compile at 0x{:x} (patched {} bytes) BUT trampoline-back allocation failed",
                                            addr as usize, patch_size
                                        ));
                                    }
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

            // 5. Append status to marker file.
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

// ---- trampoline-back helpers ------------------------------------------------

/// compile.dll::compile prologue size, confirmed by disassembly:
///   12 bytes (8 callee-saved pushes) + 7 (sub $0x118) + 8 (movaps) + 4 (pxor)
///   = 31 bytes total, ending at offset 0x35f (the `mov (%rdx), %rbp` instruction).
///
/// Hard-coded because trampoline-back JMP target must land on an instruction
/// boundary. If the game updates and prologue changes, recompute this and
/// update the constant.
const COMPILE_DLL_PROLOGUE_SIZE: usize = 31;

/// Allocate RWX memory, copy original prologue bytes there, append an
/// absolute JMP back to `compile + prologue_size`. Returns the address
/// of the buffer.
///
/// Layout:
/// ```text
/// [0..N]            original compile() prologue bytes (N = patch size)
/// [N..N+6]          FF 25 00 00 00 00     ; JMP [RIP+0]
/// [N+6..N+14]       <8-byte absolute addr> ; target = compile + N
/// ```
unsafe fn build_trampoline_back(
    compile_addr: usize,
    original: &OriginalBytes,
) -> Option<usize> {
    use windows::Win32::System::Memory::{MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};

    const JMP_INSN: [u8; 6] = [0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];
    let prologue_size = original.0.len();
    let total_size = prologue_size + 14; // saved prologue + JMP

    let mem = VirtualAlloc(
        None,
        total_size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    );
    if mem.is_null() {
        return None;
    }
    let mem = mem as *mut u8;

    // Copy original prologue.
    std::ptr::copy_nonoverlapping(original.0.as_ptr(), mem, prologue_size);

    // Append JMP [RIP+0] + absolute target.
    std::ptr::copy_nonoverlapping(JMP_INSN.as_ptr(), mem.add(prologue_size), 6);
    let target_addr = (compile_addr + prologue_size) as u64;
    std::ptr::copy_nonoverlapping(&target_addr as *const u64 as *const u8, mem.add(prologue_size + 6), 8);

    Some(mem as usize)
}

// ---- logging helpers --------------------------------------------------------

fn writeln_to_log(path: &PathBuf, args: std::fmt::Arguments) -> std::io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "[ts={}] {}", unix_now(), args)
}

// ---- path helpers -----------------------------------------------------------

/// Per-process marker file path. Proves DLL was loaded.
fn marker_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}.attached", pid))
}

/// Per-process compile-hook log path. One line per `compile()` invocation
/// (pre + post).
fn hook_log_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}-compile.log", pid))
}

/// Per-invocation machine-code dump path. Captures the JIT output of
/// `compile()` so the operator can disassemble it (objdump / Ghidra).
fn mc_dump_path(pid: u32, seq: u32) -> PathBuf {
    temp_dir()
        .join("tc-mod-hook-mc-dump")
        .join(format!("{}-{}.bin", pid, seq))
}

fn temp_dir() -> PathBuf {
    std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows\\Temp"))
}

fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(windows)]
fn debug_log(s: &str) {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    unsafe { OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr())) };
}

#[allow(dead_code)]
fn _typecheck(_: HANDLE) {}

// Resolve a use so c_void import is not unused on this module.
#[allow(dead_code)]
const _: *const c_void = std::ptr::null::<c_void>();