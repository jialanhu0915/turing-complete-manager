//! Example external mod for `tc-mod-hook`.
//!
// **Important**: This crate intentionally does NOT depend on `tc-mod-hook`
// in Cargo.toml. Mod authors who do create a hard dependency cause Windows
//! to load a duplicate of `tc_mod_hook.dll` (Cargo copies it to
//! `external_mod/target/release/deps/`), which creates two independent
//! module instances — each with its own static state. The mod and the
//! host's `MOD_REGISTRY` end up in different instances, so the mod's
//! registration never reaches the hook. The dedup manifest marker then
//! shows `registered mods: ["logger"]` only.
//!
//! **The correct pattern** (used here):
//! - Define local types with `#[repr(C)]` to match the host's layout
//! - Declare the foreign function with `extern "C"` raw FFI
//! - At runtime: `GetModuleHandleW("tc_mod_hook.dll")` + `GetProcAddress("tc_mod_hook_register_mod")` to
//!   find the host's function. This uses the host's loaded instance — not
//!   a duplicate.

#![cfg(windows)]

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::PCSTR;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

/// Mirror of `tc_mod_hook::CompileCtx`. The layout MUST match — we use
/// `#[repr(C)]` to ensure C-compatible field layout. This is what mod
/// authors do: declare the data shape, not the Rust type.
#[repr(C)]
pub struct CompileCtx {
    pub pid: u32,
    pub seq: u32,
    pub src_str_ptr: *const u8,
    pub mode: i32,
    pub flags: i32,
    pub out_buf: *mut u8,
    pub status: u32,
    pub mc_len: u64,
    pub mc_ptr_payload: *const u8,
    pub entry_off: u32,
}

/// C-ABI signature for `tc_mod_hook_register_mod`. Must match the host's
/// declaration in `tc-mod-hook/src/dll.rs`:
///   pub unsafe extern "C" fn tc_mod_hook_register_mod(
///       name: PCSTR,
///       pre_fn: Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
///       post_fn: Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
///       user_data: *mut c_void,
///   ) -> i32
type RegisterFn = unsafe extern "C" fn(
    PCSTR,
    Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
    Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,
    *mut core::ffi::c_void,
) -> i32;

/// Static counters for mod callbacks (per mod; useful for testing that
/// callbacks fire correctly).
static PRE_CALLS: AtomicU32 = AtomicU32::new(0);
static POST_CALLS: AtomicU32 = AtomicU32::new(0);

/// mod name shown in the SDK's registered_mods log.
const NAME: &[u8] = b"external-mod\0";

unsafe extern "C" fn my_pre(_user_data: *mut core::ffi::c_void, _ctx: *const CompileCtx) -> i32 {
    PRE_CALLS.fetch_add(1, Ordering::SeqCst);
    0 // Continue
}

unsafe extern "C" fn my_post(
    _user_data: *mut core::ffi::c_void,
    ctx: *const CompileCtx,
) -> i32 {
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
    // Find tc_mod_hook.dll (already loaded by inject.exe).
    // GetModuleHandleW looks up by name across all loaded modules, returning
    // the FIRST instance — typically the one inject.exe loaded directly.
    let hook_name: Vec<u16> = "tc_mod_hook.dll\0".encode_utf16().collect();
    let hook_module = unsafe {
        GetModuleHandleW(windows::core::PCWSTR(hook_name.as_ptr()))
    };
    let hook_module = match hook_module {
        Ok(h) => h,
        Err(_) => {
            // Host DLL not found — game probably doesn't have the hook
            // loaded. Silently fail (the mod just won't fire).
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
        None => return,
    };

    let register: RegisterFn = unsafe { std::mem::transmute(proc) };
    let _ = unsafe {
        register(
            PCSTR(NAME.as_ptr()),
            Some(my_pre),
            Some(my_post),
            std::ptr::null_mut(),
        )
    };
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