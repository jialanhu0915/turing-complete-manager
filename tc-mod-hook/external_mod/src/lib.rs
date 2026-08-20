//! Example external mod for `tc-mod-hook`.
//!
//! Built as a `cdylib`. Drop the resulting `.dll` into
//! `%TEMP%\tc-mod-hook-mods\` and `tc-mod-hook`'s DllMain will:
//! 1. `LoadLibraryW` it
//! 2. `GetProcAddress` the exported `register_mods` symbol
//! 3. Call `register_mods()`
//!
//! `register_mods()` in turn looks up `tc_mod_hook_register_mod` (exported
//! by `tc_mod_hook.dll`) via `GetModuleHandleW` + `GetProcAddress` and calls
//! it with our pre/post callbacks + a pointer to our static state. From that
//! point on, every `compile()` in the game process fires our callbacks.
//!
//! See `dll.rs` in `tc-mod-hook` for the host-side registration API.

#![cfg(windows)]

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tc_mod_hook::CompileCtx;
use windows::core::PCSTR;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

/// Counter mod state — bumped every time our pre_compile callback fires.
/// Stored as a static so the callback (a free function) can update it.
static PRE_CALLS: AtomicU32 = AtomicU32::new(0);
static POST_CALLS: AtomicU32 = AtomicU32::new(0);

/// mod name shown in the SDK's registered_mods log.
const NAME: &[u8] = b"external-mod\0";

unsafe extern "C" fn my_pre(_user_data: *mut c_void, ctx: *const CompileCtx) -> i32 {
    PRE_CALLS.fetch_add(1, Ordering::SeqCst);
    let ctx = unsafe { &*ctx };
    let pid = ctx.pid;
    let seq = ctx.seq;
    let src_len = ctx.src_str().map(|s| s.len()).unwrap_or(0);

    let _ = writeln_to_log(
        pid,
        format_args!(
            "[external-mod][pre ] seq={} src_len={} mod={:?}",
            seq, src_len, std::str::from_utf8(NAME).unwrap_or("?")
        ),
    );
    0 // Continue
}

unsafe extern "C" fn my_post(_user_data: *mut c_void, ctx: *const CompileCtx) -> i32 {
    POST_CALLS.fetch_add(1, Ordering::SeqCst);
    let ctx = unsafe { &*ctx };
    let pid = ctx.pid;
    let seq = ctx.seq;
    let status = ctx.status;
    let mc_len = ctx.mc_len;
    let entry_off = ctx.entry_off;

    let _ = writeln_to_log(
        pid,
        format_args!(
            "[external-mod][post] seq={} status={} mc_len={} entry_off={}",
            seq, status, mc_len, entry_off
        ),
    );
    0 // Continue
}

/// Exported entry point. `tc-mod-hook`'s mod loader calls this via
/// `GetProcAddress` after `LoadLibraryW`.
#[no_mangle]
pub unsafe extern "C" fn register_mods() {
    // Find tc_mod_hook.dll (already loaded into the game process).
    let hook_name: Vec<u16> = "tc_mod_hook.dll\0".encode_utf16().collect();
    let hook_module = unsafe {
        GetModuleHandleW(windows::core::PCWSTR(hook_name.as_ptr()))
    };
    let hook_module = match hook_module {
        Ok(h) => h,
        Err(e) => {
            // Can't find host DLL — log and bail.
            eprintln!("[external_mod] GetModuleHandleW(tc_mod_hook.dll) failed: {}", e);
            return;
        }
    };

    // Resolve tc_mod_hook_register_mod.
    let proc_name = b"tc_mod_hook_register_mod\0";
    let proc = unsafe {
        GetProcAddress(hook_module, PCSTR(proc_name.as_ptr()))
    };
    let proc = match proc {
        Some(p) => p,
        None => {
            eprintln!("[external_mod] GetProcAddress(tc_mod_hook_register_mod) failed");
            return;
        }
    };

    // Cast to the expected C-ABI signature.
    type RegisterFn = unsafe extern "C" fn(
        PCSTR,
        Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
        Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
        *mut c_void,
    ) -> i32;
    let register: RegisterFn = unsafe { std::mem::transmute(proc) };

    let rc = unsafe { register(PCSTR(NAME.as_ptr()), Some(my_pre), Some(my_post), std::ptr::null_mut()) };

    if rc == 0 {
        eprintln!("[external_mod] registered with tc-mod-hook (pre/post hooks wired)");
    } else {
        eprintln!("[external_mod] registration failed (rc = {})", rc);
    }
}

// ---- logging helpers --------------------------------------------------------

fn writeln_to_log(pid: u32, args: std::fmt::Arguments) -> std::io::Result<()> {
    let path = log_path(pid);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", args)
}

fn log_path(pid: u32) -> PathBuf {
    std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows\\Temp"))
        .join("tc-mod-hook-mods")
        .join("external_mod")
        .join(format!("{}-compile.log", pid))
}

#[allow(dead_code)]
fn _ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}