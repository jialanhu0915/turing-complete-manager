//! Standalone smoke-test executable.
//!
//! Useful for verifying the trampoline technique works without running
//! `cargo test` — e.g., inside a debugger, or as a sanity check after
//! changes.
//!
//! Run with: `cargo run --example smoke` (from `tc-mod-hook/`).

use std::sync::atomic::{AtomicU32, Ordering};
use tc_mod_hook::{install_inline_hook, uninstall_inline_hook};

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "system" fn target_fn(x: u32) -> u32 {
    x.wrapping_add(1)
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "system" fn hook_fn(x: u32) -> u32 {
    HOOK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    println!("[hook_fn] intercepted call with x = {}", x);
    0xDEAD_BEEF
}

static HOOK_CALL_COUNT: AtomicU32 = AtomicU32::new(0);

fn main() {
    let target = target_fn as *const () as usize;
    let replacement = hook_fn as *const () as usize;

    println!("== tc-mod-hook smoke test ==");
    println!();
    println!("Before hook:");
    println!("  target_fn(5)  = {}", target_fn(5));
    println!("  target_fn(10) = {}", target_fn(10));
    println!();

    let original = unsafe { install_inline_hook(target, replacement) }
        .expect("install should succeed");
    println!("[+] hook installed");
    println!();

    println!("After hook (calls should redirect):");
    println!("  target_fn(5)  = {:#x}", target_fn(5));
    println!("  target_fn(10) = {:#x}", target_fn(10));
    println!();

    unsafe { uninstall_inline_hook(target, &original) }
        .expect("uninstall should succeed");
    println!("[-] hook uninstalled");
    println!();

    println!("After uninstall (original behavior restored):");
    println!("  target_fn(5)  = {}", target_fn(5));
    println!("  target_fn(10) = {}", target_fn(10));
    println!();
    println!("hook fired {} times", HOOK_CALL_COUNT.load(Ordering::SeqCst));
}