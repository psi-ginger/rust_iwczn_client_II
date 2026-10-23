//! Network-layer primitives.
//!
//! Introduced micro-step by micro-step per `CONTRIBUTING.md`:
//! - Step 1.3: [`cipher`], the ITZ rolling link/session cipher.
//! - Step 2.1: [`session`], bounded async TCP transport.

pub mod cipher;
pub mod session;
