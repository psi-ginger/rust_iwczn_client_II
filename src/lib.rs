//! GoonZu client library — native Rust port of the GoonZu World client.
//!
//! Phase II, Step 1.1 establishes the minimal cargo workspace scaffold. The
//! protocol modules described in `CONTRIBUTING.md` (`protocol::header`,
//! `net::cipher`, `net::session`, `net::codec`, `auth::*`, `game::*`,
//! `client::*`) are introduced by their own micro-steps and are intentionally
//! absent here.

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
