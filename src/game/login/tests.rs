// Step 4.3 tests — falsifiable, byte-exact.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): every address here is
// loopback synthetic — pending live ingestion against the real Game Server
// (4010). What these tests prove is the 53-byte frame, its opcode, its
// bit-exact payload and its session-cipher pairing, not reachability.
//
// F-07: expectation sources are never the unit under test — the expected 48
// bytes are hand-placed from the `cltLogInInfo` field table
// (`CommonHeader.h:1442-1452` + `MAX_PLAYER_NAME` 20 at `:575`), the server
// view is an independent decoder written from `GameLogin::decode`'s reading
// order (`gz-proto/src/game.rs`: `i32`, `fixed_str(20)`, `fixed_str(20)`,
// `u8`), and the `FrontAuth` fed to the sender is produced by the real Step
// 3.3 capture path (`parse_login_response` → `capture`) — not a mock.
use super::*;
use crate::auth::login::{parse_login_response, LOGIN_RESPONSE_PAYLOAD_LEN, RESPONSE_LOGIN};
use crate::game::entry::RESPONSE_CLIENTCHARUNIQUE;
use crate::net::cipher::Cipher;
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
/// what the peer *means*; on the client→server half it only ever exists
/// before `Cipher::encode`.
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
/// `0x12,0x34` — what the game service builds per session
/// (`services/game/src/main.rs`, the `outbound` cipher) and writes the
/// opcode 1 handshake with.
fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// The handshake body as the server writes it: `char_unique` 42 LE, session
/// seeds `0x7E/0xAD`, `version` 1 LE — the same literals the Step 4.2 suite
/// pins, restated here so this module proves its own pairing.
const HANDSHAKE_PAYLOAD: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];
const HANDSHAKE_KEY1: u8 = 0x7E;
const HANDSHAKE_KEY2: u8 = 0xAD;

/// The account and password every payload test uses.
const ACCOUNT: &str = "haru";
const PASSWORD: &str = "s3cret";
/// All four bytes distinct and non-zero, so an offset shift cannot go
/// unnoticed. The server compares it to the account's `idnum`
/// (`admit_game_login`, `services/game/src/auth.rs`).
const AUTH_KEY: i32 = 0x0A0B_0C0D;

/// The 48-byte expectation, hand-placed from the field table — never built
/// by `game_login_payload` itself:
///
/// | Offset | Field |
/// |---|---|
/// | 0..4 | `siAuthKey` (i32 LE) |
/// | 4..24 | `szID[20]` (C-string, zero-filled) |
/// | 24..44 | `szPassword[20]` (C-string, zero-filled) |
/// | 44 | `uiGameMode` (0 = windowed) |
/// | 45..48 | MSVC `/Zp8` tail pad (3 zero bytes) |
fn expected_payload(auth_key: i32, account: &str, password: &str) -> [u8; GAME_LOGIN_PAYLOAD_LEN] {
    let mut expected = [0u8; GAME_LOGIN_PAYLOAD_LEN];
    expected[0..4].copy_from_slice(&auth_key.to_le_bytes());
    expected[4..4 + account.len()].copy_from_slice(account.as_bytes());
    expected[24..24 + password.len()].copy_from_slice(password.as_bytes());
    // 44 stays 0 (windowed), 45..48 stay 0 (struct pad).
    expected
}

/// An independent server-side view of the body, written from the server
/// port's `GameLogin::decode` reading order (`gz-proto/src/game.rs`):
/// `i32` auth_key, `fixed_str(20)` userid, `fixed_str(20)` password, `u8`
/// game_mode. The C-string rule — stop at the first NUL — is what
/// `fixed_str` applies. Returns `(auth_key, userid, password, game_mode)`.
fn decode_like_the_server(payload: &[u8]) -> (i32, String, String, u8) {
    let auth_key = i32::from_le_bytes(payload[0..4].try_into().expect("4 bytes"));
    let cstr = |at: usize| {
        let field = &payload[at..at + 20];
        let end = field.iter().position(|&b| b == 0).unwrap_or(20);
        String::from_utf8_lossy(&field[..end]).into_owned()
    };
    (
        auth_key,
        cstr(4),
        cstr(24),
        payload[44], // game_mode; 45..48 are struct pad the decode never reads
    )
}

/// The `FRONTMSG_RESPONSE_LOGIN` body a successful front login answers with,
/// at the literal offsets `parse_login_response` reads: `usResult` 0 at 0..2,
/// `siAuthKey` at 2..6, `userid[20]` at 6..26, `siAccountStatus` 1 at 103..107
/// (`FrontMsg.h:419-442`; the port sends status 1 on success).
fn front_login_response(auth_key: i32, user_id: &str) -> [u8; LOGIN_RESPONSE_PAYLOAD_LEN] {
    let mut p = [0u8; LOGIN_RESPONSE_PAYLOAD_LEN];
    p[0..2].copy_from_slice(&0u16.to_le_bytes()); // usResult = 0
    p[2..6].copy_from_slice(&auth_key.to_le_bytes()); // siAuthKey
    p[6..6 + user_id.len()].copy_from_slice(user_id.as_bytes()); // userid
    p[103..107].copy_from_slice(&1i32.to_le_bytes()); // siAccountStatus = 1
    p
}

/// A `FrontAuth` produced by the real Step 3.3 path: strict parse, then the
/// capture gate (`result == 0`, `account_status != -1`). No test-only
/// constructor exists for `FrontAuth`, by design.
fn front_auth(auth_key: i32, user_id: &str) -> FrontAuth {
    parse_login_response(RESPONSE_LOGIN, &front_login_response(auth_key, user_id))
        .expect("well-formed login response")
        .capture()
        .expect("result 0, status 1")
}

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        REQUEST_LOGIN, 2,
        "MsgType.h:22-23 — GAMEMSG_REQUEST_LOGIN follows \
         GAMEMSG_RESPONSE_CLIENTCHARUNIQUE = 1 (the server port's generated \
         GameMsg::RequestLogin = 2, opcodes_game.rs, same header)"
    );
    assert_eq!(
        GAME_LOGIN_PAYLOAD_LEN, 48,
        "CommonHeader.h:1442-1452 + MAX_PLAYER_NAME 20 at :575 — 4 + 20 + 20 \
         + 1 = 45 meaningful bytes under /Zp8 rounded to 48 (the server \
         port's GAME_LOGIN_PAYLOAD_LEN = 48, game.rs:29-30, 'including its 3 \
         trailing pad bytes')"
    );
    assert_eq!(
        PLAYER_NAME_LEN, 20,
        "CommonHeader.h:575 — MAX_PLAYER_NAME (the server port's \
         PLAYER_NAME_LEN, game.rs)"
    );
    assert_eq!(
        GAME_MODE_WINDOWED, 0,
        "CommonHeader.h:1447-1448 — uiGameMode 0 = windowed; the legacy's \
         !m_bFullScreenMode branch (LogIn.cpp:994-996)"
    );
    assert_eq!(
        5 + GAME_LOGIN_PAYLOAD_LEN,
        53,
        "MsgType.h:2967 — cltMsg's constructor sets usSize = \
         sizeof(sPacketHeader) + datasize = 5 + 48. 53 is far below the \
         200-byte LZF threshold (Client-Winsock.cpp:178-186), so this frame \
         travels uncompressed"
    );
}

#[test]
fn the_payload_is_byte_exact_at_literal_offsets() {
    let payload = game_login_payload(AuthKey::new(AUTH_KEY), ACCOUNT, PASSWORD).expect("fits");

    // The hand-built expectation (F-07), then the individual offsets from the
    // field table so a shift cannot hide behind a matching helper.
    assert_eq!(
        payload,
        expected_payload(AUTH_KEY, ACCOUNT, PASSWORD),
        "all 48 bytes as cltLogInInfo lays them out"
    );
    assert_eq!(
        payload[0..4],
        AUTH_KEY.to_le_bytes(),
        "siAuthKey at 0..4 (CommonHeader.h:1444)"
    );
    assert_eq!(
        &payload[4..8],
        b"haru",
        "szID starts at 4 (CommonHeader.h:1445)"
    );
    assert_eq!(
        &payload[24..30],
        b"s3cret",
        "szPassword starts at 24 (CommonHeader.h:1446)"
    );
    assert_eq!(payload[44], 0, "uiGameMode at 44 (CommonHeader.h:1447)");
    assert_eq!(
        payload[45..48],
        [0u8; 3],
        "the 3 /Zp8 tail pad bytes travel as zero"
    );

    // The server's own reading order recovers every field (game.rs
    // GameLogin::decode) — the acceptance criterion is that the port can
    // name this account without an "unknown account" rejection.
    assert_eq!(
        decode_like_the_server(&payload),
        (AUTH_KEY, ACCOUNT.to_owned(), PASSWORD.to_owned(), 0),
        "auth_key, userid, password and game_mode read back at their offsets"
    );
}

#[test]
fn strings_stop_at_their_terminator_and_zero_fill_the_rest() {
    // `MStrCpy` is `StringCchCopy` (Lib\commonutil.cpp:6-16): it copies up
    // to the terminator. Bytes after an embedded NUL are the legacy's
    // uninitialized buffer, never transmitted as part of the string.
    let payload =
        game_login_payload(AuthKey::new(AUTH_KEY), "haru\0junk", "s3cret\0more").expect("fits");

    // The expectations are each string zero-filled to MAX_PLAYER_NAME — the
    // field width from CommonHeader.h:575, built here rather than by the
    // unit under test (F-07).
    let mut id_field = [0u8; PLAYER_NAME_LEN];
    id_field[..4].copy_from_slice(b"haru"); // "haru", the rest zero
    assert_eq!(
        &payload[4..24],
        &id_field,
        "szID is 'haru' zero-filled to 20 — the junk after the NUL is not \
         part of the string"
    );
    let mut pw_field = [0u8; PLAYER_NAME_LEN];
    pw_field[..6].copy_from_slice(b"s3cret"); // "s3cret", the rest zero
    assert_eq!(
        &payload[24..44],
        &pw_field,
        "szPassword is 's3cret' zero-filled to 20"
    );
    assert_eq!(
        decode_like_the_server(&payload),
        (AUTH_KEY, ACCOUNT.to_owned(), PASSWORD.to_owned(), 0),
        "the server recovers the strings up to their terminators"
    );
}

#[test]
fn a_credential_without_room_for_its_terminator_is_refused() {
    // 19 bytes + the NUL is the most a 20-byte field can carry.
    let nineteen = "a".repeat(19);
    assert!(
        game_login_payload(AuthKey::new(AUTH_KEY), &nineteen, PASSWORD).is_ok(),
        "19 bytes fit a 20-byte field with its terminator"
    );
    assert!(
        game_login_payload(AuthKey::new(AUTH_KEY), ACCOUNT, &nineteen).is_ok(),
        "19 bytes fit a 20-byte field with its terminator"
    );

    // 20 bytes cannot — refused, not truncated (module docs, refuse not
    // truncate: StringCchCopy would send a 19-byte stump with the return
    // value discarded at Common.cpp:871-872).
    let twenty = "b".repeat(20);
    let err = game_login_payload(AuthKey::new(AUTH_KEY), &twenty, PASSWORD).unwrap_err();
    assert!(
        matches!(
            err,
            GameLoginError::CredentialTooLong {
                field: "szID",
                len: 20
            }
        ),
        "an over-long szID must name the field and its length, got {err:?}"
    );
    let err = game_login_payload(AuthKey::new(AUTH_KEY), ACCOUNT, &twenty).unwrap_err();
    assert!(
        matches!(
            err,
            GameLoginError::CredentialTooLong {
                field: "szPassword",
                len: 20
            }
        ),
        "an over-long szPassword must name the field and its length, got {err:?}"
    );
}

#[test]
fn every_auth_key_shape_travels_little_endian() {
    // The server decodes auth_key as a signed i32 and compares it to the
    // account's idnum (game.rs GameLogin::decode; admit_game_login), so
    // every i32 — including the negative and the extreme — must survive the
    // wire unchanged.
    for auth_key in [0, 1, AUTH_KEY, -1, i32::MIN, i32::MAX] {
        let payload = game_login_payload(AuthKey::new(auth_key), ACCOUNT, PASSWORD).expect("fits");
        assert_eq!(
            payload[0..4],
            auth_key.to_le_bytes(),
            "siAuthKey {auth_key} as LE bytes at 0..4"
        );
        let (decoded, user_id, password, game_mode) = decode_like_the_server(&payload);
        assert_eq!(
            (decoded, user_id.as_str(), password.as_str(), game_mode),
            (auth_key, ACCOUNT, PASSWORD, 0),
            "auth_key {auth_key} survives encode → decode bit-exact"
        );
    }
}

#[tokio::test]
async fn the_login_frame_is_53_bytes_enciphered_with_the_session_cipher() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // The unsolicited opcode 1 handshake, link-encoded, carrying the
        // session seeds — written before any client read, as the service
        // writes it at accept.
        stream
            .write_all(&encode_link(&frame(
                RESPONSE_CLIENTCHARUNIQUE,
                &HANDSHAKE_PAYLOAD,
            )))
            .await
            .unwrap();
        // The client's login frame, read the way the service reads: usSize
        // in the clear selects the frame length.
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
    // The account and key come from the real Step 3.3 capture path, so this
    // also proves the handoff: what the front login captured is what the
    // game login transmits.
    let auth = front_auth(AUTH_KEY, ACCOUNT);
    request_game_login(&mut link, &auth, PASSWORD)
        .await
        .expect("game login");
    let raw = server.await.unwrap();

    // usSize travelled in the clear: 5 + 48 = 53 (MsgType.h:2967).
    assert_eq!(
        u16::from_le_bytes([raw[0], raw[1]]),
        53,
        "the frame is exactly sizeof(sPacketHeader) + sizeof(cltLogInInfo)"
    );
    // The body did not travel in the clear — the F-04 property on outbound.
    assert_ne!(
        &raw[5..],
        &expected_payload(AUTH_KEY, ACCOUNT, PASSWORD)[..],
        "client→server must be enciphered with the negotiated session keys, \
         not plaintext"
    );

    // Decode with a *fresh* session cipher seeded by the handshake bytes —
    // the pairing the service uses (outbound link cipher, inbound session
    // cipher; services/game/src/main.rs).
    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap(),
        "the negotiated keys must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        REQUEST_LOGIN,
        "opcode 2 recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        &expected_payload(AUTH_KEY, ACCOUNT, PASSWORD)[..],
        "the 48 payload bytes recovered bit-exact through encode → wire → \
         decode, carrying the front's auth_key and account"
    );

    // Both directions of wrong must fail.
    let mut wrong_key = Cipher::session_cipher(HANDSHAKE_KEY1.wrapping_add(1), HANDSHAKE_KEY2);
    let mut scratch = vec![0u8; raw.len()];
    assert!(
        !wrong_key.decode(&raw, &mut scratch).unwrap_or(false),
        "an off-by-one key must fail the integrity check"
    );
    assert!(
        !Cipher::link_cipher()
            .decode(&raw, &mut scratch)
            .unwrap_or(false),
        "the link cipher must not decode the client→server half — outbound \
         is session-keyed, inbound is link-keyed (main.rs's cipher pairing)"
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
    let auth = front_auth(AUTH_KEY, ACCOUNT);
    // Build succeeds (the payload is pure); the send is what the legacy's
    // bGameServerSecretKeySwitch gate refuses (Client-Winsock.cpp:171),
    // surfaced through GameLink::send as NotHandshaken.
    let err = request_game_login(&mut link, &auth, PASSWORD)
        .await
        .unwrap_err();
    assert!(
        matches!(err, GameLoginError::Game(GameError::NotHandshaken)),
        "expected NotHandshaken before the opcode 1 handshake, got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused login must not key or tick the outbound cipher"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-handshake login must never reach the socket, got {received:02x?}"
    );
}

#[tokio::test]
async fn an_over_long_credential_sends_nothing() {
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
        // Nothing must follow the handshake.
        let mut received = Vec::new();
        let n = tokio::time::timeout(
            Duration::from_millis(200),
            stream.read_to_end(&mut received),
        )
        .await
        .expect("the client must close rather than send a truncated login");
        assert!(n.is_ok(), "read_to_end completed");
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    let auth = front_auth(AUTH_KEY, ACCOUNT);
    let err = request_game_login(&mut link, &auth, &"p".repeat(20))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            GameLoginError::CredentialTooLong {
                field: "szPassword",
                len: 20
            }
        ),
        "expected CredentialTooLong for szPassword, got {err:?}"
    );
    assert!(
        link.outbound_keys().is_some()
            && link.outbound_keys() == Some((HANDSHAKE_KEY1, HANDSHAKE_KEY2)),
        "a refused build must not tick the cipher — nothing was encoded"
    );
    drop(link);

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "the refused login must write no bytes, got {received:02x?}"
    );
}
