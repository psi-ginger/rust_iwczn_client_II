// README Step 4.5–4.7 tests — falsifiable, byte-exact.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): the parsers are pure,
// so nothing here touches a socket — what they prove is that every field is
// read at the offset its field table names, that the length and opcode gates
// fire before any field is touched, and that the region gate drops rather
// than errors. The two send-path tests run loopback (synthetic address) and
// prove the 9-byte frame, its opcode, its bit-exact payload and its
// session-cipher pairing — not reachability.
//
// F-07: expectation sources are never the unit under test — every payload
// below is hand-placed from the field tables (`MsgType-System.h:1168-1197`,
// `MsgType.h:3085-3102`, `MsgType.h:2977-2984`, `CommonHeader.h:1967-1982`,
// and the server port's own offset comments `game.rs:990-1015`), built with
// independent byte math rather than by any function under test.
use super::*;
use crate::game::char_info::{MAKE_CHAR_INFO_PAYLOAD_LEN, RESPONSE_LOGIN};
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
/// `0x12,0x34` — what the game service builds per session.
fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// The handshake body as the server writes it: `char_unique` 42 LE, session
/// seeds `0x7E/0xAD`, `version` 1 LE — the same literals the Step 4.2/4.3
/// suites pin, restated so this module proves its own pairing.
const HANDSHAKE_PAYLOAD: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];
const HANDSHAKE_KEY1: u8 = 0x7E;
const HANDSHAKE_KEY2: u8 = 0xAD;

/// The session character id the handshake hands out — the value this suite
/// holds `WorldEntryState` to, so an opcode-5 record must match it.
const MY_CHAR_UNIQUE: i32 = 42;

/// The 48-byte SERVERINFO body, hand-placed from the field table
/// (`MsgType-System.h:1168-1197`; offsets per the server port's
/// `ServerInfo::encode`, `game.rs:990-1015`). Every modeled field gets a
/// distinctive value; the pad bytes at 11 and 43..48 carry junk a shifted
/// read would trip over.
fn server_info_payload(service_area: i16) -> [u8; SERVER_INFO_PAYLOAD_LEN] {
    let mut p = [0u8; SERVER_INFO_PAYLOAD_LEN];
    p[0..2].copy_from_slice(&service_area.to_le_bytes()); // siServiceArea
    p[2..4].copy_from_slice(&7i16.to_le_bytes()); // siServerUnique
    p[4] = 1; // bItemMallUseSwitch
    p[5] = 0; // bVillageWarSwitch
    p[6] = 1; // bBonusTimeSwitch
    p[7] = 0; // bBonus2TimeSwitch
    p[8] = 1; // bSpecialQuestBonusTimeSwitch
    p[9] = 0; // bBroadcastSwitch
    p[10] = 9i8 as u8; // siBestVillageLevel
    p[11] = 0xEE; // alignment pad before siNPCPriceRate
    p[12..16].copy_from_slice(&150i32.to_le_bytes()); // siNPCPriceRate
    p[16..24].copy_from_slice(&0x1122_3344_5566_7788i64.to_le_bytes()); // siItemPriceIndex
    for (i, slot) in p[24..32].iter_mut().enumerate() {
        // bEventSwitch[MAX_EVENT_NUMBER]: alternating, so a swapped width shows.
        *slot = u8::from(i % 2 == 0);
    }
    p[32..36].copy_from_slice(&30i32.to_le_bytes()); // siMinimumTeacherLevel
    p[36..38].copy_from_slice(&(-2i16).to_le_bytes()); // siServiceAge
    p[38] = 1i8 as u8; // siEnablePVP
    p[39] = 1; // bAutoUnblockMode
    p[40] = 0; // bVoteCampaign
    p[41] = 1; // bEventToggleSwitch[0]
    p[42] = 0; // bEventToggleSwitch[1]
    p[43] = 0xDD; // trailing struct pad
    p[44] = 0xCC;
    p[45] = 0xBB;
    p[46] = 0xAA;
    p[47] = 0x99;
    p
}

/// The expectation [`server_info_payload`] implies, built independently
/// (F-07).
fn expected_server_info(service_area: i16) -> ServerInfo {
    ServerInfo {
        service_area,
        server_unique: 7,
        item_mall_use: true,
        village_war: false,
        bonus_time: true,
        bonus2_time: false,
        special_quest_bonus_time: true,
        broadcast: false,
        best_village_level: 9,
        npc_price_rate: 150,
        item_price_index: 0x1122_3344_5566_7788,
        event_switch: [true, false, true, false, true, false, true, false],
        minimum_teacher_level: 30,
        service_age: -2,
        enable_pvp: 1,
        auto_unblock_mode: true,
        vote_campaign: false,
        event_toggle_switch: [true, false],
    }
}

/// The 22-byte VILLAGESTRUCT body, hand-placed from `MsgType.h:3085-3102`:
/// `SI16 VillageUnique` + `SI16 siVillageStructureInfo[10]`.
fn village_struct_payload(village_unique: i16) -> [u8; VILLAGE_STRUCT_PAYLOAD_LEN] {
    let mut p = [0u8; VILLAGE_STRUCT_PAYLOAD_LEN];
    p[0..2].copy_from_slice(&village_unique.to_le_bytes());
    for (index, slot) in p[2..].as_chunks_mut::<2>().0.iter_mut().enumerate() {
        // Distinct signed values across the ten slots — a stride or offset
        // slip cannot produce this sequence by accident.
        let value =
            i16::try_from(i32::try_from(index).expect("fits i32") * 11 - 30).expect("fits i16");
        slot.copy_from_slice(&value.to_le_bytes());
    }
    p
}

/// A 592-byte opcode-5 `cltMakeCharInfo` body for `unique`, hand-placed from
/// the field table (`CommonHeader.h:1969-1977`, binary offsets per
/// `game.rs:986-1003`): the bitfields low-bits-first, the tail fields at
/// their measured offsets. Only the fields `CharClient` models need real
/// values; the rest carry junk a shifted read would trip over.
fn make_char_body(unique: u16) -> [u8; MAKE_CHAR_INFO_PAYLOAD_LEN] {
    let mut p = [0u8; MAKE_CHAR_INFO_PAYLOAD_LEN];

    // 0x00: uiType:1 | uiMakeCharKind:16.
    let type_kind = MAKECHARTYPE_PC_WIRE | (u32::from(0x1234u16) << 1);
    // 0x04: uiMakeCharUnique:16 | uiLevel:8 | pad:8.
    let unique_level = u32::from(unique) | (u32::from(57u8) << 16);
    // 0x08: uiLife:20 | uiX:10 | pad:2.
    let life_x = 3_210u32 | (u32::from(0x155u16) << 20);
    // 0x0C: uiY:10 | uiMapIndex:10 | uiTotalMoveSpeed:6 | pad:6.
    let y_map_speed = u32::from(0x2AAu16) | (u32::from(17u16) << 10) | (u32::from(0x3Fu8) << 20);
    p[0..4].copy_from_slice(&type_kind.to_le_bytes());
    p[4..8].copy_from_slice(&unique_level.to_le_bytes());
    p[8..12].copy_from_slice(&life_x.to_le_bytes());
    p[12..16].copy_from_slice(&y_map_speed.to_le_bytes());

    p[0x10] = 5; // siHomeVillageUnique (5)
    p[0x11] = 0xEE; // SI16 alignment pad
    p[0x12..0x14].copy_from_slice(&0i16.to_le_bytes()); // siJimCharUnique
    p[0x14..0x38].fill(b'O'); // clOrder, clIdentity, siStatus
    p[0x38..0x3A].copy_from_slice(&0x0016u16.to_le_bytes()); // clEnemyType
    p[0x3A..0x50].fill(b'P'); // siPara1, clEventInfo, siTopSkillUnique
    p[0x50] = 0; // siCurrentShowHorse
    p[0x51] = 2; // siGameMasterLevel
    p[0x52..0x54].copy_from_slice(&(-1i16).to_le_bytes()); // siMapUnique
    p[0x54..0x92].fill(b'Q'); // clShapeEquip, clRank
    p[0x92..0x94].copy_from_slice(&0i16.to_le_bytes()); // siMakeAniType
    p[0x94..].fill(b'R'); // clGuildRank + tail pad
    p
}

/// `MAKECHARTYPE_PC`'s wire value — restated here (not imported) so the test
/// body builds the bit it is asserting against independently (F-07).
const MAKECHARTYPE_PC_WIRE: u32 = 1;

/// The `CharClient` the record from [`make_char_body`] must produce, written
/// by hand from the field table (F-07).
fn expected_char_client(unique: u16) -> CharClient {
    CharClient {
        char_unique: unique,
        kind: 0x1234,
        level: 57,
        life: 3_210,
        x: 0x155,
        y: 0x2AA,
        map_index: 17,
        move_speed: 0x3F,
        home_village: 5,
        gm_level: 2,
        map_unique: -1,
    }
}

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        REQUEST_SELECTCHAR, 4,
        "MsgType.h:26 — GAMEMSG_REQUEST_SELECTCHAR (the port's \
         GameMsg::RequestSelectchar = 4, opcodes_game.rs:21, same header)"
    );
    assert_eq!(
        RESPONSE_SERVERINFO, 106,
        "MsgType.h:66 — GAMEMSG_RESPONSE_SERVERINFO's explicit 106 (the port's \
         GameMsg::ResponseServerinfo = 106, opcodes_game.rs:65)"
    );
    assert_eq!(
        REQUEST_VILLAGESTRUCT, 1101,
        "MsgType.h:589 — GAMEMSG_REQUEST_VILLAGESTRUCT (the port's \
         GameMsg::RequestVillagestruct = 1101, opcodes_game.rs:639)"
    );
    assert_eq!(
        RESPONSE_VILLAGESTRUCT, 1102,
        "MsgType.h:590 — GAMEMSG_RESPONSE_VILLAGESTRUCT (the port's \
         GameMsg::ResponseVillagestruct = 1102, opcodes_game.rs:641)"
    );
    assert_eq!(
        SELECT_CHAR_PAYLOAD_LEN, 4,
        "MsgType.h:2977-2984 — the two-arg cltMsg constructor writes a bare \
         SI32 (the port's decode_select_char reads i32, game.rs:941)"
    );
    assert_eq!(
        VILLAGE_STRUCT_REQUEST_PAYLOAD_LEN, 4,
        "Client-Login.cpp:281 builds it with the same constructor; the port's \
         decode_village_struct_request reads i32 (game.rs:1365)"
    );
    assert_eq!(
        SERVER_INFO_PAYLOAD_LEN, 48,
        "MsgType-System.h:1168-1197 under /Zp8 — the port's \
         SERVER_INFO_PAYLOAD_LEN = 48 (game.rs:35, encode at :990-1015)"
    );
    assert_eq!(
        VILLAGE_STRUCT_PAYLOAD_LEN, 22,
        "MsgType.h:3085-3102 — SI16 VillageUnique + SI16[10] (the port's \
         VILLAGE_STRUCT_PAYLOAD_LEN = 2 + 10*2, game.rs:46-48)"
    );
    assert_eq!(
        MAX_CONSTRUCTION_NUM, 10,
        "CommonHeader.h:590 (the port's MAX_CONSTRUCTION_NUM, game.rs:50-51)"
    );
    assert_eq!(
        MAX_EVENT_NUMBER, 8,
        "CommonHeader.h:765 (the port encodes event_switch: [bool; 8], \
         game.rs:1005)"
    );
    assert_eq!(
        EVENT_TOGGLE_SWITCH_MAX, 2,
        "CommonHeader.h:868 (the port encodes event_toggle_switch: [bool; 2], \
         game.rs:1013)"
    );
    assert_eq!(
        SELECT_CHAR_SLOT_MAX, 3,
        "NSelectChar.cpp:1292 gates CurrentCharInfoList to 0..MAX_CHAR_PER_USER; \
         the legacy server independently gates index >= 3 (Server-Login.cpp:186)"
    );
    assert_eq!(
        SERVICE_AREA_ENGLISH, 0x08,
        "CommonHeader.h:102 — ConstServiceArea_English; the port's service_area \
         config default asserts == 0x08 (config.rs:366,396)"
    );
    assert_eq!(
        MAX_VILLAGE_NUMBER, 100,
        "CommonHeader.h:583 (the port's MAX_VILLAGE_NUMBER = 100, game.rs:53-56)"
    );
    assert_eq!(
        5 + SELECT_CHAR_PAYLOAD_LEN,
        9,
        "MsgType.h:2981 — usSize = sizeof(sPacketHeader) + sizeof(SI32)"
    );
    assert_eq!(
        5 + SERVER_INFO_PAYLOAD_LEN,
        53,
        "MsgType.h:2967 — the SERVERINFO frame's usSize"
    );
    assert_eq!(
        5 + VILLAGE_STRUCT_PAYLOAD_LEN,
        27,
        "MsgType.h:2967 — the VILLAGESTRUCT frame's usSize"
    );
}

#[test]
fn select_char_payload_is_a_little_endian_slot() {
    for slot in [0, 1, 2] {
        let payload = select_char_payload(slot).expect("in range");
        assert_eq!(
            payload,
            slot.to_le_bytes(),
            "the body is the slot as a bare SI32 (MsgType.h:2977-2984; the \
             server reads i32, game.rs:941)"
        );
        assert_eq!(payload.len(), 4, "exactly sizeof(SI32)");
    }

    // The edge proof the bytes are LE, not BE: within the admissible slots
    // 0..3 the value lands in the low byte — a BE encoding would park it in
    // byte 3 instead.
    assert_eq!(
        select_char_payload(1).expect("in range"),
        [1, 0, 0, 0],
        "slot 1 travels as 01 00 00 00, not 00 00 00 01"
    );
    assert_eq!(
        select_char_payload(2).expect("in range"),
        [2, 0, 0, 0],
        "slot 2 travels as 02 00 00 00, not 00 00 00 02"
    );
}

#[test]
fn a_slot_the_dialog_could_not_choose_is_refused() {
    // NSelectChar.cpp:1292 returns before sending for slot < 0 or
    // >= MAX_CHAR_PER_USER; Server-Login.cpp:186 refuses the same range.
    for slot in [-1, 3, 4, i32::MIN, i32::MAX] {
        let err = select_char_payload(slot).unwrap_err();
        assert!(
            matches!(
                err,
                WorldEntryError::SlotOutOfRange {
                    slot: s,
                    max: SELECT_CHAR_SLOT_MAX
                } if s == slot
            ),
            "slot {slot} must be refused as SlotOutOfRange, got {err:?}"
        );
    }
}

#[test]
fn village_struct_request_is_a_little_endian_ref() {
    let payload = village_struct_request_payload(3).expect("in range");
    assert_eq!(
        payload,
        3i32.to_le_bytes(),
        "the body is the ref as a bare SI32 (Client-Login.cpp:281; the server \
         reads i32, game.rs:1365)"
    );
    assert_eq!(
        village_struct_request_payload(1).expect("in range"),
        [1, 0, 0, 0],
        "ref 1 travels as 01 00 00 00"
    );
    assert_eq!(
        village_struct_request_payload(99).expect("in range"),
        [99, 0, 0, 0],
        "ref 99 (MAX_VILLAGE_NUMBER - 1) travels as 63 00 00 00"
    );
}

#[test]
fn a_ref_no_server_would_honour_is_refused() {
    // The server accepts 1 <= ref < MAX_VILLAGE_NUMBER (game.rs:53-56);
    // ref 0 is the legacy's "not a village" sentinel that is never requested
    // at all (Client-Login.cpp:277).
    for village_ref in [0, -1, 100, 1_000, i32::MIN, i32::MAX] {
        let err = village_struct_request_payload(village_ref).unwrap_err();
        assert!(
            matches!(
                err,
                WorldEntryError::VillageRefOutOfRange {
                    village_ref: r,
                    max: MAX_VILLAGE_NUMBER
                } if r == village_ref
            ),
            "ref {village_ref} must be refused as VillageRefOutOfRange, got {err:?}"
        );
    }
}

#[test]
fn server_info_fields_read_their_literal_offsets() {
    let payload = server_info_payload(SERVICE_AREA_ENGLISH);
    let parsed = parse_server_info(RESPONSE_SERVERINFO, &payload).expect("well-formed banner");

    // The whole struct against the hand-built expectation (F-07), then the
    // offset-critical fields individually so a shift cannot hide behind it.
    assert_eq!(
        parsed,
        expected_server_info(SERVICE_AREA_ENGLISH),
        "every modeled cltGameMsgResponse_ServerInfo field at its literal offset"
    );
    assert_eq!(parsed.service_area, 0x08, "siServiceArea at 0..2");
    assert_eq!(parsed.server_unique, 7, "siServerUnique at 2..4");
    assert!(parsed.item_mall_use, "bItemMallUseSwitch at 4");
    assert!(!parsed.village_war, "bVillageWarSwitch at 5");
    assert!(parsed.bonus_time, "bBonusTimeSwitch at 6");
    assert!(
        parsed.special_quest_bonus_time,
        "bSpecialQuestBonusTimeSwitch at 8"
    );
    assert_eq!(parsed.best_village_level, 9, "siBestVillageLevel at 10");
    assert_eq!(parsed.npc_price_rate, 150, "siNPCPriceRate at 12..16");
    assert_eq!(
        parsed.item_price_index, 0x1122_3344_5566_7788,
        "siItemPriceIndex (GMONEY, SI64) at 16..24"
    );
    assert_eq!(
        parsed.event_switch,
        [true, false, true, false, true, false, true, false],
        "bEventSwitch[8] at 24..32"
    );
    assert_eq!(
        parsed.minimum_teacher_level, 30,
        "siMinimumTeacherLevel at 32..36"
    );
    assert_eq!(parsed.service_age, -2, "siServiceAge at 36..38 (signed)");
    assert_eq!(parsed.enable_pvp, 1, "siEnablePVP at 38 (signed)");
    assert!(parsed.auto_unblock_mode, "bAutoUnblockMode at 39");
    assert!(!parsed.vote_campaign, "bVoteCampaign at 40");
    assert_eq!(
        parsed.event_toggle_switch,
        [true, false],
        "bEventToggleSwitch[2] at 41..43"
    );
}

#[test]
fn server_info_rejects_foreign_opcodes_and_wrong_lengths() {
    let payload = server_info_payload(SERVICE_AREA_ENGLISH);

    // Opcode before length: a foreign command is refused without its payload
    // ever being measured.
    let err = parse_server_info(RESPONSE_LOGIN, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::UnexpectedOpcode {
                expected: RESPONSE_SERVERINFO,
                cmd: RESPONSE_LOGIN
            }
        ),
        "a foreign opcode must be refused first, got {err:?}"
    );

    // Then exact size both ways — where the legacy would read out of bounds
    // (short) or silently ignore the tail (long).
    let short = &payload[..SERVER_INFO_PAYLOAD_LEN - 1];
    let err = parse_server_info(RESPONSE_SERVERINFO, short).unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::Truncated {
                expected: SERVER_INFO_PAYLOAD_LEN,
                got,
                opcode: RESPONSE_SERVERINFO
            } if got == SERVER_INFO_PAYLOAD_LEN - 1
        ),
        "one byte short must be Truncated, got {err:?}"
    );

    let mut long = payload.to_vec();
    long.push(0);
    let err = parse_server_info(RESPONSE_SERVERINFO, &long).unwrap_err();
    assert!(
        matches!(
        err,
        WorldEntryError::TrailingBytes {
            expected: SERVER_INFO_PAYLOAD_LEN,
            got,
            opcode: RESPONSE_SERVERINFO
        } if got == SERVER_INFO_PAYLOAD_LEN + 1
        ),
        "one byte long must be TrailingBytes, got {err:?}"
    );
}

#[test]
fn the_region_gate_drops_a_mismatched_banner_without_storing_it() {
    // Client-System.cpp:1682: if(siServiceArea != pclInfo->siServiceArea)
    // return; — a silent drop, no switch set, no error.
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);

    let dropped = state
        .apply_server_info(
            RESPONSE_SERVERINFO,
            &server_info_payload(0x01), // ConstServiceArea_Korea
            SERVICE_AREA_ENGLISH,
        )
        .expect("well-formed banner parses even when the gate drops it");
    assert!(!dropped, "a region mismatch must report the drop");
    assert!(
        state.server_info().is_none(),
        "a dropped banner must not set bReceiveServerInfoSwitch"
    );
    assert!(
        !state.is_in_world(),
        "with no banner the entry cannot complete"
    );

    let accepted = state
        .apply_server_info(
            RESPONSE_SERVERINFO,
            &server_info_payload(SERVICE_AREA_ENGLISH),
            SERVICE_AREA_ENGLISH,
        )
        .expect("well-formed banner");
    assert!(accepted, "a matching banner must be accepted");
    assert_eq!(
        state.server_info(),
        Some(&expected_server_info(SERVICE_AREA_ENGLISH)),
        "bReceiveServerInfoSwitch now holds the full banner"
    );
}

#[test]
fn village_struct_fields_read_their_literal_offsets() {
    let payload = village_struct_payload(42);
    let parsed =
        parse_village_struct(RESPONSE_VILLAGESTRUCT, &payload).expect("well-formed blueprint");

    let expected_structure: [i16; MAX_CONSTRUCTION_NUM] = [-30, -19, -8, 3, 14, 25, 36, 47, 58, 69];
    assert_eq!(
        parsed.village_unique, 42,
        "VillageUnique at 0..2 (MsgType.h:3087)"
    );
    assert_eq!(
        parsed.structure_info, expected_structure,
        "siVillageStructureInfo[10] at 2..22, one SI16 per slot (MsgType.h:3088)"
    );
}

#[test]
fn village_struct_rejects_foreign_opcodes_and_wrong_lengths() {
    let payload = village_struct_payload(1);

    let err = parse_village_struct(RESPONSE_SERVERINFO, &payload).unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::UnexpectedOpcode {
                expected: RESPONSE_VILLAGESTRUCT,
                cmd: RESPONSE_SERVERINFO
            }
        ),
        "a foreign opcode must be refused first, got {err:?}"
    );

    let short = &payload[..VILLAGE_STRUCT_PAYLOAD_LEN - 1];
    let err = parse_village_struct(RESPONSE_VILLAGESTRUCT, short).unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::Truncated {
                expected: VILLAGE_STRUCT_PAYLOAD_LEN,
                got,
                opcode: RESPONSE_VILLAGESTRUCT
            } if got == VILLAGE_STRUCT_PAYLOAD_LEN - 1
        ),
        "one byte short must be Truncated, got {err:?}"
    );

    let mut long = payload.to_vec();
    long.push(0);
    let err = parse_village_struct(RESPONSE_VILLAGESTRUCT, &long).unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::TrailingBytes {
                expected: VILLAGE_STRUCT_PAYLOAD_LEN,
                got,
                opcode: RESPONSE_VILLAGESTRUCT
            } if got == VILLAGE_STRUCT_PAYLOAD_LEN + 1
        ),
        "one byte long must be TrailingBytes, got {err:?}"
    );
}

#[test]
fn a_negative_village_answer_is_refused_rather_than_applied() {
    // Client-SystemNPC.cpp:880/897 — the legacy raises a message box for a
    // negative VillageUnique; this port refuses before touching the wait.
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);
    state.mark_village_struct_requested();
    assert!(state.village_struct_pending(), "the wait is outstanding");

    let err = state
        .apply_village_struct(RESPONSE_VILLAGESTRUCT, &village_struct_payload(-1))
        .unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::NegativeVillageUnique { village_unique: -1 }
        ),
        "a negative village must be refused, got {err:?}"
    );
    assert!(
        state.village_struct_pending(),
        "a refused answer must not clear the wait — the entry stays blocked"
    );
}

#[test]
fn the_opcode_5_record_populates_the_local_char_client() {
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);
    let body = make_char_body(MY_CHAR_UNIQUE as u16);

    let adopted = state
        .apply_login(RESPONSE_LOGIN, &body)
        .expect("well-formed opcode-5 record");
    assert!(adopted, "the record is this connection's own");
    assert_eq!(
        state.char_client(),
        Some(&expected_char_client(MY_CHAR_UNIQUE as u16)),
        "every modeled cltCharClient field copied from the record"
    );

    // The README Step 4 gate reads straight off the struct: level, HP and
    // coordinates equal the wire payload.
    let char = state.char_client().expect("populated");
    assert_eq!(char.level, 57, "Level matches the wire payload");
    assert_eq!(char.life, 3_210, "HP matches the wire payload");
    assert_eq!(
        (char.x, char.y),
        (0x155, 0x2AA),
        "coordinates match the wire payload"
    );
    assert_eq!(char.map_index, 17, "map index matches the wire payload");
}

#[test]
fn an_opcode_5_record_for_another_character_is_not_adopted() {
    // Client-Login.cpp:116 — if(uiMakeCharUnique != MyCharUnique) return;
    // the legacy exits before SetChar, so the local char stays unpopulated.
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);
    let body = make_char_body((MY_CHAR_UNIQUE + 1) as u16);

    let adopted = state
        .apply_login(RESPONSE_LOGIN, &body)
        .expect("well-formed opcode-5 record");
    assert!(!adopted, "a foreign record must not be adopted");
    assert!(
        state.char_client().is_none(),
        "the local cltCharClient must stay empty for another character's record"
    );
}

#[test]
fn a_malformed_opcode_5_record_is_a_char_info_error() {
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);
    let body = make_char_body(MY_CHAR_UNIQUE as u16);
    let err = state
        .apply_login(RESPONSE_LOGIN, &body[..MAKE_CHAR_INFO_PAYLOAD_LEN - 1])
        .unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::CharInfo(crate::game::char_info::CharInfoError::Truncated {
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN,
                opcode: RESPONSE_LOGIN,
                ..
            })
        ),
        "the Step 4.4 strictness must surface unchanged, got {err:?}"
    );
    assert!(
        state.char_client().is_none(),
        "a refused record must populate nothing"
    );
}

#[test]
fn entry_completes_only_when_banner_record_and_village_agree() {
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);

    // 1. Nothing received: not in world.
    assert!(!state.is_in_world(), "no banner, no character, no entry");

    // 2. Banner accepted, no character yet: still not in world — the legacy's
    //    SetCharReservedChar would run but produce nothing.
    state
        .apply_server_info(
            RESPONSE_SERVERINFO,
            &server_info_payload(SERVICE_AREA_ENGLISH),
            SERVICE_AREA_ENGLISH,
        )
        .expect("banner");
    assert!(!state.is_in_world(), "banner alone is not entry");

    // 3. Character populated, no village requested (a non-village map never
    //    sets the wait, Client-Login.cpp:277): in world.
    state
        .apply_login(RESPONSE_LOGIN, &make_char_body(MY_CHAR_UNIQUE as u16))
        .expect("record");
    assert!(
        state.is_in_world(),
        "banner + character with no outstanding blueprint is entry"
    );

    // 4. A village is requested: the wait blocks entry until the answer —
    //    Client-SetChar.cpp:66, PopChar refuses while the wait stands.
    state.mark_village_struct_requested();
    assert!(
        !state.is_in_world(),
        "an outstanding village blueprint blocks entry"
    );
    assert!(state.village_struct_pending(), "the wait is recorded");

    // 5. The answer clears it: entry resumes.
    state
        .apply_village_struct(RESPONSE_VILLAGESTRUCT, &village_struct_payload(3))
        .expect("blueprint");
    assert!(
        state.is_in_world(),
        "banner + character + cleared wait is entry"
    );
    assert!(
        !state.village_struct_pending(),
        "the answer cleared bWaitVillageStructureInfoSwitch (Client-SystemNPC.cpp:896)"
    );
}

#[test]
fn the_banner_survives_a_later_dropped_answer() {
    // A refused (negative) blueprint never clears the wait, and the state
    // keeps the banner and character it already holds — the entry is stalled,
    // not reset.
    let mut state = WorldEntryState::new(MY_CHAR_UNIQUE);
    state
        .apply_server_info(
            RESPONSE_SERVERINFO,
            &server_info_payload(SERVICE_AREA_ENGLISH),
            SERVICE_AREA_ENGLISH,
        )
        .expect("banner");
    state
        .apply_login(RESPONSE_LOGIN, &make_char_body(MY_CHAR_UNIQUE as u16))
        .expect("record");
    state.mark_village_struct_requested();
    assert!(!state.is_in_world(), "wait outstanding");

    let err = state
        .apply_village_struct(RESPONSE_VILLAGESTRUCT, &village_struct_payload(-5))
        .unwrap_err();
    assert!(
        matches!(err, WorldEntryError::NegativeVillageUnique { .. }),
        "got {err:?}"
    );
    assert!(state.server_info().is_some(), "banner retained");
    assert!(state.char_client().is_some(), "character retained");
    assert!(!state.is_in_world(), "entry still blocked by the wait");
}

/// Reads one complete client→server frame the way the service reads it: the
/// in-clear `usSize` selects the frame length.
async fn read_frame(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.unwrap();
    let size = usize::from(u16::from_le_bytes(head));
    let mut raw = vec![0u8; size];
    raw[..2].copy_from_slice(&head);
    stream.read_exact(&mut raw[2..]).await.unwrap();
    raw
}

#[tokio::test]
async fn the_select_char_frame_is_9_bytes_enciphered_with_the_session_cipher() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // The unsolicited opcode 1 handshake, link-encoded, as the service
        // writes it at accept.
        stream
            .write_all(&encode_link(&frame(
                RESPONSE_CLIENTCHARUNIQUE,
                &HANDSHAKE_PAYLOAD,
            )))
            .await
            .unwrap();
        read_frame(&mut stream).await
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    request_select_char(&mut link, 1).await.expect("selectchar");
    let raw = server.await.unwrap();

    // usSize travelled in the clear: 5 + 4 = 9 (MsgType.h:2981).
    assert_eq!(
        u16::from_le_bytes([raw[0], raw[1]]),
        9,
        "the frame is exactly sizeof(sPacketHeader) + sizeof(SI32)"
    );
    // The body did not travel in the clear — the F-04 outbound property.
    assert_ne!(
        &raw[5..],
        &1i32.to_le_bytes(),
        "client→server must be enciphered with the negotiated session keys"
    );

    // Decode with a fresh session cipher seeded by the handshake bytes — the
    // pairing the service uses.
    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap(),
        "the negotiated keys must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        REQUEST_SELECTCHAR,
        "opcode 4 recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        &1i32.to_le_bytes(),
        "the SI32 slot recovered bit-exact through encode → wire → decode"
    );

    // Both directions of wrong must fail.
    let mut wrong_key = Cipher::session_cipher(HANDSHAKE_KEY1.wrapping_add(1), HANDSHAKE_KEY2);
    let mut scratch = vec![0u8; raw.len()];
    assert!(
        !wrong_key.decode(&raw, &mut scratch).unwrap_or(false),
        "an off-by-one key must fail the integrity check"
    );
}

#[tokio::test]
async fn the_village_struct_frame_is_9_bytes_enciphered_with_the_session_cipher() {
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
        read_frame(&mut stream).await
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    request_village_struct(&mut link, 3)
        .await
        .expect("villagestruct");
    let raw = server.await.unwrap();

    assert_eq!(
        u16::from_le_bytes([raw[0], raw[1]]),
        9,
        "the frame is exactly sizeof(sPacketHeader) + sizeof(SI32)"
    );
    assert_ne!(
        &raw[5..],
        &3i32.to_le_bytes(),
        "client→server must be enciphered with the negotiated session keys"
    );

    let mut cipher = Cipher::session_cipher(HANDSHAKE_KEY1, HANDSHAKE_KEY2);
    let mut plain = vec![0u8; raw.len()];
    assert!(
        cipher.decode(&raw, &mut plain).unwrap(),
        "the negotiated keys must satisfy the integrity check"
    );
    assert_eq!(
        u16::from_le_bytes([plain[3], plain[4]]),
        REQUEST_VILLAGESTRUCT,
        "opcode 1101 recovered through the negotiated cipher"
    );
    assert_eq!(
        &plain[5..],
        &3i32.to_le_bytes(),
        "the SI32 ref recovered bit-exact through encode → wire → decode"
    );
}

#[tokio::test]
async fn a_refused_request_before_the_handshake_writes_nothing() {
    let (listener, addr) = local_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    // The range gate runs before the send gate: the slot is refused on its
    // own terms (NSelectChar.cpp:1292), and nothing reaches the socket.
    let err = request_select_char(&mut link, 3).await.unwrap_err();
    assert!(
        matches!(err, WorldEntryError::SlotOutOfRange { slot: 3, .. }),
        "expected SlotOutOfRange, got {err:?}"
    );
    // And with a valid slot, the legacy's bGameServerSecretKeySwitch gate
    // refuses it as NotHandshaken instead (Client-Winsock.cpp:171).
    let err = request_select_char(&mut link, 0).await.unwrap_err();
    assert!(
        matches!(err, WorldEntryError::Game(GameError::NotHandshaken)),
        "expected NotHandshaken before the opcode 1 handshake, got {err:?}"
    );
    assert!(
        link.outbound_keys().is_none(),
        "a refused request must not key or tick the outbound cipher"
    );
    drop(link); // close so the server's read_to_end completes

    let received = server.await.unwrap();
    assert!(
        received.is_empty(),
        "nothing may reach the socket before the handshake, got {received:02x?}"
    );
}

#[tokio::test]
async fn a_refused_request_after_the_handshake_writes_nothing() {
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
        .expect("the peer must be able to observe the close");
        assert!(n.is_ok(), "read_to_end completed");
        received
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    link.handshake().await.expect("opcode 1 handshake");
    let err = request_village_struct(&mut link, 0).await.unwrap_err();
    assert!(
        matches!(
            err,
            WorldEntryError::VillageRefOutOfRange { village_ref: 0, .. }
        ),
        "ref 0 is the 'not a village' sentinel and must be refused, got {err:?}"
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
        "the refused request must write no bytes, got {received:02x?}"
    );
}
