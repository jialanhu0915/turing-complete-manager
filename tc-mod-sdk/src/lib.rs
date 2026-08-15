//! `tc-mod-sdk` — Rust SDK for Turing Complete circuit tooling.
//!
//! Two modules:
//! - [`circuit`]: read/write `circuit.data` (v7/v13/v14/v15) + connectivity model
//! - [`dll`]: drive the game's `compile.dll` (via a Nim shim) to compile & run circuits
//!
//! This crate is a *wrapper* around the game's data formats and its
//! `compile.dll`; it never redistributes game binaries — it loads them from
//! the user's own installation at runtime.

pub mod circuit;
pub mod dll;
