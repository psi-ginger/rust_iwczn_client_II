//! Wire-level protocol types.
//!
//! Introduced micro-step by micro-step per `CONTRIBUTING.md`; Step 1.2 adds
//! [`header`], the 5-byte GoonZu frame header. Step 7.1 adds [`bytes`], the
//! shared little-endian readers extracted by the Step 7.0 lean audit (C1–C4)
//! and reused by the `graphics` decoders.

pub mod bytes;
pub mod header;
