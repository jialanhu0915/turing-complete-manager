//! Smoke test: install a trampoline hook on a known function in our test
//! process, verify it intercepts calls, uninstall, verify restoration.
//!
//! This validates the trampoline technique in isolation. It does NOT test
//! against `compile.dll::compile` — that's a separate concern (DLL injection
//! into the game process) and is deferred to the next phase.

use std::sync::atomic::{AtomicU32, Ordering};
use tc_mod_hook::{install_inline_hook, uninstall_inline_hook};

// `#[inline(never)]` keeps `target_fn` as a real separate function symbol,
// so the trampoline has something to overwrite. Without it, the compiler
// might inline `target_fn(5)` directly at the call site, leaving the symbol
// unused and the trampoline pointing to dead code.

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "system" fn target_fn(x: u32) -> u32 {
    x.wrapping_add(1)
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "system" fn hook_fn(x: u32) -> u32 {
    HOOK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    HOOK_LAST_ARG.store(x, Ordering::SeqCst);
    0xDEAD_BEEF
}

static HOOK_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
static HOOK_LAST_ARG: AtomicU32 = AtomicU32::new(0);

#[test]
fn trampoline_intercepts_and_restores() {
    let target = target_fn as *const () as usize;
    let replacement = hook_fn as *const () as usize;

    // Reset state (in case tests run in shared process).
    HOOK_CALL_COUNT.store(0, Ordering::SeqCst);
    HOOK_LAST_ARG.store(0, Ordering::SeqCst);

    // --- Phase 1: unhooked, target_fn returns x + 1.
    assert_eq!(target_fn(5), 6, "baseline before hook");
    assert_eq!(HOOK_CALL_COUNT.load(Ordering::SeqCst), 0, "hook not yet installed");

    // --- Phase 2: install hook.
    let original = unsafe { install_inline_hook(target, replacement) }
        .expect("install should succeed");

    // After install, calls to target_fn redirect to hook_fn.
    let r1 = target_fn(5);
    assert_eq!(r1, 0xDEAD_BEEF, "hook should have intercepted");
    assert_eq!(HOOK_CALL_COUNT.load(Ordering::SeqCst), 1, "hook fired once");
    assert_eq!(HOOK_LAST_ARG.load(Ordering::SeqCst), 5, "arg passed through");

    // Call again — arg should change.
    let r2 = target_fn(42);
    assert_eq!(r2, 0xDEAD_BEEF, "hook should still be installed");
    assert_eq!(HOOK_CALL_COUNT.load(Ordering::SeqCst), 2, "hook fired twice");
    assert_eq!(HOOK_LAST_ARG.load(Ordering::SeqCst), 42, "second arg passed through");

    // --- Phase 3: uninstall — original behavior restored.
    unsafe { uninstall_inline_hook(target, &original) }
        .expect("uninstall should succeed");

    let r3 = target_fn(5);
    assert_eq!(r3, 6, "after uninstall, original behavior restored");
    // Hook may have fired one last time during uninstall? No — uninstall
    // writes back original bytes; subsequent calls don't go through the
    // hook. HOOK_CALL_COUNT should still be 2.
    assert_eq!(HOOK_CALL_COUNT.load(Ordering::SeqCst), 2, "no extra hook fire");
}

#[test]
fn install_uninstall_install_cycle() {
    // Verify hook can be reinstalled after uninstall without issues.
    let target = target_fn as *const () as usize;
    let replacement = hook_fn as *const () as usize;

    HOOK_CALL_COUNT.store(0, Ordering::SeqCst);

    // Round 1
    let orig1 = unsafe { install_inline_hook(target, replacement) }.unwrap();
    target_fn(1);
    target_fn(2);
    unsafe { uninstall_inline_hook(target, &orig1) }.unwrap();
    let count_after_round1 = HOOK_CALL_COUNT.load(Ordering::SeqCst);
    assert_eq!(count_after_round1, 2);

    // Round 2 — must still work.
    let orig2 = unsafe { install_inline_hook(target, replacement) }.unwrap();
    target_fn(10);
    target_fn(20);
    unsafe { uninstall_inline_hook(target, &orig2) }.unwrap();
    let count_after_round2 = HOOK_CALL_COUNT.load(Ordering::SeqCst);
    assert_eq!(count_after_round2, count_after_round1 + 2, "second install round fires hook twice");
}