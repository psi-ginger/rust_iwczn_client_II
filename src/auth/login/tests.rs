// Step 3.3 tests — falsifiable, byte-exact.
//
// F-01 (QC decision on CAR-2026-10-24-RETRO01): every address in this suite
// is loopback or documentation-range synthetic — pending live ingestion
// against the real Front (5700) and Game (4010) servers. What these tests
// prove is byte arithmetic and cipher integrity, not reachability (F-02: the
// 5 s read windows bound every socket wait until then).
//
// F-07: expectation sources are never the parser under test — the response
// payload below is written at literal numeric offsets with the C++ layout
// cited inline, the request expectation is a hand-built byte array (never
// `login_request_payload` compared against itself), and the frame headers
// are handwritten numbers from `FrontMsg.h`/`AuthProcess.cpp`.
use super::*;
use crate::auth::world_list::RESPONSE_WORLDLIST;
use crate::net::cipher::Cipher;
use crate::net::codec::Decoder;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// `FRONTMSG_RESPONSE_SECRETKEY` as it sits on the wire, in the clear:
/// usSize 10, ucChk 0, usCmd 30000 LE, seed slots 0xA7/0x5C at payload
/// offsets 2 and 4 (`FrontMsg.h:132-141`).
const SECRETKEY_FRAME: [u8; 10] = [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];
const SEED_KEY1: u8 = 0xA7;
const SEED_KEY2: u8 = 0x5C;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
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

/// A complete 108-byte `sResponse_LogIn` payload with every field written at
/// its **literal wire offset** — the numbers below come from the packed
/// layout in the module docs (`FrontMsg.h:419-442`, arithmetic corroborated
/// by `gz-proto/src/front.rs:24`), never from the parser under test.
///
/// Deliberate traps:
/// - `userid`, `szBlockReason`, and `szServerName` carry junk **after**
///   their NUL (uninitialized-buffer reality; a C-string parser must stop
///   at the terminator);
/// - `siBlockYear` is negative (offset/sign handling);
/// - `siAccountStatus` defaults to 1 (what the server port sends on
///   success, `main.rs:255-262`) — `-1` is reserved for the gate tests.
fn wire_response(
    result: u16,
    auth_key: i32,
    account_status: i32,
) -> [u8; LOGIN_RESPONSE_PAYLOAD_LEN] {
    let mut p = [0u8; LOGIN_RESPONSE_PAYLOAD_LEN];

    p[0..2].copy_from_slice(&result.to_le_bytes()); // usResult
    p[2..6].copy_from_slice(&auth_key.to_le_bytes()); // siAuthKey
    p[6..10].copy_from_slice(b"haru"); // userid[20]
    p[11..16].fill(b'J'); // junk after the NUL — not text
    p[26..28].copy_from_slice(&3i16.to_le_bytes()); // siGameMasterLevel
    p[28..32].copy_from_slice(&2i32.to_le_bytes()); // siBlockMode
    p[32..36].copy_from_slice(&(-1i32).to_le_bytes()); // siBlockYear (signed)
    p[36..40].copy_from_slice(&2006i32.to_le_bytes()); // siBlockMonth
    p[40..44].copy_from_slice(&7i32.to_le_bytes()); // siBlockDay
    p[44..48].copy_from_slice(&13i32.to_le_bytes()); // siBlockHour
    p[48..52].copy_from_slice(&59i32.to_le_bytes()); // siBlockMinute
    p[52..59].copy_from_slice(b"expired"); // szBlockReason[30]
    p[60..64].fill(b'J'); // junk after the NUL
    p[82] = 0x01; // bAlreadyMakeCharSwitch (bool, 1 byte)
    p[83..93].copy_from_slice(b"GoonZuMain"); // szServerName[20]
    p[94..98].fill(b'J'); // junk after the NUL
    p[103..107].copy_from_slice(&account_status.to_le_bytes()); // siAccountStatus
    p[107] = 19; // m_uiRealAge

    p
}

/// The 60-byte request expectation for `("haru", "s3cret",
/// "A1B2C3D4E5F6")`, hand-built from the field table (`FrontMsg.h:344-347`):
/// userid 0..20, userpw 20..40, macaddress 40..60, each zero-padded.
fn expected_request_payload() -> [u8; LOGIN_REQUEST_PAYLOAD_LEN] {
    let mut expected = [0u8; LOGIN_REQUEST_PAYLOAD_LEN];
    expected[0..4].copy_from_slice(b"haru");
    expected[20..26].copy_from_slice(b"s3cret");
    expected[40..52].copy_from_slice(b"A1B2C3D4E5F6");
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
    assert_eq!(REQUEST_LOGIN, 1, "FrontMsg.h:44 (opcodes_front.rs:15)");
    assert_eq!(RESPONSE_LOGIN, 1000, "FrontMsg.h:93 (opcodes_front.rs:63)");
    assert_eq!(CREDENTIAL_LEN, 20, "FrontMsg.h:344-347");
    assert_eq!(
        LOGIN_REQUEST_PAYLOAD_LEN, 60,
        "20+20+20 under pack(1), FrontMsg.h:336-348"
    );
    assert_eq!(
        LOGIN_RESPONSE_PAYLOAD_LEN, 108,
        "FrontMsg.h:419-442 (gz-proto/src/front.rs:24)"
    );

    // Frame arithmetic straight from the structs: the header is 5 bytes
    // (`itzbase.h:63-68`), the legacy sends `sizeof(sendmsg)`
    // (LogIn.cpp:738, AuthProcess.cpp:88), and the legacy front closes
    // sockets on a request size mismatch (FrontClientProtocol.cpp:352-360).
    const fn response_layout() -> usize {
        2 + 4 + 20 + 2 + 4 + 20 + 30 + 1 + 20 + 4 + 1
    }
    assert_eq!(response_layout(), LOGIN_RESPONSE_PAYLOAD_LEN);
    assert_eq!(5 + LOGIN_REQUEST_PAYLOAD_LEN, 65, "login request usSize");
    assert_eq!(5 + LOGIN_RESPONSE_PAYLOAD_LEN, 113, "login response usSize");
}

#[test]
fn login_request_payload_is_byte_exact_at_literal_offsets() {
    let payload = login_request_payload("haru", "s3cret", "A1B2C3D4E5F6").unwrap();
    assert_eq!(
        payload,
        expected_request_payload(),
        "userid 0..20, userpw 20..40, macaddress 40..60, zero-padded"
    );
    assert_eq!(
        &payload[0..4],
        b"haru",
        "userid at offset 0 (FrontMsg.h:344)"
    );
    assert_eq!(
        &payload[20..26],
        b"s3cret",
        "userpw at offset 20 (FrontMsg.h:345)"
    );
    assert_eq!(
        &payload[40..52],
        b"A1B2C3D4E5F6",
        "macaddress at offset 40 (FrontMsg.h:347) — the legacy's %02X × 6 format, Client.cpp:7286"
    );
    assert!(
        payload[52..].iter().all(|&b| b == 0),
        "the mac field is NUL-padded past the 12 hex chars"
    );

    // StringCchCopy semantics (commonutil.cpp:14): cut at the first NUL,
    // zero-fill after — the legacy's remainder is uninitialized stack,
    // observationally identical because servers read up to the terminator.
    let p = login_request_payload("ab\0cd", "", "").unwrap();
    assert_eq!(&p[0..2], b"ab");
    assert!(p[2..].iter().all(|&b| b == 0), "empty fields are all-zero");

    // Boundary: 19 chars + terminator = exactly 20 (StringCchCopy fits).
    let p19 = login_request_payload(&"a".repeat(19), &"b".repeat(19), &"c".repeat(19)).unwrap();
    assert_eq!(
        p19[19], 0,
        "the 19-char id keeps its terminator at index 19"
    );
    assert_eq!(
        p19[39], 0,
        "the 19-char password keeps its terminator at index 39"
    );
    assert_eq!(
        p19[59], 0,
        "the 19-char mac keeps its terminator at index 59"
    );
}

#[test]
fn credentials_that_do_not_fit_are_refused_not_truncated() {
    // Each field independently: 20+ chars cannot fit with a NUL. The legacy
    // sent StringCchCopy's truncated remainder; this port refuses with the
    // exact length and the legacy field name — never a silent shrink.
    let long20 = "x".repeat(20);
    let long21 = "y".repeat(21);
    let long64 = "z".repeat(64);
    for field in ["userid", "userpw", "macaddress"] {
        for value in [&long20, &long21, &long64] {
            let len = value.len();
            // Put the overflow in the field under test, valid values
            // elsewhere (a valid mac is 12 hex chars, Client.cpp:7286).
            let (id, pw, mac): (&str, &str, &str) = match field {
                "userid" => (value.as_str(), "pw", "A1B2C3D4E5F6"),
                "userpw" => ("id", value.as_str(), "A1B2C3D4E5F6"),
                _ => ("id", "pw", value.as_str()),
            };
            let err = login_request_payload(id, pw, mac).unwrap_err();
            assert!(
                matches!(
                    err,
                    LoginError::CredentialTooLong { field: f, len: got }
                        if f == field && got == len
                ),
                "{field} len {len}: expected CredentialTooLong {{ field: {field}, len: {len} }}, got {err:?}"
            );
        }
    }

    // Nothing partial escapes: a refused build produces no payload at all
    // (the Result carries only the error).
    assert!(login_request_payload(&long20, "pw", "").is_err());
}

#[test]
fn login_response_parses_every_field_at_its_literal_offset() {
    let payload = wire_response(0, 0x1122_3344, 1);
    assert_eq!(payload.len(), 108, "usSize 113 minus the 5-byte header");

    let response = parse_login_response(RESPONSE_LOGIN, &payload).unwrap();
    assert_eq!(response.result, 0, "u16 at offset 0 (FrontMsg.h:423)");
    assert_eq!(
        response.auth_key, 0x1122_3344,
        "i32 at offset 2 (FrontMsg.h:424) — gz-proto/src/front.rs:288 places auth_key at payload offset two"
    );
    assert_eq!(
        response.user_id, "haru",
        "NUL-terminated at offset 6 — the post-NUL 'J' junk is not text"
    );
    assert_eq!(response.gm_level, 3, "i16 at offset 26 (FrontMsg.h:429)");
    assert_eq!(response.block_mode, 2, "i32 at offset 28 (FrontMsg.h:431)");
    assert_eq!(
        response.block_time,
        [-1, 2006, 7, 13, 59],
        "five i32 at offsets 32/36/40/44/48 (FrontMsg.h:432) — signed, wire order"
    );
    assert_eq!(
        response.block_reason, "expired",
        "NUL-terminated at offset 52 (FrontMsg.h:433)"
    );
    assert!(
        response.already_made_char,
        "the byte at offset 82 is 1 (FrontMsg.h:435)"
    );
    assert_eq!(
        response.server_name, "GoonZuMain",
        "NUL-terminated at offset 83 (FrontMsg.h:437)"
    );
    assert_eq!(
        response.account_status, 1,
        "i32 at offset 103 (FrontMsg.h:438)"
    );
    assert_eq!(response.real_age, 19, "u8 at offset 107 (FrontMsg.h:440)");

    // The bool gate is `!= 0`: a zeroed byte is false.
    let mut off = wire_response(0, 7, 0);
    off[82] = 0;
    off[103..107].copy_from_slice(&0i32.to_le_bytes());
    let off = parse_login_response(RESPONSE_LOGIN, &off).unwrap();
    assert!(!off.already_made_char, "byte 0 at offset 82 is false");

    // A frame-shaped parse: the same payload behind a real header, decoded
    // and dispatched through from_frame (opcode included).
    let mut decoder = Decoder::new();
    decoder
        .feed(&frame(RESPONSE_LOGIN, &payload))
        .expect("framing accepts the 113-byte frame");
    let frame = decoder
        .next_frame()
        .expect("no refusal")
        .expect("one frame");
    assert_eq!(frame.size(), 113, "usSize travels in the clear");
    let via_frame = LoginResponse::from_frame(&frame).unwrap();
    assert_eq!(via_frame, response, "both entry points agree");
}

#[test]
fn payload_length_boundaries_are_refused_exactly() {
    // Short of the struct: every length below 108 — the full sweep, so any
    // off-by-one in the layout arithmetic fails here. The legacy would read
    // siAccountStatus at offset 103 from these buffers
    // (Client-Winsock.cpp:285-287).
    for len in 0..LOGIN_RESPONSE_PAYLOAD_LEN {
        let payload = vec![0u8; len];
        let err = parse_login_response(RESPONSE_LOGIN, &payload).unwrap_err();
        assert!(
            matches!(
                err,
                LoginError::Truncated {
                    expected: 108,
                    got
                } if got == len
            ),
            "len {len}: expected Truncated {{ expected: 108, got: {len} }}, got {err:?}"
        );
    }

    // Exactly 108 parses.
    let exact = parse_login_response(RESPONSE_LOGIN, &wire_response(0, 5, 1)).unwrap();
    assert_eq!(exact.auth_key, 5);

    // One byte over is trailing — surfaced, never dropped (Step 3.2's
    // acceptance criterion carried into this step).
    for len in [109usize, 110, 113, 8000] {
        let mut payload = wire_response(0, 5, 1).to_vec();
        payload.resize(len, 0xAA);
        let err = parse_login_response(RESPONSE_LOGIN, &payload).unwrap_err();
        assert!(
            matches!(
                err,
                LoginError::TrailingBytes {
                    expected: 108,
                    got
                } if got == len
            ),
            "len {len}: expected TrailingBytes {{ expected: 108, got: {len} }}, got {err:?}"
        );
    }
}

#[test]
fn the_parser_refuses_foreign_opcodes() {
    let payload = wire_response(0, 5, 1);
    for cmd in [REQUEST_LOGIN, 2, 3, 1001, 1002, 30_000, 0, 0xFFFF] {
        let err = parse_login_response(cmd, &payload).unwrap_err();
        assert!(
            matches!(err, LoginError::UnexpectedOpcode { cmd: got } if got == cmd),
            "login parser accepted opcode {cmd}: {err:?}"
        );
    }

    // Opcode is checked before length: a foreign opcode with a short
    // payload reports the opcode, not the size.
    let err = parse_login_response(RESPONSE_WORLDLIST, &[0u8; 3]).unwrap_err();
    assert!(
        matches!(
            err,
            LoginError::UnexpectedOpcode {
                cmd: RESPONSE_WORLDLIST
            }
        ),
        "expected UnexpectedOpcode {{ cmd: 1002 }}, got {err:?}"
    );

    // The same through the Frame entry point: a 1002 frame is not a login.
    let mut decoder = Decoder::new();
    decoder
        .feed(&frame(RESPONSE_WORLDLIST, &[0x00, 0x00]))
        .expect("framing accepts the frame");
    let frame = decoder
        .next_frame()
        .expect("no refusal")
        .expect("one frame");
    let err = LoginResponse::from_frame(&frame).unwrap_err();
    assert!(
        matches!(
            err,
            LoginError::UnexpectedOpcode {
                cmd: RESPONSE_WORLDLIST
            }
        ),
        "from_frame accepted a 1002 frame: {err:?}"
    );
}

#[test]
fn capture_gates_the_auth_key_behind_both_rejection_checks() {
    // The legacy consumption order (Client-Winsock.cpp:287-350): the
    // siAccountStatus == -1 gate runs BEFORE the usResult switch.
    let gated = parse_login_response(RESPONSE_LOGIN, &wire_response(2, 0x0A0B_0C0D, -1)).unwrap();
    let err = gated.capture().unwrap_err();
    assert!(
        matches!(err, LoginError::AccountStatusRejected),
        "status -1 must beat the result switch: {err:?}"
    );

    // Every non-zero usResult is a refusal — and crucially, a failure frame
    // that still carries auth_key bytes must NOT yield state: the token
    // never moves without a full pass (dispatch scope item 4).
    for result in [1u16, 2, 3, 4, 5, 0xFF] {
        let rejected =
            parse_login_response(RESPONSE_LOGIN, &wire_response(result, 0x0BAD_F00D, 0)).unwrap();
        assert_eq!(
            rejected.auth_key, 0x0BAD_F00D,
            "parsing stays pure — the raw token is inspectable"
        );
        let err = rejected.capture().unwrap_err();
        assert!(
            matches!(err, LoginError::Rejected { result: r } if r == result),
            "result {result}: expected Rejected {{ result: {result} }}, got {err:?}"
        );
    }

    // Success: both gates pass, the token and the id echo are captured.
    let ok = parse_login_response(RESPONSE_LOGIN, &wire_response(0, 0x0102_0304, 1)).unwrap();
    let auth = ok.capture().expect("result 0, status 1");
    assert_eq!(
        auth.auth_key().get(),
        0x0102_0304,
        "the 4-byte handoff token for MOD-04 (Step 4.3)"
    );
    assert_eq!(
        auth.user_id(),
        "haru",
        "the canonical id echo the legacy copies over its input (Client-Winsock.cpp:377)"
    );
}

#[tokio::test]
async fn the_login_flow_round_trips_over_loopback() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        stream.write_all(&SECRETKEY_FRAME).await.unwrap();
        // Decode with the session cipher seeded by the very bytes the
        // handshake hands out — exactly what the front service does after
        // `Cipher::session_cipher(key1, key2)`.
        let mut cipher = Cipher::session_cipher(SEED_KEY1, SEED_KEY2);
        let (raw, plain) = read_ciphered_frame(&mut stream, &mut cipher).await;

        // usSize 65 in the clear, opcode 1 recovered from the plaintext.
        assert_eq!(
            u16::from_le_bytes([raw[0], raw[1]]),
            65,
            "login request usSize travels in the clear"
        );
        assert_eq!(plain.len(), 65);
        assert_eq!(
            u16::from_le_bytes([plain[3], plain[4]]),
            REQUEST_LOGIN,
            "opcode 1 recovered through the seeded cipher"
        );
        assert_eq!(
            &plain[5..],
            &expected_request_payload(),
            "userid/userpw/macaddress arrived bit-exact"
        );

        // F-04: 100 % cipher integrity — the credentials never appear in
        // the clear.
        assert_ne!(
            &raw[5..],
            &expected_request_payload()[..],
            "the outbound credentials must be enciphered, never plaintext"
        );

        // The answer both front servers send: usSize 113
        // (AuthProcess.cpp:88), usCmd 1000, the 108-byte packed struct.
        let response = frame(RESPONSE_LOGIN, &wire_response(0, 0x7EAD_C0DE, 1));
        assert_eq!(
            &response[..2],
            &[0x71, 0x00],
            "113 = 0x0071 LE (AuthProcess.cpp:88)"
        );
        stream.write_all(&response).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = FrontLink::connect(addr).await.unwrap();
    link.handshake().await.unwrap();
    request_login(&mut link, "haru", "s3cret", "A1B2C3D4E5F6")
        .await
        .unwrap();

    let frame = link
        .next_frame()
        .await
        .expect("no transport failure")
        .expect("the login response");
    assert_eq!(frame.cmd(), RESPONSE_LOGIN);
    assert_eq!(frame.size(), 113);

    let response = LoginResponse::from_frame(&frame).unwrap();
    assert_eq!(response.result, 0);
    let auth = response.capture().expect("result 0, status 1");
    assert_eq!(
        auth.auth_key().get(),
        0x7EAD_C0DE,
        "the captured handoff token matches the server's siAuthKey"
    );
    assert_eq!(auth.user_id(), "haru");

    server.await.unwrap();
}

#[tokio::test]
async fn a_request_before_the_handshake_is_refused_and_writes_nothing() {
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
    let err = request_login(&mut link, "haru", "s3cret", "")
        .await
        .unwrap_err();
    assert!(
        matches!(err, LoginError::Front(FrontError::NotKeyed)),
        "expected Front(NotKeyed) (the bFrontServerSecretKeySwitch gate, \
         Client-Winsock.cpp:141), got {err:?}"
    );
    drop(link);

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-key login must never reach the socket, got {received:02x?}"
    );
}
