//! Format layer for Turing Complete circuit files.
//!
//! Public API surface:
//! - [`codec::decode_v15`] / [`codec::encode_v15`] — strict v15 read/write
//! - [`codec::decode_circuit`] — auto-dispatch by leading version byte
//! - [`legacy::decode_v7`] / [`legacy::decode_v13`] / [`legacy::decode_v14`] — read-only legacy
//!
//! Wire format: `[1 byte version] + [Snappy-compressed body]`. Body is a sequence
//! of typed records; see [`model`] for field layouts and [`codec`] for the v15
//! record order.

pub mod binary;
pub mod codec;
pub mod legacy;
pub mod model;
pub mod snappy;