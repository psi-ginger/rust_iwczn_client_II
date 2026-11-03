// Step 5 (T-3e) — shared test support for the client suites.
//
// The loopback listener below was duplicated byte-for-byte across the
// event_loop, heartbeat and teardown test suites. It now lives here exactly
// once, with its citation doc moved **verbatim** — no citation lost, no
// expectation re-derived from a parser under test (F-07). This is the
// client leg's counterpart to Step 3's `auth/test_support.rs` and Step 4's
// `game/test_support.rs`.
//
// The wire literals the client suites share with other groups are NOT
// redefined here: `heartbeat` imports `HANDSHAKE_PAYLOAD` (plus its two
// seeds) from `game::test_support` and `driver` imports `SECRETKEY_FRAME`
// from `auth::test_support`, so each literal keeps exactly one definition
// tree-wide (F-07 zero redefinitions). Those two cross-group imports are
// why `mod test_support` is `pub(crate)` in `game/mod.rs` and `auth/mod.rs`.
//
// F-01 still holds at every use site: `local_listener` binds `127.0.0.1:0`
// only, so no suite importing it can ever name a non-loopback address.

use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
pub(crate) async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}
