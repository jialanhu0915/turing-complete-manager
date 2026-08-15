//! Output struct layout for `compile.dll::compile`.
//!
//! `compile()` writes 40 bytes (5 × 8) to the caller-provided buffer.
//! Field semantics are partially known — see `docs/10-investigation/compile-signature.md`.
//!
//! ## Field map (from Phase A reverse engineering)
//!
//! | Offset | Field            | Inferred meaning          |
//! |--------|------------------|---------------------------|
//! | 0x00   | `machine_code`   | ptr to JIT bytes (Nim seq data) |
//! | 0x08   | `mode_or_len`    | input mode, or machine_code length |
//! | 0x10   | `error_flag`     | low 32-bit status enum |
//! | 0x18   | `error_info`     | error context / message ptr |
//! | 0x20   | `fn_ptr`         | **JIT function pointer** (callable) |
//!
//! The semantics are NOT fully verified; this is a best-effort mapping from
//! register usage at compile()'s epilogue. Field names will be refined once
//! the function-pointer call convention is determined.

use std::ptr::NonNull;

/// 40-byte compile() output buffer, zero-copy over a caller-provided buffer.
///
/// This struct is `#[repr(C)]` with explicit field layout. Total size must
/// stay at 40 bytes; `compile()` writes exactly 5 × 8 = 40 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompileOutput {
    pub field_0: u64,
    pub field_1: u64,
    pub field_2: u32,
    pub field_3: u64,
    pub field_4: u64,
}

// Compile-time size assertion.
const _: () = assert!(std::mem::size_of::<CompileOutput>() == 40);

impl CompileOutput {
    /// Construct a zeroed output buffer.
    pub const fn zeroed() -> Self {
        Self {
            field_0: 0,
            field_1: 0,
            field_2: 0,
            field_3: 0,
            field_4: 0,
        }
    }

    /// The function pointer field, if non-zero.
    ///
    /// Once Phase D demo confirms the calling convention, this will become
    /// a typed `extern "C" fn(...)` cast.
    pub fn fn_ptr(&self) -> Option<NonNull<u8>> {
        NonNull::new(self.field_4 as *mut u8)
    }

    /// Returns true if any field is non-zero (i.e. compile() actually wrote).
    pub fn is_populated(&self) -> bool {
        self.field_0 != 0
            || self.field_1 != 0
            || self.field_2 != 0
            || self.field_3 != 0
            || self.field_4 != 0
    }
}