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
//! # What this PoC delivers
//!
//! - [`trampoline::install_inline_hook`] / [`trampoline::uninstall_inline_hook`]
//!   — pure 14-byte x64 JMP trampoline. Saves original bytes, redirects
//!   `target` → `replacement`, restores on uninstall.
//! - `DllMain` (built as cdylib) — loads cleanly and announces attach. Real
//!   hook installation (find `compile.dll`, resolve `compile`, install
//!   trampoline) is **not** in this PoC; it requires a config handoff
//!   mechanism (next phase).
//!
//! # What this PoC does NOT deliver (deferred)
//!
//! - DLL injection (`CreateRemoteThread` + `LoadLibrary`) — separate tool,
//!   next phase.
//! - Mod callback API (`Mod` trait, `PreCompile` / `PostCompile` pipeline) —
//!   design phase.
//! - Steam / anti-virus / Defender-false-positive mitigation — later.
//!
//! # Safety
//!
//! All hook functions are `unsafe` because:
//! - They patch executable memory.
//! - Caller must guarantee the target function's first 14 bytes can be safely
//!   overwritten without crossing a branch instruction.
//! - `replacement` must have the same calling convention as `target` so
//!   registers / stack are compatible.
//!
//! See [`trampoline`] for detailed safety docs.

#![cfg_attr(not(windows), allow(dead_code))]

// Suppress the benign "linker stdout: creating .lib" message that MSVC
// prints whenever we build a cdylib. This is informational only — the
// linker is reporting it produced the import-library file alongside the
// DLL, which is normal Windows behavior. Doesn't apply to non-Windows.
#![cfg_attr(windows, allow(linker_messages))]

pub mod trampoline;

#[cfg(windows)]
mod dll;

// Re-export the trampoline API at crate root for convenience.
pub use trampoline::{install_inline_hook, uninstall_inline_hook, HookError, OriginalBytes};