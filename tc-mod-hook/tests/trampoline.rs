//! Smoke test: install a trampoline hook on a known function in our test
//! process, verify it intercepts calls, uninstall, verify restoration.
//!
//! This validates the trampoline technique in isolation. It does NOT test
//! against `compile.dll::compile` — that's a separate concern (DLL injection
//! into the game process) and is deferred to the next phase.
//!
//! ## Why one test (not two)
//!
//! The two functions (`target_fn`, `hook_fn`) are global symbols shared
//! across all tests in this binary. If two tests run in parallel, both
//! threads' `target_fn` calls would route through whichever `hook_fn`
//! install happened first, causing spurious failures. Combining into one
//! sequential test avoids the race entirely.

use std::cell::Cell;
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
    HOOK_CALL_COUNT.with(|c| c.set(c.get() + 1));
    HOOK_LAST_ARG.with(|c| c.set(x));
    0xDEAD_BEEF
}

// Thread-local counters. (We only have one thread here, but using
// thread_local keeps the API consistent with what a parallel-test setup
// would need.)
thread_local! {
    static HOOK_CALL_COUNT: Cell<u32> = const { Cell::new(0) };
    static HOOK_LAST_ARG: Cell<u32> = const { Cell::new(0) };
}

#[test]
fn trampoline_full_lifecycle() {
    let target = target_fn as *const () as usize;
    let replacement = hook_fn as *const () as usize;

    // ---- Phase 1: unhooked, target_fn returns x + 1. ----
    HOOK_CALL_COUNT.with(|c| c.set(0));
    HOOK_LAST_ARG.with(|c| c.set(0));
    assert_eq!(target_fn(5), 6, "baseline before hook");
    assert_eq!(HOOK_CALL_COUNT.with(|c| c.get()), 0, "hook not yet installed");

    // ---- Phase 2: install hook. ----
    let original = unsafe { install_inline_hook(target, replacement) }
        .expect("install should succeed");

    let r1 = target_fn(5);
    assert_eq!(r1, 0xDEAD_BEEF, "hook should have intercepted");
    assert_eq!(HOOK_CALL_COUNT.with(|c| c.get()), 1, "hook fired once");
    assert_eq!(HOOK_LAST_ARG.with(|c| c.get()), 5, "arg passed through");

    let r2 = target_fn(42);
    assert_eq!(r2, 0xDEAD_BEEF, "hook should still be installed");
    assert_eq!(HOOK_CALL_COUNT.with(|c| c.get()), 2, "hook fired twice");
    assert_eq!(HOOK_LAST_ARG.with(|c| c.get()), 42, "second arg passed through");

    // ---- Phase 3: uninstall — original behavior restored. ----
    unsafe { uninstall_inline_hook(target, &original) }
        .expect("uninstall should succeed");

    let r3 = target_fn(5);
    assert_eq!(r3, 6, "after uninstall, original behavior restored");
    assert_eq!(HOOK_CALL_COUNT.with(|c| c.get()), 2, "no extra hook fire");

    // ---- Phase 4: install/uninstall cycle. ----
    let orig2 = unsafe { install_inline_hook(target, replacement) }.unwrap();
    target_fn(100);
    target_fn(200);
    unsafe { uninstall_inline_hook(target, &orig2) }.unwrap();
    let count_after_round2 = HOOK_CALL_COUNT.with(|c| c.get());
    assert_eq!(
        count_after_round2, 4,
        "second install round fires hook twice (got {} expected 4)",
        count_after_round2
    );

    // ---- Phase 5: confirm post-second-uninstall still clean. ----
    let r4 = target_fn(5);
    assert_eq!(r4, 6, "after second uninstall, original behavior restored");
}