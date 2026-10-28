//! Step 4.4 - the character roster (opcode 3) and the 592-byte
//! `cltMakeCharInfo` detail record (opcodes 5 and 6)
//! (`CONTRIBUTING.md:87-90`).
//!
//! Two wire structs, both parsed field by field at its literal byte offset:
//!
//! | Frame | Opcode | Body | Struct |
//! |---|---|---|---|
//! | [`RESPONSE_CHARLIST`] | 3 | [`CHAR_LIST_PAYLOAD_LEN`] (984) | `cltCharListInfo` (`Char-Common.h:453-471`) |
//! | [`RESPONSE_LOGIN`] | 5 | [`MAKE_CHAR_INFO_PAYLOAD_LEN`] (592) | one `cltMakeCharInfo` (`CommonHeader.h:2166`) |
//! | [`RESPONSE_LOGINMANY`] | 6 | N x (592 or 68, by `uiType`) | `cltMakeCharInfo` (PC) or its `cltMakeCharInfoCommon` head (NPC), packed back to back |
//!
//! The roster arrives first: the game service answers a successful
//! [`crate::game::login`] submission with [`CHAR_SLOTS`] `cltCharListInfo`
//! frames, one per slot and zeroed where a slot is empty, because the legacy
//! copies each one into `clCharListInfo[clBI.uiIndex]` unconditionally
//! (`PutCharList`, `Client.cpp:4591-4599`; the port's `send_char_list`,
//! `services/game/src/auth.rs:67-117`). Opcode 5 is the "you are in the
//! world" record for the character the client selected; opcode 6 repeats
//! records for everyone else already standing in the caller's cell and
//! dispatches on each record's `uiType`: a
//! [`MAKECHARTYPE_PC`] record is the same 592-byte struct, any other record
//! is its 68-byte `cltMakeCharInfoCommon` head (`DoMsg_GAMEMSG_RESPONSE_
//! LOGIN` / `_LOGINMANY`, `Client-Login.cpp:88-93` / `:53-85`). The live
//! server's first in-world broadcast is NPC-led — 34 x 68-byte
//! `MAKECHARTYPE_NPC` records (`evidence/live/decode2.log`) — so the NPC
//! stride is the load-bearing half of the walk (LIVE-01,
//! `docs/rca/2026-10-28-live-opcode6-npc-stride.md`).
//!
//! # Which source owns which fact
//!
//! The vendored snapshot and the shipped binary disagree about these
//! structs' *sizes*: `cltMakeCharInfo` derives 524 from the source but the
//! binary's `PushChar` copies `0x94` dwords and `PopChar` strides `0x250`
//! = 592, and `cltCharListInfo`'s `cltPBasicInfo` derives 832 while the
//! binary reads `siLevel` at `[&clBI + 0x3A4]` (932) and strides `0x3D8`
//! = 984 (the server port's `MAKE_CHAR_INFO_PAYLOAD_LEN` /
//! `CHAR_LIST_PAYLOAD_LEN`, `gz-proto/src/game.rs:25-43`, which documents
//! both measurements). So this module splits the authority the way that
//! evidence does:
//!
//! - **field names, declared types and the bitfield block** come from the
//!   vendored source (`CommonHeader.h:1967-1982`, `:2565-2658`,
//!   `parametabox.h:95-114`, `:243-249`) - MSVC lays those out predictably,
//!   so the offsets below are derived here and cited;
//! - **absolute offsets past the bitfield header** come from the binary
//!   measurements the server port reports, since the snapshot's derived
//!   sizes are the thing proven stale.
//!
//! # Compression is the caller's job
//!
//! A 989-byte char-list frame and a 597-byte login frame are both above the
//! 200-byte LZF threshold the legacy applies to its **own** outbound frames
//! (`Client-Winsock.cpp:178-196`). The server port never compresses toward
//! the client (`send_raw` builds a bare `Frame`, `main.rs:2049-2056`), so
//! frames from it arrive uncompressed; a legacy server does compress, and
//! its consumer clears the flag only *after* decompressing
//! (`GameProtocol.cpp:109-141`). The codec leaves bit 15 in
//! [`Frame::cmd`] and exposes the cleared opcode as [`Frame::cmd_id`]
//! (`net/codec.rs:312-322`) - so these parsers are documented and tested
//! against an **already-decompressed** payload with the flag cleared, which
//! is the one contract both peers satisfy. This crate carries no LZF
//! implementation yet; when the receive loop needs one it lands beside the
//! codec, not inside a struct parser.
//!
//! # Test addresses are synthetic (F-01)
//!
//! As on every link so far (QC decisions F-01/F-02 on
//! CAR-2026-10-24-RETRO01): these are pure parsers, so the suite feeds them
//! hand-placed bytes and proves each field round-trips at its declared
//! offset. No socket, no server, nothing mocked.

use crate::game::login::PLAYER_NAME_LEN;
use crate::net::codec::Frame;

/// `GAMEMSG_RESPONSE_CHARLIST` - one `cltCharListInfo`, sent once per slot
/// (`MsgType.h:24`, the enumerator after `GAMEMSG_REQUEST_LOGIN`; the server
/// port's `GameMsg::ResponseCharlist`, `opcodes_game.rs`, same header).
pub const RESPONSE_CHARLIST: u16 = 3;

/// `GAMEMSG_RESPONSE_LOGIN` - the single `cltMakeCharInfo` that enters the
/// world (`MsgType.h:27`; the port's `GameMsg::ResponseLogin`).
pub const RESPONSE_LOGIN: u16 = 5;

/// `GAMEMSG_RESPONSE_LOGINMANY` - the same record, repeated for the other
/// players in the cell (`MsgType.h:28`; the port's `GameMsg::ResponseLoginmany`).
pub const RESPONSE_LOGINMANY: u16 = 6;

/// `MAX_TOTAL_CHAR_PER_USER` = `MAX_CHAR_PER_USER` (3) +
/// `MAX_CHAR_HOUSE_PER_USER` (12) (`CommonHeader.h:579-581`) - the roster's
/// slot count, and how many opcode 3 frames a login answers with (the port's
/// `CHAR_SLOTS`, walked in `send_char_list`).
pub const CHAR_SLOTS: usize = 15;

/// `sizeof(cltCharListInfo)`: `cltPBasicInfo` ([`P_BASIC_INFO_LEN`] 904) +
/// `cltPIPInfo` ([`PIP_INFO_LEN`] 64) + `DelCharTime` (`SYSTEMTIME`,
/// 16 bytes) = 984 (`Char-Common.h:453-471`); the port's
/// `CHAR_LIST_PAYLOAD_LEN` is the same 984, measured from the binary
/// (`game.rs:25-32`). The frame's `usSize` is therefore 989
/// (`MsgType.h:2967`).
pub const CHAR_LIST_PAYLOAD_LEN: usize = 984;

/// `sizeof(cltPBasicInfo)` - the identity-and-position block every roster
/// entry opens with (`CommonHeader.h:2565-2658`); the port measures 904
/// from the binary, superseding the snapshot's derived 832
/// (`game.rs:27-31`).
pub const P_BASIC_INFO_LEN: usize = 904;

/// `sizeof(cltPIPInfo)` - the ability/level/life/mana/exp/purse block
/// (`parametabox.h:95-114`, `:243-249`).
pub const PIP_INFO_LEN: usize = 64;

/// `sizeof(cltPBasicInfo) + sizeof(cltPIPInfo)` = 968 - where
/// `DelCharTime` begins.
pub const BASIC_INFO_LEN: usize = P_BASIC_INFO_LEN + PIP_INFO_LEN;

/// `sizeof(SYSTEMTIME)` - 8 `WORD`s (`WinDef.h`); all-zero means "no
/// deletion pending" (the port's `CharListEntry` reading).
pub const DEL_CHAR_TIME_LEN: usize = 16;

/// `sizeof(cltMakeCharInfo)` - the opcode 5 body and the opcode 6 record
/// stride: 592 = `0x250`, measured from the binary (`PushChar` copies `0x94`
/// dwords, `PopChar` strides `0x250`), superseding the snapshot's derived
/// 524 (`game.rs:37-43`).
pub const MAKE_CHAR_INFO_PAYLOAD_LEN: usize = 592;

/// `sizeof(cltMakeCharInfoCommon)` — the opcode 6 stride of a
/// [`MAKECHARTYPE_NPC`] record: 68 (LIVE-01,
/// `docs/rca/2026-10-28-live-opcode6-npc-stride.md`).
///
/// Dual-sourced, because this struct family's snapshot sizes are proven
/// stale (its `cltMakeCharInfo` derives 524 against the binary's 592,
/// `game.rs:37-43`):
///
/// - **Wire.** The live server's first in-world broadcast
///   (`evidence/live/decode2.log`) is 34 records of exactly 68 bytes —
///   `payload=2312 divisible by 68: true` — every head `uiType = 0`
///   ([`MAKECHARTYPE_NPC`]), ids monotone 8500→8533.
/// - **Source.** The MSVC field sum of `cltMakeCharInfoCommon`
///   (`CommonHeader.h:1967-1995`): 4 `UI32` bitfield words = 16
///   (`:1969-1977`) + `siHomeVillageUnique` `SI08` + pad +
///   `siJimCharUnique` `SI16` = 4 (`:1978-1979`) + `clOrder` = 20
///   (`cltOrderDataInfo`, `:1495-1514`) + `clIdentity` = 12 (`:1846`, the
///   `SI08` + pad + 8-byte identity union)
///   + `clInnerStatus` = 16 (`cltInnerStatusInfo`, `:1744-1767`) = **68**.
///
/// The two sources agree on the layout, not just the total: byte 56 of every
/// captured record is `0x01` = `ENEMYTYPE_ALLPC`, the
/// `clInnerStatus.clEnemyType.siEnemyType` word at offset 0x38 under that
/// sum.
pub const MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN: usize = 68;

/// `MAKECHARTYPE_PC` (`CommonHeader.h:1893`) - the `uiType` value that marks
/// a record as a full player character.
pub const MAKECHARTYPE_PC: u32 = 1;

/// `MAKECHARTYPE_NPC` (`CommonHeader.h:1892`).
pub const MAKECHARTYPE_NPC: u32 = 0;

/// Why a character-info parse failed.
#[derive(Debug, thiserror::Error)]
pub enum CharInfoError {
    /// The frame's opcode is not the one this parser accepts - dispatch
    /// drift or a frame this port's contract does not cover
    /// (`Client-Message.cpp:315` registers opcode 3, `Client-Login.cpp:88`
    /// opcode 5, `:53` opcode 6).
    #[error("expected game opcode {expected}, got {cmd}")]
    UnexpectedOpcode {
        /// The opcode this parser accepts.
        expected: u16,
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The body was shorter than the struct - where the legacy would read
    /// out of bounds (`Client-SystemNPC.cpp:836`, `Client-Login.cpp:90`).
    #[error("game opcode {opcode} body truncated: expected {expected} bytes, got {got}")]
    Truncated {
        /// Required length.
        expected: usize,
        /// Payload length actually received.
        got: usize,
        /// The opcode being parsed, for the message.
        opcode: u16,
    },
    /// The body ran past the struct - trailing bytes the legacy would
    /// silently ignore (`Client-SystemNPC.cpp:836` copies `sizeof` only).
    #[error("game opcode {opcode} body has trailing bytes: expected {expected} bytes, got {got}")]
    TrailingBytes {
        /// Required length.
        expected: usize,
        /// Payload length actually received.
        got: usize,
        /// The opcode being parsed, for the message.
        opcode: u16,
    },
    /// An opcode 6 body that does not end on a whole record boundary - the
    /// client walks the stream by stride with no length prefix
    /// (`Client-Login.cpp:60-84`), so a partial tail would be read as the
    /// head of a record. `expected` is the stride the walk demanded at that
    /// offset: 592 behind a [`MAKECHARTYPE_PC`] head, [`MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN`]
    /// below the common-head minimum.
    #[error("LOGINMANY record at offset {offset} is only {have} of {expected} bytes")]
    PartialRecord {
        /// Byte offset of the incomplete record.
        offset: usize,
        /// Bytes available from that offset.
        have: usize,
        /// Required record length at that offset.
        expected: usize,
    },
    /// An opcode 6 record whose `uiType` is neither [`MAKECHARTYPE_PC`] nor
    /// [`MAKECHARTYPE_NPC`]. The legacy strides by that field
    /// (`Client-Login.cpp:62-75`); `uiType` is declared `:1`
    /// (`CommonHeader.h:1969`), so the walker's mask can only yield 0 or 1
    /// and this error is unreachable from a well-formed wire. It is kept
    /// loud as the refusal for the day the field widens and the stride
    /// table goes stale - a stale stride would mis-read every record after
    /// the change, which is exactly the LIVE-01 failure mode.
    #[error("LOGINMANY record at offset {offset} has uiType {ui_type}, neither MAKECHARTYPE_PC (1) nor MAKECHARTYPE_NPC (0)")]
    UnknownRecordType {
        /// Byte offset of the record whose type cannot be strided.
        offset: usize,
        /// The `uiType` value found there.
        ui_type: u32,
    },
}

/// Reads a `MAX_PLAYER_NAME`-byte C-string field: bytes up to the first NUL,
/// as `StringCchCopy` writes it (`Lib\commonutil.cpp:6-16`) and as the port's
/// `fixed_str` reads it (`game.rs`, `Reader::fixed_str`).
fn fixed_str(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// A `u32` read little-endian at `at`.
fn u32_at(payload: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(
        payload[at..at + 4]
            .try_into()
            .expect("4 bytes checked by the length gate"),
    )
}

/// An `i16` read little-endian at `at`.
fn i16_at(payload: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(
        payload[at..at + 2]
            .try_into()
            .expect("2 bytes checked by the length gate"),
    )
}

/// An `i32` read little-endian at `at`.
fn i32_at(payload: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(
        payload[at..at + 4]
            .try_into()
            .expect("4 bytes checked by the length gate"),
    )
}

/// An `i64` read little-endian at `at`.
fn i64_at(payload: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(
        payload[at..at + 8]
            .try_into()
            .expect("8 bytes checked by the length gate"),
    )
}

/// One roster slot: `cltCharListInfo` (`Char-Common.h:453-471`) as it sits
/// on the wire, with every field at its literal offset.
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0..4 | [`account_idnum`](Self::account_idnum) | `clBI.siIDNum` `SI32` |
/// | 4..24 | [`account_id`](Self::account_id) | `clBI.szAccountID[20]` |
/// | 24 | [`slot`](Self::slot) | `clBI.uiIndex` `UI08` |
/// | 25 | [`sex`](Self::sex) | `clBI.uiSex` `UI08` |
/// | 26..28 | [`class_kind`](Self::class_kind) | `clBI.uiKind` `UI16` |
/// | 28 | [`home_village`](Self::home_village) | `clBI.siHomeVillage` `SI08` |
/// | 29..32 | (unparsed) | `uiGRank`, `uiAngelSwitch`, `uiPoliceSwitch` |
/// | 32..34 | [`map_index`](Self::map_index) | `clBI.clPos.siMapIndex` `SI16` |
/// | 34..36 | [`x`](Self::x) | `clPos.siX` `SI16` |
/// | 36..38 | [`y`](Self::y) | `clPos.siY` `SI16` |
/// | 38..40 | (unparsed) | `clPos.siReserved` `SI16` |
/// | 40..76 | (unparsed) | play seconds, last main-map pos, school, wealth |
/// | 76..80 | [`character_id`](Self::character_id) | `clPerson.siPersonID` `SI32` |
/// | 80..100 | [`name`](Self::name) | `clPerson.szName[20]` |
/// | 100..904 | (unparsed) | father/recommender, services, PVP, naming, attendance |
/// | 904..932 | [`str_`](Self::str_) .. [`wis`](Self::wis) | `clIP` <- `cltBasicAbility` |
/// | 932..934 | [`level`](Self::level) | `clIP.siLevel` `SI16` |
/// | 934..936 | [`bonus`](Self::bonus) | `clIP.siBonus` `SI16` |
/// | 936..940 | [`life`](Self::life) | `clIP.Life` `SI32` |
/// | 940..944 | [`mana`](Self::mana) | `clIP.Mana` `SI32` |
/// | 944..952 | [`exp`](Self::exp) | `clIP.siExp` `SI64` |
/// | 952..960 | (unparsed) | `siFameLevel`, `siSkillBonus`, `siFameExp` |
/// | 960..968 | [`money`](Self::money) | `clIP.siMoney` `GMONEY` |
/// | 968..984 | [`del_char_time`](Self::del_char_time) | `DelCharTime` `SYSTEMTIME` |
///
/// The offsets come from the binary the server port measured (module docs,
/// "which source owns which fact"); the names and types are the snapshot's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharListEntry {
    /// `clBI.siIDNum` at 0..4 - the owning account's `idnum`, which is also
    /// the `auth_key` the front handed out (Step 3.3).
    pub account_idnum: i32,
    /// `clBI.szAccountID[20]` at 4..24 - the account name the server echoes
    /// back inside every roster row.
    pub account_id: String,
    /// `clBI.uiIndex` at 24 - the slot `PutCharList` writes into
    /// (`Client.cpp:4595`), hence [`CHAR_SLOTS`] wide.
    pub slot: u8,
    /// `clBI.uiSex` at 25 - 1 male, 0 female (`CommonHeader.h:2570`).
    pub sex: u8,
    /// `clBI.uiKind` at 26..28 - the class/avatar id, the same value
    /// `MakeCharInfo::make_char_kind` carries.
    pub class_kind: u16,
    /// `clBI.siHomeVillage` at 28 - the home village unique.
    pub home_village: i8,
    /// `clBI.clPos.siMapIndex` at 32..34 - the map index the client holds,
    /// not a `map_id`.
    pub map_index: i16,
    /// `clBI.clPos.siX` at 34..36 - the stored tile x.
    pub x: i16,
    /// `clBI.clPos.siY` at 36..38 - the stored tile y.
    pub y: i16,
    /// `clBI.clPerson.siPersonID` at 76..80 - the character's own id.
    pub character_id: i32,
    /// `clBI.clPerson.szName[20]` at 80..100 - the character name.
    pub name: String,
    /// `clIP.siStr` at 904..908 - strength (`CommonHeader.h:2746`).
    pub str_: i32,
    /// `clIP.siDex` at 908..912 - dexterity (`:2747`).
    pub dex: i32,
    /// `clIP.siVit` at 912..916 - vitality (`:2748`).
    pub vit: i32,
    /// `clIP.siMag` at 916..920 - magic (`:2749`).
    pub mag: i32,
    /// `clIP.siHnd` at 920..924 - handling (`:2750`).
    pub hnd: i32,
    /// `clIP.siLuk` at 924..928 - luck (`:2751`).
    pub luk: i32,
    /// `clIP.siWis` at 928..932 - wisdom (`:2752`).
    pub wis: i32,
    /// `clIP.siLevel` at 932..934 - the level the roster dialog renders in
    /// `%s(Lv.%d)` (the port cites the read at `[clIP + 0x1C]`, `game.rs:224-227`).
    pub level: i16,
    /// `clIP.siBonus` at 934..936 - unspent stat points.
    pub bonus: i16,
    /// `clIP.Life` at 936..940 - hit points (the "hp" of the scope's field
    /// list).
    pub life: i32,
    /// `clIP.Mana` at 940..944 - magic points (the "mp").
    pub mana: i32,
    /// `clIP.siExp` at 944..952 - experience (`SI64`, `parametabox.h:111`).
    pub exp: i64,
    /// `clIP.siMoney` at 960..968 - the purse, the last eight bytes of
    /// `cltPIPInfo` (`parametabox.h:249`).
    pub money: i64,
    /// `DelCharTime` at 968..984 - the raw `SYSTEMTIME`; all-zero means no
    /// deletion is pending.
    pub del_char_time: [u8; DEL_CHAR_TIME_LEN],
}

impl CharListEntry {
    /// Parses a whole roster frame: opcode first (matched against
    /// [`Frame::cmd_id`], so the caller has already cleared the compression
    /// flag along with decompressing, module docs), then exactly
    /// [`CHAR_LIST_PAYLOAD_LEN`] bytes (module docs, the layout).
    ///
    /// # Errors
    ///
    /// As [`parse_char_list_entry`].
    pub fn from_frame(frame: &Frame) -> Result<Self, CharInfoError> {
        parse_char_list_entry(frame.cmd_id(), frame.payload())
    }

    /// Whether `DelCharTime` is all zero - "no deletion pending", the value
    /// the port sends for every roster row (`game.rs`, `CharListEntry::encode`).
    #[must_use]
    pub const fn is_delete_pending(&self) -> bool {
        let mut i = 0;
        while i < DEL_CHAR_TIME_LEN {
            if self.del_char_time[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }
}

/// The byte-level roster parser behind [`CharListEntry::from_frame`],
/// callable with raw `(cmd, payload)` for byte-exact testing.
///
/// Opcode **before** length: anything other than [`RESPONSE_CHARLIST`] is
/// refused without its payload ever being measured. Then exactly
/// [`CHAR_LIST_PAYLOAD_LEN`] bytes.
///
/// # Errors
///
/// [`CharInfoError::UnexpectedOpcode`] for a foreign command;
/// [`CharInfoError::Truncated`] below [`CHAR_LIST_PAYLOAD_LEN`] (the legacy
/// casts `cData` and copies `sizeof(cltCharListInfo)` at
/// `Client-SystemNPC.cpp:836-837`);
/// [`CharInfoError::TrailingBytes`] above it (the legacy ignores them).
pub fn parse_char_list_entry(cmd: u16, payload: &[u8]) -> Result<CharListEntry, CharInfoError> {
    if cmd != RESPONSE_CHARLIST {
        return Err(CharInfoError::UnexpectedOpcode {
            expected: RESPONSE_CHARLIST,
            cmd,
        });
    }
    match payload.len() {
        len if len < CHAR_LIST_PAYLOAD_LEN => {
            return Err(CharInfoError::Truncated {
                expected: CHAR_LIST_PAYLOAD_LEN,
                got: len,
                opcode: cmd,
            });
        }
        len if len > CHAR_LIST_PAYLOAD_LEN => {
            return Err(CharInfoError::TrailingBytes {
                expected: CHAR_LIST_PAYLOAD_LEN,
                got: len,
                opcode: cmd,
            });
        }
        _ => {}
    }

    // cltPBasicInfo (0..904): CommonHeader.h:2565-2658, offsets as measured.
    let account_idnum = i32_at(payload, 0);
    let account_id = fixed_str(&payload[4..4 + PLAYER_NAME_LEN]);
    let slot = payload[24];
    let sex = payload[25];
    let class_kind = u16::from_le_bytes([payload[26], payload[27]]);
    let home_village = payload[28] as i8;
    let map_index = i16_at(payload, 32);
    let x = i16_at(payload, 34);
    let y = i16_at(payload, 36);
    let character_id = i32_at(payload, 76);
    let name = fixed_str(&payload[80..80 + PLAYER_NAME_LEN]);

    // cltPIPInfo at 904: cltBasicAbility (parametabox.h:95 -> CommonHeader.h:
    // 2743-2752, seven SI32 in declaration order), then cltPIPInfoCommon's
    // level/bonus/life/mana/exp and cltPIPInfo's fame/skill/purse tail.
    let str_ = i32_at(payload, 904);
    let dex = i32_at(payload, 908);
    let vit = i32_at(payload, 912);
    let mag = i32_at(payload, 916);
    let hnd = i32_at(payload, 920);
    let luk = i32_at(payload, 924);
    let wis = i32_at(payload, 928);
    let level = i16_at(payload, 932);
    let bonus = i16_at(payload, 934);
    let life = i32_at(payload, 936);
    let mana = i32_at(payload, 940);
    let exp = i64_at(payload, 944);
    let money = i64_at(payload, 960);

    let mut del_char_time = [0u8; DEL_CHAR_TIME_LEN];
    del_char_time.copy_from_slice(&payload[968..968 + DEL_CHAR_TIME_LEN]);

    Ok(CharListEntry {
        account_idnum,
        account_id,
        slot,
        sex,
        class_kind,
        home_village,
        map_index,
        x,
        y,
        character_id,
        name,
        str_,
        dex,
        vit,
        mag,
        hnd,
        luk,
        wis,
        level,
        bonus,
        life,
        mana,
        exp,
        money,
        del_char_time,
    })
}

/// One world-entry record: `cltMakeCharInfo` (`CommonHeader.h:2166`, whose
/// base `cltMakeCharInfoCommon` is `:1967-1995`) as it sits on the wire.
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0x00 bit 0 | [`is_player`](Self::is_player) | `uiType:1` |
/// | 0x00 bits 1-16 | [`make_char_kind`](Self::make_char_kind) | `uiMakeCharKind:16` |
/// | 0x04..0x06 | [`make_char_unique`](Self::make_char_unique) | `uiMakeCharUnique:16` |
/// | 0x06 | [`level`](Self::level) | `uiLevel:8` |
/// | 0x07 | (pad) | MSVC aligns the next `UI32` to 4 |
/// | 0x08 bits 0-19 | [`life`](Self::life) | `uiLife:20` |
/// | 0x08 bits 20-29 | [`x`](Self::x) | `uiX:10` |
/// | 0x0C bits 0-9 | [`y`](Self::y) | `uiY:10` |
/// | 0x0C bits 10-19 | [`map_index`](Self::map_index) | `uiMapIndex:10` |
/// | 0x0C bits 20-25 | [`total_move_speed`](Self::total_move_speed) | `uiTotalMoveSpeed:6` |
/// | 0x10 | [`home_village_unique`](Self::home_village_unique) | `siHomeVillageUnique` `SI08` |
/// | 0x11 | (pad) | `SI16` alignment of the next field |
/// | 0x12..0x14 | [`jim_char_unique`](Self::jim_char_unique) | `siJimCharUnique` `SI16` |
/// | 0x14..0x38 | (unparsed) | `clOrder`, `clIdentity`, `siStatus` |
/// | 0x38..0x3A | [`enemy_type`](Self::enemy_type) | `clEnemyType.siEnemyType` |
/// | 0x3A..0x50 | (unparsed) | `siPara1`, `clEventInfo`, `siTopSkillUnique` |
/// | 0x50 | [`current_show_horse`](Self::current_show_horse) | `siCurrentShowHorse` |
/// | 0x51 | [`game_master_level`](Self::game_master_level) | `siGameMasterLevel` |
/// | 0x52..0x54 | [`map_unique`](Self::map_unique) | `siMapUnique` `SI16` |
/// | 0x54..0x84 | (unparsed) | `clShapeEquip` |
/// | 0x84..0x92 | (unparsed) | `clRank` |
/// | 0x92..0x94 | [`make_ani_type`](Self::make_ani_type) | `siMakeAniType` `SI16` |
/// | 0x94..0x250 | (unparsed) | `clGuildRank` + tail pad |
///
/// The header block's bit positions are derived from the declarations above
/// under MSVC's low-bits-first rule; the absolute offsets from 0x38 onward
/// are the binary measurements the server port reports (module docs, "which
/// source owns which fact").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeCharInfo {
    /// `uiType` at 0x00 bit 0 == [`MAKECHARTYPE_PC`] - whether the record is
    /// a player character rather than an NPC.
    pub is_player: bool,
    /// `uiMakeCharKind` at 0x00 bits 1-16 - the class/kind id, the same
    /// value the roster's `class_kind` carries.
    pub make_char_kind: u16,
    /// `uiMakeCharUnique` at 0x04..0x06 - the session character id, which
    /// the client compares against its own `MyCharUnique`
    /// (`Client-Login.cpp:116`) and indexes `CharUniqueInfo[]` with.
    pub make_char_unique: u16,
    /// `uiLevel` at 0x06 - the level (0..255, `CommonHeader.h:1972`).
    pub level: u8,
    /// `uiLife` at 0x08 bits 0-19 - current hit points (20 bits).
    pub life: u32,
    /// `uiX` at 0x08 bits 20-29 - the map tile x (10 bits), copied straight
    /// into `clPos.siX` by the client (`Client-Login.cpp:192-194`).
    pub x: u16,
    /// `uiY` at 0x0C bits 0-9 - the map tile y (10 bits)
    /// (`Client-Login.cpp:194`).
    pub y: u16,
    /// `uiMapIndex` at 0x0C bits 10-19 - the `MapData` index the client
    /// loads (`Client-Login.cpp:243-245`).
    pub map_index: u16,
    /// `uiTotalMoveSpeed` at 0x0C bits 20-25 - base movement speed (6 bits).
    pub total_move_speed: u8,
    /// `siHomeVillageUnique` at 0x10 - home village unique. The snapshot
    /// declares `SI08`, so this is signed; the server port models the same
    /// byte as `u8`, which differs only for values the field cannot carry
    /// (`0 <= unique < MAX_VILLAGE_NUMBER`, `CommonHeader.h:583`).
    pub home_village_unique: i8,
    /// `siJimCharUnique` at 0x12..0x14 - the slave/jim character unique
    /// (`CommonHeader.h:1979`).
    pub jim_char_unique: i16,
    /// `clEnemyType.siEnemyType` at 0x38..0x3A - the hostility mask; the
    /// one body field the port always sets for a player
    /// (`ENEMYTYPE_PC`-style as the port names it, `game.rs:62-71`).
    pub enemy_type: u16,
    /// `siCurrentShowHorse` at 0x50 - which summoned horse is displayed.
    pub current_show_horse: u8,
    /// `siGameMasterLevel` at 0x51 - the GM level, forwarded from the
    /// account row by the port.
    pub game_master_level: u8,
    /// `siMapUnique` at 0x52..0x54 - 0 lets the client resolve the map from
    /// its own `MapData` (`Client-Login.cpp:243`).
    pub map_unique: i16,
    /// `siMakeAniType` at 0x92..0x94 - the personal make-item animation
    /// type.
    pub make_ani_type: i16,
}

impl MakeCharInfo {
    /// Parses a whole opcode 5 frame: opcode first (matched against
    /// [`Frame::cmd_id`], so the caller has already cleared the compression
    /// flag along with decompressing, module docs), then exactly
    /// [`MAKE_CHAR_INFO_PAYLOAD_LEN`] bytes (module docs, the layout).
    ///
    /// # Errors
    ///
    /// As [`parse_make_char_info`].
    pub fn from_frame(frame: &Frame) -> Result<Self, CharInfoError> {
        parse_make_char_info(frame.cmd_id(), frame.payload())
    }
}

/// The byte-level record parser behind [`MakeCharInfo::from_frame`],
/// callable with raw `(cmd, payload)` for byte-exact testing.
///
/// Opcode **before** length, then exactly [`MAKE_CHAR_INFO_PAYLOAD_LEN`]
/// bytes - the binary's stride for a [`MAKECHARTYPE_PC`] record.
///
/// # Errors
///
/// [`CharInfoError::UnexpectedOpcode`] for a command other than
/// [`RESPONSE_LOGIN`]; [`CharInfoError::Truncated`] /
/// [`CharInfoError::TrailingBytes`] for any other length (the legacy casts
/// `cData` to `cltMakeCharInfo*` and reads `sizeof` worth, with no length
/// check at all, `Client-Login.cpp:90`).
pub fn parse_make_char_info(cmd: u16, payload: &[u8]) -> Result<MakeCharInfo, CharInfoError> {
    if cmd != RESPONSE_LOGIN {
        return Err(CharInfoError::UnexpectedOpcode {
            expected: RESPONSE_LOGIN,
            cmd,
        });
    }
    match payload.len() {
        len if len < MAKE_CHAR_INFO_PAYLOAD_LEN => {
            return Err(CharInfoError::Truncated {
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN,
                got: len,
                opcode: cmd,
            });
        }
        len if len > MAKE_CHAR_INFO_PAYLOAD_LEN => {
            return Err(CharInfoError::TrailingBytes {
                expected: MAKE_CHAR_INFO_PAYLOAD_LEN,
                got: len,
                opcode: cmd,
            });
        }
        _ => {}
    }
    Ok(decode_record(payload))
}

/// Decodes the `cltMakeCharInfoCommon` head every opcode 6 record opens
/// with - the fields through 0x3A, all inside the 68-byte common prefix
/// ([`MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN`], dual-sourced there). The PC-only
/// tail fields stay at zero, which mirrors the legacy's NPC path:
/// `clinfo.Set(pclinfo)` copies only the common prefix into its record and
/// leaves the rest at `Init()` zeros (`Client-Login.cpp:73`).
fn decode_head(payload: &[u8]) -> MakeCharInfo {
    // 0x00: uiType:1 | uiMakeCharKind:16 - declared order, low bits first
    // (CommonHeader.h:1969-1970).
    let type_kind = u32_at(payload, 0);
    // 0x04: uiMakeCharUnique:16 | uiLevel:8, then a pad byte at 0x07
    // (the next UI32 is 4-aligned; :1971-1972).
    let unique_level = u32_at(payload, 0x04);
    // 0x08: uiLife:20 | uiX:10 (:1973-1974).
    let life_x = u32_at(payload, 0x08);
    // 0x0C: uiY:10 | uiMapIndex:10 | uiTotalMoveSpeed:6 (:1975-1977).
    let y_map_speed = u32_at(payload, 0x0C);

    MakeCharInfo {
        is_player: type_kind & 0x1 == MAKECHARTYPE_PC,
        make_char_kind: (type_kind >> 1) as u16,
        make_char_unique: (unique_level & 0xFFFF) as u16,
        level: ((unique_level >> 16) & 0xFF) as u8,
        life: life_x & 0xF_FFFF,
        x: ((life_x >> 20) & 0x3FF) as u16,
        y: (y_map_speed & 0x3FF) as u16,
        map_index: ((y_map_speed >> 10) & 0x3FF) as u16,
        // Masked to the declared 6-bit width (:1977); the port's own read
        // leaves the mask off, which differs only if the field's reserved
        // high bits were ever set.
        total_move_speed: ((y_map_speed >> 20) & 0x3F) as u8,
        home_village_unique: payload[0x10] as i8,
        jim_char_unique: i16_at(payload, 0x12),
        enemy_type: u16::from_le_bytes([payload[0x38], payload[0x39]]),
        current_show_horse: 0,
        game_master_level: 0,
        map_unique: 0,
        make_ani_type: 0,
    }
}

/// The PC-only tail fields past the common prefix: `siCurrentShowHorse` at
/// 0x50, `siGameMasterLevel` at 0x51, `siMapUnique` at 0x52 and
/// `siMakeAniType` at 0x92 - the binary-measured offsets (module docs,
/// "which source owns which fact"). No field here exists in
/// `cltMakeCharInfoCommon`, which is what makes 68 the whole of an NPC
/// record.
fn fill_pc_tail(record: &mut MakeCharInfo, payload: &[u8]) {
    record.current_show_horse = payload[0x50];
    record.game_master_level = payload[0x51];
    record.map_unique = i16_at(payload, 0x52);
    record.make_ani_type = i16_at(payload, 0x92);
}

/// Decodes one already-length-checked 592-byte [`MAKECHARTYPE_PC`] record:
/// the common head plus the tail [`fill_pc_tail`] adds. Opcode 5's parser
/// uses it whole; the opcode 6 walker splits the two halves so an NPC
/// record gets the head alone.
fn decode_record(payload: &[u8]) -> MakeCharInfo {
    let mut record = decode_head(payload);
    fill_pc_tail(&mut record, payload);
    record
}

/// Parses an opcode 6 body: records packed back to back with no length
/// prefix, each strided by its own leading `uiType` exactly as the legacy
/// walk (`DoMsg_GAMEMSG_RESPONSE_LOGINMANY`, `Client-Login.cpp:53-85`):
/// [`MAKECHARTYPE_PC`] → [`MAKE_CHAR_INFO_PAYLOAD_LEN`] (592, the binary-
/// measured `cltMakeCharInfo`), [`MAKECHARTYPE_NPC`] →
/// [`MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN`] (68, `sizeof(cltMakeCharInfoCommon)`,
/// dual-sourced at the constant). A mixed batch is the normal case, not an
/// edge: the live server's first in-world broadcast is 34 NPC records led
/// (`evidence/live/decode2.log`, LIVE-01).
///
/// An NPC record decodes to its common-head fields with the PC-only tail
/// zeroed, mirroring the legacy `clinfo.Set(pclinfo)` copy
/// (`Client-Login.cpp:73`); the dispatcher decides what state, if any, such
/// a record carries.
///
/// Empty bodies are refused rather than parsed as zero records: the client
/// reads `uiType` before it knows whether it got any bytes, and the port
/// therefore sends nothing at all when a cell is empty rather than a
/// zero-length frame (`game.rs`, `encode_login_many_batches`, which returns
/// no batch at all for no records).
///
/// # Errors
///
/// [`CharInfoError::UnexpectedOpcode`] for a command other than
/// [`RESPONSE_LOGINMANY`]; [`CharInfoError::Truncated`] for an empty body;
/// [`CharInfoError::PartialRecord`] for a body that does not end on a whole
/// record boundary, named by offset, shortfall and the stride that offset
/// demanded (68 below the head minimum, 592 behind a PC head);
/// [`CharInfoError::UnknownRecordType`] for an `uiType` that is neither PC
/// nor NPC - unreachable from a 1-bit field, kept loud.
pub fn parse_make_char_info_many(
    cmd: u16,
    payload: &[u8],
) -> Result<Vec<MakeCharInfo>, CharInfoError> {
    if cmd != RESPONSE_LOGINMANY {
        return Err(CharInfoError::UnexpectedOpcode {
            expected: RESPONSE_LOGINMANY,
            cmd,
        });
    }
    if payload.is_empty() {
        return Err(CharInfoError::Truncated {
            expected: MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN,
            got: 0,
            opcode: cmd,
        });
    }

    let mut records = Vec::new();
    let mut offset = 0;
    while offset < payload.len() {
        let remaining = payload.len() - offset;
        // Every record is at least a `cltMakeCharInfoCommon` head - the
        // legacy reads `uiType` straight off the wire at this offset with no
        // bounds check (`Client-Login.cpp:62`), so a short tail is garbage,
        // not a record.
        if remaining < MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN {
            return Err(CharInfoError::PartialRecord {
                offset,
                have: remaining,
                expected: MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN,
            });
        }
        // `uiType` is the head's first bit (`CommonHeader.h:1969`); the
        // legacy switches the stride on it (`Client-Login.cpp:64-75`).
        let ui_type = u32_at(payload, offset) & 0x1;
        let stride = match ui_type {
            MAKECHARTYPE_PC => MAKE_CHAR_INFO_PAYLOAD_LEN,
            MAKECHARTYPE_NPC => MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN,
            // A 1-bit field cannot carry any other value - this arm is
            // unreachable from the wire and exists only as the loud refusal
            // for a widened field meeting a stale stride table.
            other => {
                return Err(CharInfoError::UnknownRecordType {
                    offset,
                    ui_type: other,
                });
            }
        };
        if remaining < stride {
            return Err(CharInfoError::PartialRecord {
                offset,
                have: remaining,
                expected: stride,
            });
        }
        let record = &payload[offset..offset + stride];
        let mut decoded = decode_head(record);
        if ui_type == MAKECHARTYPE_PC {
            fill_pc_tail(&mut decoded, record);
        }
        records.push(decoded);
        offset += stride;
    }

    Ok(records)
}

#[cfg(test)]
mod tests;
