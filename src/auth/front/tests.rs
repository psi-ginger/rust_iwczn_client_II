use super::*;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// `FRONTMSG_RESPONSE_SECRETKEY` as it sits on the wire, in the clear:
/// usSize 10 (`FrontMsg.h:132-141`), ucChk 0 (what the server port writes;
/// the legacy leaves it uninitialized — either parses identically), usCmd
/// 30000 LE, then `ucTemp1 ucTemp2 ucKey1 ucTemp3 ucKey2` with seeds
/// 0xA7/0x5C.
const SECRETKEY_FRAME: [u8; 10] = [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];

/// The two seeds `SECRETKEY_FRAME` carries, at payload offsets 2 and 4.
const SEED_KEY1: u8 = 0xA7;
const SEED_KEY2: u8 = 0x5C;

/// A server→client push (opcode 9002, LE `2A 23`) with a 7-byte payload:
/// usSize 12. Server→client is plaintext on this link (module docs), so the
/// header and payload travel verbatim.
const PUSH_FRAME: [u8; 12] = [
    0x0C, 0x00, 0x00, 0x2A, 0x23, b'n', b'o', b't', b'i', b'c', b'e', b'!',
];
const PUSH_OPCODE: u16 = 9002;
const PUSH_PAYLOAD: &[u8] = b"notice!";

/// A `FRONTMSG_RESPONSE_SECRETKEY` whose payload is only 2 bytes — too
/// short to hold `ucKey1`/`ucKey2` at offsets 2/4 (usSize 7).
const SHORT_SECRETKEY_FRAME: [u8; 7] = [0x07, 0x00, 0x00, 0x30, 0x75, 0xAA, 0xBB];

#[test]
fn constants_match_the_legacy_front_server() {
    assert_eq!(DEFAULT_PORT, 5700, "CONTRIBUTING.md Step 3.1 / legacy dial");
    assert_eq!(RESPONSE_SECRETKEY, 30_000, "FrontMsg.h:114");
    assert_eq!(SECRET_KEY_PAYLOAD_LEN, 5, "FrontMsg.h:132-141");
}

#[test]
fn secret_key_reads_the_two_seed_offsets_only() {
    // The ucTemp* slots are never read, here or in the legacy client: the
    // legacy server leaves them uninitialized (FrontServer.h:108-117 assigns
    // only the key bytes), so junk in 0/1/3 must parse identically.
    let key = SecretKey::from_payload(&[0xFF, 0xEE, SEED_KEY1, 0xDD, SEED_KEY2]).unwrap();
    assert_eq!(
        key.key1(),
        SEED_KEY1,
        "ucKey1 is payload[2] (FrontMsg.h:138)"
    );
    assert_eq!(
        key.key2(),
        SEED_KEY2,
        "ucKey2 is payload[4] (FrontMsg.h:140)"
    );

    // Trailing bytes past the struct are ignored, not misread as seeds.
    let key = SecretKey::from_payload(&[0x00, 0x00, 0x11, 0x00, 0x22, 0x99, 0x88]).unwrap();
    assert_eq!((key.key1(), key.key2()), (0x11, 0x22));

    // Anything shorter than 5 bytes cannot hold both seeds — an error, not
    // the legacy's out-of-bounds read (module docs, divergences).
    for len in 0..SECRET_KEY_PAYLOAD_LEN {
        let err = SecretKey::from_payload(&vec![0u8; len]).unwrap_err();
        assert!(
            matches!(err, FrontError::ShortSecretKeyPayload { len: got } if got == len),
            "payload of {len} bytes: expected ShortSecretKeyPayload{{len: {len}}}, got {err:?}"
        );
    }
}

#[tokio::test]
async fn handshake_over_loopback_extracts_the_seeds_and_ciphers_the_outbound_channel() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Unsolicited on accept, like cClient::Init (FrontServer.h:95-118).
        stream.write_all(&SECRETKEY_FRAME).await.unwrap();
        // Read the client's first keyed frame exactly as the goonzu-rs front
        // service does: usSize in the clear selects the frame length.
        let mut head = [0u8; 2];
        stream.read_exact(&mut head).await.unwrap();
        let size = usize::from(u16::from_le_bytes(head));
        let mut raw = vec![0u8; size];
        raw[..2].copy_from_slice(&head);
        stream.read_exact(&mut raw[2..]).await.unwrap();
        raw
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    assert!(!link.is_keyed(), "nothing keyed before the handshake");
    assert_eq!(link.key(), None);

    let key = link.handshake().await.unwrap();
    assert_eq!(key.key1(), SEED_KEY1);
    assert_eq!(key.key2(), SEED_KEY2);
    assert!(link.is_keyed());
    assert_eq!(link.key(), Some(key));

    // Client→server is ciphered after the handshake (module docs). Send an
    // empty-payload-free frame and capture the raw bytes server-side.
    const ECHO_CMD: u16 = 3; // FRONTMSG_REQUEST_WORLDLIST (FrontMsg.h:46)
    const ECHO_PAYLOAD: &[u8] = b"unit-test";
    link.send(ECHO_CMD, ECHO_PAYLOAD).await.unwrap();
    let raw = server.await.unwrap();

    // usSize traveled in the clear (cipher.rs: usSize is never encrypted).
    let expect_size = (5 + ECHO_PAYLOAD.len()) as u16;
    assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), expect_size);
    // The payload did not: the wire is not the plaintext frame.
    assert_ne!(
        &raw[5..],
        ECHO_PAYLOAD,
        "client→server must be encrypted, not plaintext"
    );

    // Decode with a session cipher seeded by the extracted bytes — the same
    // thing the server does after handing the seeds out (its session cipher
    // is keyed in cClient::Init, FrontServer.h:116). A matching key must
    // recover the frame bit-exactly.
    let mut cipher = Cipher::session_cipher(key.key1(), key.key2());
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap(),
        "the seeded key must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        ECHO_CMD,
        "opcode recovered through the seeded cipher"
    );
    assert_eq!(
        &plain[5..],
        ECHO_PAYLOAD,
        "payload recovered through the seeded cipher"
    );

    // A wrong key must fail the same integrity check — the bytes really are
    // a function of the two seeds, not of anything else.
    let mut wrong = Cipher::session_cipher(key.key1().wrapping_add(1), key.key2());
    let mut scratch = vec![0u8; raw.len()];
    assert!(
        !wrong.decode(&raw, &mut scratch).unwrap_or(false),
        "an off-by-one key must fail the integrity check"
    );
}

#[tokio::test]
async fn inbound_frames_stay_plaintext_after_the_handshake() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Both frames in one write: anything buffered behind the handshake
        // frame must still surface on the next read (module docs).
        let mut both = Vec::from(SECRETKEY_FRAME);
        both.extend_from_slice(&PUSH_FRAME);
        stream.write_all(&both).await.unwrap();
        // Hold the link open so the client keeps reading plaintext.
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let key = link.handshake().await.unwrap();
    assert_eq!((key.key1(), key.key2()), (SEED_KEY1, SEED_KEY2));

    // The decoder is never keyed on this link, so the push frame arrives as
    // written — header and payload verbatim, no Cipher::decode involved.
    let frame = link.next_frame().await.unwrap().expect("push frame");
    assert_eq!(frame.cmd(), PUSH_OPCODE);
    assert_eq!(frame.payload(), PUSH_PAYLOAD);

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_refuses_an_unexpected_first_opcode() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&PUSH_FRAME).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let err = link.handshake().await.unwrap_err();
    assert!(
        matches!(err, FrontError::UnexpectedOpcode { cmd } if cmd == PUSH_OPCODE),
        "expected UnexpectedOpcode{{cmd: {PUSH_OPCODE}}}, got {err:?}"
    );
    assert!(!link.is_keyed(), "a refused handshake keys nothing");

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_rejects_a_short_secretkey_payload() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&SHORT_SECRETKEY_FRAME).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let err = link.handshake().await.unwrap_err();
    assert!(
        matches!(err, FrontError::ShortSecretKeyPayload { len: 2 }),
        "expected ShortSecretKeyPayload{{len: 2}}, got {err:?}"
    );
    assert!(!link.is_keyed());

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_reassembles_a_frame_split_across_reads() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Deliver the handshake frame in two chunks — the decoder must
        // buffer the partial frame and cut it on usSize when the rest lands.
        stream.write_all(&SECRETKEY_FRAME[..4]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        stream.write_all(&SECRETKEY_FRAME[4..]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let key = link.handshake().await.unwrap();
    assert_eq!((key.key1(), key.key2()), (SEED_KEY1, SEED_KEY2));
    assert!(link.is_keyed());

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_surfaces_eof_before_the_frame_arrives() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.unwrap();
        // Accept and close immediately: EOF mid-handshake.
        drop(stream);
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let err = link.handshake().await.unwrap_err();
    match err {
        FrontError::Session(SessionError::Io(io)) => {
            assert_eq!(io.kind(), io::ErrorKind::UnexpectedEof);
        }
        other => panic!("expected Session(Io(UnexpectedEof)), got {other:?}"),
    }
    assert!(!link.is_keyed());

    server.await.unwrap();
}

#[tokio::test]
async fn handshake_is_idempotent_once_keyed() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&SECRETKEY_FRAME).await.unwrap();
        // Hold the socket open and silent: a second handshake() that touched
        // the wire would either block or fail instead of returning the key.
        tokio::time::sleep(Duration::from_secs(1)).await;
    });

    // A short read window bounds the damage if a regression makes the
    // second call read: it errors in ~300ms instead of hanging the suite.
    let timeouts = SessionTimeouts {
        read: Duration::from_millis(300),
        ..SessionTimeouts::default()
    };
    let mut link = FrontLink::connect_with_timeouts(addr, timeouts)
        .await
        .unwrap();
    let first = link.handshake().await.unwrap();

    let started = Instant::now();
    let second = link.handshake().await.unwrap();
    assert_eq!(second, first, "the installed seeds come back unchanged");
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "the second handshake must return the installed key without reading \
         (took {:?})",
        started.elapsed()
    );

    server.await.unwrap();
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

    let mut link = FrontLink::connect(addr).await.unwrap();
    let err = link.send(3, b"too early").await.unwrap_err();
    assert!(
        matches!(err, FrontError::NotKeyed),
        "expected NotKeyed (the bFrontServerSecretKeySwitch gate, \
         Client-Winsock.cpp:141), got {err:?}"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-key send must never reach the socket, got {received:02x?}"
    );
}
