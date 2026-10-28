// Step 4.4 tests — falsifiable, byte-exact.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): these parsers are
// pure, so nothing here touches a socket — what the suite proves is that
// every field is read at the offset the wire table names, that the
// bitfield boundaries do not bleed into each other, and that the length
// and opcode gates fire before any field is touched. No server, no mock.
//
// F-07: expectation sources are never the unit under test — every payload
// here is hand-placed from the field tables (`Char-Common.h:453-471`,
// `CommonHeader.h:1967-1982` / `:2565-2658`, `parametabox.h:95-114` /
// `:243-249`, and the binary-measured offsets the server port reports),
// built with independent byte math rather than by any parser under test.
use super::*;

/// Fills a 984-byte roster body from the field table, leaving junk in every
/// region this parser does not model so a shifted read cannot hide behind a
/// matching gap.
fn char_list_payload() -> Vec<u8> {
    let mut p = vec![0u8; CHAR_LIST_PAYLOAD_LEN];

    // cltPBasicInfo (0..904).
    p[0..4].copy_from_slice(&0x1122_3344i32.to_le_bytes()); // siIDNum
    p[4..8].copy_from_slice(b"haru"); // szAccountID[20]
    p[8] = 0; // the terminator
    p[9..24].fill(b'X'); // junk after the NUL — not part of the string
    p[24] = 7; // uiIndex
    p[25] = 1; // uiSex
    p[26..28].copy_from_slice(&0x2A3Bu16.to_le_bytes()); // uiKind
    p[28] = (-3i8) as u8; // siHomeVillage (signed)
    p[29..32].fill(b'J'); // uiGRank / uiAngelSwitch / uiPoliceSwitch
    p[32..34].copy_from_slice(&41i16.to_le_bytes()); // clPos.siMapIndex
    p[34..36].copy_from_slice(&512i16.to_le_bytes()); // clPos.siX
    p[36..38].copy_from_slice(&271i16.to_le_bytes()); // clPos.siY
    p[38..40].copy_from_slice(&(-7i16).to_le_bytes()); // clPos.siReserved
    p[40..76].fill(b'K'); // play seconds .. wealth
    p[76..80].copy_from_slice(&0x5566_7788i32.to_le_bytes()); // siPersonID
    p[80..88].copy_from_slice(b"haruchan"); // szName[20]
    p[88] = 0; // the terminator
    p[89..100].fill(b'N'); // junk after the NUL
    p[100..904].fill(b'M'); // father/recommender .. attendance

    // cltPIPInfo (904..968).
    for (i, value) in (10i32..=16).enumerate() {
        // cltBasicAbility: str, dex, vit, mag, hnd, luk, wis.
        let at = 904 + i * 4;
        p[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    p[932..934].copy_from_slice(&34i16.to_le_bytes()); // siLevel
    p[934..936].copy_from_slice(&5i16.to_le_bytes()); // siBonus
    p[936..940].copy_from_slice(&1234i32.to_le_bytes()); // Life
    p[940..944].copy_from_slice(&567i32.to_le_bytes()); // Mana
    p[944..952].copy_from_slice(&0x0102_0304_0506_0708i64.to_le_bytes()); // siExp
    p[952..960].fill(b'F'); // siFameLevel / siSkillBonus / siFameExp
    p[960..968].copy_from_slice(&999_999i64.to_le_bytes()); // siMoney

    // DelCharTime (968..984) stays zero: no deletion pending.
    p
}

/// The values [`char_list_payload`] places, as the independent expectation
/// the parser is checked against.
fn expected_char_list() -> CharListEntry {
    CharListEntry {
        account_idnum: 0x1122_3344,
        account_id: "haru".to_owned(),
        slot: 7,
        sex: 1,
        class_kind: 0x2A3B,
        home_village: -3,
        map_index: 41,
        x: 512,
        y: 271,
        character_id: 0x5566_7788,
        name: "haruchan".to_owned(),
        str_: 10,
        dex: 11,
        vit: 12,
        mag: 13,
        hnd: 14,
        luk: 15,
        wis: 16,
        level: 34,
        bonus: 5,
        life: 1234,
        mana: 567,
        exp: 0x0102_0304_0506_0708,
        money: 999_999,
        del_char_time: [0u8; DEL_CHAR_TIME_LEN],
    }
}

/// The `MakeCharInfo` record the payload builders below construct, held
/// apart from both the builder and the parser (F-07).
#[derive(Debug, Clone, Copy)]
struct WireRecord {
    kind: u16,
    unique: u16,
    level: u8,
    life: u32,
    x: u16,
    y: u16,
    map_index: u16,
    speed: u8,
    home: i8,
    jim: i16,
}

/// The bitfield-maximal record: every field at the top of its declared
/// width, so any mask one bit too wide bleeds a neighbour.
fn maximal_record() -> WireRecord {
    WireRecord {
        kind: 0x7ABC,
        unique: 42,
        level: 255,
        life: 0xF_FFFF, // 20 bits
        x: 0x3FF,       // 10 bits
        y: 0x155,
        map_index: 0x2AA,
        speed: 0x3F, // 6 bits
        home: -2,
        jim: -300,
    }
}

/// Builds one 592-byte `cltMakeCharInfo` from the field table, placing the
/// bitfields from their declared widths (low bits first, `CommonHeader.h:
/// 1969-1977`) and the tail fields at the binary-measured offsets.
fn make_char_payload(r: &WireRecord, reserved_bits_set: bool) -> Vec<u8> {
    let mut p = vec![0u8; MAKE_CHAR_INFO_PAYLOAD_LEN];

    // 0x00: uiType:1 | uiMakeCharKind:16.
    let mut type_kind = MAKECHARTYPE_PC | (u32::from(r.kind) << 1);
    // 0x04: uiMakeCharUnique:16 | uiLevel:8 | pad:8.
    let mut unique_level = u32::from(r.unique) | (u32::from(r.level) << 16);
    // 0x08: uiLife:20 | uiX:10 | pad:2.
    let life_x = r.life | (u32::from(r.x) << 20);
    // 0x0C: uiY:10 | uiMapIndex:10 | uiTotalMoveSpeed:6 | pad:6.
    let mut y_map_speed =
        u32::from(r.y) | (u32::from(r.map_index) << 10) | (u32::from(r.speed) << 20);
    if reserved_bits_set {
        // Park 1s in every reserved bit the parsers must ignore: bits 17-31
        // above uiMakeCharKind (which occupies bits 1-16), the pad byte at
        // 0x07, the 2 bits above uiX, and the 6 bits above
        // uiTotalMoveSpeed.
        type_kind |= 0xFFFE_0000;
        unique_level |= 0xFF00_0000;
        y_map_speed |= 0xFC00_0000;
    }
    p[0..4].copy_from_slice(&type_kind.to_le_bytes());
    p[4..8].copy_from_slice(&unique_level.to_le_bytes());
    p[8..12].copy_from_slice(&life_x.to_le_bytes());
    p[12..16].copy_from_slice(&y_map_speed.to_le_bytes());

    p[0x10] = r.home as u8; // siHomeVillageUnique
    p[0x11] = 0xEE; // pad byte the SI16 alignment inserts
    p[0x12..0x14].copy_from_slice(&r.jim.to_le_bytes()); // siJimCharUnique
    p[0x14..0x38].fill(b'O'); // clOrder, clIdentity, siStatus
    p[0x38..0x3A].copy_from_slice(&0x0016u16.to_le_bytes()); // clEnemyType
    p[0x3A..0x50].fill(b'P'); // siPara1, clEventInfo, siTopSkillUnique
    p[0x50] = 3; // siCurrentShowHorse
    p[0x51] = 2; // siGameMasterLevel
    p[0x52..0x54].copy_from_slice(&(-1i16).to_le_bytes()); // siMapUnique
    p[0x54..0x92].fill(b'Q'); // clShapeEquip, clRank
    p[0x92..0x94].copy_from_slice(&7i16.to_le_bytes()); // siMakeAniType
    p[0x94..].fill(b'R'); // clGuildRank + tail pad
    p
}

/// The expectation [`maximal_record`] + the fixed tail values imply.
fn expected_make_char(r: &WireRecord) -> MakeCharInfo {
    MakeCharInfo {
        is_player: true,
        make_char_kind: r.kind,
        make_char_unique: r.unique,
        level: r.level,
        life: r.life,
        x: r.x,
        y: r.y,
        map_index: r.map_index,
        total_move_speed: r.speed,
        home_village_unique: r.home,
        jim_char_unique: r.jim,
        enemy_type: 0x0016,
        current_show_horse: 3,
        game_master_level: 2,
        map_unique: -1,
        make_ani_type: 7,
    }
}

/// Builds one 68-byte [`MAKECHARTYPE_NPC`] record: the same common-head
/// field table as [`make_char_payload`] with `uiType` cleared and the
/// PC-only tail *absent* — the record ends where `cltMakeCharInfoCommon`
/// does (`CommonHeader.h:1967-1995`, 68 bytes dual-sourced at
/// [`MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN`]).
fn make_npc_payload(r: &WireRecord) -> Vec<u8> {
    let mut p = vec![0u8; MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN];

    // 0x00: uiType:1 (= MAKECHARTYPE_NPC, 0) | uiMakeCharKind:16.
    let type_kind = u32::from(r.kind) << 1;
    // 0x04: uiMakeCharUnique:16 | uiLevel:8 | pad:8.
    let unique_level = u32::from(r.unique) | (u32::from(r.level) << 16);
    // 0x08: uiLife:20 | uiX:10 | pad:2.
    let life_x = r.life | (u32::from(r.x) << 20);
    // 0x0C: uiY:10 | uiMapIndex:10 | uiTotalMoveSpeed:6 | pad:6.
    let y_map_speed = u32::from(r.y) | (u32::from(r.map_index) << 10) | (u32::from(r.speed) << 20);
    p[0..4].copy_from_slice(&type_kind.to_le_bytes());
    p[4..8].copy_from_slice(&unique_level.to_le_bytes());
    p[8..12].copy_from_slice(&life_x.to_le_bytes());
    p[12..16].copy_from_slice(&y_map_speed.to_le_bytes());

    p[0x10] = r.home as u8; // siHomeVillageUnique
    p[0x11] = 0xEE; // pad byte the SI16 alignment inserts
    p[0x12..0x14].copy_from_slice(&r.jim.to_le_bytes()); // siJimCharUnique
    p[0x14..0x38].fill(b'O'); // clOrder, clIdentity, siStatus
                              // The head's last placed field is clEnemyType at 0x38..0x3A; the span
                              // through 0x44 (siPara1 .. end of clInnerStatus) stays zero, and the
                              // record ends at 68.
    p[0x38..0x3A].copy_from_slice(&0x0016u16.to_le_bytes()); // clEnemyType
    p
}

/// The expectation a [`make_npc_payload`] record of [`maximal_record`]'s
/// fields implies: the head fields as decoded, `is_player` false, and the
/// PC-only tail zeroed (the legacy `clinfo.Set(pclinfo)` copy leaves it at
/// `Init()` zeros, `Client-Login.cpp:73`).
fn expected_npc(r: &WireRecord) -> MakeCharInfo {
    let mut expected = expected_make_char(r);
    expected.is_player = false;
    expected.current_show_horse = 0;
    expected.game_master_level = 0;
    expected.map_unique = 0;
    expected.make_ani_type = 0;
    expected
}

#[test]
fn constants_match_the_wire_truth() {
    assert_eq!(
        RESPONSE_CHARLIST, 3,
        "MsgType.h:24 — GAMEMSG_RESPONSE_CHARLIST follows REQUEST_LOGIN (the \
         port's GameMsg::ResponseCharlist = 3, opcodes_game.rs, same header)"
    );
    assert_eq!(
        RESPONSE_LOGIN, 5,
        "MsgType.h:27 — the port's GameMsg::ResponseLogin = 5"
    );
    assert_eq!(
        RESPONSE_LOGINMANY, 6,
        "MsgType.h:28 — the port's GameMsg::ResponseLoginmany = 6"
    );
    assert_eq!(
        CHAR_LIST_PAYLOAD_LEN, 984,
        "Char-Common.h:453-471 — the port's binary-measured CHAR_LIST_PAYLOAD_LEN \
         (game.rs:25-32, stride 0x3D8)"
    );
    assert_eq!(
        MAKE_CHAR_INFO_PAYLOAD_LEN, 592,
        "CommonHeader.h:2166 — the port's binary-measured MAKE_CHAR_INFO_PAYLOAD_LEN \
         (game.rs:37-43, PopChar strides 0x250)"
    );
    assert_eq!(
        MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN, 68,
        "CommonHeader.h:1967-1995 — sizeof(cltMakeCharInfoCommon): 16 (4 UI32 \
         bitfield words) + 4 (SI08 + pad + SI16) + 20 (clOrder) + 12 (clIdentity) \
         + 16 (clInnerStatus); wire-confirmed by evidence/live/decode2.log \
         (payload=2312 = 34 x 68, byte 56 = ENEMYTYPE_ALLPC at clEnemyType)"
    );
    assert_eq!(
        CHAR_SLOTS, 15,
        "CommonHeader.h:579-581 — MAX_CHAR_PER_USER (3) + MAX_CHAR_HOUSE_PER_USER (12)"
    );
    assert_eq!(
        P_BASIC_INFO_LEN, 904,
        "CommonHeader.h:2565-2658 — the port's binary measurement (game.rs:27-31), \
         superseding the snapshot's derived 832"
    );
    assert_eq!(
        PIP_INFO_LEN, 64,
        "parametabox.h:95-114 + :243-249 — 7 SI32 abilities + level/bonus/life/mana/exp \
         + fame/skill/purse"
    );
    assert_eq!(
        BASIC_INFO_LEN, 968,
        "904 + 64: where DelCharTime begins (the port reads siLevel at 932 and \
         DelCharTime at 968, game.rs:224-227)"
    );
    assert_eq!(
        CHAR_LIST_PAYLOAD_LEN,
        BASIC_INFO_LEN + DEL_CHAR_TIME_LEN,
        "Char-Common.h:453-471 — cltPBasicInfo + cltPIPInfo + SYSTEMTIME"
    );
    assert_eq!(DEL_CHAR_TIME_LEN, 16, "SYSTEMTIME is 8 WORDs");
    assert_eq!(MAKECHARTYPE_PC, 1, "CommonHeader.h:1893");
    assert_eq!(MAKECHARTYPE_NPC, 0, "CommonHeader.h:1892");
    assert_eq!(
        5 + CHAR_LIST_PAYLOAD_LEN,
        989,
        "MsgType.h:2967 — the roster frame's usSize"
    );
}

#[test]
fn char_list_fields_read_their_literal_offsets() {
    let payload = char_list_payload();
    let parsed =
        parse_char_list_entry(RESPONSE_CHARLIST, &payload).expect("well-formed roster row");

    // The whole struct against the hand-built expectation (F-07), then the
    // headline fields individually so a shift cannot hide behind it.
    assert_eq!(
        parsed,
        expected_char_list(),
        "every modeled cltCharListInfo field at its literal offset"
    );
    assert_eq!(parsed.account_idnum, 0x1122_3344, "clBI.siIDNum at 0..4");
    assert_eq!(&parsed.account_id, "haru", "clBI.szAccountID at 4..24");
    assert_eq!(parsed.slot, 7, "clBI.uiIndex at 24");
    assert_eq!(parsed.sex, 1, "clBI.uiSex at 25");
    assert_eq!(parsed.class_kind, 0x2A3B, "clBI.uiKind at 26..28");
    assert_eq!(parsed.home_village, -3, "clBI.siHomeVillage at 28 (signed)");
    assert_eq!(parsed.map_index, 41, "clBI.clPos.siMapIndex at 32..34");
    assert_eq!(parsed.x, 512, "clBI.clPos.siX at 34..36");
    assert_eq!(parsed.y, 271, "clBI.clPos.siY at 36..38");
    assert_eq!(
        parsed.character_id, 0x5566_7788,
        "clPerson.siPersonID at 76..80"
    );
    assert_eq!(&parsed.name, "haruchan", "clPerson.szName at 80..100");
    assert_eq!(
        (parsed.str_, parsed.dex),
        (10, 11),
        "clIP str/dex at 904/908"
    );
    assert_eq!(
        (parsed.vit, parsed.mag),
        (12, 13),
        "clIP vit/mag at 912/916"
    );
    assert_eq!(
        (parsed.hnd, parsed.luk),
        (14, 15),
        "clIP hnd/luk at 920/924"
    );
    assert_eq!(parsed.wis, 16, "clIP siWis at 928..932");
    assert_eq!(parsed.level, 34, "clIP.siLevel at 932..934");
    assert_eq!(parsed.bonus, 5, "clIP.siBonus at 934..936");
    assert_eq!(parsed.life, 1234, "clIP.Life at 936..940 (hp)");
    assert_eq!(parsed.mana, 567, "clIP.Mana at 940..944 (mp)");
    assert_eq!(
        parsed.exp, 0x0102_0304_0506_0708,
        "clIP.siExp at 944..952 (SI64, parametabox.h:111)"
    );
    assert_eq!(parsed.money, 999_999, "clIP.siMoney at 960..968");
    assert!(
        parsed.is_delete_pending(),
        "an all-zero DelCharTime at 968..984 means no deletion pending"
    );

    // The junk this parser skips must be the only thing it skips: re-derive
    // the name by hand from the payload and compare, so a 4-byte shift in
    // either direction cannot agree by accident.
    let name_end = payload[80..100]
        .iter()
        .position(|&b| b == 0)
        .expect("the name is NUL-terminated");
    assert_eq!(
        &payload[80..80 + name_end],
        parsed.name.as_bytes(),
        "the parsed name is exactly the bytes before the first NUL at 80"
    );
}

#[test]
fn char_list_strings_stop_at_their_terminator() {
    let mut payload = char_list_payload();
    // Embed a NUL mid-field and junk after it, exactly as an uninitialized
    // C buffer carries (the Step 3.3 suite's construction).
    payload[4..9].copy_from_slice(b"haru\0");
    payload[9..24].fill(b'Z');
    payload[80..85].copy_from_slice(b"haru\0");
    payload[85..100].fill(b'Z');

    let parsed = parse_char_list_entry(RESPONSE_CHARLIST, &payload).expect("parses");
    assert_eq!(
        parsed.account_id, "haru",
        "szAccountID stops at its terminator, not at the field's end"
    );
    assert_eq!(
        parsed.name, "haru",
        "szName stops at its terminator, not at the field's end"
    );

    // A field with no NUL at all is the whole 20 bytes.
    payload[80..100].fill(b'n');
    let parsed = parse_char_list_entry(RESPONSE_CHARLIST, &payload).expect("parses");
    assert_eq!(
        parsed.name,
        "n".repeat(PLAYER_NAME_LEN),
        "an unterminated name is the full 20 bytes, as fixed_str reads it"
    );
}

#[test]
fn del_char_time_any_nonzero_byte_means_pending() {
    let mut payload = char_list_payload();
    let mut parsed = parse_char_list_entry(RESPONSE_CHARLIST, &payload).expect("zeroed");
    assert!(parsed.is_delete_pending(), "all-zero is not pending");

    // The last byte of the SYSTEMTIME, i.e. a shift in the block's tail
    // would still be caught.
    payload[968 + DEL_CHAR_TIME_LEN - 1] = 1;
    parsed = parse_char_list_entry(RESPONSE_CHARLIST, &payload).expect("parses");
    assert!(
        !parsed.is_delete_pending(),
        "one non-zero byte anywhere in DelCharTime means a deletion is pending"
    );
}

#[test]
fn opcode_is_checked_before_the_length_is_looked_at() {
    // A short payload under a foreign opcode must name the opcode, never the
    // length: the legacy dispatches on the opcode first.
    let short = [0u8; 4];
    let err = parse_char_list_entry(RESPONSE_LOGIN, &short).unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::UnexpectedOpcode {
                expected: RESPONSE_CHARLIST,
                cmd: RESPONSE_LOGIN
            }
        ),
        "opcode 5 where a roster row was expected, got {err:?}"
    );
    let err = parse_make_char_info(RESPONSE_CHARLIST, &char_list_payload()).unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::UnexpectedOpcode {
                expected: RESPONSE_LOGIN,
                cmd: RESPONSE_CHARLIST
            }
        ),
        "opcode 3 where a MakeCharInfo was expected, got {err:?}"
    );
    let err =
        parse_make_char_info_many(RESPONSE_LOGIN, &make_char_payload(&maximal_record(), false))
            .unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::UnexpectedOpcode {
                expected: RESPONSE_LOGINMANY,
                cmd: RESPONSE_LOGIN
            }
        ),
        "opcode 5 where a LOGINMANY batch was expected, got {err:?}"
    );
}

#[test]
fn char_list_length_boundaries_are_refused_exactly() {
    // Below: refused as truncated, all the way down to an empty body.
    for len in 0..CHAR_LIST_PAYLOAD_LEN {
        let err = parse_char_list_entry(RESPONSE_CHARLIST, &vec![0u8; len]).unwrap_err();
        assert!(
            matches!(
            err,
            CharInfoError::Truncated {
                expected: CHAR_LIST_PAYLOAD_LEN,
                got: l,
                opcode: RESPONSE_CHARLIST
            } if l == len
            ),
            "a {len}-byte roster body must be refused as truncated, got {err:?}"
        );
    }
    // Above: refused as trailing bytes.
    for len in CHAR_LIST_PAYLOAD_LEN + 1..CHAR_LIST_PAYLOAD_LEN + 8 {
        let err = parse_char_list_entry(RESPONSE_CHARLIST, &vec![0u8; len]).unwrap_err();
        assert!(
            matches!(
            err,
            CharInfoError::TrailingBytes {
                expected: CHAR_LIST_PAYLOAD_LEN,
                got: l,
                opcode: RESPONSE_CHARLIST
            } if l == len
            ),
            "a {len}-byte roster body must be refused as trailing, got {err:?}"
        );
    }
    // Exactly the length parses.
    assert!(
        parse_char_list_entry(RESPONSE_CHARLIST, &vec![0u8; CHAR_LIST_PAYLOAD_LEN]).is_ok(),
        "exactly 984 bytes is the one accepted length"
    );
}

#[test]
fn make_char_info_fields_read_their_literal_offsets() {
    let record = maximal_record();
    let payload = make_char_payload(&record, false);
    let parsed = parse_make_char_info(RESPONSE_LOGIN, &payload).expect("well-formed record");

    assert_eq!(
        parsed,
        expected_make_char(&record),
        "every modeled cltMakeCharInfo field at its literal offset/bit"
    );
    assert!(parsed.is_player, "uiType bit 0 == MAKECHARTYPE_PC (1)");
    assert_eq!(
        parsed.make_char_kind, 0x7ABC,
        "uiMakeCharKind bits 1-16 @0x00"
    );
    assert_eq!(parsed.make_char_unique, 42, "uiMakeCharUnique @0x04..0x06");
    assert_eq!(parsed.level, 255, "uiLevel @0x06 (8 bits)");
    assert_eq!(parsed.life, 0xF_FFFF, "uiLife @0x08 bits 0-19 (20 bits)");
    assert_eq!(parsed.x, 0x3FF, "uiX @0x08 bits 20-29 (10 bits)");
    assert_eq!(parsed.y, 0x155, "uiY @0x0C bits 0-9 (10 bits)");
    assert_eq!(parsed.map_index, 0x2AA, "uiMapIndex @0x0C bits 10-19");
    assert_eq!(
        parsed.total_move_speed, 0x3F,
        "uiTotalMoveSpeed @0x0C bits 20-25"
    );
    assert_eq!(
        parsed.home_village_unique, -2,
        "siHomeVillageUnique @0x10 (SI08, signed)"
    );
    assert_eq!(parsed.jim_char_unique, -300, "siJimCharUnique @0x12..0x14");
    assert_eq!(parsed.enemy_type, 0x0016, "clEnemyType.siEnemyType @0x38");
    assert_eq!(parsed.current_show_horse, 3, "siCurrentShowHorse @0x50");
    assert_eq!(parsed.game_master_level, 2, "siGameMasterLevel @0x51");
    assert_eq!(parsed.map_unique, -1, "siMapUnique @0x52..0x54 (SI16)");
    assert_eq!(parsed.make_ani_type, 7, "siMakeAniType @0x92..0x94");
}

#[test]
fn every_bitfield_mask_stops_at_its_declared_width() {
    // Park 1s in every reserved bit the layout carries — the pad byte at
    // 0x07, the 2 bits above uiX, the 6 bits above uiTotalMoveSpeed, and the
    // bits above uiLevel — and prove no field absorbs them.
    let record = maximal_record();
    let payload = make_char_payload(&record, true);
    let parsed = parse_make_char_info(RESPONSE_LOGIN, &payload).expect("parses");

    assert_eq!(
        parsed.level, 255,
        "uiLevel is 8 bits: the reserved byte at 0x07 must not leak into it"
    );
    assert_eq!(
        parsed.life, 0xF_FFFF,
        "uiLife is 20 bits: uiX's bits must not leak downward"
    );
    assert_eq!(
        parsed.x, 0x3FF,
        "uiX is 10 bits: the 2 reserved bits above it must not leak into it"
    );
    assert_eq!(
        parsed.total_move_speed, 0x3F,
        "uiTotalMoveSpeed is 6 bits: the 6 reserved bits above it must not leak into it"
    );
    assert_eq!(
        parsed.make_char_kind, 0x7ABC,
        "uiMakeCharKind is 16 bits: the reserved bits at 17-31 must not leak into it"
    );
    assert_eq!(
        parsed.make_char_unique, 42,
        "uiMakeCharUnique is 16 bits: uiLevel's bits must not leak downward"
    );
    assert_eq!(
        expected_make_char(&record),
        parsed,
        "with every reserved bit set the record still decodes to the same fields"
    );
}

#[test]
fn make_char_info_length_boundaries_are_refused_exactly() {
    for len in 0..MAKE_CHAR_INFO_PAYLOAD_LEN {
        let err = parse_make_char_info(RESPONSE_LOGIN, &vec![0u8; len]).unwrap_err();
        assert!(
            matches!(
            err,
            CharInfoError::Truncated {
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN,
                got: l,
                opcode: RESPONSE_LOGIN
            } if l == len
            ),
            "a {len}-byte record must be refused as truncated, got {err:?}"
        );
    }
    for len in MAKE_CHAR_INFO_PAYLOAD_LEN + 1..MAKE_CHAR_INFO_PAYLOAD_LEN + 8 {
        let err = parse_make_char_info(RESPONSE_LOGIN, &vec![0u8; len]).unwrap_err();
        assert!(
            matches!(
            err,
            CharInfoError::TrailingBytes {
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN,
                got: l,
                opcode: RESPONSE_LOGIN
            } if l == len
            ),
            "a {len}-byte record must be refused as trailing, got {err:?}"
        );
    }
    assert!(
        parse_make_char_info(RESPONSE_LOGIN, &vec![0u8; MAKE_CHAR_INFO_PAYLOAD_LEN]).is_ok(),
        "exactly 592 bytes is the one accepted length"
    );
}

#[test]
fn login_many_walks_records_back_to_back_by_stride() {
    // Three records, the middle one distinguished so a stride error cannot
    // read record 1 twice.
    let first = maximal_record();
    let second = WireRecord {
        unique: 7,
        kind: 9,
        level: 1,
        life: 2,
        x: 3,
        y: 4,
        map_index: 5,
        speed: 6,
        home: 0,
        jim: 0,
    };
    let third = WireRecord {
        unique: 65_535,
        kind: 65_535,
        level: 0,
        life: 0,
        x: 0,
        y: 0,
        map_index: 0,
        speed: 0,
        home: 127,
        jim: -1,
    };

    let mut body = make_char_payload(&first, false);
    body.extend_from_slice(&make_char_payload(&second, false));
    body.extend_from_slice(&make_char_payload(&third, false));
    assert_eq!(
        body.len(),
        3 * MAKE_CHAR_INFO_PAYLOAD_LEN,
        "the batch is exactly three strides long"
    );

    let records = parse_make_char_info_many(RESPONSE_LOGINMANY, &body).expect("three records");
    assert_eq!(records.len(), 3, "one record per 592-byte stride");
    assert_eq!(
        records[0],
        expected_make_char(&first),
        "record 0 is read from offset 0"
    );
    assert_eq!(
        records[1],
        expected_make_char(&second),
        "record 1 is read from offset 592, not re-read from 0"
    );
    assert_eq!(
        records[2],
        expected_make_char(&third),
        "record 2 is read from offset 1184"
    );
    // The stride is the binary's, so a whole batch is the byte count the
    // client's own walk consumes (`Client-Login.cpp:60-84`).
    assert_eq!(
        records.len() * MAKE_CHAR_INFO_PAYLOAD_LEN,
        body.len(),
        "the walk consumes the body exactly, with no remainder"
    );
}

#[test]
fn login_many_refuses_an_empty_body() {
    // The client reads uiType before it knows it got bytes
    // (`Client-Login.cpp:62-64`), so an empty body is garbage, not "no
    // records": the port sends nothing at all for an empty cell instead.
    // The minimum record is a 68-byte NPC head, so that is the shortfall an
    // empty body is measured against (an NPC-only broadcast is legal — the
    // live server's first in-world frame is exactly that, 34 x 68,
    // `evidence/live/decode2.log`).
    let err = parse_make_char_info_many(RESPONSE_LOGINMANY, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::Truncated {
                expected: MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN,
                got: 0,
                opcode: RESPONSE_LOGINMANY
            }
        ),
        "an empty LOGINMANY body must be refused, got {err:?}"
    );
}

#[test]
fn login_many_refuses_partial_records_of_either_stride() {
    let record = maximal_record();

    // A PC-typed tail: the head at that offset declares a 592-byte record,
    // so the shortfall is measured against 592 (`Client-Login.cpp:64-67`).
    let mut pc_led = make_char_payload(&record, false);
    let pc_offset = pc_led.len();
    let pc_head = make_char_payload(&record, false);
    pc_led.extend_from_slice(&pc_head[..100]);
    let err = parse_make_char_info_many(RESPONSE_LOGINMANY, &pc_led).unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::PartialRecord {
                offset,
                have: 100,
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN
            } if offset == pc_offset
        ),
        "a partial trailing PC record must name its offset and shortfall, got {err:?}"
    );

    // An NPC-typed tail: only 68 bytes are demanded, and 40 are refused all
    // the same — the walk never silently truncates a record.
    let mut npc_led = make_npc_payload(&record);
    let npc_offset = npc_led.len();
    npc_led.extend_from_slice(&[0xEE; 40]);
    let err = parse_make_char_info_many(RESPONSE_LOGINMANY, &npc_led).unwrap_err();
    assert!(
        matches!(
            err,
            CharInfoError::PartialRecord {
                offset,
                have: 40,
                expected: MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN
            } if offset == npc_offset
        ),
        "a partial trailing NPC record must name its offset and shortfall, got {err:?}"
    );
}

#[test]
fn login_many_decodes_an_npc_record_at_its_68_byte_stride() {
    // The live server's world-entry broadcast is made of exactly this
    // record: uiType 0, stride sizeof(cltMakeCharInfoCommon) = 68
    // (`Client-Login.cpp:70-75`, the constant's dual citation).
    let record = maximal_record();
    let body = make_npc_payload(&record);
    assert_eq!(
        body.len(),
        MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN,
        "the NPC fixture is one common head, nothing more"
    );

    let records = parse_make_char_info_many(RESPONSE_LOGINMANY, &body).expect("one NPC record");
    assert_eq!(
        records,
        vec![expected_npc(&record)],
        "the common head decodes field for field; the PC-only tail is zero"
    );
}

#[test]
fn login_many_walks_a_mixed_batch_by_each_records_own_stride() {
    // The LIVE-01 shape: NPC, PC, NPC back to back. The legacy strides by
    // each head's own uiType (`Client-Login.cpp:62-75`), so one wrong
    // stride anywhere mis-reads every record after it — which is why each
    // record's own fields pin its position here.
    let npc_head = WireRecord {
        kind: 0x23,
        unique: 8500,
        level: 1,
        life: 40,
        x: 20,
        y: 338,
        map_index: 158,
        speed: 2,
        home: 1,
        jim: 0,
    };
    let pc_middle = WireRecord {
        kind: 0x7ABC,
        unique: 4242,
        level: 60,
        life: 999,
        x: 123,
        y: 456,
        map_index: 7,
        speed: 9,
        home: -2,
        jim: -300,
    };
    let npc_tail = WireRecord {
        kind: 0x23,
        unique: 8501,
        level: 3,
        life: 80,
        x: 21,
        y: 339,
        map_index: 158,
        speed: 2,
        home: 1,
        jim: 0,
    };

    let mut body = make_npc_payload(&npc_head);
    let pc_offset = body.len(); // 68
    body.extend_from_slice(&make_char_payload(&pc_middle, false));
    let tail_offset = body.len(); // 68 + 592 = 660
    body.extend_from_slice(&make_npc_payload(&npc_tail));
    assert_eq!(body.len(), 2 * 68 + MAKE_CHAR_INFO_PAYLOAD_LEN);

    let records = parse_make_char_info_many(RESPONSE_LOGINMANY, &body).expect("three records");
    assert_eq!(records.len(), 3, "one record per typed stride");
    assert_eq!(
        records[0],
        expected_npc(&npc_head),
        "record 0 is the NPC head at offset 0"
    );
    assert_eq!(
        records[1],
        expected_make_char(&pc_middle),
        "record 1 is the PC record at offset {pc_offset}, tail fields and all"
    );
    assert_eq!(
        records[2],
        expected_npc(&npc_tail),
        "record 2 is the NPC head at offset {tail_offset}"
    );
    assert!(
        records.iter().filter(|r| r.is_player).count() == 1,
        "exactly the middle record is a player"
    );
}

#[test]
fn login_many_decodes_the_live_broadcast_shape() {
    // `evidence/live/decode2.log`: `payload=2312 divisible by 68`, 34
    // records, every head `uiType=0`, ids monotone; rec[00] carries
    // w0=0x00000046 (kind 0x23), id@4=0x00012134 (u16 8500, level 1),
    // w8=0x01400028 (life 40, x 20), w12=0x00027852 (y 338, map 158,
    // speed 2), and byte 56 = 0x01 = ENEMYTYPE_ALLPC at clEnemyType.
    // Rebuilt from those literals, the whole broadcast must walk clean.
    let head = WireRecord {
        kind: 0x23,
        unique: 8500,
        level: 1,
        life: 40,
        x: 20,
        y: 338,
        map_index: 158,
        speed: 2,
        home: 0,
        jim: 0,
    };
    let mut body = Vec::with_capacity(34 * MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN);
    for i in 0..34u16 {
        let mut record = make_npc_payload(&head);
        record[4..6].copy_from_slice(&(head.unique + i).to_le_bytes()); // id@4
        record[0x38..0x3A].copy_from_slice(&0x0001u16.to_le_bytes()); // byte 56
        body.extend_from_slice(&record);
    }
    assert_eq!(body.len(), 2312, "decode2.log: payload=2312");

    let records = parse_make_char_info_many(RESPONSE_LOGINMANY, &body).expect("34 NPC records");
    assert_eq!(records.len(), 34, "34 x 68 walks the body exactly");
    assert!(
        records.iter().all(|r| !r.is_player),
        "every head carries uiType = 0"
    );
    let first = &records[0];
    assert_eq!(first.make_char_kind, head.kind);
    assert_eq!(first.make_char_unique, head.unique, "ids start at 8500");
    assert_eq!(first.level, head.level);
    assert_eq!(first.life, head.life);
    assert_eq!(first.x, head.x);
    assert_eq!(first.y, head.y);
    assert_eq!(first.map_index, head.map_index);
    assert_eq!(
        first.enemy_type, 1,
        "byte 56 is ENEMYTYPE_ALLPC at clEnemyType"
    );
    assert_eq!(
        records[33].make_char_unique,
        head.unique + 33,
        "ids are monotone through 8533"
    );
}

#[test]
fn the_compression_flag_must_be_cleared_by_the_caller_before_parsing() {
    // A legacy peer LZF-compresses frames of 200+ bytes and sets bit 15 of
    // usCmd (`Client-Winsock.cpp:178-196`); the codec keeps the flag in
    // `cmd` and exposes the cleared opcode as `Frame::cmd_id()`
    // (`net/codec.rs:312-322`), leaving decompression to the caller (module
    // docs, "compression is the caller's job"). These parsers therefore do
    // *not* mask bit 15 themselves — a flagged opcode must be refused rather
    // than silently accepted, which is what makes the contract falsifiable.
    let flagged = RESPONSE_CHARLIST | crate::protocol::header::COMPRESSION_FLAG;
    let err = parse_char_list_entry(flagged, &char_list_payload()).unwrap_err();
    assert!(
        matches!(
        err,
        CharInfoError::UnexpectedOpcode {
            expected: RESPONSE_CHARLIST,
            cmd
        } if cmd == flagged
        ),
        "a flagged opcode must be refused by its full value, not masked away, got {err:?}"
    );
    let flagged = RESPONSE_LOGIN | crate::protocol::header::COMPRESSION_FLAG;
    let err =
        parse_make_char_info(flagged, &make_char_payload(&maximal_record(), false)).unwrap_err();
    assert!(
        matches!(err, CharInfoError::UnexpectedOpcode { cmd, .. } if cmd == flagged),
        "a flagged opcode must be refused by its full value, got {err:?}"
    );
}
