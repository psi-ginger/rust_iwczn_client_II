// Steps 4.1–4.2 tests — falsifiable, byte-exact.
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
// pairing at `main.rs:680`), handshake fields are asserted against literal
// payload bytes (`MsgType.h:3013-3027`), and key expectations come from
// `ITZSession.cpp:56` / the payload literals — never from `GameLink`'s own
// state.
use super::*;
use crate::net::cipher::{LINK_KEY1, LINK_KEY2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

/// Opcode 1 (`GAMEMSG_RESPONSE_CLIENTCHARUNIQUE`) with a deliberately
/// non-conforming 7-byte body: Step 4.1's tests prove *decryption* only and
/// never interpret the payload, so the 4.2 parser — which demands exactly
/// 8 bytes — is not exercised here; it has its own literals below.
const OPAQUE_CMD: u16 = 1;
/// Its handwritten payload, asserted bit-exactly on both sides.
const OPAQUE_PAYLOAD: &[u8] = &[0x2A, 0x00, 0x00, 0x00, 0xA7, 0x5C, 0xFF];

/// The handshake body as the server writes it (`game.rs:98-110`):
/// `char_unique` 42 LE, keys `0x7E/0xAD`, `version` 1 LE — the advertised
/// `CLIENT_COMPAT_VERSION`, `game.rs:81-85`.
const HANDSHAKE_PAYLOAD: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];
const HANDSHAKE_CHAR_UNIQUE: i32 = 42;
const HANDSHAKE_KEY1: u8 = 0x7E;
const HANDSHAKE_KEY2: u8 = 0xAD;
const HANDSHAKE_VERSION: i16 = 1;

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
    assert_eq!(
        RESPONSE_CLIENTCHARUNIQUE, 1,
        "MsgType.h:22 — GAMEMSG_RESPONSE_CLIENTCHARUNIQUE = 1 \
         (opcodes_game.rs:15, ResponseClientcharunique = 1)"
    );
    assert_eq!(
        CHAR_UNIQUE_PAYLOAD_LEN, 8,
        "MsgType.h:3013-3027 — SI32 + UI08 + UI08 + SI16 \
         (game.rs:16, CHAR_UNIQUE_PAYLOAD_LEN = 8)"
    );
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

// ---------------------------------------------------------------------------
// Step 4.2 — the opcode 1 (ClientCharUnique) handshake.
// ---------------------------------------------------------------------------

#[test]
fn client_char_unique_reads_the_wire_offsets() {
    // The exact bytes a server frame carries (game.rs:98-110 writes i32 LE,
    // u8, u8, i16 LE into these eight slots).
    let state = parse_client_char_unique(RESPONSE_CLIENTCHARUNIQUE, &HANDSHAKE_PAYLOAD)
        .expect("an exact 8-byte body parses");
    assert_eq!(
        state.char_unique, HANDSHAKE_CHAR_UNIQUE,
        "char_unique is payload[0..4] LE (MsgType.h:3015, siCharUnique)"
    );
    assert_eq!(
        state.key1, HANDSHAKE_KEY1,
        "key1 is payload byte 4 (MsgType.h:3016, uiKey1)"
    );
    assert_eq!(
        state.key2, HANDSHAKE_KEY2,
        "key2 is payload byte 5 (MsgType.h:3017, uiKey2)"
    );
    assert_eq!(
        state.version, HANDSHAKE_VERSION,
        "version is payload[6..8] LE (MsgType.h:3018, siVersion)"
    );

    // Signedness: bytes D6 FF FF FF and FF FF are -42 and -1, not
    // 4294967254 and 65535 — SI32/SI16 like the legacy fields
    // (MsgType.h:3015,3018). A u32/u16 port would fail right here.
    let neg = parse_client_char_unique(
        RESPONSE_CLIENTCHARUNIQUE,
        &[0xD6, 0xFF, 0xFF, 0xFF, 0x11, 0x22, 0xFF, 0xFF],
    )
    .expect("parse");
    assert_eq!(
        (neg.char_unique, neg.key1, neg.key2, neg.version),
        (-42, 0x11, 0x22, -1),
        "two's-complement reads for siCharUnique and siVersion"
    );
}

#[test]
fn the_handshake_body_must_be_exactly_eight_bytes() {
    let mut exact = [0u8; CHAR_UNIQUE_PAYLOAD_LEN];
    exact[4] = HANDSHAKE_KEY1;
    exact[5] = HANDSHAKE_KEY2;
    assert!(
        parse_client_char_unique(RESPONSE_CLIENTCHARUNIQUE, &exact).is_ok(),
        "exactly 8 bytes must parse"
    );

    // Short: every length below 8 is Truncated, with the exact count — the
    // legacy would read out of bounds here (Client-SystemNPC.cpp:695).
    for len in 0..CHAR_UNIQUE_PAYLOAD_LEN {
        let err = parse_client_char_unique(RESPONSE_CLIENTCHARUNIQUE, &exact[..len]).unwrap_err();
        assert!(
            matches!(
                err,
                GameError::Truncated { expected: 8, got } if got == len
            ),
            "length {len}: expected Truncated {{ expected: 8, got: {len} }}, \
             got {err:?}"
        );
    }

    // Long: trailing bytes are rejected, not silently dropped the way the
    // legacy's blind cast would drop them.
    for &len in &[9usize, 10, 64] {
        let mut long = vec![0u8; len];
        long[..CHAR_UNIQUE_PAYLOAD_LEN].copy_from_slice(&exact);
        let err = parse_client_char_unique(RESPONSE_CLIENTCHARUNIQUE, &long).unwrap_err();
        assert!(
            matches!(
                err,
                GameError::TrailingBytes { expected: 8, got } if got == len
            ),
            "length {len}: expected TrailingBytes {{ expected: 8, got: {len} }}, \
             got {err:?}"
        );
    }
}

#[test]
fn a_foreign_opcode_is_refused_before_its_length_is_looked_at() {
    // GAMEMSG_RESPONSE_CHARLIST (3) — a plausible server push, not the
    // handshake (opcodes_game.rs:18-19).
    const FOREIGN: u16 = 3;
    // The order matters: even a body that could not possibly hold the
    // struct must report the *opcode*, because the handshake contract is
    // keyed on the command first (module docs, "strict length"). A parser
    // that measured length first would answer Truncated here.
    for len in [0usize, 3, 8, 9] {
        let payload = vec![0u8; len];
        let err = parse_client_char_unique(FOREIGN, &payload).unwrap_err();
        assert!(
            matches!(
                err,
                GameError::UnexpectedOpcode { cmd } if cmd == FOREIGN
            ),
            "foreign opcode with payload length {len}: expected \
             UnexpectedOpcode {{ cmd: {FOREIGN} }}, got {err:?}"
        );
    }
}

#[tokio::test]
async fn handshake_refuses_a_foreign_frame_and_leaves_the_link_unkeyed() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // A well-formed frame under the *link* cipher (so decryption
        // succeeds) carrying opcode 3 instead of 1: if the opcode check
        // were missing, this garbage would key the outbound cipher.
        let plain = frame(3, &[0u8; 8]);
        stream.write_all(&encode_link(&plain)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let err = link.handshake().await.unwrap_err();
    assert!(
        matches!(err, GameError::UnexpectedOpcode { cmd } if cmd == 3),
        "expected UnexpectedOpcode {{ cmd: 3 }}, got {err:?}"
    );
    assert!(
        link.negotiated().is_none(),
        "a refused handshake must store no negotiated state"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused handshake must install no outbound cipher — a wrong key \
         here would silently encipher everything that follows"
    );

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_keys_the_outbound_channel_and_ciphers_every_frame_after_it() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // The unsolicited handshake frame at accept, link-encoded exactly
        // as main.rs:687-699 writes it — before any client read.
        stream
            .write_all(&encode_link(&frame(
                RESPONSE_CLIENTCHARUNIQUE,
                &HANDSHAKE_PAYLOAD,
            )))
            .await
            .unwrap();
        // Then the client's first outbound frame, read the way the service
        // reads: usSize in the clear selects the frame length.
        let mut head = [0u8; 2];
        stream.read_exact(&mut head).await.unwrap();
        let size = usize::from(u16::from_le_bytes(head));
        let mut raw = vec![0u8; size];
        raw[..2].copy_from_slice(&head);
        stream.read_exact(&mut raw[2..]).await.unwrap();
        raw
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    assert!(
        link.negotiated().is_none(),
        "nothing negotiated before the handshake"
    );
    assert!(
        link.outbound_keys().is_none(),
        "the outbound encoder starts unkeyed"
    );

    let state = link.handshake().await.expect("opcode 1 handshake");
    // Every field asserted against the handwritten literals (F-07), never
    // against something the link reports about itself.
    assert_eq!(
        (state.char_unique, state.key1, state.key2, state.version),
        (
            HANDSHAKE_CHAR_UNIQUE,
            HANDSHAKE_KEY1,
            HANDSHAKE_KEY2,
            HANDSHAKE_VERSION
        ),
        "all four fields off the wire at their documented offsets"
    );
    assert_eq!(link.negotiated(), Some(state), "the state is stored");
    assert_eq!(
        link.outbound_keys(),
        Some((HANDSHAKE_KEY1, HANDSHAKE_KEY2)),
        "outbound seeded from payload[4], payload[5] \
         (Client-SystemNPC.cpp:701-703, SetKey)"
    );
    assert_eq!(
        link.inbound_keys(),
        Some((LINK_KEY1 + 1, LINK_KEY2 + 1)),
        "inbound stayed on the link cipher and ticked exactly one frame — \
         the server keeps encoding that way for the connection's life \
         (main.rs:680)"
    );

    // The first thing a real client sends after the handshake: an ordinary
    // game opcode (GAMEMSG_REQUEST_HEARTBEAT, opcodes_game.rs:52-53) with
    // a synthetic body — the loopback peer is not a parser (F-01/F-07).
    const CLIENT_CMD: u16 = 100;
    const CLIENT_PAYLOAD: &[u8] = b"tick";
    link.send(CLIENT_CMD, CLIENT_PAYLOAD).await.unwrap();
    let raw = server.await.unwrap();

    // usSize travelled in the clear (cipher.rs: usSize is never encrypted).
    assert_eq!(
        u16::from_le_bytes([raw[0], raw[1]]),
        (5 + CLIENT_PAYLOAD.len()) as u16,
        "frame length readable without the key"
    );
    // The body did not — this is the F-04 integrity property on outbound.
    assert_ne!(
        &raw[5..],
        CLIENT_PAYLOAD,
        "client→server must be enciphered with the negotiated keys, not \
         plaintext"
    );

    // Decode with a *fresh* session cipher seeded by the literal bytes —
    // the same thing the server does after sending them (main.rs:704).
    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap(),
        "the negotiated keys must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        CLIENT_CMD,
        "opcode recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        CLIENT_PAYLOAD,
        "payload recovered bit-exact through encode → wire → decode"
    );

    // An off-by-one key must fail: the wire bytes are a function of
    // payload[4..6] and nothing else.
    let mut wrong = Cipher::session_cipher(HANDSHAKE_KEY1.wrapping_add(1), HANDSHAKE_KEY2);
    let mut scratch = vec![0u8; raw.len()];
    assert!(
        !wrong.decode(&raw, &mut scratch).unwrap_or(false),
        "an off-by-one key must fail the integrity check"
    );
}

#[tokio::test]
async fn send_before_the_handshake_is_refused_and_writes_nothing() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Everything the client writes, until it closes: must be empty.
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let err = link.send(100, b"too early").await.unwrap_err();
    assert!(
        matches!(err, GameError::NotHandshaken),
        "expected NotHandshaken (the bGameServerSecretKeySwitch gate, \
         Client-Winsock.cpp:171), got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused send must not key or tick anything"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-handshake send must never reach the socket, got {received:02x?}"
    );
}

#[tokio::test]
async fn handshake_is_idempotent_once_negotiated() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream
            .write_all(&encode_link(&frame(
                RESPONSE_CLIENTCHARUNIQUE,
                &HANDSHAKE_PAYLOAD,
            )))
            .await
            .unwrap();
        // Close right away: a second handshake() that touched the wire
        // would find EOF and fail instead of returning the stored state.
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let first = link.handshake().await.expect("first handshake");
    let second = link
        .handshake()
        .await
        .expect("the second call must not read (the peer has closed)");
    assert_eq!(
        second, first,
        "the stored state comes back unchanged, and only from storage"
    );
    assert_eq!(
        link.outbound_keys(),
        Some((HANDSHAKE_KEY1, HANDSHAKE_KEY2)),
        "the outbound cipher neither re-seeded nor ticked on the second call"
    );

    server.await.unwrap();
}
