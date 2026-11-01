// Step 4 (T-3g) — shared test support for the game suites.
//
// The loopback listener, the plaintext frame builder, the link encoder and
// the handshake literals below were duplicated byte-for-byte across the
// entry, login and world_entry test suites. They now live here exactly
// once, with their citation docs **merged** — no citation lost, no
// expectation re-derived from a parser under test (F-07). This is the game
// leg's counterpart to Step 3's `auth/test_support.rs`.
//
// F-01 still holds at every use site: `local_listener` binds `127.0.0.1:0`
// only, so no suite in this module can ever name a non-loopback address.

use tokio::net::TcpListener;

use crate::net::cipher::Cipher;

/// Binds a loopback listener on an ephemeral port.
pub(crate) async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// A complete plaintext frame: `[usSize LE][ucChk][usCmd LE][payload]` —
/// what the peer *means*; on either half of the link — the server→client
/// link-ciphered half and the client→server half alike — it only ever
/// exists before `Cipher::encode`.
pub(crate) fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The server→client encoding: a fresh link cipher per connection, keyed
/// `0x12,0x34` — exactly what the game service builds per session
/// (`main.rs:680`; the `outbound` cipher, `services/game/src/main.rs`) and
/// writes with (`send_encoded`, `main.rs:2057`) — the opcode 1 handshake
/// included.
pub(crate) fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// The handshake body as the server writes it (`game.rs:98-110`):
/// `char_unique` 42 LE, session seeds `0x7E/0xAD`, `version` 1 LE — the
/// advertised `CLIENT_COMPAT_VERSION`, `game.rs:81-85`. Every game suite
/// imports this literal rather than restating it, so each one proves its
/// own pairing against the same hand-placed bytes.
pub(crate) const HANDSHAKE_PAYLOAD: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];
/// The first session seed `HANDSHAKE_PAYLOAD` carries, at payload offset 4.
pub(crate) const HANDSHAKE_KEY1: u8 = 0x7E;
/// The second session seed `HANDSHAKE_PAYLOAD` carries, at payload offset 5.
pub(crate) const HANDSHAKE_KEY2: u8 = 0xAD;
