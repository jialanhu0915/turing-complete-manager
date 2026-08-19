//! x64 inline-function trampoline hook.
//!
//! Technique: replace the first 14 bytes of a target function with an
//! absolute JMP that redirects to a replacement function. On `uninstall`,
//! the original 14 bytes are restored.
//!
//! # Patch layout (14 bytes)
//!
//! ```text
//!   FF 25 00 00 00 00       ; JMP [RIP+0]   (6 bytes)
//!   <8-byte absolute addr>  ; replacement    (8 bytes)
//! ```
//!
//! After the 6-byte `JMP [RIP+0]` instruction, RIP points at the next 8
//! bytes — those are the absolute address we want to jump to. Total: 14
//! bytes, replacing the function's first 14 bytes of prologue.
//!
//! # Why 14 bytes
//!
//! We need to overwrite enough bytes to ensure the patched code is reached
//! before any branch instruction in the original prologue. 14 bytes covers
//! a typical prologue (push + sub + movaps = ~12 bytes). For functions with
//! longer prologues (uncommon), this technique needs a longer patch (rare;
//! compile.dll::compile's prologue is ~12 bytes — safe).
//!
//! # Arguments and return value
//!
//! Because the patch is an unconditional JMP, the replacement function is
//! called with the **caller's** register / stack state — i.e., the same
//! arguments and calling convention as the original. As long as
//! `replacement` has the same signature as `target`, arguments pass through
//! transparently.
//!
//! # Safety
//!
//! Caller must guarantee:
//! - `target` is the address of a real function whose first 14 bytes can be
//!   safely overwritten (no crossing branch boundaries, no `endbr64` /
//!   indirect-branch markers in the first 14 bytes if CET is enabled).
//! - `replacement` is a function pointer with the **same signature** as
//!   `target` (so register-passed args and stack alignment match).
//! - No concurrent calls to `target` during install / uninstall. (Single
//!   thread of install; once installed, calls are atomic w.r.t. x64 fetch.)
//! - `target`'s memory page is not subsequently moved (no `LoadLibrary`
//!   unload / remap during hook lifetime).

use windows::Win32::System::Memory::{VirtualProtect, PAGE_PROTECTION_FLAGS};

/// 14 bytes of original function prologue, saved at install time. Pass
/// to [`uninstall_inline_hook`] to restore.
#[derive(Debug, Clone)]
pub struct OriginalBytes(pub [u8; 14]);

#[derive(Debug)]
pub enum HookError {
    /// `VirtualProtect` failed. Cause is typically insufficient privileges
    /// (e.g., targeting code in another process — out of scope for this PoC)
    /// or the target page is not valid.
    VirtualProtect(windows::core::Error),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::VirtualProtect(e) => write!(f, "VirtualProtect failed: {}", e),
        }
    }
}

impl std::error::Error for HookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HookError::VirtualProtect(e) => Some(e),
        }
    }
}

impl From<windows::core::Error> for HookError {
    fn from(e: windows::core::Error) -> Self {
        HookError::VirtualProtect(e)
    }
}

/// Patch `target`'s first 14 bytes with an absolute JMP to `replacement`.
///
/// # Returns
///
/// `OriginalBytes` containing the saved prologue — pass to
/// [`uninstall_inline_hook`] to restore.
///
/// # Safety
///
/// See module-level docs. Caller must satisfy the safety invariants.
pub unsafe fn install_inline_hook(
    target: usize,
    replacement: usize,
) -> Result<OriginalBytes, HookError> {
    const HOOK_SIZE: usize = 14;
    // JMP [RIP+0]  — 6 bytes. The 8 bytes immediately following hold the
    // absolute target address (resolved at install time).
    const JMP_INSN: [u8; 6] = [0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];

    // 1. Snapshot original bytes.
    let mut original = [0u8; HOOK_SIZE];
    std::ptr::copy_nonoverlapping(
        target as *const u8,
        original.as_mut_ptr(),
        HOOK_SIZE,
    );

    // 2. Build patch: JMP [RIP+0] + absolute address.
    let mut patch = [0u8; HOOK_SIZE];
    patch[..JMP_INSN.len()].copy_from_slice(&JMP_INSN);
    let replacement_bytes = (replacement as u64).to_le_bytes();
    patch[JMP_INSN.len()..HOOK_SIZE].copy_from_slice(&replacement_bytes);

    // 3. Make target page writable + executable.
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    VirtualProtect(
        target as *const _,
        HOOK_SIZE,
        PAGE_PROTECTION_FLAGS(0x40), // PAGE_EXECUTE_READWRITE
        &mut old_protect,
    )?;

    // 4. Overwrite target's first 14 bytes with the patch.
    std::ptr::copy_nonoverlapping(patch.as_ptr(), target as *mut u8, HOOK_SIZE);

    // 5. Restore original page protection.
    VirtualProtect(
        target as *const _,
        HOOK_SIZE,
        old_protect,
        &mut old_protect,
    )?;

    Ok(OriginalBytes(original))
}

/// Restore `target`'s original 14 bytes (undoing a prior install).
///
/// # Safety
///
/// `original` must come from a successful call to [`install_inline_hook`] on
/// the same `target` address (no other patches applied in between).
pub unsafe fn uninstall_inline_hook(
    target: usize,
    original: &OriginalBytes,
) -> Result<(), HookError> {
    const HOOK_SIZE: usize = 14;

    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    VirtualProtect(
        target as *const _,
        HOOK_SIZE,
        PAGE_PROTECTION_FLAGS(0x40),
        &mut old_protect,
    )?;

    std::ptr::copy_nonoverlapping(
        original.0.as_ptr(),
        target as *mut u8,
        HOOK_SIZE,
    );

    VirtualProtect(
        target as *const _,
        HOOK_SIZE,
        old_protect,
        &mut old_protect,
    )?;

    Ok(())
}