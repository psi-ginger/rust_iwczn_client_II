//! Network-layer primitives.
//!
//! Introduced micro-step by micro-step per `CONTRIBUTING.md`:
//! - Step 1.3: [`cipher`], the ITZ rolling link/session cipher.
//! - Step 2.1: [`session`], bounded async TCP transport.
//! - Step 2.2: [`codec`], the streaming framed-packet decoder.
//! - Step 2.3: [`codec`], the framed-packet encoder (packets → wire).

pub mod cipher;
pub mod codec;
pub mod session;
