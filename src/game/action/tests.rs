// Step 7 Phase 1 tests — outbound action encoders (WO §10: T1-T7, T13-T14).
//
// F-07: every expectation is a WO §8.3 pre-computed literal or a §8.2/§5
// constant citation — never derived from the encoder under test. The goldens
// (A-G) are hand-placed bytes from `docs/agents/WO-step7-in-world-actions.md`
// §8.3, the shared second source for bytes (D9); frame bytes are composed
// with the shared plaintext `frame` builder (`game/test_support.rs:28`),
// which encodes the header contract (`protocol/header.rs:45`) — neither the
// builders nor the encoders are their own expectation.
//
// F-01: the two send-path tests bind `127.0.0.1:0` only (loopback), prove
// the frame/opcode/payload/cipher pairing — not reachability. No external
// network, GPU, window, or display (WO §11).
use super::*;

use crate::game::entry::RESPONSE_CLIENTCHARUNIQUE;
use crate::game::test_support::{
    encode_link, frame, local_listener, HANDSHAKE_KEY1, HANDSHAKE_KEY2, HANDSHAKE_PAYLOAD,
};
use crate::net::cipher::Cipher;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// T1 — the constant table, each value with its §8/legacy citation (the
// `constants_match_the_wire_truth` pattern, `world_entry/tests.rs:176`).
#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        REQUEST_ORDER, 240,
        "MsgType.h:289 — GAMEMSG_REQUEST_USERORDER (D1: opcode identity retained)"
    );
    assert_eq!(
        RESPONSE_ORDER, 241,
        "MsgType.h:290 — GAMEMSG_RESPONSE_USERORDER's inbound twin (Phase 2, D8)"
    );
    assert_eq!(
        REQUEST_CHAT, 117,
        "MsgType.h:86 — GAMEMSG_REQUEST_CHAT (D1)"
    );
    assert_eq!(REQUEST_QUIT, 107, "MsgType.h:68 — HOPETOQUIT (D1)");
    assert_eq!(REQUEST_FRONT, 109, "MsgType.h:71 — HOPETOFRONT (D1)");
    assert_eq!(REQUEST_WARP, 146, "MsgType.h:144 — WARPVILLAGE (D1)");
    assert_eq!(
        REQUEST_NPCPOS, 5206,
        "MsgType.h:1420 — anchored at the 5200 family on :1411 (D1)"
    );

    assert_eq!(
        OrderKind::Move as u8,
        0,
        "§8.2/D3 — kind 0; CommonHeader.h:519 ORDER_MOVE(4) semantics inherited"
    );
    assert_eq!(
        OrderKind::Attack as u8,
        1,
        "§8.2/D3 — kind 1; CommonHeader.h:521 ORDER_ATTACK(5)/ORDER_ATTACK_CLIENT(18)"
    );
    assert_eq!(
        OrderKind::SitDown as u8,
        2,
        "§8.2/D3 — kind 2; CommonHeader.h:531 ORDER_SITDOWN(17); 3..=255 reserved \
         and unrepresentable on send"
    );

    assert_eq!(ChatMode::Normal as u8, 1, "MsgType-System.h:1085");
    assert_eq!(ChatMode::Secret as u8, 2, "MsgType-System.h:1086");
    assert_eq!(ChatMode::Town as u8, 3, "MsgType-System.h:1087");
    assert_eq!(ChatMode::Head as u8, 4, "MsgType-System.h:1088");
    assert_eq!(ChatMode::Home as u8, 5, "MsgType-System.h:1089");
    assert_eq!(ChatMode::Ask as u8, 6, "MsgType-System.h:1090");
    assert_eq!(
        ChatMode::System as u8,
        8,
        "MsgType-System.h:1091 — folds the legacy siSystemTxt field away (D4); \
         value 7 is not in the table"
    );
    assert_eq!(ChatMode::Villager as u8, 9, "MsgType-System.h:1092");
    assert_eq!(ChatMode::Party as u8, 10, "MsgType-System.h:1093");
    assert_eq!(ChatMode::King as u8, 11, "MsgType-System.h:1094");
    assert_eq!(ChatMode::NewsItem as u8, 12, "MsgType-System.h:1095");
    assert_eq!(ChatMode::Guild as u8, 13, "MsgType-System.h:1096");

    assert_eq!(
        MAX_CHAT_BYTES, 255,
        "§8.2/D6 — the byte cap; over it is a typed refusal, never a clamp"
    );
}

// T2 — Golden A (WO §8.3-A): payload `00 64 00 C8 00`, frame
// `0A 00 00 F0 00 00 64 00 C8 00`.
#[test]
fn move_order_is_golden_a_byte_for_byte() {
    let payload = encode_order(&Order::Move { x: 100, y: 200 });
    assert_eq!(
        payload,
        [0x00, 0x64, 0x00, 0xC8, 0x00],
        "WO §8.3-A payload: kind 00, x=0x0064 LE, y=0x00C8 LE (5 B)"
    );

    let bytes = frame(REQUEST_ORDER, &payload);
    assert_eq!(
        bytes,
        [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x64, 0x00, 0xC8, 0x00,],
        "WO §8.3-A frame: usSize 10, ucChk 0, cmd 240 = 0x00F0 (D2: header only)"
    );
}

// T3 — Goldens B/C (WO §8.3-B/C): the kind tag is the first byte and only
// the variant's fields follow (D3).
#[test]
fn attack_and_sitdown_are_goldens_b_and_c() {
    let attack = encode_order(&Order::Attack { target_char: 12345 });
    assert_eq!(
        attack,
        [0x01, 0x39, 0x30, 0x00, 0x00],
        "WO §8.3-B payload: kind 01, target 12345 = 0x00003039 LE (5 B)"
    );
    assert_eq!(
        frame(REQUEST_ORDER, &attack),
        [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x01, 0x39, 0x30, 0x00, 0x00,],
        "WO §8.3-B frame"
    );

    let sit = encode_order(&Order::SitDown);
    assert_eq!(sit, [0x02], "WO §8.3-C payload: the kind tag alone (1 B)");
    assert_eq!(
        frame(REQUEST_ORDER, &sit),
        [0x06, 0x00, 0x00, 0xF0, 0x00, 0x02],
        "WO §8.3-C frame: usSize 6 = 5 + 1"
    );

    // D3: a reserved kind (3..=255) is unrepresentable on send — every
    // encoded kind byte is one of the three §8.2 values, by construction.
    for order in [
        Order::Move { x: 0, y: 0 },
        Order::Attack { target_char: 0 },
        Order::SitDown,
    ] {
        let kind = encode_order(&order)[0];
        assert!(
            kind <= OrderKind::SitDown as u8,
            "D3 — reserved kind {kind:#04x} must never be emitted"
        );
    }
}

// T4 — D5: x then y, each a clean little-endian word, for the boundary
// tuples. The fixed offsets (1..3, 3..5) are §8.3-A's layout, so a word
// swap or a 10-bit superword packing (`CommonHeader.h:1974-1975`) fails
// the round trip.
#[test]
fn move_coordinates_survive_a_little_endian_round_trip() {
    for (x, y) in [(0u16, 0u16), (100, 200), (u16::MAX, u16::MAX)] {
        let payload = encode_order(&Order::Move { x, y });
        assert_eq!(
            payload.len(),
            5,
            "kind + two words, no padding (D4), for ({x}, {y})"
        );
        assert_eq!(
            payload[0],
            OrderKind::Move as u8,
            "kind tag first (D3) for ({x}, {y})"
        );
        let rx = u16::from_le_bytes([payload[1], payload[2]]);
        let ry = u16::from_le_bytes([payload[3], payload[4]]);
        assert_eq!(
            (rx, ry),
            (x, y),
            "D5 — x at bytes 1..3, y at 3..5, both LE, for ({x}, {y}): a word \
             swap or the legacy superword packing changes these bytes"
        );
    }
}

// T5 — Golden D (WO §8.3-D): `mode ‖ text`, no siDataSize, no NUL, and a
// frame whose usSize is exactly 6 + text.len() (D2/D6/D7).
#[test]
fn chat_payload_is_golden_d_mode_then_utf8_text() {
    let payload = encode_chat(ChatMode::Normal, "hi").expect("golden chat encodes");
    assert_eq!(
        payload,
        [0x01, 0x68, 0x69],
        "WO §8.3-D payload: mode 01, then 'h' 'i' (3 B)"
    );
    assert_eq!(
        payload.len(),
        1 + "hi".len(),
        "D2/D6 — no siDataSize prefix and no NUL terminator: length comes \
         from the frame alone"
    );

    let bytes = frame(REQUEST_CHAT, &payload);
    assert_eq!(
        bytes,
        [0x08, 0x00, 0x00, 0x75, 0x00, 0x01, 0x68, 0x69],
        "WO §8.3-D frame: cmd 117 = 0x0075, usSize 8 = 6 + 2"
    );
    assert_eq!(
        u16::from_le_bytes([bytes[0], bytes[1]]),
        6 + "hi".len() as u16,
        "D2 — usSize == 6 + text.len()"
    );
}

// T6 — D6 refusals by name: `EmptyChat`, `ChatTooLong { len: 256, max:
// 255 }` (WO §8.3 refusal fixtures) — never a clamp, never silent.
#[test]
fn chat_refusals_are_typed_and_never_clamp() {
    let err = encode_chat(ChatMode::Normal, "").expect_err("empty chat is refused");
    assert!(
        matches!(err, ActionError::EmptyChat),
        "expected ActionError::EmptyChat, got {err:?}"
    );

    let text_256 = "x".repeat(MAX_CHAT_BYTES + 1);
    let err = encode_chat(ChatMode::Normal, &text_256).expect_err("256 B is over cap");
    assert!(
        matches!(err, ActionError::ChatTooLong { len: 256, max: 255 }),
        "expected ActionError::ChatTooLong {{ len: 256, max: 255 }}, got {err:?}"
    );

    // The cap is inclusive at exactly 255 bytes (§8.2).
    assert!(
        encode_chat(ChatMode::Normal, &"x".repeat(MAX_CHAT_BYTES)).is_ok(),
        "255 bytes must still encode — the cap is >255, not >=255"
    );

    // D6 counts bytes, not chars: 128 × 'é' is 256 UTF-8 bytes — refused
    // whole, not truncated to fit (D7: strict UTF-8, whole-text refusals).
    let multi_byte: String = "é".repeat(128);
    assert_eq!(multi_byte.len(), 256, "128 × 'é' = 256 UTF-8 bytes");
    let err = encode_chat(ChatMode::Secret, &multi_byte).expect_err("byte cap applies");
    assert!(
        matches!(err, ActionError::ChatTooLong { len: 256, max: 255 }),
        "expected byte-based ActionError::ChatTooLong, got {err:?}"
    );
}

// T7 — Goldens E/F/G (WO §8.3-E/F/G): quit/front are the bare 5-byte
// frames; warp/npcpos carry a 2-byte LE payload.
#[test]
fn quit_front_warp_and_npcpos_match_goldens_e_f_g() {
    let quit = frame(REQUEST_QUIT, &EMPTY_BODY);
    assert_eq!(
        quit,
        [0x05, 0x00, 0x00, 0x6B, 0x00],
        "WO §8.3-E — quit: usSize 5, cmd 107 = 0x006B, empty body (D2)"
    );

    let front = frame(REQUEST_FRONT, &EMPTY_BODY);
    assert_eq!(
        front,
        [0x05, 0x00, 0x00, 0x6D, 0x00],
        "WO §8.3-E — front: usSize 5, cmd 109 = 0x006D, empty body"
    );

    let warp = encode_warp(3);
    assert_eq!(warp, [0x03, 0x00], "WO §8.3-F payload: village 3 LE (2 B)");
    assert_eq!(
        frame(REQUEST_WARP, &warp),
        [0x07, 0x00, 0x00, 0x92, 0x00, 0x03, 0x00],
        "WO §8.3-F frame: cmd 146 = 0x0092, usSize 7"
    );

    let npc = encode_npc_pos(5);
    assert_eq!(npc, [0x05, 0x00], "WO §8.3-G payload: kind 5 LE (2 B)");
    assert_eq!(
        frame(REQUEST_NPCPOS, &npc),
        [0x07, 0x00, 0x00, 0x56, 0x14, 0x05, 0x00],
        "WO §8.3-G frame: cmd 5206 = 0x1456 LE, usSize 7"
    );

    // Q-1: npcpos kind is an opaque server-defined u16 — any value encodes;
    // neither endpoint may hold a "known set" the other does not.
    assert_eq!(
        encode_npc_pos(u16::MAX),
        [0xFF, 0xFF],
        "Q-1 — no client-side enumeration exists for the npcpos kind"
    );
}

/// Reads one complete client→server frame the way the service reads it: the
/// in-clear `usSize` selects the frame length (`world_entry/tests.rs:725`).
async fn read_frame(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.unwrap();
    let size = usize::from(u16::from_le_bytes(head));
    let mut raw = vec![0u8; size];
    raw[..2].copy_from_slice(&head);
    stream.read_exact(&mut raw[2..]).await.unwrap();
    raw
}

// T13 — the send gate (`entry.rs:530-535`, cf. `world_entry/tests.rs:850`
// and `heartbeat/tests.rs:234`): before the opcode 1 handshake the call is
// refused and the socket never sees a byte.
#[tokio::test]
async fn a_pre_handshake_action_send_writes_nothing() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    // No handshake: the bGameServerSecretKeySwitch gate refuses the whole
    // call before the cipher is reached (Client-Winsock.cpp:171).
    let err = send_order(&mut link, &Order::Move { x: 1, y: 2 })
        .await
        .unwrap_err();
    assert!(
        matches!(err, GameError::NotHandshaken),
        "expected NotHandshaken before the opcode 1 handshake, got {err:?}"
    );
    let err = send_chat(&mut link, ChatMode::Normal, "hi")
        .await
        .unwrap_err();
    assert!(
        matches!(err, ActionError::Send(GameError::NotHandshaken)),
        "expected ActionError::Send(NotHandshaken), got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused action must not key or tick the outbound cipher"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "a pre-handshake action must never reach the socket, got {received:02x?}"
    );
}

// T14 — encode → session-cipher wire → peer read-back: both frames come
// through bit-exact against the §8.3 goldens (the `world_entry` round-trip
// pattern, `:735`; keys tick per call, `net/cipher.rs:23`).
#[tokio::test]
async fn order_and_chat_survive_the_session_cipher_bit_exact() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // The unsolicited opcode 1 handshake, link-encoded, as the service
        // writes it at accept (`entry.rs` handshake path).
        stream
            .write_all(&encode_link(&frame(
                RESPONSE_CLIENTCHARUNIQUE,
                &HANDSHAKE_PAYLOAD,
            )))
            .await
            .unwrap();
        let order = read_frame(&mut stream).await;
        let chat = read_frame(&mut stream).await;
        (order, chat)
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    send_order(&mut link, &Order::Move { x: 100, y: 200 })
        .await
        .expect("move order");
    send_chat(&mut link, ChatMode::Normal, "hi")
        .await
        .expect("chat");
    let (raw_order, raw_chat) = server.await.unwrap();

    // usSize travelled in the clear: 10 for the order, 8 for the chat.
    assert_eq!(
        u16::from_le_bytes([raw_order[0], raw_order[1]]),
        10,
        "5 + 5 — the order frame's header size (MsgType.h:289, D2)"
    );
    assert_eq!(
        u16::from_le_bytes([raw_chat[0], raw_chat[1]]),
        8,
        "6 + 2 — the chat frame's header size (D2)"
    );

    // Decode both with the negotiated session keys — one cipher, ticking
    // per frame exactly as the link's encoder did.
    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);

    let mut plain = vec![0u8; raw_order.len()];
    assert!(
        cipher.decode(&raw_order, &mut plain).unwrap(),
        "the negotiated keys must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        REQUEST_ORDER,
        "opcode 240 recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        &[0x00, 0x64, 0x00, 0xC8, 0x00],
        "WO §8.3-A payload recovered bit-exact — golden A on the wire"
    );

    let mut plain = vec![0u8; raw_chat.len()];
    assert!(
        cipher.decode(&raw_chat, &mut plain).unwrap(),
        "the second frame keys must still satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        REQUEST_CHAT,
        "opcode 117 recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        &[0x01, 0x68, 0x69],
        "WO §8.3-D payload recovered bit-exact — golden D on the wire"
    );
}
