//! `DllMain` for the cdylib build of `tc-mod-hook`.
//!
//! When the resulting `tc_mod_hook.dll` is loaded into a target process
//! (via `tc-mod-inject.exe` → `LoadLibraryW`), Windows calls this function.
//!
//! # Behavior
//!
//! On `DLL_PROCESS_ATTACH`:
//! 1. Write a marker file at `%TEMP%\tc-mod-hook-<pid>.attached` so the
//!    operator can confirm the DLL loaded successfully.
//! 2. Pre-create dump directories (mc-dump).
//! 3. Try to find `compile.dll` in this process via `GetModuleHandleW`.
//!    If found, resolve the `compile()` export via `GetProcAddress`.
//! 4. Install an inline trampoline hook on `compile()` so calls redirect
//!    to our `compile_hook`.
//! 5. Build a **trampoline-back** buffer: original 31 prologue bytes +
//!    absolute JMP back to `compile() + 31`. This lets our hook call the
//!    *original* `compile()` so the game keeps working.
//! 6. Register built-in mods (`logger_mod`). Future phases will scan a
//!    `tc-mod-hook-mods/` directory and load external mod DLLs.
//! 7. Log everything to the marker file and an `OutputDebugStringW`.
//!
//! On every `compile()` call (via the hook):
//! 1. Build a `CompileCtx` (pre-state: src, mode, flags).
//! 2. Run all `pre_compile` mods.
//! 3. If mods say Continue, call original via trampoline-back.
//! 4. Update ctx with post-state (mc_len, mc_ptr, status, entry_off).
//! 5. Run all `post_compile` mods.
//! 6. Dump JIT MC to disk (for offline objdump analysis).
//! 7. Log [pre]/[post] lines to SDK log file (independent of mods).
//!
//! On `DLL_PROCESS_DETACH`: announce. No uninstall.

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
use windows::Win32::System::Memory::{
    VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};
use windows::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

use crate::logger_mod;
use crate::mod_api::{
    run_post_compile_hooks, run_pre_compile_hooks, CompileCtx, ModAction,
};
use crate::trampoline::{install_inline_hook_with_size, OriginalBytes};

/// Address of the trampoline-back buffer (allocated in DllMain).
/// Set after hook install + trampoline-back setup. 0 = not set.
static TRAMPOLINE_BACK_ADDR: AtomicUsize = AtomicUsize::new(0);

/// Sequence number for SDK log + MC dump filenames. Incremented per
/// `compile()` invocation so dumps don't collide.
static DUMP_SEQ: AtomicU32 = AtomicU32::new(0);

/// compile.dll::compile prologue size, confirmed by disassembly:
///   12 bytes (8 callee-saved pushes) + 7 (sub $0x118) + 8 (movaps) + 4 (pxor)
///   = 31 bytes total, ending at offset 0x35f (the `mov (%rdx),%rbp` instruction).
///
/// Hard-coded because trampoline-back JMP target must land on an instruction
/// boundary. If the game updates and prologue changes, recompute this and
/// update the constant.
const COMPILE_DLL_PROLOGUE_SIZE: usize = 31;

unsafe extern "system" fn compile_hook(
    out_buf: *mut c_void,
    src_str: *const c_void,
    mode: i32,
    flags: i32,
) {
    let pid = GetCurrentProcessId();
    let seq = DUMP_SEQ.fetch_add(1, Ordering::SeqCst);
    let log_path = hook_log_path(pid);

    // ---- Build pre-state ctx ---------------------------------------------
    // src_str is NimStringV2: { i64 length, *mut [u8; 8..] payload_cap, payload[8..] }
    let ctx = CompileCtx {
        pid,
        seq,
        src_str_ptr: src_str as *const u8,
        mode,
        flags,
        out_buf: out_buf as *mut u8,
        status: 0,
        mc_len: 0,
        mc_ptr_payload: std::ptr::null(),
        entry_off: 0,
    };

    // ---- SDK observability log (independent of mods) -------------------
    let (src_len, src_ptr_addr) = if !src_str.is_null() {
        let len = unsafe { *(src_str as *const i64) };
        let ptr = unsafe { *((src_str as *const u8).add(8) as *const *const u8) };
        (len, ptr as usize)
    } else {
        (-1, 0)
    };

    let _ = writeln_to_log(
        &log_path,
        format_args!(
            "[pre ] seq={} compile() called: out_buf={:p} src_str.len={} src_str.ptr=0x{:x} mode={} flags={}",
            seq, out_buf, src_len, src_ptr_addr, mode, flags
        ),
    );

    // ---- Run pre-compile mods ------------------------------------------
    let pre_action = run_pre_compile_hooks(&ctx);
    if !matches!(pre_action, ModAction::Continue) {
        // Abort (or future: ReplaceDsl). For now only Abort exists; bail.
        let _ = writeln_to_log(
            &log_path,
            format_args!(
                "[pre ] seq={} pre_compile hook returned Abort — skipping original compile()",
                seq
            ),
        );
        return;
    }

    // ---- Call original via trampoline-back -----------------------------
    let trampoline_addr = TRAMPOLINE_BACK_ADDR.load(Ordering::SeqCst);
    if trampoline_addr != 0 {
        let trampoline: unsafe extern "system" fn(*mut c_void, *const c_void, i32, i32) =
            std::mem::transmute(trampoline_addr);
        trampoline(out_buf, src_str, mode, flags);
    } else {
        // No trampoline-back — install failed earlier. Best-effort: set
        // rax = out_buf (compile() is documented to put out_buf in rax).
        std::arch::asm!("mov rax, rcx", out("rax") _, in("rcx") out_buf);
    }

    // ---- Read out_buf post-state ----------------------------------------
    // out_buf layout (per compile-signature.md):
    //   0..8   u64  machine_code_length
    //   8..16  u64  machine_code_ptr (NimStringV2; +8 = payload bytes)
    //   16..20 u32  entry_offset
    //   20..24 pad
    //   24..28 u32  status (0 = success, 13 = compiler error)
    //   28..32 pad
    //   32..40 u64  error_msg_ptr (valid when status != 0)
    let (status, mc_len, mc_ptr_addr, entry_off) = unsafe {
        let base = out_buf as *const u8;
        let mc_len = *(base as *const u64);
        let mc_ptr = *((base as *const u64).add(1));
        let entry_off = *((base as *const u32).add(4));
        let status = *((base as *const u32).add(6));
        (status, mc_len, mc_ptr, entry_off)
    };
    let mc_ptr_payload = mc_ptr_addr as *const u8;

    // ---- Build post-state ctx ------------------------------------------
    let mut ctx = ctx;
    ctx.status = status;
    ctx.mc_len = mc_len;
    ctx.mc_ptr_payload = mc_ptr_payload;
    ctx.entry_off = entry_off;

    // ---- SDK observability log (post) -----------------------------------
    let _ = writeln_to_log(
        &log_path,
        format_args!(
            "[post] seq={} compile() returned: status={} mc_len={} mc_ptr=0x{:x} entry_off={}",
            seq, status, mc_len, mc_ptr_payload as usize, entry_off
        ),
    );

    // ---- Dump JIT MC (for offline objdump / Ghidra analysis) -----------
    if status == 0 && mc_len > 0 && mc_len <= 8 * 1024 * 1024 && !mc_ptr_payload.is_null() {
        unsafe {
            let mc_slice = std::slice::from_raw_parts(mc_ptr_payload, mc_len as usize);
            let _ = std::fs::write(mc_dump_path(pid, seq), mc_slice);
        }
    }

    // ---- Log error message on failure (for SDK observability) ----------
    if status != 0 && !out_buf.is_null() {
        unsafe {
            let err_ptr = *((out_buf as *const u64).add(4));
            if err_ptr != 0 {
                let err_len = *(err_ptr as *const i64);
                let err_data_ptr = *((err_ptr as *const u8).add(8) as *const *const u8);
                if !err_data_ptr.is_null() && err_len > 0 && err_len < 4096 {
                    let err_slice =
                        std::slice::from_raw_parts(err_data_ptr, err_len.min(200) as usize);
                    let err_str = String::from_utf8_lossy(err_slice);
                    let _ = writeln_to_log(
                        &log_path,
                        format_args!("[err ] seq={} compile error ({} bytes): {}", seq, err_len, err_str),
                    );
                }
            }
        }
    }

    // ---- Run post-compile mods -----------------------------------------
    let _ = run_post_compile_hooks(&ctx);
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

            // 2. Pre-create dump dirs (std::fs::write doesn't create parents).
            let _ = std::fs::create_dir_all(temp_dir().join("tc-mod-hook-mc-dump"));

            // 3. Try to find compile.dll in this process.
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
                            // 4. Install trampoline hook (31-byte patch for compile.dll).
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
                                    // 5. Build trampoline-back.
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

            // 6. Register built-in mods. Phase 1: hardcoded logger. Phase 2:
            //    will scan tc-mod-hook-mods/ and LoadLibraryW each *.dll.
            logger_mod::register();
            let mods = crate::mod_api::registered_mod_names();
            status_lines.push(format!("registered mods: {:?}", mods));

            // 7. Append status to marker file.
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

/// Allocate RWX memory, copy original prologue bytes there, append an
/// absolute JMP back to `compile + prologue_size`. Returns the address
/// of the buffer.
unsafe fn build_trampoline_back(
    compile_addr: usize,
    original: &OriginalBytes,
) -> Option<usize> {
    const JMP_INSN: [u8; 6] = [0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];
    let prologue_size = original.0.len();
    let total_size = prologue_size + 14;

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

    std::ptr::copy_nonoverlapping(original.0.as_ptr(), mem, prologue_size);
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

fn marker_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}.attached", pid))
}

fn hook_log_path(pid: u32) -> PathBuf {
    temp_dir().join(format!("tc-mod-hook-{}-compile.log", pid))
}

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

const _: *const c_void = std::ptr::null::<c_void>();