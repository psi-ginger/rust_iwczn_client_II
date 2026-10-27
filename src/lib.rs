//! GoonZu client library — native Rust port of the GoonZu World client.
//!
//! Phase II micro-steps land here one at a time per `CONTRIBUTING.md`:
//! `protocol::header` (Step 1.2) delivers the 5-byte wire frame header,
//! `net::cipher` (Step 1.3) the ITZ rolling link/session cipher,
//! `net::session` (Step 2.1) the bounded async TCP transport,
//! `net::codec` (Steps 2.2–2.3) the framed-packet decoder and encoder,
//! `auth::front` (Step 3.1) the Front Server link and `SECRETKEY` handshake,
//! `auth::world_list` (Step 3.2) the Notice/WorldList requests and the
//! `WORLDINFO` response parser, `auth::login` (Step 3.3) the front login and
//! the `auth_key` handoff token, `game::entry` (Steps 4.1–4.2) the Game
//! Server link keyed by the fixed inbound link cipher and its opcode 1
//! handshake, `game::login` (Step 4.3) the opcode 2 game login submission,
//! `game::char_info` (Step 4.4) the character roster and the 592-byte
//! `MakeCharInfo` detail record, `game::world_entry` (README Step 4.5–4.7)
//! the select-character request, the SERVERINFO banner with its region gate,
//! the opcode-5 record that populates the local `CharClient`, the village
//! blueprint exchange and the derived in-world condition,
//! `game::dispatcher` (README Step 5 items 2-3) the in-world router for
//! opcodes 6/11/118/200/215 and its delta reflection into the `CR[]`-modelled
//! [`DispatcherState`](crate::game::dispatcher::DispatcherState),
//! `client::event_loop` (Step 5.1) the
//! non-blocking `select!` receive loop with its strict 100 ms read budget and
//! background tickers, `client::heartbeat` (Step 5.2) the in-game keepalive
//! ticker and its header-only opcode 100 frame, `client::teardown` (Step
//! 5.3) the graceful exit path — first-wins shutdown flag, tracked tasks,
//! bounded drain; `client::driver` (Step 6.1, `CONTRIBUTING_PHASE2.md`) the
//! end-to-end orchestrator that runs the whole session above as one
//! conversation.

pub mod auth;
pub mod client;
pub mod game;
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
