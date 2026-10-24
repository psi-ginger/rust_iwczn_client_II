//! GoonZu client library — native Rust port of the GoonZu World client.
//!
//! Phase II micro-steps land here one at a time per `CONTRIBUTING.md`:
//! `protocol::header` (Step 1.2) delivers the 5-byte wire frame header,
//! `net::cipher` (Step 1.3) the ITZ rolling link/session cipher,
//! `net::session` (Step 2.1) the bounded async TCP transport,
//! `net::codec` (Step 2.2) the stream-to-frame decoder;
//! `auth::*`, `game::*`, and `client::*` arrive with their own steps and are
//! intentionally absent.

pub mod net;
pub mod protocol;

/// Version string of the client scaffold, sourced from `Cargo.toml`.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::CLIENT_VERSION;

    #[test]
    fn version_constant_is_linked_to_cargo_manifest() {
        assert!(!CLIENT_VERSION.is_empty(), "version must not be empty");
        assert_eq!(CLIENT_VERSION, env!("CARGO_PKG_VERSION"));
    }
}
