// README Step 5, items 2-3 tests — falsifiable, byte-exact.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): the dispatcher is
// pure, so nothing here touches a socket — what the suite proves is that
// every field of every one of the five bodies is read at the offset its
// field table names, that each length gate fires in both directions *before*
// any field is read and *before* any state is touched, and that the
// self/other/unmatched routing and the reserved-life ratchet behave exactly
// as the legacy writers do.
//
// F-07: expectation sources are never the unit under test — every payload
// below is hand-placed from the field tables in the module docs
// (`CommonLogic.h:385-394`, `MsgType-System.h:1123-1160` and `:1139-1158`,
// `MsgType-Person.h:16-27`, `parametabox.h:107-111`/`:243-249` with the
// ability prefix at `CommonHeader.h:2746-2752`,
// `MsgType.h:2977-2983`) and the port's binary measurements
// (`game.rs:426`, `game.rs:1500`), built with independent byte math
// rather than by any function under test. The opcode-6 expectations reuse
// the Step 4.4 field table (`char_info`, QC'd) with a local record builder.
use super::*;
use crate::game::char_info::{MAKECHARTYPE_PC, MAKE_CHAR_INFO_PAYLOAD_LEN};
use crate::game::login::PLAYER_NAME_LEN;
use crate::protocol::header::HEADER_SIZE;

/// The local avatar tests adopt: the Step 4.5-4.7 `CharClient` with values
/// every later write is distinguishable from.
fn local_char_client() -> CharClient {
    CharClient {
        char_unique: 7,
        kind: 1,
        level: 10,
        life: 500,
        x: 10,
        y: 20,
        map_index: 5,
        move_speed: 9,
        home_village: 1,
        gm_level: 0,
        map_unique: 0,
    }
}

/// A state with the local avatar adopted, ready for delta tests.
fn in_world_state() -> DispatcherState {
    let mut state = DispatcherState::new(7);
    state.adopt_local(&local_char_client());
    state
}

/// Builds a chat body the constructor's arithmetic produces: 20 fixed bytes,
/// then name + text + NUL (`MsgType-System.h:1139-1158`). The declared
/// `siDataSize` is computed here from the placed bytes, never copied from the
/// parser.
fn chat_body(
    char_unique: i16,
    player_name: &str,
    text: &str,
    mode: i16,
    is_game_master: bool,
    system_txt: i32,
    level: i32,
) -> Vec<u8> {
    let mut region = player_name.as_bytes().to_vec();
    region.extend_from_slice(text.as_bytes());
    region.push(0); // the NUL `_tcslen` counts
    let declared = (CHAT_FIXED_LEN + region.len()) as i16;
    let mut body = vec![0u8; CHAT_FIXED_LEN + region.len()];
    body[0..2].copy_from_slice(&declared.to_le_bytes());
    body[2..4].copy_from_slice(&char_unique.to_le_bytes());
    body[4..6].copy_from_slice(&(player_name.len() as i16).to_le_bytes());
    body[6..8].copy_from_slice(&mode.to_le_bytes());
    body[8] = u8::from(is_game_master);
    body[12..16].copy_from_slice(&system_txt.to_le_bytes());
    body[16..20].copy_from_slice(&level.to_le_bytes());
    body[CHAT_FIXED_LEN..].copy_from_slice(&region);
    body
}

/// Builds a 24-byte `cltBasicCharUpdateInfo` from the field table
/// (`CommonLogic.h:385-394`, /Zp8: the SI08 and bool pack at 2..4, the
/// tail byte is alignment pad).
#[allow(clippy::too_many_arguments)] // one arg per wire field keeps the byte placement explicit.
fn char_update_body(
    char_unique: i16,
    move_speed: i8,
    urgent_mode: bool,
    life: i32,
    reserved_life: i32,
    mana: i32,
    x: i16,
    y: i16,
    gm_mode: u8,
    pc_room: bool,
    make_ani_type: i8,
) -> Vec<u8> {
    let mut body = vec![0u8; CHAR_UPDATE_PAYLOAD_LEN];
    body[0..2].copy_from_slice(&char_unique.to_le_bytes());
    body[2] = move_speed as u8;
    body[3] = u8::from(urgent_mode);
    body[4..8].copy_from_slice(&life.to_le_bytes());
    body[8..12].copy_from_slice(&reserved_life.to_le_bytes());
    body[12..16].copy_from_slice(&mana.to_le_bytes());
    body[16..18].copy_from_slice(&x.to_le_bytes());
    body[18..20].copy_from_slice(&y.to_le_bytes());
    body[20] = gm_mode;
    body[21] = u8::from(pc_room);
    body[22] = make_ani_type as u8;
    body[23] = 0xEE; // alignment pad — parsers must ignore it
    body
}

/// The `PipInfo` opcode-215 tests place, every field at the top of a
/// distinguishable value.
fn wire_pip() -> PipInfo {
    PipInfo {
        str_: 11,
        dex: 12,
        vit: 13,
        mag: 14,
        hnd: 15,
        luk: 16,
        wis: 17,
        level: 34,
        bonus: 5,
        life: 161,
        mana: 40,
        exp: 0x0102_0304_0506_0708,
        fame_level: 1,
        skill_bonus: 2,
        fame_exp: 3,
        money: 999_999,
    }
}

/// Builds a 72-byte `cltGameMsgResponse_IP`: `siCharUnique` at 0, the
/// 64-byte block at 8, the /Zp8 pad between filled with a sentinel the
/// parser must ignore (`game.rs:426`).
fn person_ip_body(char_unique: i16, pip: &PipInfo) -> Vec<u8> {
    let mut body = vec![0u8; PERSON_IP_PAYLOAD_LEN];
    body[0..2].copy_from_slice(&char_unique.to_le_bytes());
    body[2..8].fill(0xEE); // /Zp8 pad
    let b = 8;
    body[b..b + 4].copy_from_slice(&pip.str_.to_le_bytes());
    body[b + 4..b + 8].copy_from_slice(&pip.dex.to_le_bytes());
    body[b + 8..b + 12].copy_from_slice(&pip.vit.to_le_bytes());
    body[b + 12..b + 16].copy_from_slice(&pip.mag.to_le_bytes());
    body[b + 16..b + 20].copy_from_slice(&pip.hnd.to_le_bytes());
    body[b + 20..b + 24].copy_from_slice(&pip.luk.to_le_bytes());
    body[b + 24..b + 28].copy_from_slice(&pip.wis.to_le_bytes());
    body[b + 28..b + 30].copy_from_slice(&pip.level.to_le_bytes());
    body[b + 30..b + 32].copy_from_slice(&pip.bonus.to_le_bytes());
    body[b + 32..b + 36].copy_from_slice(&pip.life.to_le_bytes());
    body[b + 36..b + 40].copy_from_slice(&pip.mana.to_le_bytes());
    body[b + 40..b + 48].copy_from_slice(&pip.exp.to_le_bytes());
    body[b + 48..b + 50].copy_from_slice(&pip.fame_level.to_le_bytes());
    body[b + 50..b + 52].copy_from_slice(&pip.skill_bonus.to_le_bytes());
    body[b + 52..b + 56].copy_from_slice(&pip.fame_exp.to_le_bytes());
    body[b + 56..b + 64].copy_from_slice(&pip.money.to_le_bytes());
    body
}

/// Builds one 592-byte `MAKECHARTYPE_PC` record from the Step 4.4 field
/// table (`CommonHeader.h:1969-1977`, low bits first, tail at the
/// binary-measured offsets).
fn pc_record(
    unique: u16,
    level: u8,
    life: u32,
    x: u16,
    y: u16,
    map_index: u16,
    speed: u8,
) -> Vec<u8> {
    let mut record = vec![0u8; MAKE_CHAR_INFO_PAYLOAD_LEN];
    let type_kind = MAKECHARTYPE_PC | (0x2A_u32 << 1);
    let unique_level = u32::from(unique) | (u32::from(level) << 16);
    let life_x = life | (u32::from(x) << 20);
    let y_map_speed = u32::from(y) | (u32::from(map_index) << 10) | (u32::from(speed) << 20);
    record[0..4].copy_from_slice(&type_kind.to_le_bytes());
    record[4..8].copy_from_slice(&unique_level.to_le_bytes());
    record[8..12].copy_from_slice(&life_x.to_le_bytes());
    record[12..16].copy_from_slice(&y_map_speed.to_le_bytes());
    record
}

/// The same record with the `uiType` bit cleared — the non-PC form whose
/// stride no verified source pins.
fn non_pc_record() -> Vec<u8> {
    let mut record = pc_record(9, 1, 10, 1, 1, 1, 1);
    record[0] &= !1; // uiType -> not MAKECHARTYPE_PC
    record
}

// ---------------------------------------------------------------------
// Opcode 118 — chat.
// ---------------------------------------------------------------------

#[test]
fn a_chat_body_lands_every_field_at_its_declared_offset() {
    let body = chat_body(1471, "haru", "hello", 2, true, 77, 34);
    // 20 fixed + 4 name + 5 text + 1 NUL — the constructor's own arithmetic.
    assert_eq!(body.len(), 30);

    let message = parse_chat(RESPONSE_CHAT, &body).unwrap();
    assert_eq!(message.declared_size, 30);
    assert_eq!(message.char_unique, 1471);
    assert_eq!(message.mode, 2, "CHATMODE_SECRET (MsgType-System.h:1086)");
    assert!(message.is_game_master);
    assert_eq!(message.system_txt, 77);
    assert_eq!(message.level, 34);
    assert_eq!(message.player_name, "haru");
    assert_eq!(message.text, "hello");
}

#[test]
fn a_chat_body_that_disagrees_with_its_si_data_size_is_refused() {
    let mut state = in_world_state();
    let before = state.clone();
    let declared_23 = chat_body(1, "a", "b", 1, false, 0, 1);
    assert_eq!(declared_23.len(), 23);

    let mut short = declared_23.clone();
    short.pop();
    let err = state.dispatch(RESPONSE_CHAT, &short).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::Truncated {
                opcode: RESPONSE_CHAT,
                expected: 23,
                got: 22
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before, "a refused chat frame touches nothing");

    let mut long = declared_23;
    long.push(0);
    let err = state.dispatch(RESPONSE_CHAT, &long).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::TrailingBytes {
                opcode: RESPONSE_CHAT,
                expected: 23,
                got: 24
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);
}

#[test]
fn a_chat_si_data_size_the_constructor_cannot_produce_is_refused() {
    let mut state = in_world_state();
    let before = state.clone();

    // 20 fixed bytes leave no room for the NUL `_tcslen` counts (min 21).
    let mut at_min_minus_one = chat_body(1, "", "", 1, false, 0, 1);
    at_min_minus_one[0..2].copy_from_slice(&20i16.to_le_bytes());
    assert!(state.dispatch(RESPONSE_CHAT, &at_min_minus_one).is_err());

    // One past the constructor's maximum (20 + 19 name + 100 text + NUL).
    let mut over_max = chat_body(1, "", "", 1, false, 0, 1);
    over_max[0..2].copy_from_slice(&((CHAT_PAYLOAD_MAX + 1) as i16).to_le_bytes());
    let err = state.dispatch(RESPONSE_CHAT, &over_max).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::ChatDeclaredSize {
                declared,
                min: CHAT_PAYLOAD_MIN,
                max: CHAT_PAYLOAD_MAX,
            } if declared as usize == CHAT_PAYLOAD_MAX + 1
        ),
        "{err:?}"
    );

    // A negative SI16 declaration is below the minimum, never a size.
    let mut negative = chat_body(1, "", "", 1, false, 0, 1);
    negative[0..2].copy_from_slice(&(-5i16).to_le_bytes());
    assert!(state.dispatch(RESPONSE_CHAT, &negative).is_err());

    assert_eq!(state, before, "refused declarations touch nothing");
}

#[test]
fn a_chat_player_name_length_outside_the_transmitted_region_is_refused() {
    let mut state = in_world_state();
    let before = state.clone();
    let mut body = chat_body(1, "haru", "text", 1, false, 0, 1);
    let region = body.len() - CHAT_FIXED_LEN; // 4 + 4 + 1

    // The handler's own guard: >= MAX_PLAYER_NAME returns
    // (Client-System.cpp:1473).
    body[4..6].copy_from_slice(&(PLAYER_NAME_LEN as i16).to_le_bytes());
    let err = state.dispatch(RESPONSE_CHAT, &body).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::ChatNameLength {
                player_name_length,
                region: got_region,
            } if player_name_length as usize == PLAYER_NAME_LEN && got_region == region
        ),
        "{err:?}"
    );
    assert_eq!(state, before);

    // A name longer than the transmitted region cannot be a prefix of it
    // (Client-System.cpp:1484 reads the name out of the same buffer).
    body[4..6].copy_from_slice(&((region + 5) as i16).to_le_bytes());
    assert!(state.dispatch(RESPONSE_CHAT, &body).is_err());

    // A negative length is not a length.
    body[4..6].copy_from_slice(&(-1i16).to_le_bytes());
    assert!(state.dispatch(RESPONSE_CHAT, &body).is_err());

    assert_eq!(state, before);
}

#[test]
fn a_chat_frame_never_mutates_state() {
    // The handler pushes into dialogs only (Client-System.cpp:1468-1597);
    // the parsed message is the whole outcome.
    let mut state = in_world_state();
    let before = state.clone();
    let outcome = state
        .dispatch(
            RESPONSE_CHAT,
            &chat_body(3, "someone", "hi all", 1, false, 0, 9),
        )
        .unwrap();
    match outcome {
        DispatchOutcome::Chat(message) => {
            assert_eq!(message.player_name, "someone");
            assert_eq!(message.text, "hi all");
        }
        other => panic!("expected a chat outcome, got {other:?}"),
    }
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------
// Opcode 200 — character state sync.
// ---------------------------------------------------------------------

#[test]
fn a_char_update_lands_every_field_and_self_skips_move_speed() {
    let mut state = in_world_state();
    assert_eq!(state.local_char().unwrap().move_speed, 9);

    let outcome = state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(7, 3, true, 4321, 90, 88, 300, 400, 1, true, 6),
        )
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 7,
            target: ApplyTarget::Local,
        }
    );

    let local = state.local_char().unwrap();
    assert_eq!(local.life, 4321, "SetLife (Client-Person-BigInfo.cpp:1228)");
    assert_eq!(local.x, 300, "MoveTo (Client-Person-BigInfo.cpp:1218)");
    assert_eq!(local.y, 400);
    assert_eq!(
        local.move_speed, 9,
        "GivenMoveSpeed is others-only (Client-Person-BigInfo.cpp:1241-1244)"
    );

    let delta = state.local_delta();
    assert_eq!(delta.reserved_life, Some(90));
    assert_eq!(delta.mana, Some(88), "SetMana (:1238)");
    assert_eq!(delta.gm_mode, Some(1), ":1255");
    assert_eq!(delta.pc_room, Some(true), ":1256");
    assert_eq!(delta.make_ani_type, Some(6), ":1257");
}

#[test]
fn a_char_update_for_a_nearby_character_writes_move_speed_too() {
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();

    let outcome = state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(42, 5, false, 650, 40, 33, 55, 66, 0, false, 1),
        )
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 42,
            target: ApplyTarget::Nearby,
        }
    );

    let entry = state.nearby().get(&42).unwrap();
    assert_eq!(entry.life, 650);
    assert_eq!(entry.reserved_life, Some(40));
    assert_eq!(entry.mana, Some(33));
    assert_eq!(entry.x, 55);
    assert_eq!(entry.y, 66);
    assert_eq!(entry.move_speed, 5);
    assert_eq!(entry.gm_mode, 0);
    assert!(!entry.pc_room);
    assert_eq!(entry.make_ani_type, 1);
    assert_eq!(entry.map_index, 3, "the birth field survives the delta");
}

#[test]
fn a_char_update_reserved_life_ratchets_and_never_regresses() {
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();

    for (incoming, expected) in [(100, Some(100)), (50, Some(100)), (150, Some(150))] {
        state
            .dispatch(
                RESPONSE_CHARUPDATE,
                &char_update_body(42, 1, false, 1, incoming, 1, 1, 1, 0, false, 0),
            )
            .unwrap();
        assert_eq!(
            state.nearby().get(&42).unwrap().reserved_life,
            expected,
            "incoming {incoming}"
        );
    }
}

#[test]
fn a_char_update_mana_is_clamped_to_zero_and_life_is_not() {
    // The legacy SetMana store chain clamps a negative mana to 0
    // (Char-Common.cpp:1295-1298 -> parametabox.h:194-198); the SetLife chain
    // is an unclamped store (Char-Common.cpp:1333 -> parametabox.h:179).
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();

    state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(7, 1, false, -5, 1, -9, 1, 1, 0, false, 0),
        )
        .unwrap();
    assert_eq!(state.local_delta().mana, Some(0), "negative mana clamps");
    assert_eq!(
        state.local_char().unwrap().life,
        (-5i32) as u32,
        "life is stored unclamped, bit-reinterpreted"
    );

    state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(42, 1, false, 1, 1, -3, 1, 1, 0, false, 0),
        )
        .unwrap();
    assert_eq!(
        state.nearby().get(&42).unwrap().mana,
        Some(0),
        "the same chain runs for other characters"
    );
}

#[test]
fn a_char_update_for_an_unknown_character_is_dropped_without_touching_state() {
    let mut state = in_world_state();
    let before = state.clone();
    let outcome = state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(999, 1, false, 1, 1, 1, 1, 1, 0, false, 0),
        )
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 999,
            target: ApplyTarget::Unmatched,
        }
    );
    assert_eq!(state, before, "the legacy's charid <= 0 return (:1169)");
}

#[test]
fn a_char_update_length_boundary_is_refused_exactly() {
    let mut state = in_world_state();
    let before = state.clone();
    let full = char_update_body(7, 1, false, 2, 3, 4, 5, 6, 0, false, 0);
    assert_eq!(full.len(), CHAR_UPDATE_PAYLOAD_LEN);

    let mut short = full.clone();
    short.pop();
    let err = state.dispatch(RESPONSE_CHARUPDATE, &short).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::Truncated {
                opcode: RESPONSE_CHARUPDATE,
                expected: CHAR_UPDATE_PAYLOAD_LEN,
                got: 23,
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);

    let mut long = full;
    long.push(0);
    let err = state.dispatch(RESPONSE_CHARUPDATE, &long).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::TrailingBytes {
                opcode: RESPONSE_CHARUPDATE,
                expected: CHAR_UPDATE_PAYLOAD_LEN,
                got: 25,
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------
// Opcode 215 — the parameta box (HP/MP).
// ---------------------------------------------------------------------

#[test]
fn a_person_ip_lands_the_cl_block_at_offset_8_for_self() {
    let mut state = in_world_state();
    let outcome = state
        .dispatch(RESPONSE_PERSON_IP, &person_ip_body(7, &wire_pip()))
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 7,
            target: ApplyTarget::Local,
        }
    );
    assert_eq!(state.local_delta().pip, Some(wire_pip()));

    // clIP.Set writes the parameta box only: the opcode-5 mirrors stay put
    // (Client-Person-BigInfo.cpp:243 vs :1228/:1238).
    let local = state.local_char().unwrap();
    assert_eq!(local.life, 500, "215 never touches the SetLife mirror");
    assert_eq!(state.local_delta().mana, None, "nor the SetMana mirror");
}

#[test]
fn a_person_ip_lands_on_a_nearby_character_and_an_unknown_one_is_dropped() {
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();

    let outcome = state
        .dispatch(RESPONSE_PERSON_IP, &person_ip_body(42, &wire_pip()))
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 42,
            target: ApplyTarget::Nearby,
        }
    );
    assert_eq!(state.nearby().get(&42).unwrap().pip, Some(wire_pip()));
    assert_eq!(
        state.nearby().get(&42).unwrap().life,
        777,
        "215 never touches the SetLife mirror"
    );

    let before = state.clone();
    let outcome = state
        .dispatch(RESPONSE_PERSON_IP, &person_ip_body(999, &wire_pip()))
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Applied {
            char_unique: 999,
            target: ApplyTarget::Unmatched,
        }
    );
    assert_eq!(state, before, "IsValidID false (:241) leaves state alone");
}

#[test]
fn a_person_ip_length_boundary_is_refused_exactly() {
    let mut state = in_world_state();
    let before = state.clone();
    let full = person_ip_body(7, &wire_pip());
    assert_eq!(full.len(), PERSON_IP_PAYLOAD_LEN);

    let mut short = full.clone();
    short.pop();
    let err = state.dispatch(RESPONSE_PERSON_IP, &short).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::Truncated {
                opcode: RESPONSE_PERSON_IP,
                expected: PERSON_IP_PAYLOAD_LEN,
                got: 71,
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);

    let mut long = full;
    long.push(0);
    let err = state.dispatch(RESPONSE_PERSON_IP, &long).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::TrailingBytes {
                opcode: RESPONSE_PERSON_IP,
                expected: PERSON_IP_PAYLOAD_LEN,
                got: 73,
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);
}

#[test]
fn the_person_ip_body_is_the_ports_measured_frame_minus_its_header() {
    // The port pins the whole frame at 77 bytes (game.rs:1500) with the
    // same 5-byte header this client reads (header.rs:44-45).
    assert_eq!(PERSON_IP_PAYLOAD_LEN + HEADER_SIZE, 77);
}

// ---------------------------------------------------------------------
// Opcode 6 — nearby players.
// ---------------------------------------------------------------------

#[test]
fn a_login_many_body_upserts_one_entry_per_record() {
    let mut state = in_world_state();
    let body = [
        pc_record(42, 12, 777, 11, 22, 3, 4),
        pc_record(43, 60, 4000, 300, 400, 5, 6),
    ]
    .concat();

    let outcome = state.dispatch(RESPONSE_LOGINMANY, &body).unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Entered {
            char_uniques: vec![42, 43],
        }
    );
    assert_eq!(state.nearby().len(), 2);

    let first = state.nearby().get(&42).unwrap();
    assert_eq!(first.kind, 0x2A);
    assert_eq!(first.level, 12);
    assert_eq!(first.life, 777);
    assert_eq!(first.x, 11);
    assert_eq!(first.y, 22);
    assert_eq!(first.map_index, 3);
    assert_eq!(first.move_speed, 4);
    assert_eq!(first.reserved_life, None, "no frame has carried it yet");
}

#[test]
fn a_re_received_record_refreshes_the_birth_fields_and_keeps_the_deltas() {
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();
    state
        .dispatch(
            RESPONSE_CHARUPDATE,
            &char_update_body(42, 5, false, 650, 40, 33, 55, 66, 0, false, 1),
        )
        .unwrap();

    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 13, 900, 12, 23, 3, 4))
        .unwrap();
    let entry = state.nearby().get(&42).unwrap();
    assert_eq!(entry.level, 13, "the record is the birth state");
    assert_eq!(entry.life, 900);
    assert_eq!(entry.reserved_life, Some(40), "deltas are not reset");
    assert_eq!(entry.mana, Some(33));
}

#[test]
fn a_login_many_non_player_record_is_refused_and_state_is_untouched() {
    let mut state = in_world_state();
    let before = state.clone();
    let body = [pc_record(42, 12, 777, 11, 22, 3, 4), non_pc_record()].concat();

    let err = state.dispatch(RESPONSE_LOGINMANY, &body).unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::CharInfo(CharInfoError::NonPlayerRecord {
                offset: 592,
                ui_type: 0
            })
        ),
        "the NPC stride is unpinned (module docs), so the frame is refused: {err:?}"
    );
    assert_eq!(state, before, "one bad record poisons nothing");
}

#[test]
fn an_empty_or_partial_login_many_body_is_refused() {
    let mut state = in_world_state();
    let before = state.clone();

    let err = state.dispatch(RESPONSE_LOGINMANY, &[]).unwrap_err();
    assert!(matches!(
        err,
        DispatchError::CharInfo(CharInfoError::Truncated { .. })
    ));

    let mut partial = pc_record(42, 12, 777, 11, 22, 3, 4);
    partial.extend_from_slice(&[0u8; 100]);
    let err = state.dispatch(RESPONSE_LOGINMANY, &partial).unwrap_err();
    assert!(matches!(
        err,
        DispatchError::CharInfo(CharInfoError::PartialRecord { offset: 592, .. })
    ));
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------
// Opcode 11 — logout.
// ---------------------------------------------------------------------

#[test]
fn a_logout_removes_a_nearby_character() {
    let mut state = in_world_state();
    state
        .dispatch(RESPONSE_LOGINMANY, &pc_record(42, 12, 777, 11, 22, 3, 4))
        .unwrap();

    let outcome = state
        .dispatch(RESPONSE_LOGOUT, &42i32.to_le_bytes())
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Removed {
            char_unique: 42,
            applied: true,
        }
    );
    assert!(state.nearby().get(&42).is_none());
}

#[test]
fn a_logout_naming_the_local_avatar_is_never_applied() {
    let mut state = in_world_state();
    let before = state.clone();
    let outcome = state
        .dispatch(RESPONSE_LOGOUT, &7i32.to_le_bytes())
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Removed {
            char_unique: 7,
            applied: false,
        }
    );
    assert_eq!(
        state, before,
        "the legacy's quitid == 1 early return (Client-Login.cpp:402-405)"
    );
    assert_eq!(state.local_char().unwrap().life, 500);
}

#[test]
fn a_logout_naming_an_unknown_character_is_a_no_op() {
    let mut state = in_world_state();
    let before = state.clone();
    let outcome = state
        .dispatch(RESPONSE_LOGOUT, &999i32.to_le_bytes())
        .unwrap();
    assert_eq!(
        outcome,
        DispatchOutcome::Removed {
            char_unique: 999,
            applied: false,
        }
    );
    assert_eq!(state, before, "IsValidID false (Client-Login.cpp:408-411)");
}

#[test]
fn a_logout_length_boundary_is_refused_exactly() {
    let mut state = in_world_state();
    let before = state.clone();

    let err = state
        .dispatch(RESPONSE_LOGOUT, &[0u8; LOGOUT_PAYLOAD_LEN - 1])
        .unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::Truncated {
                opcode: RESPONSE_LOGOUT,
                expected: LOGOUT_PAYLOAD_LEN,
                got: 3,
            }
        ),
        "{err:?}"
    );

    let err = state
        .dispatch(RESPONSE_LOGOUT, &[0u8; LOGOUT_PAYLOAD_LEN + 1])
        .unwrap_err();
    assert!(
        matches!(
            err,
            DispatchError::TrailingBytes {
                opcode: RESPONSE_LOGOUT,
                expected: LOGOUT_PAYLOAD_LEN,
                got: 5,
            }
        ),
        "{err:?}"
    );
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------
// The router itself.
// ---------------------------------------------------------------------

#[test]
fn a_foreign_opcode_is_unhandled_not_an_error() {
    let mut state = in_world_state();
    let before = state.clone();
    let outcome = state.dispatch(999, &[]).unwrap();
    assert_eq!(outcome, DispatchOutcome::Unhandled { cmd: 999 });
    assert_eq!(state, before);
}

#[test]
fn every_parser_checks_its_opcode_before_any_length() {
    // An empty body under the wrong opcode must report the opcode, never a
    // length: the gate order is the world-entry convention.
    let empty: &[u8] = &[];
    assert!(matches!(
        parse_chat(RESPONSE_CHARUPDATE, empty).unwrap_err(),
        DispatchError::UnexpectedOpcode {
            expected: RESPONSE_CHAT,
            cmd: RESPONSE_CHARUPDATE,
        }
    ));
    assert!(matches!(
        parse_char_update(RESPONSE_PERSON_IP, empty).unwrap_err(),
        DispatchError::UnexpectedOpcode {
            expected: RESPONSE_CHARUPDATE,
            cmd: RESPONSE_PERSON_IP,
        }
    ));
    assert!(matches!(
        parse_person_ip(RESPONSE_LOGOUT, empty).unwrap_err(),
        DispatchError::UnexpectedOpcode {
            expected: RESPONSE_PERSON_IP,
            cmd: RESPONSE_LOGOUT,
        }
    ));
    assert!(matches!(
        parse_logout(RESPONSE_CHAT, empty).unwrap_err(),
        DispatchError::UnexpectedOpcode {
            expected: RESPONSE_LOGOUT,
            cmd: RESPONSE_CHAT,
        }
    ));
    assert!(matches!(
        parse_make_char_info_many(RESPONSE_LOGOUT, empty).unwrap_err(),
        CharInfoError::UnexpectedOpcode {
            expected: RESPONSE_LOGINMANY,
            cmd: RESPONSE_LOGOUT,
        }
    ));
}
