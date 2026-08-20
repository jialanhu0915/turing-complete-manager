//! `tc-mod-hook` — hook layer for the Turing Complete mod SDK.
//!
//! **Phase: M8-5 PoC** (2026-08-19).
//!
//! Provides primitives for inline-trampoline hooking on Windows x64. The
//! intended use is: build this crate as a `cdylib` (`tc-mod-hook.dll`),
//! inject that DLL into a running Turing Complete process, hijack
//! `compile.dll::compile` so mods can register callbacks that observe or
//! modify compile behavior.
//!
//! # Modules
//!
//! - [`trampoline`]: x64 inline JMP trampoline + variable patch sizes.
//! - [`mod_api`]:    mod callback API (`Mod` trait, `CompileCtx`, registry).
//! - [`logger_mod`]: example mod — hardcoded into the hook DLL for PoC.
//! - [`dll`] (cfg(windows)): `DllMain` for the `cdylib` build.
//!
//! # Safety
//!
//! All hook functions are `unsafe` because:
//! - They patch executable memory.
//! - Caller must guarantee the target function's first N bytes can be safely
//!   overwritten without crossing a branch instruction.
//! - `replacement` must have the same calling convention as `target` so
//!   registers / stack are compatible.
//!
//! See [`trampoline`] for detailed safety docs.

#![cfg_attr(not(windows), allow(dead_code))]

pub mod logger_mod;
pub mod mod_api;
pub mod trampoline;

#[cfg(windows)]
mod dll;

// Re-export the trampoline API at crate root for convenience.
pub use trampoline::{
    install_inline_hook, install_inline_hook_with_size, uninstall_inline_hook, OriginalBytes,
};

// Re-export the mod API surface.
pub use mod_api::{
    register_mod, registered_mod_names, run_post_compile_hooks, run_pre_compile_hooks,
    CompileCtx, Mod, ModAction,
};