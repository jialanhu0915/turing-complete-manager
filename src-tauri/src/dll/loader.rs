//! Safe wrapper around `libloading::Library` for `sim-shim.dll`.
//!
//! The shim is loaded lazily and cached for the process lifetime. `NimMain`
//! is invoked exactly once via `OnceLock`.

use libloading::{Library, Symbol};
use std::path::Path;
use std::sync::OnceLock;

/// C-ABI signatures exported by shim.dll.
///
/// All functions use the default Windows x64 cdecl-equivalent ABI.
type TccNimMain = unsafe extern "C" fn() -> i32;
type TccVersion = unsafe extern "C" fn() -> i32;
type TccCompile = unsafe extern "C" fn(
    out_buf: *mut u8,          // 40-byte result struct (caller-allocated)
    src: *const std::ffi::c_char, // DSL source as NUL-terminated cstring
    mode: i32,
    flags: i32,
) -> i32;

/// Wrapped shim.dll + cached symbol pointers.
///
/// `Shim::load()` is the only entry point; the underlying `Library` is kept
/// alive for the lifetime of the `Shim`.
pub struct Shim {
    _lib: Library,
    nim_main: TccNimMain,
    compile: TccCompile,
    version: TccVersion,
}

static SHIM: OnceLock<Result<Shim, String>> = OnceLock::new();

/// Get the cached shim, loading on first call. Returns an error if shim.dll
/// is not present or cannot be loaded.
pub fn shim() -> Result<&'static Shim, String> {
    SHIM.get_or_init(load).as_ref().map_err(|e| e.clone())
}

fn load() -> Result<Shim, String> {
    let path = shim_dll_path();
    if !path.exists() {
        return Err(format!("DLL_NOT_FOUND|{}", path.display()));
    }
    // SAFETY: shim.dll is a valid PE; we immediately resolve symbols below.
    let lib = unsafe { Library::new(&path) }
        .map_err(|e| format!("DLL_LOAD_FAILED|{}|{e}", path.display()))?;

    // SAFETY: symbols are exported by shim.dll (see sim-shim/shim.nim).
    // `into_raw()` returns a `*mut std::ffi::c_void` valid while `lib` is alive.
    // We extract typed fn pointers via transmute, then can move `lib`.
    let nim_main_ptr = unsafe {
        lib.get::<TccNimMain>(b"tccNimMain\0")
            .map_err(|e| format!("DLL_SYMBOL|tccNimMain|{e}"))?
            .into_raw()
    };
    let compile_ptr = unsafe {
        lib.get::<TccCompile>(b"tccCompile\0")
            .map_err(|e| format!("DLL_SYMBOL|tccCompile|{e}"))?
            .into_raw()
    };
    let version_ptr = unsafe {
        lib.get::<TccVersion>(b"tccVersion\0")
            .map_err(|e| format!("DLL_SYMBOL|tccVersion|{e}"))?
            .into_raw()
    };

    // SAFETY: pointers above came from `lib` and stay valid while `lib` is alive.
    let nim_main: TccNimMain = unsafe { std::mem::transmute(nim_main_ptr) };
    let compile: TccCompile = unsafe { std::mem::transmute(compile_ptr) };
    let version: TccVersion = unsafe { std::mem::transmute(version_ptr) };

    // Probe version (also forces compile.dll to be loaded transitively).
    let v = unsafe { version() };
    if v != 1 {
        return Err(format!("DLL_VERSION|got {v}, expected 1"));
    }

    // NOTE: We do NOT call NimMain here. Calling it eagerly interferes with
    // compile.dll's internal module-init order — `reset_globals` asserts
    // `source_buffer.len == 0`, which fails if global state has been touched
    // out of order. compile() invokes NimMain itself when needed.
    // (Verified 2026-08-08 via cargo test dll::runtime.)

    Ok(Shim {
        _lib: lib,
        nim_main,
        compile,
        version,
    })
}

impl Shim {
    /// Forward to compile.dll::compile via the shim.
    ///
    /// # Safety
    ///
    /// Caller must ensure `out_buf` is at least 40 bytes, and `src` is a
    /// valid NUL-terminated C string (the shim converts it to a Nim string).
    /// See `docs/10-investigation/compile-signature.md`.
    pub unsafe fn compile(
        &self,
        out_buf: *mut u8,
        src: *const std::ffi::c_char,
        mode: i32,
        flags: i32,
    ) -> i32 {
        (self.compile)(out_buf, src, mode, flags)
    }

    /// Forward to compile.dll::NimMain. Already called once during shim load;
    /// exposed for tests / re-init scenarios.
    ///
    /// # Safety
    ///
    /// Must be called from the same thread that holds the shim, and not
    /// concurrently with `compile()`.
    pub unsafe fn nim_main(&self) -> i32 {
        (self.nim_main)()
    }

    /// Version probe. Returns 1 if shim.dll loaded successfully.
    pub fn version(&self) -> i32 {
        unsafe { (self.version)() }
    }
}

/// Path to `sim-shim.dll`. Search order:
/// 1. `<CARGO_MANIFEST_DIR>/../sim-shim/shim.dll` (dev tree)
/// 2. `sim-shim/shim.dll` relative to CWD (when running tests)
/// 3. `<exe_dir>/shim.dll` (installed MSI bundles shim.dll next to verify.exe)
fn shim_dll_path() -> std::path::PathBuf {
    // 1. dev tree (src-tauri/../sim-shim/shim.dll)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dev_path = Path::new(manifest_dir).join("..").join("sim-shim").join("shim.dll");
    if dev_path.exists() {
        return dev_path;
    }

    // 2. cwd-relative (cargo test runs from src-tauri/)
    let cwd_path = Path::new("sim-shim").join("shim.dll");
    if cwd_path.exists() {
        return cwd_path;
    }

    // 3. installed layout (verify.exe / tauri exe sit in INSTALLDIR with shim.dll)
    if let Ok(exe) = std::env::current_exe() {
        let installed = exe.with_file_name("shim.dll");
        if installed.exists() {
            return installed;
        }
    }

    // Fall back to the cwd-relative path even if it doesn't exist, so the
    // error message names the place we tried.
    cwd_path
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: verify shim.dll loads and version() returns 1.
    /// Skips if shim.dll not built (CI without game dev environment).
    #[test]
    fn shim_loads_and_version_is_one() {
        let path = shim_dll_path();
        if !path.exists() {
            eprintln!("SKIP: shim.dll not at {}", path.display());
            return;
        }
        match shim() {
            Ok(s) => {
                assert_eq!(s.version(), 1, "tccVersion must return 1");
            }
            Err(e) => panic!("shim load failed: {e}"),
        }
    }
}