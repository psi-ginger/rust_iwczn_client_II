// Step 5.2 tests — falsifiable, byte-exact, loopback.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): the live `goonzu-rs`
// server is unavailable, so every peer here is a loopback socket that writes
// the real opcode 1 handshake with a fresh `Cipher::link_cipher()` and reads
// the client's frames back the way `services/game/src/main.rs` does — `usSize`
// in the clear selects the length, then the negotiated session cipher decodes
// the rest. What these prove is *this module's* contract: the 5-byte frame,
// its opcode, its session-cipher pairing, the pre-handshake refusal, the
// ticker identity, and that the event loop actually pushes heartbeats over a
// connection that stays open.
//
// F-07: the expectation sources are never the unit under test. The pre-cipher
// frame image is handwritten `05 00 00 64 00` (from `MsgType.h:56` +
// `itzbase.h:63-68` + the `cltMsg` constructor at `MsgType.h:2967`), the
// opcode literal is 100, the intervals are the literals 10 s
// (`CONTRIBUTING.md:150`) and 30 s (`Client.cpp:5035`), and the loop test's
// frame count is what the *peer* decoded off the socket — not what the client
// believes it sent.
use super::*;
use crate::client::event_loop::{run, Flow};
use crate::game::entry::RESPONSE_CLIENTCHARUNIQUE;
use crate::net::cipher::Cipher;
use crate::net::codec::Encoder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// A complete plaintext frame: `[usSize LE][ucChk][usCmd LE][payload]` — the
/// pre-cipher image of anything the client sends.
fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The server→client encoding: a fresh link cipher keyed `0x12,0x34` — what
/// the game service builds per session and writes the handshake with.
fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// The handshake body as the server writes it: `char_unique` 42 LE, session
/// seeds `0x7E/0xAD`, `version` 1 LE — the same literals Steps 4.2/4.3 pin,
/// restated so this module proves its own pairing.
const HANDSHAKE_PAYLOAD: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];
const HANDSHAKE_KEY1: u8 = 0x7E;
const HANDSHAKE_KEY2: u8 = 0xAD;

/// The unsolicited opcode 1 handshake, link-encoded, written before any
/// client read — exactly as the service writes it at accept.
async fn write_handshake(stream: &mut tokio::net::TcpStream) {
    stream
        .write_all(&encode_link(&frame(
            RESPONSE_CLIENTCHARUNIQUE,
            &HANDSHAKE_PAYLOAD,
        )))
        .await
        .unwrap();
}

/// The five bytes a heartbeat is, pre-cipher: `usSize` 5 LE, `ucChk` 0,
/// `usCmd` 100 LE — hand-written from the header layout
/// (`itzbase.h:63-68`), never produced by this module's own encoder.
const EXPECTED_WIRE: [u8; 5] = [0x05, 0x00, 0x00, 0x64, 0x00];

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        REQUEST_HEARTBEAT, 100,
        "MsgType.h:56 — GAMEMSG_REQUEST_HEARTBEAT = 100 (the server port's \
         generated GameMsg::RequestHeartbeat = 100, opcodes_game.rs:52-53, \
         same header)"
    );
    assert_eq!(
        HEARTBEAT_INTERVAL,
        Duration::from_secs(10),
        "CONTRIBUTING.md:150 — '10-second recurring ticker firing keepalive \
         pings over the active game session'"
    );
    assert_eq!(
        LEGACY_HEARTBEAT_INTERVAL,
        Duration::from_secs(30),
        "Client.cpp:5035 — TABS(CurrentClock - m_SentHeartBeatClock) > 30 * 1000"
    );
    assert!(
        HEARTBEAT_INTERVAL < LEGACY_HEARTBEAT_INTERVAL,
        "the spec's cadence must be strictly more frequent than the legacy's, \
         so the divergence can only add liveness (10 s vs 30 s)"
    );
    assert!(
        LEGACY_HEARTBEAT_INTERVAL * 2 < Duration::from_secs(3 * 60),
        "even the legacy's own 30 s cadence fires twice inside the server's \
         three-minute AutoLogOut window (char.cpp:5149), which is what the \
         heartbeat exists to stay under"
    );
    assert_eq!(
        HEARTBEAT_TICKER_NAME, "heartbeat",
        "the label is a wire-adjacent identifier: is_heartbeat keys on it, so \
         it is asserted literally rather than through the module's own accessor"
    );

    let ticker = heartbeat_ticker();
    assert_eq!(
        ticker.name, "heartbeat",
        "heartbeat_ticker() must carry the name literal above"
    );
    assert_eq!(
        ticker.interval,
        Duration::from_secs(10),
        "heartbeat_ticker() must carry the spec's 10 s literal, not a value \
         read back from HEARTBEAT_INTERVAL"
    );
}

#[tokio::test]
async fn the_heartbeat_is_a_header_only_frame_session_ciphered_and_uncompressed() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        write_handshake(&mut stream).await;
        // Read the client's frame the way the service reads: usSize in the
        // clear selects the length, then the rest follows.
        let mut head = [0u8; 2];
        stream.read_exact(&mut head).await.unwrap();
        let size = usize::from(u16::from_le_bytes(head));
        let mut raw = vec![0u8; size];
        raw[..2].copy_from_slice(&head);
        stream.read_exact(&mut raw[2..]).await.unwrap();
        raw
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    send_heartbeat(&mut link).await.expect("heartbeat");
    let raw = server.await.unwrap();

    // The frame is header-only: `cltMsg(100, 0, NULL)` sets
    // `usSize = sizeof(sPacketHeader) + 0` (MsgType.h:2967).
    assert_eq!(
        raw.len(),
        5,
        "usSize 5 — no payload follows the header, so the whole frame is the \
         header"
    );
    // usSize travels in the clear: the service reads exactly that many bytes
    // before it ever decodes (`usSize` is kept clear by Cipher::encode).
    assert_eq!(
        u16::from_le_bytes([raw[0], raw[1]]),
        5,
        "the in-clear usSize must be 5, little-endian (itzbase.h:63-68)"
    );
    assert!(
        raw.len() < 200,
        "the legacy compresses only usSize >= 200 (Client-Winsock.cpp:178); \
         a 5-byte heartbeat always takes the uncompressed else branch (:197-201)"
    );
    // ...and everything after it did NOT travel in the clear: the session
    // cipher ran, exactly as SendMsgToServer's GetEncrypt()->Encode does
    // (Client-Winsock.cpp:174,200).
    assert_ne!(
        raw.as_slice(),
        &EXPECTED_WIRE[..],
        "client→server must be enciphered with the negotiated session keys, \
         not the plaintext image"
    );

    // The plaintext image is recovered only with the handshake-negotiated
    // session cipher — the pairing the service uses (outbound session cipher,
    // inbound link cipher; services/game/src/main.rs).
    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap_or(false),
        "the negotiated session keys must satisfy the integrity check"
    );
    assert_eq!(
        plain.as_slice(),
        &EXPECTED_WIRE[..],
        "decoding recovers [usSize=5][ucChk=0][usCmd=100] bit-exact — opcode \
         100 with no 0x8000 compression flag and no payload byte"
    );

    // The inbound link cipher must not decode the client→server half —
    // outbound is session-keyed, inbound is link-keyed (the service's cipher
    // pairing; Client-Winsock.cpp:174,200).
    let mut scratch = vec![0u8; raw.len()];
    assert!(
        !Cipher::link_cipher()
            .decode(&raw, &mut scratch)
            .unwrap_or(false),
        "the link cipher must not decode a session-ciphered heartbeat"
    );

    // Wrong keys must not recover the frame.
    //
    // Two separate facts, both pinned. First, an off-by-one `key1` is really
    // an off-by-one of *both* keys, because `tick()` advances them together
    // on every call (`net/cipher.rs:171-174`) — and for this header-only
    // frame the trailing integrity check (`ITZEncrypt.cpp:111`, ported at
    // `net/cipher.rs:293-295`) reports a *match* under the shifted keys: its
    // two operands, `src[2] - key1` and the last decoded byte, move together.
    // The check alone would let this frame through; the recovered content is
    // what stops it, which is the second assertion.
    let mut wrong_plain = vec![0u8; raw.len()];
    assert!(
        Cipher::session_cipher(HANDSHAKE_KEY1.wrapping_add(1), HANDSHAKE_KEY2)
            .decode(&raw, &mut wrong_plain)
            .unwrap_or(false),
        "the both-keys-shifted pair reports an integrity match on this 5-byte \
         frame — pinned so any change to the cipher's check is caught"
    );
    assert_ne!(
        wrong_plain.as_slice(),
        &EXPECTED_WIRE[..],
        "an off-by-one key pair must not recover the heartbeat image"
    );

    // Second: a key2-only slip fails the check outright — the integrity
    // operand is keyed on `key1` alone (`net/cipher.rs:279`), so with `key1`
    // correct the operand is right and only the walked byte is wrong.
    let mut key2_slip = vec![0u8; raw.len()];
    assert!(
        !Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2.wrapping_add(1))
            .decode(&raw, &mut key2_slip)
            .unwrap_or(false),
        "a key2 off-by-one must fail the integrity check (ITZEncrypt.cpp:111)"
    );
}

#[tokio::test]
async fn send_before_the_handshake_is_refused_and_writes_nothing() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    // No handshake: the legacy's bGameServerSecretKeySwitch gate
    // (Client-Winsock.cpp:171) refuses the whole call before the cipher is
    // reached, surfaced through GameLink::send as NotHandshaken.
    let err = send_heartbeat(&mut link).await.unwrap_err();
    assert!(
        matches!(err, GameError::NotHandshaken),
        "expected NotHandshaken before the opcode 1 handshake, got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused heartbeat must not key or tick the outbound cipher"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-handshake heartbeat must never reach the socket, got \
         {received:02x?}"
    );
}

#[test]
fn is_heartbeat_keys_on_the_ticker_name_not_the_interval() {
    assert!(
        is_heartbeat(&LoopEvent::Ticked {
            name: HEARTBEAT_TICKER_NAME,
            fired: 1
        }),
        "the module's own ticker must be recognised on its first fire"
    );
    assert!(
        is_heartbeat(&LoopEvent::Ticked {
            name: heartbeat_ticker().name,
            fired: 9
        }),
        "the fire count is irrelevant — identity is the name"
    );
    assert!(
        is_heartbeat(&LoopEvent::Ticked {
            name: HEARTBEAT_TICKER_NAME,
            fired: u64::MAX
        }),
        "any ordinal of the heartbeat ticker still matches"
    );
    assert!(
        !is_heartbeat(&LoopEvent::Ticked {
            name: "not-the-heartbeat",
            fired: 1
        }),
        "any other ticker must not be mistaken for the heartbeat — the event \
         carries no interval at all, so identity can only be the name"
    );
    let inbound = Encoder::new()
        .encode(REQUEST_HEARTBEAT, &[])
        .expect("encode");
    assert!(
        !is_heartbeat(&LoopEvent::Frame(inbound)),
        "an inbound opcode-100 frame is a received heartbeat, not a prompt to \
         transmit one; the predicate is about the ticker firing"
    );
}

#[tokio::test(start_paused = true)]
async fn the_loop_pushes_a_heartbeat_on_every_tick_over_an_open_connection() {
    // The acceptance's substance (CONTRIBUTING.md:151): recurring heartbeats
    // over a connection that never drops. The peer counts what actually
    // arrives on the socket — the client's own tally is asserted too, but the
    // wire is the expectation source.
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        write_handshake(&mut stream).await;
        let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
        let mut ops = Vec::new();
        loop {
            let mut head = [0u8; 2];
            match stream.read_exact(&mut head).await {
                Ok(_) => {}
                // A clean FIN with no pending bytes: the client closed after
                // its last heartbeat.
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
                // Anything else between frames is a dropped connection — the
                // exact failure the acceptance forbids.
                Err(err) => panic!("the connection broke between frames: {err}"),
            }
            let size = usize::from(u16::from_le_bytes(head));
            let mut raw = vec![0u8; size];
            raw[..2].copy_from_slice(&head);
            // A failure here means the peer died mid-frame.
            stream
                .read_exact(&mut raw[2..])
                .await
                .expect("the connection must not drop mid-frame");
            let mut plain = vec![0u8; raw.len()];
            assert!(
                cipher.decode(&raw, &mut plain).unwrap_or(false),
                "every heartbeat must satisfy the session integrity check, got \
                 {raw:02x?}"
            );
            ops.push(u16::from_le_bytes([plain[3], plain[4]]));
        }
        ops
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");

    // The real ticker name with a shortened period: on a loopback socket the
    // 10 s spec interval would take ten seconds of wall clock, and F-01/F-02
    // make live waiting impossible anyway. The name, the opcode and the wire
    // bytes are the real thing; only the period is a test knob — which is
    // exactly what is_heartbeat keys on.
    let tickers = [Ticker::new(
        HEARTBEAT_TICKER_NAME,
        Duration::from_millis(60),
    )];
    let mut sent = 0u32;
    // Watchdog: under a paused clock the loop's own ticker is what advances
    // time, so a stall surfaces as this deadline instead of a hang.
    tokio::time::timeout(
        Duration::from_secs(3),
        run(
            &mut link,
            &tickers,
            async |link, ev| -> Result<Flow, GameError> {
                if is_heartbeat(&ev) {
                    send_heartbeat(link).await?;
                    sent += 1;
                    if sent >= 3 {
                        return Ok(Flow::Break);
                    }
                }
                Ok(Flow::Continue)
            },
        ),
    )
    .await
    .expect("the loop must stop after three heartbeats instead of stalling")
    .expect("the loop must run without a link or handler error");
    drop(link); // FIN, so the peer's read loop ends

    let ops = server.await.unwrap();
    assert_eq!(
        ops,
        vec![100, 100, 100],
        "three heartbeats on the wire, opcode 100 each (MsgType.h:56), in \
         order, with no other frame mixed in — the peer decoded them, not the \
         sender"
    );
    assert_eq!(
        sent, 3,
        "the handler saw exactly three heartbeat ticks before breaking"
    );
}
