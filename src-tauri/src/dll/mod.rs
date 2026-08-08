//! FFI bindings for `sim-shim.dll`, which wraps `compile.dll::compile`.
//!
//! See `docs/10-investigation/compile-signature.md` for the full ABI.
//!
//! Submodules:
//! - [`loader`]: safe wrapper around `libloading::Library` for shim.dll
//! - [`signature`]: compile() output struct layout (40 bytes, 5 fields)
//! - [`runtime`]: high-level `run_circuit_test()` end-to-end driver

pub mod loader;
pub mod runtime;
pub mod signature;