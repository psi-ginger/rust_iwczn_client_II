// Step 3.2 tests — falsifiable, byte-exact.
//
// F-01 (QC decision on CAR-2026-10-24-RETRO01): every address in this suite
// is loopback or documentation-range synthetic — pending live ingestion
// against the real Front (5700) and Game (4010) servers. What these tests
// prove is byte arithmetic and cipher integrity, not reachability.
//
// F-07: expectation sources are never the parser under test — entry bytes
// below are written at literal numeric offsets with the C++ layout cited
// inline, the 7-byte empty frame is a handwritten byte array, and the
// request frames are asserted after decoding with the handshake seeds.
use super::*;
use crate::net::cipher::Cipher;
use crate::net::codec::Decoder;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// `FRONTMSG_RESPONSE_SECRETKEY` as it sits on the wire, in the clear:
/// usSize 10, ucChk 0, usCmd 30000 LE, then the seed slots 0xA7/0x5C at
/// payload offsets 2 and 4 (`FrontMsg.h:132-141`).
const SECRETKEY_FRAME: [u8; 10] = [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];
const SEED_KEY1: u8 = 0xA7;
const SEED_KEY2: u8 = 0x5C;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// One `WORLDINFO` entry (428 bytes) with every field set at its **literal
/// wire offset** — the numbers below come from the default-packing layout in
/// the module docs (`FrontMsg.h:6-41`, pads corroborated by
/// `gz-proto/src/front.rs:210-233`), never from the parser under test.
///
/// Deliberate traps:
/// - the three pad regions hold `0xFF` (the legacy moves them around inside
///   uninitialized struct copies — a parser that reads them, or that assumes
///   the packed 421-byte layout, misaligns every field after offset 2);
/// - `worldname` carries junk **after** its NUL (uninitialized-buffer
///   reality; a C-string parser must stop at the terminator);
/// - `refresh.worldId` differs from `baseinfo.worldId` (each field is read
///   where it lives, no cross-field assumptions);
/// - signed fields are negative (offset/sign handling).
fn sample_entry() -> [u8; WORLDINFO_LEN] {
    let mut e = [0u8; WORLDINFO_LEN];

    // GAMESERVER_BASEINFO (default packing).
    e[0..2].copy_from_slice(&7u16.to_le_bytes()); // worldId
    e[2..9].copy_from_slice(b"World-A"); // worldname[20]
    e[10..16].fill(b'Z'); // junk after the NUL — not text
    e[22..24].fill(0xFF); // pad (align siServerID)
    e[24..28].copy_from_slice(&0x1122_3344i32.to_le_bytes()); // siServerID
    e[28..36].copy_from_slice(b"10.1.2.3"); // gameserverip[16], NUL-padded
    e[44..46].copy_from_slice(&4010u16.to_le_bytes()); // gameserverport
    e[46..56].copy_from_slice(b"2004-07-13"); // opendate[15]
    e[61..74].copy_from_slice(b"world message"); // shortmessage[64]
    e[125..128].fill(0xFF); // pad (struct tail)

    // GAMESERVER_REFRESHINFO.
    e[128..130].copy_from_slice(&0x0808u16.to_le_bytes()); // worldId (≠ base copy)
    e[130..132].copy_from_slice(&2u16.to_le_bytes()); // state = maintenance
    e[132..134].copy_from_slice(&0x1234u16.to_le_bytes()); // currentConnections
    e[134..136].fill(0xFF); // pad (struct tail)

    // GAMESERVER_GAMESERVERINFO.
    e[136..142].copy_from_slice(b"GoonZu"); // goonzuname[20]
    e[156..169].copy_from_slice(b"server banner"); // goonzutext[256]
    e[412..416].copy_from_slice(&(-7i32).to_le_bytes()); // population
    e[416..420].copy_from_slice(&0x2233_4455i32.to_le_bytes()); // currentPrice
    e[420..424].copy_from_slice(&100i32.to_le_bytes()); // npcRate
    e[424..428].copy_from_slice(&(-1i32).to_le_bytes()); // fixationmoney

    e
}

/// `usCount` + one [`sample_entry`] — a valid 430-byte world-list payload.
fn one_entry_payload() -> Vec<u8> {
    let mut payload = vec![0x01, 0x00];
    payload.extend_from_slice(&sample_entry());
    payload
}

/// Builds raw plaintext frame bytes: `[usSize LE][ucChk][usCmd LE][payload]`.
/// Server → client is plaintext on this link, so this is a literal wire
/// image (`front.rs` module docs, asymmetry table).
fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The 20-byte `szAccountID` expectation for `id`: bytes then NUL padding —
/// what `worldlist_request_payload` must put on the wire.
fn expected_id(id: &[u8]) -> [u8; ACCOUNT_ID_LEN] {
    let mut expected = [0u8; ACCOUNT_ID_LEN];
    expected[..id.len()].copy_from_slice(id);
    expected
}

/// Reads one ciphered frame the way the front service does: usSize in the
/// clear selects the length, then the session cipher decodes it. Returns
/// `(wire bytes, plaintext frame)`.
async fn read_ciphered_frame(stream: &mut TcpStream, cipher: &mut Cipher) -> (Vec<u8>, Vec<u8>) {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.unwrap();
    let size = usize::from(u16::from_le_bytes(head));
    let mut raw = vec![0u8; size];
    raw[..2].copy_from_slice(&head);
    stream.read_exact(&mut raw[2..]).await.unwrap();
    let mut plain = vec![0u8; size];
    let accepted = cipher.decode(&raw, &mut plain).expect("decode");
    assert!(
        accepted,
        "the seeded session cipher must satisfy the integrity check"
    );
    (raw, plain)
}

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(REQUEST_NOTICE, 2, "FrontMsg.h:45");
    assert_eq!(
        REQUEST_WORLDLIST, 3,
        "FrontMsg.h:46 — CONTRIBUTING.md:65's \"Opcode 5\" is PATCHTEXT (FrontMsg.h:48)"
    );
    assert_eq!(RESPONSE_NOTICE, 1001, "FrontMsg.h:94");
    assert_eq!(RESPONSE_WORLDLIST, 1002, "FrontMsg.h:95");
    assert_eq!(ACCOUNT_ID_LEN, 20, "FrontMsg.h:490");
    assert_eq!(
        WORLDINFO_LEN, 428,
        "FrontMsg.h:6-41 default-packed (gz-proto/src/front.rs:31)"
    );
    assert_eq!(
        NOTICE_PAYLOAD_LEN, 16,
        "FrontClientProtocol.cpp:760 (usSize = 5 + 16)"
    );

    // Frame arithmetic straight from the legacy builder —
    // WorldList.cpp:236: usSize = 5 (header) + 2 (usCount) + count * 428;
    // and :227-229 for the empty-list special case.
    const fn response_size(count: usize) -> usize {
        5 + 2 + count * WORLDINFO_LEN
    }
    assert_eq!(5 + ACCOUNT_ID_LEN, 25, "worldlist request usSize, packed");
    assert_eq!(response_size(0), 7, "empty response usSize");
    assert_eq!(response_size(1), 435, "one-world response usSize");
    assert_eq!(response_size(2), 863, "two-world response usSize");
}

#[test]
fn account_id_payload_follows_stringchcopy_semantics() {
    // Ordinary id: bytes then NUL padding across the full 20-byte field.
    let p = worldlist_request_payload("haru").unwrap();
    assert_eq!(p.len(), ACCOUNT_ID_LEN);
    assert_eq!(&p[..4], b"haru");
    assert!(
        p[4..].iter().all(|&b| b == 0),
        "the field is NUL-padded, got {:02x?}",
        &p[4..]
    );

    // Boundary: 19 chars + terminator = exactly 20 (StringCchCopy fits).
    let p19 = worldlist_request_payload(&"a".repeat(19)).unwrap();
    assert_eq!(
        p19[19], 0,
        "the 19-char id keeps its terminator at index 19"
    );
    assert!(p19[..19].iter().all(|&b| b == b'a'));

    // One byte more cannot fit: refused with the exact length, never
    // silently truncated (the legacy ignored StringCchCopy's failure).
    for len in [20usize, 21, 64] {
        let err = worldlist_request_payload(&"x".repeat(len)).unwrap_err();
        assert!(
            matches!(err, WorldListError::AccountIdTooLong { len: got } if got == len),
            "len {len}: expected AccountIdTooLong {{ len: {len} }}, got {err:?}"
        );
    }

    // C-string copy stops at the first NUL (StringCchCopy), zero-fill after.
    let p = worldlist_request_payload("ab\0cd").unwrap();
    assert_eq!(&p[..2], b"ab");
    assert!(p[2..].iter().all(|&b| b == 0));

    // The empty id is an empty string, not an error.
    assert!(worldlist_request_payload("")
        .unwrap()
        .iter()
        .all(|&b| b == 0));
}

#[test]
fn worldlist_response_parses_every_field_at_its_literal_offset() {
    let payload = one_entry_payload();
    assert_eq!(payload.len(), 430, "usCount + exactly one 428-byte entry");

    let list = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap();
    assert_eq!(list.worlds.len(), 1, "usCount governs the entry count");

    let w = &list.worlds[0];
    // GAMESERVER_BASEINFO.
    assert_eq!(w.world_id, 7, "u16 at offset 0");
    assert_eq!(
        w.world_name, "World-A",
        "NUL-terminated at offset 2 — the post-NUL 'Z' junk is not text"
    );
    assert_eq!(w.server_id, 0x1122_3344, "i32 at offset 24");
    assert_eq!(w.game_server_ip, "10.1.2.3", "offset 28");
    assert_eq!(
        w.game_server_port, 4010,
        "u16 at offset 44 — a packed 421-byte layout would read offset 41 here"
    );
    assert_eq!(w.open_date, "2004-07-13", "offset 46");
    assert_eq!(w.short_message, "world message", "offset 61");
    // GAMESERVER_REFRESHINFO.
    assert_eq!(
        w.refresh_world_id, 0x0808,
        "u16 at offset 128, parsed independently of the base copy at 0"
    );
    assert_eq!(w.state, 2, "status u16 at offset 130");
    assert_eq!(w.current_connections, 0x1234, "u16 at offset 132");
    // GAMESERVER_GAMESERVERINFO.
    assert_eq!(w.goonzu_name, "GoonZu", "offset 136");
    assert_eq!(w.goonzu_text, "server banner", "offset 156");
    assert_eq!(w.population, -7, "signed i32 at offset 412");
    assert_eq!(w.current_price, 0x2233_4455, "i32 at offset 416");
    assert_eq!(w.npc_rate, 100, "i32 at offset 420");
    assert_eq!(w.fixation_money, -1, "signed i32 at offset 424");

    // The 0xFF pad regions were skipped, not read: had they been treated as
    // data (or had a packed 421-byte layout been assumed), every field from
    // offset 22 onward would already have failed the assertions above.
}

#[test]
fn count_and_length_must_agree_at_every_boundary() {
    // usCount itself needs two bytes.
    for len in 0..2usize {
        let payload = vec![0u8; len];
        let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
        assert!(
            matches!(err, WorldListError::MissingCount { len: got } if got == len),
            "len {len}: expected MissingCount {{ len: {len} }}, got {err:?}"
        );
    }

    // count = 0 is explained by exactly 2 bytes (WorldList.cpp:227-229).
    let empty = parse_world_list(RESPONSE_WORLDLIST, &[0x00, 0x00]).unwrap();
    assert!(empty.worlds.is_empty());

    // Any surplus at count = 0 is trailing — surfaced, never dropped.
    for len in [3usize, 4, 100, 430] {
        let mut payload = vec![0u8; 2];
        payload.resize(len, 0xAA);
        let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
        assert!(
            matches!(err, WorldListError::TrailingBytes { expected: 2, got } if got == len),
            "count 0, len {len}: expected TrailingBytes {{ expected: 2, got: {len} }}, got {err:?}"
        );
    }

    // count = 1: every length below the exact 430 is short — the full
    // boundary sweep, so any off-by-one in the size formula fails here.
    for len in 2..430usize {
        let mut payload = vec![0x01, 0x00];
        payload.resize(len, 0x00);
        let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
        assert!(
            matches!(
                err,
                WorldListError::Truncated {
                    count: 1,
                    expected: 430,
                    got
                } if got == len
            ),
            "count 1, len {len}: expected Truncated, got {err:?}"
        );
    }

    // Exactly 430 parses; one byte over is trailing.
    let mut payload = one_entry_payload();
    let list = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap();
    assert_eq!(list.worlds.len(), 1);
    payload.push(0xAA);
    let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::TrailingBytes {
                expected: 430,
                got: 431
            }
        ),
        "expected TrailingBytes {{ expected: 430, got: 431 }}, got {err:?}"
    );
}

#[test]
fn empty_world_list_is_the_literal_seven_byte_frame() {
    // WorldList.cpp:227-229: usSize = 5 + 2, usCmd = 1002 (0x03EA LE),
    // usCount = 0 — all seven bytes handwritten.
    const EMPTY_FRAME: [u8; 7] = [0x07, 0x00, 0x00, 0xEA, 0x03, 0x00, 0x00];

    let mut decoder = Decoder::new();
    decoder
        .feed(&EMPTY_FRAME)
        .expect("framing accepts a 7-byte frame");
    let frame = decoder
        .next_frame()
        .expect("no refusal")
        .expect("one frame");
    assert_eq!(frame.size(), 7);
    assert_eq!(frame.cmd(), RESPONSE_WORLDLIST);
    assert_eq!(frame.payload(), &[0x00, 0x00], "usCount = 0, nothing else");

    let list = WorldList::from_frame(&frame).unwrap();
    assert!(list.worlds.is_empty());
}

#[test]
fn a_count_larger_than_the_frame_is_refused_without_reading_out_of_bounds() {
    // 0xFFFF entries would be 28,048,982 bytes; a 4-byte frame claims them.
    // The refusal must surface the arithmetic instantly — no allocation of
    // the claimed size, no slice past the payload.
    let err = parse_world_list(RESPONSE_WORLDLIST, &[0xFF, 0xFF, 0x00, 0x00]).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::Truncated {
                count: 0xFFFF,
                expected: 28_048_982,
                got: 4
            }
        ),
        "expected Truncated {{ count: 65535, expected: 28_048_982, got: 4 }}, got {err:?}"
    );

    // Two entries declared, one present: exact numbers, no out-of-bounds
    // read (the legacy would copy blindly, WorldList.cpp:189-194).
    let mut payload = vec![0x02, 0x00];
    payload.extend_from_slice(&sample_entry());
    let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::Truncated {
                count: 2,
                expected: 858,
                got: 430
            }
        ),
        "expected Truncated {{ count: 2, expected: 858, got: 430 }}, got {err:?}"
    );
}

#[test]
fn trailing_bytes_are_surfaced_not_dropped() {
    // count = 0 with a full entry hidden behind it.
    let mut payload = vec![0x00, 0x00];
    payload.extend_from_slice(&sample_entry());
    let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::TrailingBytes {
                expected: 2,
                got: 430
            }
        ),
        "expected TrailingBytes {{ expected: 2, got: 430 }}, got {err:?}"
    );

    // count = 1 with a second entry smuggled in: the surplus entry is
    // reported, never parsed away in silence.
    let mut payload = one_entry_payload();
    payload.extend_from_slice(&sample_entry());
    let err = parse_world_list(RESPONSE_WORLDLIST, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::TrailingBytes {
                expected: 430,
                got: 858
            }
        ),
        "expected TrailingBytes {{ expected: 430, got: 858 }}, got {err:?}"
    );
}

#[test]
fn the_parsers_refuse_foreign_opcodes() {
    let payload = one_entry_payload();
    for cmd in [
        REQUEST_NOTICE,
        REQUEST_WORLDLIST,
        RESPONSE_NOTICE,
        0,
        9002,
        0xFFFF,
    ] {
        let err = parse_world_list(cmd, &payload).unwrap_err();
        assert!(
            matches!(err, WorldListError::UnexpectedOpcode { cmd: got } if got == cmd),
            "worldlist parser accepted opcode {cmd}: {err:?}"
        );
    }

    // Cross-wise: a worldlist frame through the notice parser, and back.
    let err = parse_notice(RESPONSE_WORLDLIST, &[0u8; NOTICE_PAYLOAD_LEN]).unwrap_err();
    assert!(
        matches!(
            err,
            WorldListError::UnexpectedOpcode {
                cmd: RESPONSE_WORLDLIST
            }
        ),
        "notice parser accepted 1002: {err:?}"
    );
}

#[test]
fn notice_response_is_exactly_sixteen_bytes_to_the_first_nul() {
    // FrontClientProtocol.cpp:760-763: usSize = 5 + 16, text[0] = NUL.
    let mut payload = [0u8; NOTICE_PAYLOAD_LEN];
    payload[..2].copy_from_slice(b"hi");
    assert_eq!(parse_notice(RESPONSE_NOTICE, &payload).unwrap(), "hi");

    // No NUL anywhere: every byte is text (C-string semantics, wire.rs:241).
    let full = [b'z'; NOTICE_PAYLOAD_LEN];
    assert_eq!(
        parse_notice(RESPONSE_NOTICE, &full).unwrap(),
        "zzzzzzzzzzzzzzzz"
    );

    // Every other length is drift: refused with the observed length.
    for len in [0usize, 1, 15, 17, 64, 8000] {
        let payload = vec![b'n'; len];
        let err = parse_notice(RESPONSE_NOTICE, &payload).unwrap_err();
        assert!(
            matches!(err, WorldListError::UnexpectedNoticeLength { len: got } if got == len),
            "len {len}: expected UnexpectedNoticeLength {{ len: {len} }}, got {err:?}"
        );
    }
}

#[tokio::test]
async fn both_requests_are_ciphered_and_byte_exact_on_the_wire() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&SECRETKEY_FRAME).await.unwrap();
        // Decode with the session cipher seeded by the very bytes the
        // handshake hands out — exactly what the front service does after
        // `Cipher::session_cipher(key1, key2)`.
        let mut cipher = Cipher::session_cipher(SEED_KEY1, SEED_KEY2);
        let notice = read_ciphered_frame(&mut stream, &mut cipher).await;
        let worldlist = read_ciphered_frame(&mut stream, &mut cipher).await;
        (notice, worldlist)
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    link.handshake().await.unwrap();
    request_notice(&mut link).await.unwrap();
    request_worldlist(&mut link, "haru").await.unwrap();

    let ((raw_notice, plain_notice), (raw_worldlist, plain_worldlist)) = server.await.unwrap();

    // Notice: usSize 5, opcode 2, no payload — sRequest_Notice is
    // header-only (FrontMsg.h:477-480), sent as at LogIn.cpp:767-772.
    assert_eq!(
        u16::from_le_bytes([raw_notice[0], raw_notice[1]]),
        5,
        "notice usSize travels in the clear"
    );
    assert_eq!(plain_notice.len(), 5);
    assert_eq!(
        u16::from_le_bytes([plain_notice[3], plain_notice[4]]),
        REQUEST_NOTICE,
        "opcode 2 recovered through the seeded cipher"
    );

    // WorldList: usSize 25, opcode 3, exactly the 20-byte account field.
    assert_eq!(
        u16::from_le_bytes([raw_worldlist[0], raw_worldlist[1]]),
        25,
        "worldlist usSize travels in the clear"
    );
    assert_eq!(plain_worldlist.len(), 25);
    assert_eq!(
        u16::from_le_bytes([plain_worldlist[3], plain_worldlist[4]]),
        REQUEST_WORLDLIST,
        "opcode 3 — CONTRIBUTING.md:65's \"Opcode 5\" would be PATCHTEXT"
    );
    let expected = expected_id(b"haru");
    assert_eq!(
        &plain_worldlist[5..],
        &expected,
        "szAccountID recovered bit-exactly"
    );

    // F-04: 100 % cipher integrity — the payload never appears in the clear.
    assert_ne!(
        &raw_worldlist[5..],
        &expected[..],
        "the outbound account id must be enciphered, never plaintext"
    );
}

#[tokio::test]
async fn worldlist_round_trip_over_loopback_is_byte_exact() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&SECRETKEY_FRAME).await.unwrap();
        let mut cipher = Cipher::session_cipher(SEED_KEY1, SEED_KEY2);
        let (_raw, plain) = read_ciphered_frame(&mut stream, &mut cipher).await;
        assert_eq!(u16::from_le_bytes([plain[3], plain[4]]), REQUEST_WORLDLIST);
        assert_eq!(
            &plain[5..25],
            &expected_id(b"haru"),
            "szAccountID arrived bit-exact"
        );
        // Respond with the exact one-world response both front servers send:
        // usSize 435 (WorldList.cpp:236), usCmd 1002, usCount 1, one entry.
        let response = frame(RESPONSE_WORLDLIST, &one_entry_payload());
        assert_eq!(&response[..2], &[0xB3, 0x01], "435 = 0x01B3 LE");
        stream.write_all(&response).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    link.handshake().await.unwrap();
    request_worldlist(&mut link, "haru").await.unwrap();

    let frame = link
        .next_frame()
        .await
        .expect("no transport failure")
        .expect("the worldlist response");
    assert_eq!(frame.cmd(), RESPONSE_WORLDLIST);
    assert_eq!(frame.size(), 435);

    let list = WorldList::from_frame(&frame).unwrap();
    assert_eq!(list.worlds.len(), 1);
    let w = &list.worlds[0];
    assert_eq!(w.world_name, "World-A");
    assert_eq!(w.game_server_ip, "10.1.2.3");
    assert_eq!(w.game_server_port, 4010, "the Step 4.1 dial target");
    assert_eq!(w.state, 2, "status field");

    server.await.unwrap();
}

#[tokio::test]
async fn requests_before_the_handshake_are_refused_and_write_nothing() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Nothing to read: a pre-key send must never reach the socket.
        let mut received = Vec::new();
        let _ = tokio::time::timeout(
            Duration::from_millis(400),
            stream.read_to_end(&mut received),
        )
        .await;
        received
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    let err = request_notice(&mut link).await.unwrap_err();
    assert!(
        matches!(err, WorldListError::Front(FrontError::NotKeyed)),
        "expected Front(NotKeyed) (the bFrontServerSecretKeySwitch gate, \
         Client-Winsock.cpp:141), got {err:?}"
    );
    let err = request_worldlist(&mut link, "haru").await.unwrap_err();
    assert!(
        matches!(err, WorldListError::Front(FrontError::NotKeyed)),
        "expected Front(NotKeyed), got {err:?}"
    );
    drop(link);

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "pre-key requests must never reach the socket, got {received:02x?}"
    );
}
