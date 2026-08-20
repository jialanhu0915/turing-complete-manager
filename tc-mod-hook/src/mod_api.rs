//! `tc-mod-hook` mod callback API.
//!
//! Lets mod developers register callbacks that fire on each `compile()` call
//! from the game. The hook DLL iterates registered mods before/after calling
//! the original `compile()`.
//!
//! ## Lifetime / threading
//!
//! compile() may be called from any thread (compile.dll is not thread-safe —
//! the caller is responsible for external locking). The mod registry uses a
//! `Mutex<Vec<&'static dyn Mod>>` and individual mods are expected to use
//! interior mutability for any state.
//!
//! ## Mod registration
//!
//! Mods are registered via [`register_mod`]. The argument must be
//! `'static` (mod instances are typically declared as `static` items).
//!
//! ```ignore
//! // In your mod file:
//! struct MyMod;
//! impl Mod for MyMod { ... }
//! static MY_MOD: MyMod = MyMod;
//!
//! // Then somewhere reachable at DLL load time:
//! crate::mod_api::register_mod(&MY_MOD);
//! ```
//!
//! ## Mod loading
//!
//! In Phase 1 (this PoC), mods are compiled directly into the hook DLL —
//! `tc-mod-hook`'s `DllMain` calls each mod's `register()` function manually.
//! See `logger_mod::register()` for an example.
//!
//! In Phase 2 (later), the hook DLL will scan `%TEMP%\tc-mod-hook-mods\*.dll`,
//! `LoadLibraryW` each, and call an exported `register_mods()` function.
//!
//! ## What mods can do
//!
//! - **Observe**: read DSL source (`ctx.src_str()`), read MC info
//!   (`ctx.mc_len`, `ctx.mc_ptr`, `ctx.entry_off`, `ctx.status`).
//! - **Decide**: return [`ModAction::Continue`] (proceed normally) or
//!   [`ModAction::Abort`] (skip original call — game will see garbage r//).
//!
//! Phase 2 will add [`ModAction::ReplaceDsl`] to let mods substitute the
//! source string before the original compile() runs.

#![cfg(windows)]

use std::sync::Mutex;

/// What a mod sees on each compile() invocation. Lifetime is the duration of
/// the hook call (one compile() invocation). All pointers point into the
/// game process's address space — mods must not assume anything about the
/// target process layout.
pub struct CompileCtx {
    /// Process ID where compile() was called (for log correlation).
    pub pid: u32,
    /// Sequence number (0, 1, 2...) within this process. Unique per call.
    pub seq: u32,

    // -- Pre-compile state (set before trampoline-back runs) --------------

    /// `rcx` arg to compile(). Pointer to a NimStringV2 `{ i64 len, *u8 cap_ptr }`
    /// at offset 0 in game memory. Use [`Self::src_str`] to read the DSL.
    pub src_str_ptr: *const u8,
    /// `r8` arg to compile(). Low 32 bits only.
    pub mode: i32,
    /// `r9` arg to compile(). Low 32 bits only.
    pub flags: i32,

    // -- Post-compile state (filled AFTER original runs) -------------------

    /// `rcx` arg to compile(). Pointer to 40-byte output struct. Use the
    /// `mc_*` / `status` / `entry_off` fields below; don't read raw bytes
    /// unless you know the ABI.
    pub out_buf: *mut u8,
    /// out_buf[24..28] as u32 (0 = success, 13 = compile error).
    pub status: u32,
    /// out_buf[0..8] as u64 — length of JIT machine code.
    pub mc_len: u64,
    /// out_buf[8..16] as u64 — pointer to NimStringV2 holding the MC. Add 8
    /// to get the actual code bytes.
    pub mc_ptr_payload: *const u8,
    /// out_buf[16..20] as u32 — entry point offset within MC (relative to
    /// `mc_ptr_payload + 8`).
    pub entry_off: u32,
}

impl CompileCtx {
    /// Read the DSL source bytes for this compile() call. Returns None if
    /// src_str is null or has zero length.
    ///
    /// **Safety note**: the returned slice points into the GAME process's
    /// memory. If the mod stores it, it may become invalid after compile()
    /// returns. For PoC: just log or inspect immediately.
    pub fn src_str(&self) -> Option<&[u8]> {
        if self.src_str_ptr.is_null() {
            return None;
        }
        unsafe {
            let len = *(self.src_str_ptr as *const i64);
            let cap_ptr = *((self.src_str_ptr as *const u8).add(8) as *const *const u8);
            if cap_ptr.is_null() || len <= 0 || len > 8 * 1024 * 1024 {
                return None;
            }
            Some(std::slice::from_raw_parts(cap_ptr, len as usize))
        }
    }

    /// Read the JIT machine code bytes (after trampoline-back runs). Same
    /// lifetime caveat as `src_str()` — use immediately.
    pub fn machine_code(&self) -> Option<&[u8]> {
        if self.mc_ptr_payload.is_null() || self.mc_len == 0 || self.mc_len > 8 * 1024 * 1024 {
            return None;
        }
        unsafe {
            Some(std::slice::from_raw_parts(
                self.mc_ptr_payload,
                self.mc_len as usize,
            ))
        }
    }
}

/// What a mod returns from its callback.
#[derive(Debug, Clone)]
pub enum ModAction {
    /// Continue normally. If we're in `pre_compile`, call original compile()
    /// then run post hooks. If we're in `post_compile`, no further action.
    Continue,
    /// Abort this compile() — do NOT call original. Game will see whatever
    /// happens to be in rax (game will likely crash). For PoC / testing only.
    Abort,
}

/// Trait mod developers implement.
///
/// All methods take `&self` (mods are typically `static`). Use interior
/// mutability (`Mutex`, atomics) for any state mods need to track.
pub trait Mod: Send + Sync {
    /// Short name for logs (e.g., "logger", "auto-save", "anti-cheat").
    fn name(&self) -> &str;

    /// Called BEFORE the original compile() runs. Default: pass-through.
    ///
    /// `ctx.src_str_ptr`, `ctx.mode`, `ctx.flags` are populated. The
    /// `mc_*` / `status` fields are zeroed — they fill in after original runs.
    fn pre_compile(&self, _ctx: &CompileCtx) -> ModAction {
        ModAction::Continue
    }

    /// Called AFTER the original compile() runs successfully (or fails —
    /// `ctx.status` tells you). Default: pass-through.
    ///
    /// All fields are populated. Use `ctx.machine_code()` to read MC.
    fn post_compile(&self, _ctx: &CompileCtx) -> ModAction {
        ModAction::Continue
    }
}

static MOD_REGISTRY: Mutex<Vec<&'static dyn Mod>> = Mutex::new(Vec::new());

/// Register a mod. Must be called at DLL load time (DllMain or constructor).
///
/// The `&'static` bound lets the registry hold raw references without
/// refcount overhead. Mods are typically declared as `static` items.
pub fn register_mod(m: &'static dyn Mod) {
    MOD_REGISTRY.lock().expect("mod registry poisoned").push(m);
}

/// Run all registered mods' `pre_compile` hooks. Returns the first
/// non-Continue action (or Continue if all pass).
pub fn run_pre_compile_hooks(ctx: &CompileCtx) -> ModAction {
    let mods = MOD_REGISTRY.lock().expect("mod registry poisoned");
    for m in mods.iter() {
        match m.pre_compile(ctx) {
            ModAction::Continue => continue,
            other => return other,
        }
    }
    ModAction::Continue
}

/// Run all registered mods' `post_compile` hooks. Returns the first
/// non-Continue action (or Continue if all pass).
pub fn run_post_compile_hooks(ctx: &CompileCtx) -> ModAction {
    let mods = MOD_REGISTRY.lock().expect("mod registry poisoned");
    for m in mods.iter() {
        match m.post_compile(ctx) {
            ModAction::Continue => continue,
            other => return other,
        }
    }
    ModAction::Continue
}

/// Snapshot of registered mod names — for diagnostics / debugging.
pub fn registered_mod_names() -> Vec<String> {
    MOD_REGISTRY
        .lock()
        .expect("mod registry poisoned")
        .iter()
        .map(|m| m.name().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingMod {
        name: String,
        pre_calls: &'static AtomicU32,
        post_calls: &'static AtomicU32,
    }
    impl Mod for CountingMod {
        fn name(&self) -> &str {
            &self.name
        }
        fn pre_compile(&self, _ctx: &CompileCtx) -> ModAction {
            self.pre_calls.fetch_add(1, Ordering::SeqCst);
            ModAction::Continue
        }
        fn post_compile(&self, _ctx: &CompileCtx) -> ModAction {
            self.post_calls.fetch_add(1, Ordering::SeqCst);
            ModAction::Continue
        }
    }

    #[test]
    fn registry_iterates_in_registration_order() {
        static PRE_A: AtomicU32 = AtomicU32::new(0);
        static POST_A: AtomicU32 = AtomicU32::new(0);
        static PRE_B: AtomicU32 = AtomicU32::new(0);
        static POST_B: AtomicU32 = AtomicU32::new(0);

        let mod_a = CountingMod {
            name: "A".into(),
            pre_calls: &PRE_A,
            post_calls: &POST_A,
        };
        let mod_b = CountingMod {
            name: "B".into(),
            pre_calls: &PRE_B,
            post_calls: &POST_B,
        };

        // Use Box::leak to get 'static references (CountingMod isn't Copy,
        // can't have a true `static` of it without const-init support).
        let a_box: &'static CountingMod = Box::leak(Box::new(mod_a));
        let b_box: &'static CountingMod = Box::leak(Box::new(mod_b));

        register_mod(a_box);
        register_mod(b_box);

        let names = registered_mod_names();
        assert_eq!(names, vec!["A", "B"]);

        // Build a stub context (all zeros / null is fine for these methods
        // because CountingMod doesn't dereference them).
        let ctx = CompileCtx {
            pid: 0,
            seq: 0,
            src_str_ptr: std::ptr::null(),
            mode: 0,
            flags: 0,
            out_buf: std::ptr::null_mut(),
            status: 0,
            mc_len: 0,
            mc_ptr_payload: std::ptr::null(),
            entry_off: 0,
        };

        let _ = run_pre_compile_hooks(&ctx);
        let _ = run_post_compile_hooks(&ctx);

        assert_eq!(PRE_A.load(Ordering::SeqCst), 1);
        assert_eq!(PRE_B.load(Ordering::SeqCst), 1);
        assert_eq!(POST_A.load(Ordering::SeqCst), 1);
        assert_eq!(POST_B.load(Ordering::SeqCst), 1);
    }
}