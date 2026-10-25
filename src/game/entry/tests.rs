// Step 4.1 tests — falsifiable, byte-exact.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): every address in this
// suite is loopback synthetic — pending live ingestion against the real
// Game Server (4010). What these tests prove is cipher state, byte
// arithmetic, and the timeout/teardown contract, not reachability. Read
// windows are pinned small (200 ms) so the F-02 behaviour is observable in
// milliseconds, not seconds.
//
// F-07: expectation sources are never the unit under test — every frame is
// a handwritten byte array (usSize/ucChk/usCmd literals), the cipher under
// the *server* side of each test is a fresh `Cipher::link_cipher()` (the
// pairing at `main.rs:680`), and key expectations come from
// `ITZSession.cpp:56`, never from `GameLink`'s own state.
use super::*;
use crate::net::cipher::{LINK_KEY1, LINK_KEY2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// A complete plaintext frame: `[usSize LE][ucChk][usCmd LE][payload]` —
/// what the peer *means*; on this link it only ever exists before
/// `Cipher::encode`.
fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
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
/// (`main.rs:680`) and writes with (`send_encoded`, `main.rs:2057`).
fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// Opcode 1 (`GAMEMSG_RESPONSE_CLIENTCHARUNIQUE`) with a placeholder body —
/// parsed in Step 4.2, opaque here: 4.1 owns decryption, not interpretation.
const OPAQUE_CMD: u16 = 1;
/// Its handwritten payload, asserted bit-exactly on both sides.
const OPAQUE_PAYLOAD: &[u8] = &[0x2A, 0x00, 0x00, 0x00, 0xA7, 0x5C, 0xFF];

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        DEFAULT_PORT, 4010,
        "CONTRIBUTING.md Step 4.1 (services/game/src/config.rs:56 default listen)"
    );
    assert_eq!(
        LINK_KEY1, 0x12,
        "ITZSession.cpp:56 — m_pEncrypt2->SetKey(0x12, 0x34)"
    );
    assert_eq!(LINK_KEY2, 0x34, "ITZSession.cpp:56");
}

#[tokio::test]
async fn the_link_cipher_is_installed_before_the_first_read() {
    let (listener, addr) = local_listener().await;
    // The server side stays silent: no byte is ever read in this test, so
    // the keys below cannot have come from any wire activity.
    let _hold_open = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let link = GameLink::connect(addr).await.expect("connect");
    assert_eq!(
        link.inbound_keys(),
        Some((LINK_KEY1, LINK_KEY2)),
        "a fresh link already carries 0x12,0x34 — the constructor keys the \
         decoder before any read exists (ITZSession.cpp:56); a lazy install \
         would show None here"
    );
}

#[tokio::test]
async fn an_immediately_sent_encrypted_frame_decodes_without_corruption() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // The game service writes the first frame at accept, before its
        // read loop (main.rs:687-699) — no handshake, no client prompt.
        let plain = frame(OPAQUE_CMD, OPAQUE_PAYLOAD);
        let wire = encode_link(&plain);
        assert_ne!(
            wire, plain,
            "sanity: the frame must not leave the server in the clear"
        );
        stream.write_all(&wire).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let frame = link
        .next_frame()
        .await
        .expect("no cipher corruption error (the Step 4.1 acceptance)")
        .expect("the server sent a frame");
    assert_eq!(
        frame.cmd(),
        OPAQUE_CMD,
        "opcode survives the link cipher — a wrong key fails integrity instead"
    );
    assert_eq!(
        frame.payload(),
        OPAQUE_PAYLOAD,
        "payload bit-exact through encode → wire → decode"
    );
    assert_eq!(
        link.inbound_keys(),
        Some((LINK_KEY1 + 1, LINK_KEY2 + 1)),
        "the keys tick once per pulled frame — the keys ARE the counter \
         (cipher.rs); no tick would mean the cipher never ran"
    );

    server.await.unwrap();
}

#[tokio::test]
async fn a_plaintext_frame_is_refused_because_byte_zero_is_enciphered() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Raw, unenciphered bytes: valid framing, wrong crypto. If the
        // inbound decoder were plaintext (or keyed differently), this
        // would be accepted — that is exactly what must not happen.
        let plain = frame(OPAQUE_CMD, OPAQUE_PAYLOAD);
        stream.write_all(&plain).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let err = link.next_frame().await.unwrap_err();
    assert!(
        matches!(
            err,
            GameError::Codec(CodecError::Integrity { size, .. }) if size == 12
        ),
        "expected Codec(Integrity) for a plaintext frame (usSize 12 = 5 + 7), got {err:?}"
    );

    // Sticky: the refusal poisoned the decoder, so the next pull refuses
    // again without touching the socket — the legacy returns out of the
    // receive path the same way (Client-Winsock.cpp:872-890).
    let err = link.next_frame().await.unwrap_err();
    assert!(
        matches!(err, GameError::Codec(CodecError::Integrity { .. })),
        "the integrity refusal must be sticky, got {err:?}"
    );

    server.await.unwrap();
}

#[tokio::test]
async fn a_read_timeout_is_never_a_teardown_on_an_idle_game_link() {
    // F-02: ReadTimeout bounds one wait only — the socket stays open, the
    // partial frame stays buffered, the cipher counter stands still. The
    // server's writes are gated on flags set by this side, so every wait
    // below is deterministic: no sleep arithmetic, only the 200 ms window
    // under test.
    let (listener, addr) = local_listener().await;
    let phase1 = Arc::new(AtomicBool::new(false));
    let phase2 = Arc::new(AtomicBool::new(false));
    let write_head = Arc::clone(&phase1);
    let write_tail = Arc::clone(&phase2);
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let plain = frame(OPAQUE_CMD, OPAQUE_PAYLOAD);
        let wire = encode_link(&plain);
        // First 3 bytes (usSize + ucChk) — an incomplete frame — then,
        // only when told, the remainder: a frame straddling idle gaps,
        // which a game link does between world ticks.
        while !write_head.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        stream.write_all(&wire[..3]).await.unwrap();
        while !write_tail.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        stream.write_all(&wire[3..]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let timeouts = SessionTimeouts {
        connect: Duration::from_secs(5),
        read: Duration::from_millis(200),
    };
    let mut link = GameLink::connect_with_timeouts(addr, timeouts)
        .await
        .expect("connect");

    // First wait: a fully idle link times out — surfaced as an ordinary
    // error, nothing closed.
    let err = link.next_frame().await.unwrap_err();
    assert!(
        matches!(
            err,
            GameError::Session(SessionError::ReadTimeout { timeout })
                if timeout == Duration::from_millis(200)
        ),
        "expected Session(ReadTimeout {{ 200ms }}) on an idle link, got {err:?}"
    );

    // The 3-byte head lands while we are between calls: the next wait
    // consumes it, finds the frame incomplete, and times out again — the
    // partial frame must stay buffered across the teardown.
    phase1.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let err = link.next_frame().await.unwrap_err();
    assert!(
        matches!(err, GameError::Session(SessionError::ReadTimeout { .. })),
        "expected a second ReadTimeout with the head buffered, got {err:?}"
    );

    // The tail lands: the frame completes — buffered head survived both
    // timeouts and the cipher was still in step (a counter bumped by the
    // failed waits would fail integrity right here).
    phase2.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let frame = link
        .next_frame()
        .await
        .expect("the link survives its own timeouts")
        .expect("the completed frame");
    assert_eq!(frame.cmd(), OPAQUE_CMD);
    assert_eq!(
        frame.payload(),
        OPAQUE_PAYLOAD,
        "byte-exact after two timeouts — F-02 non-fatal, cipher untouched"
    );

    server.await.unwrap();
}

#[tokio::test]
async fn a_clean_server_close_is_none_not_an_error() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        // Dropping the stream closes the socket — EOF is Ok(None), not a
        // failure (FrontLink's contract, mirrored here).
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let frame = link.next_frame().await.expect("EOF is not an error");
    assert!(
        frame.is_none(),
        "a clean close yields Ok(None), got {frame:?}"
    );

    server.await.unwrap();
}
