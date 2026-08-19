//! `DllMain` for the cdylib build of `tc-mod-hook`.
//!
//! When the resulting `tc-mod-hook.dll` is loaded into a target process
//! (e.g., via `CreateRemoteThread` + `LoadLibrary` from an injector tool,
//! or via the `AppInit_DLLs` registry value), Windows calls this function.
//!
//! # PoC behavior
//!
//! For the PoC, this is intentionally minimal:
//! - On `DLL_PROCESS_ATTACH`, emit an `OutputDebugStringW` announcement so
//!   the load can be observed via DebugView / VS Code debugger output.
//! - **No hook installation is performed.** Real hook install requires:
//!   1. A config handoff mechanism (env var, named pipe, file in temp dir,
//!      or just a hardcoded target list).
//!   2. Resolving `compile.dll`'s base address (e.g., `GetModuleHandleW` on
//!      the game process — but we're inside the game process, so `GetModuleHandleW(NULL)`
//!      + enumeration, or via PEB walking).
//!   3. Resolving `compile()`'s export address (e.g., `GetProcAddress`).
//!   4. Installing the trampoline via [`crate::trampoline`].
//!
//!   Steps 2-4 are deferred to the next phase — they require a way to pass
//!   target-process configuration into this DLL at load time.
//!
//! # Safety
//!
//! `DllMain` runs under loader lock — many Win32 APIs are off-limits here.
//! We only call `OutputDebugStringW`, which is safe under loader lock.

#![cfg(windows)]

use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            // Future: invoke hook install logic here (find compile.dll, resolve
            // compile() export, install trampoline, register mod callbacks).
            // For PoC we just announce so DebugView shows the load.
            debug_log("tc-mod-hook: DLL_PROCESS_ATTACH (PoC build, no hooks installed)\n");
        }
        DLL_PROCESS_DETACH => {
            debug_log("tc-mod-hook: DLL_PROCESS_DETACH\n");
        }
        DLL_THREAD_ATTACH | DLL_THREAD_DETACH => {}
        _ => return 0,
    }
    1 // TRUE — DLL load succeeded
}

/// Emit a UTF-8 debug string to the Windows debugger. Visible in DebugView,
/// VS Code's debug console, or any attached debugger. Safe under loader lock.
#[cfg(windows)]
fn debug_log(s: &str) {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0); // NUL terminator (OutputDebugStringW expects LPWSTR).
    unsafe { OutputDebugStringW(windows::core::PCWSTR(wide.as_ptr())) };
}