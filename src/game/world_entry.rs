//! Steps 4.5–4.7 — the world-entry flow: select a character
//! (`GAMEMSG_REQUEST_SELECTCHAR`, opcode 4), receive the server's world
//! banner (`GAMEMSG_RESPONSE_SERVERINFO`, opcode 106) and the world-entry
//! record (`GAMEMSG_RESPONSE_LOGIN`, opcode 5) that populates the local
//! [`CharClient`], then request the village blueprint
//! (`GAMEMSG_REQUEST_VILLAGESTRUCT`, opcode 1101) and answer
//! (`GAMEMSG_RESPONSE_VILLAGESTRUCT`, opcode 1102) that gates avatar
//! creation (`README.md:87-98`, items 5–7).
//!
//! # The wire sequence this module models
//!
//! ```text
//! client                                   game service
//!   |  REQUEST_SELECTCHAR (4)  slot = SI32      |
//!   |------------------------------------------>|
//!   |  RESPONSE_SERVERINFO (106)   48 bytes     |   Server-Login.cpp:184
//!   |<------------------------------------------|
//!   |  RESPONSE_LOGIN (5)       592 bytes       |   MakeCharInfo, opcode 5
//!   |<------------------------------------------|
//!   |  REQUEST_VILLAGESTRUCT (1101) ref = SI32  |   Client-Login.cpp:281
//!   |------------------------------------------>|
//!   |  RESPONSE_VILLAGESTRUCT (1102) 22 bytes   |   Client-SystemNPC.cpp:875
//!   |<------------------------------------------|
//! ```
//!
//! The service answers a single `REQUEST_SELECTCHAR` with SERVERINFO **then**
//! RESPONSE_LOGIN, "matching the legacy order in `ServerDB-Person.cpp`: the
//! server info first, then `GAMEMSG_RESPONSE_LOGIN` carrying `clMakeCharInfo`"
//! (`services/game/src/auth.rs`, `handle_select_char`). SERVERINFO must
//! precede LOGIN because it sets the switch the avatar-creation path checks
//! (below). The client's village request is *not* part of that burst: it is
//! issued only once the local map is loaded and only when that map is a
//! village (`Client-Login.cpp:279-288`).
//!
//! # Which source owns which fact
//!
//! Field names and declared types come from the vendored C++ snapshot;
//! absolute payload sizes and the `cltMakeCharInfo` bit offsets come from the
//! binary measurements the server port reports (the same split
//! `game::char_info` documents). The server port is the executable cross-check
//! for every constant here: its `decode_select_char` / `decode_village_struct_request`
//! read a bare `SI32` (`gz-proto/src/game.rs:941,1365`), its `ServerInfo::encode`
//! lays out the same 48 bytes this parser reads (`game.rs:990-1015`), and its
//! `VillageStruct` is the same 22 (`game.rs:48`).
//!
//! # The three switches that *are* the in-world condition
//!
//! The legacy tracks world entry on `cltClient`'s boolean switches, and those
//! switches — not a state-box value — are what actually gate avatar creation:
//!
//! | Switch | Set by | Cleared by | Checked by |
//! |---|---|---|---|
//! | `bReceiveServerInfoSwitch` | SERVERINFO handler (`Client-System.cpp:1720`) | session reset (`Client.cpp:1003`) | `SetCharReservedChar` (`Client-SetChar.cpp:110`) |
//! | `bWaitVillageStructureInfoSwitch` | the LOGIN handler when it requests a village (`Client-Login.cpp:288`) | the VILLAGESTRUCT handler (`Client-SystemNPC.cpp:896`) | `PopChar` (`Client-SetChar.cpp:66`) |
//! | the populated `cltCharClient` | `SetChar` from the opcode-5 record whose `uiMakeCharUnique == MyCharUnique` (`Client-Login.cpp:116`) | `DeleteChar` on re-login (`Client-Login.cpp:219`) | every per-frame path |
//!
//! [`WorldEntryState`] models exactly those three. [`WorldEntryState::is_in_world`]
//! is therefore derived, not stored: a character is in the world when the
//! server banner matched, the local [`CharClient`] is populated, and no
//! village blueprint is still outstanding.
//!
//! **Why this is derived rather than a `USERSTATE_GAME_PLAYING` transition:**
//! the state box declares `USERSTATE_GAME_PLAYING` (`StateBox.h:17`), but no
//! `clUserState.SetState(USERSTATE_GAME_PLAYING)` call exists anywhere in the
//! shipped client source — the only game-state write is
//! `USERSTATE_GAME_AUTHOK` at `LogIn.cpp:992`, set on the opcode 2 login
//! *before* a character is even chosen. The state box, in other words, tracks
//! connection phases and never records the in-world transition; the switches
//! above are the observable that does. This port models the observable and
//! does not fabricate a state-box write the legacy never performs.
//!
//! # The service-area gate — a packet that can legally be dropped
//!
//! The SERVERINFO handler opens with a region check and returns *without
//! setting any switch* when it fails:
//!
//! ```cpp
//! if(siServiceArea != pclInfo->siServiceArea) { return; }   // Client-System.cpp:1682
//! ```
//!
//! [`WorldEntryState::apply_server_info`] reproduces that: a banner whose
//! `siServiceArea` differs from the client's own is **dropped** — reported as
//! `Ok(false)` with the state untouched — not raised as an error, because the
//! legacy treats it as an expected non-event. The English deployment's value
//! is [`SERVICE_AREA_ENGLISH`] (0x08), which the server port advertises from
//! its own `service_area` config default (`services/game/src/config.rs:85,119`).
//!
//! # The village ref comes from map data the port does not hold yet
//!
//! The legacy requests the blueprint for `pclMapManager->pclCurrentMap->siVillageUnique`
//! (`Client-Login.cpp:281`) — a value read out of the `MapData` row the client
//! loads for `uiMapIndex`. This port has no map-data module, so the *lookup*
//! is the caller's: [`request_village_struct`] takes the ref, and
//! [`WorldEntryState::mark_village_struct_requested`] records the outstanding wait.
//! The sender, the strict-range check, and the wait tracking are complete
//! here; only the map-data query — a separate concern with its own wire truth
//! — is out of scope, and that boundary is stated rather than stubbed.
//!
//! # Refuse, not truncate
//!
//! Every parser here is strict on both ends of the length, exactly like Steps
//! 3.2–4.4: the opcode is checked first, then the body must be *exactly* the
//! struct's size — shorter is [`WorldEntryError::Truncated`] (where the legacy
//! would read out of bounds), longer is [`WorldEntryError::TrailingBytes`]
//! (where the legacy would silently ignore the tail). The two `SI32` request
//! bodies validate their value ranges the way the receiving side does, so a
//! slot or ref no server would honour is refused before it reaches the socket.
//!
//! # Test addresses are synthetic (F-01)
//!
//! As on every link so far (QC decisions F-01/F-02 on
//! CAR-2026-10-24-RETRO01): the send-path suite runs against a loopback peer,
//! proving the frame size, opcode, bit-exact payload and session-cipher
//! pairing — not reachability of a live Game Server. The parsers are pure and
//! are fed hand-placed bytes.

use crate::game::char_info::{parse_make_char_info, CharInfoError, MakeCharInfo};
use crate::game::entry::{GameError, GameLink};
use crate::protocol::bytes::{i16_at, i32_at, i64_at};

/// `GAMEMSG_REQUEST_SELECTCHAR` — "the message that selects a character"
/// (`MsgType.h:26`; the server port's `GameMsg::RequestSelectchar = 4`,
/// `opcodes_game.rs:21`, generated from the same header).
pub const REQUEST_SELECTCHAR: u16 = 4;

/// `GAMEMSG_RESPONSE_SERVERINFO` — "informs the client of the server's basic
/// information", value 106 (`MsgType.h:66`; the server port's
/// `GameMsg::ResponseServerinfo = 106`, `opcodes_game.rs:65`).
pub const RESPONSE_SERVERINFO: u16 = 106;

/// `GAMEMSG_REQUEST_VILLAGESTRUCT` — "requests the village's building
/// information" (`MsgType.h:589`; the server port's
/// `GameMsg::RequestVillagestruct = 1101`, `opcodes_game.rs:639`).
pub const REQUEST_VILLAGESTRUCT: u16 = 1101;

/// `GAMEMSG_RESPONSE_VILLAGESTRUCT` (`MsgType.h:590`; the server port's
/// `GameMsg::ResponseVillagestruct = 1102`, `opcodes_game.rs:641`).
pub const RESPONSE_VILLAGESTRUCT: u16 = 1102;

/// `REQUEST_SELECTCHAR`'s body: a bare `SI32` slot index. The two-argument
/// `cltMsg` constructor "writes a bare `SI32` and hardcodes the size to
/// `sizeof(sPacketHeader) + sizeof(SI32)`" (`MsgType.h:2977-2984`), and the
/// server port's `decode_select_char` reads exactly `i32`
/// (`gz-proto/src/game.rs:941`, citing `NSelectChar.cpp:1353` and the handler
/// `Server-Login.cpp:184-187`).
pub const SELECT_CHAR_PAYLOAD_LEN: usize = 4;

/// `REQUEST_VILLAGESTRUCT`'s body: also a bare `SI32`, the village ref. The
/// server port's `decode_village_struct_request` reads `i32`
/// (`game.rs:1365`, citing `Server-System.cpp:2997`'s `siData32[0]`), and the
/// client builds it through the same two-argument constructor
/// (`Client-Login.cpp:281`).
pub const VILLAGE_STRUCT_REQUEST_PAYLOAD_LEN: usize = 4;

/// `sizeof(cltGameMsgResponse_ServerInfo)` under MSVC default packing: the
/// server port measures 48 (`SERVER_INFO_PAYLOAD_LEN`, `game.rs:35`) and its
/// `ServerInfo::encode` documents each field's offset to that length
/// (`game.rs:987-1015`, "Encodes the 48-byte body. `Server.cpp:6470-6487`").
pub const SERVER_INFO_PAYLOAD_LEN: usize = 48;

/// `sizeof(cltGameMsgResponse_VillageStruct)`: `SI16 VillageUnique` +
/// `SI16 siVillageStructureInfo[MAX_CONSTRUCTION_NUM]` = 2 + 10*2 = 22
/// (`MsgType.h:3085-3102`; the server port's `VILLAGE_STRUCT_PAYLOAD_LEN`,
/// `game.rs:46-48`, "SI16 siVillageStructureInfo[MAX_CONSTRUCTION_NUM] =
/// 2 + 10*2").
pub const VILLAGE_STRUCT_PAYLOAD_LEN: usize = 22;

/// `MAX_CONSTRUCTION_NUM` = 10, the building slots a village blueprint
/// carries (`CommonHeader.h:590`; the server port's `MAX_CONSTRUCTION_NUM`,
/// `game.rs:50-51`).
pub const MAX_CONSTRUCTION_NUM: usize = 10;

/// `MAX_EVENT_NUMBER` = 8, the width of SERVERINFO's `bEventSwitch`
/// (`CommonHeader.h:765`; the server port encodes `event_switch: [bool; 8]`,
/// `game.rs:1003-1005`).
pub const MAX_EVENT_NUMBER: usize = 8;

/// `EVENT_TOGGLE_SWITCH_MAX` = 2, the width of SERVERINFO's
/// `bEventToggleSwitch` (`CommonHeader.h:868`; the server port encodes
/// `event_toggle_switch: [bool; 2]`, `game.rs:1011-1013`).
pub const EVENT_TOGGLE_SWITCH_MAX: usize = 2;

/// `MAX_CHAR_PER_USER` = 3 — the select dialog refuses any
/// `CurrentCharInfoList` outside `0..MAX_CHAR_PER_USER`
/// (`NSelectChar.cpp:1292`), and the legacy server independently gates the
/// same range, `if(index < 0 || index >= 3) return;`
/// (`Server-Login.cpp:186`). This is the *selection* gate, narrower than the
/// 15-slot roster of `game::char_info::CHAR_SLOTS` (3 main + 12 house,
/// `CommonHeader.h:579-581`): the dialog only offers the 3 main slots.
pub const SELECT_CHAR_SLOT_MAX: i32 = 3;

/// `ConstServiceArea_English` = 0x08 (`CommonHeader.h:102`) — the region value
/// the SERVERINFO handler compares its `siServiceArea` against
/// (`Client-System.cpp:1682`). The server port advertises the same 0x08 from
/// its `service_area` config default (`services/game/src/config.rs:85,119`,
/// asserted `== 0x08` at `:366,396`), so an English-built client accepts its
/// banner.
pub const SERVICE_AREA_ENGLISH: i16 = 0x08;

/// `MAX_VILLAGE_NUMBER` = 100 (`CommonHeader.h:583`). The server port refuses
/// a village-struct request unless `1 <= ref < MAX_VILLAGE_NUMBER`
/// (`game.rs:53-56`, citing the legacy server's range check); this port's
/// sender applies the same bound so a ref no server would honour never
/// reaches the wire.
pub const MAX_VILLAGE_NUMBER: i32 = 100;

/// Why a world-entry operation failed.
#[derive(Debug, thiserror::Error)]
pub enum WorldEntryError {
    /// The link layer failed — most commonly [`GameError::NotHandshaken`],
    /// the send gate standing in for the legacy's
    /// `bGameServerSecretKeySwitch == false` check (`Client-Winsock.cpp:171`),
    /// then the send failure cases.
    #[error(transparent)]
    Game(#[from] GameError),
    /// An opcode-5 `RESPONSE_LOGIN` body could not be parsed — the Step 4.4
    /// [`MakeCharInfo`] parser's own strictness, surfaced unchanged.
    #[error(transparent)]
    CharInfo(#[from] CharInfoError),
    /// A SERVERINFO or VILLAGESTRUCT frame carried a foreign opcode — dispatch
    /// drift or a frame this contract does not cover.
    #[error("expected game opcode {expected}, got {cmd}")]
    UnexpectedOpcode {
        /// The opcode this parser accepts.
        expected: u16,
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The body was shorter than the struct — where the legacy would read out
    /// of bounds (`Client-System.cpp:1679`, `Client-SystemNPC.cpp:877` both
    /// cast `cData` and read `sizeof` worth with no length check).
    #[error("game opcode {opcode} body truncated: expected {expected} bytes, got {got}")]
    Truncated {
        /// Required length.
        expected: usize,
        /// Payload length actually received.
        got: usize,
        /// The opcode being parsed, for the message.
        opcode: u16,
    },
    /// The body ran past the struct — trailing bytes the legacy would silently
    /// ignore (it copies `sizeof` only).
    #[error("game opcode {opcode} body has trailing bytes: expected {expected} bytes, got {got}")]
    TrailingBytes {
        /// Required length.
        expected: usize,
        /// Payload length actually received.
        got: usize,
        /// The opcode being parsed, for the message.
        opcode: u16,
    },
    /// A `REQUEST_SELECTCHAR` slot outside `0..`[`SELECT_CHAR_SLOT_MAX`]. The
    /// dialog returns before sending (`NSelectChar.cpp:1292`) and the legacy
    /// server returns before acting (`Server-Login.cpp:186`); this port
    /// refuses before the payload is built.
    #[error("selectchar slot {slot} is outside 0..{max}")]
    SlotOutOfRange {
        /// The slot that was requested.
        slot: i32,
        /// [`SELECT_CHAR_SLOT_MAX`], restated for the message.
        max: i32,
    },
    /// A `REQUEST_VILLAGESTRUCT` ref outside `1..`[`MAX_VILLAGE_NUMBER`] —
    /// the server port's own range check (`game.rs:53-56`). Ref 0 is the
    /// legacy's "this map is not a village, ask for nothing" sentinel
    /// (`Client-Login.cpp:279`), which is why it is refused here rather than
    /// sent.
    #[error("village-struct ref {village_ref} is outside 1..{max}")]
    VillageRefOutOfRange {
        /// The ref that was requested.
        village_ref: i32,
        /// [`MAX_VILLAGE_NUMBER`], restated for the message.
        max: i32,
    },
    /// A VILLAGESTRUCT answer naming a negative village — the legacy's error
    /// branch, which raises a message box rather than applying the blueprint
    /// (`Client-SystemNPC.cpp:898-900`). Refused here instead of displayed.
    #[error("VILLAGESTRUCT answer names village {village_unique}, which is negative")]
    NegativeVillageUnique {
        /// The `VillageUnique` from the answer.
        village_unique: i16,
    },
}

/// Checks an `SI32` request body's value range before it is built.
fn check_slot(slot: i32) -> Result<(), WorldEntryError> {
    if (0..SELECT_CHAR_SLOT_MAX).contains(&slot) {
        Ok(())
    } else {
        Err(WorldEntryError::SlotOutOfRange {
            slot,
            max: SELECT_CHAR_SLOT_MAX,
        })
    }
}

/// Checks a village ref against the server's acceptance range.
fn check_village_ref(village_ref: i32) -> Result<(), WorldEntryError> {
    if (1..MAX_VILLAGE_NUMBER).contains(&village_ref) {
        Ok(())
    } else {
        Err(WorldEntryError::VillageRefOutOfRange {
            village_ref,
            max: MAX_VILLAGE_NUMBER,
        })
    }
}

/// Builds the 4-byte body of a `GAMEMSG_REQUEST_SELECTCHAR`: the slot as a
/// little-endian `SI32` (`MsgType.h:2977-2984`; the server port's
/// `decode_select_char` reads `i32`, `game.rs:941`).
///
/// # Errors
///
/// [`WorldEntryError::SlotOutOfRange`] for a slot outside
/// `0..`[`SELECT_CHAR_SLOT_MAX`] — the range both the dialog
/// (`NSelectChar.cpp:1292`) and the legacy server (`Server-Login.cpp:186`)
/// accept. Nothing is built for a refused slot.
pub fn select_char_payload(slot: i32) -> Result<[u8; SELECT_CHAR_PAYLOAD_LEN], WorldEntryError> {
    check_slot(slot)?;
    Ok(slot.to_le_bytes())
}

/// Builds the 4-byte body of a `GAMEMSG_REQUEST_VILLAGESTRUCT`: the village
/// ref as a little-endian `SI32` (`Client-Login.cpp:281`; the server port's
/// `decode_village_struct_request` reads `i32`, `game.rs:1365`).
///
/// # Errors
///
/// [`WorldEntryError::VillageRefOutOfRange`] for a ref outside
/// `1..`[`MAX_VILLAGE_NUMBER`] — including 0, which the legacy uses as the
/// "not a village, request nothing" sentinel (`Client-Login.cpp:279`) and
/// which therefore must never be sent.
pub fn village_struct_request_payload(
    village_ref: i32,
) -> Result<[u8; VILLAGE_STRUCT_REQUEST_PAYLOAD_LEN], WorldEntryError> {
    check_village_ref(village_ref)?;
    Ok(village_ref.to_le_bytes())
}

/// Sends `GAMEMSG_REQUEST_SELECTCHAR` carrying `slot` — the frame is 9 bytes
/// on the wire (`sizeof(sPacketHeader)` 5 + `sizeof(SI32)` 4,
/// `MsgType.h:2981`) and enciphered with the negotiated session cipher
/// (`GameLink::send`; the legacy's `SendMsgToServer`, `NSelectChar.cpp:1354`).
///
/// # Errors
///
/// [`WorldEntryError::SlotOutOfRange`] first, when the slot is not one the
/// dialog could have chosen (nothing is written); then
/// [`WorldEntryError::Game`] — [`GameError::NotHandshaken`] before the
/// handshake, then the [`GameLink::send`] failure cases.
pub async fn request_select_char(link: &mut GameLink, slot: i32) -> Result<(), WorldEntryError> {
    let payload = select_char_payload(slot)?;
    link.send(REQUEST_SELECTCHAR, &payload).await?;
    Ok(())
}

/// Sends `GAMEMSG_REQUEST_VILLAGESTRUCT` carrying `village_ref` — also 9 bytes
/// on the wire, session-enciphered.
///
/// `village_ref` is the current map's `siVillageUnique`
/// (`Client-Login.cpp:281`); the caller supplies it from map data (module
/// docs, "the village ref comes from map data").
///
/// # Errors
///
/// [`WorldEntryError::VillageRefOutOfRange`] first, when no server would
/// honour the ref (nothing is written); then [`WorldEntryError::Game`] as
/// [`request_select_char`].
pub async fn request_village_struct(
    link: &mut GameLink,
    village_ref: i32,
) -> Result<(), WorldEntryError> {
    let payload = village_struct_request_payload(village_ref)?;
    link.send(REQUEST_VILLAGESTRUCT, &payload).await?;
    Ok(())
}

/// Enforces the strict (cmd, payload) length contract shared by every parser
/// in this module: opcode first, then an exact-size body.
fn gate_frame(
    cmd: u16,
    expected_cmd: u16,
    expected_len: usize,
    payload: &[u8],
) -> Result<(), WorldEntryError> {
    if cmd != expected_cmd {
        return Err(WorldEntryError::UnexpectedOpcode {
            expected: expected_cmd,
            cmd,
        });
    }
    match payload.len() {
        len if len < expected_len => Err(WorldEntryError::Truncated {
            expected: expected_len,
            got: len,
            opcode: cmd,
        }),
        len if len > expected_len => Err(WorldEntryError::TrailingBytes {
            expected: expected_len,
            got: len,
            opcode: cmd,
        }),
        _ => Ok(()),
    }
}

/// `cltGameMsgResponse_ServerInfo` (`MsgType-System.h:1168-1197`) as it sits on
/// the wire — the opcode-106 body, every field at its literal offset.
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0..2 | [`service_area`](Self::service_area) | `siServiceArea` `SI16` |
/// | 2..4 | [`server_unique`](Self::server_unique) | `siServerUnique` `SI16` |
/// | 4 | [`item_mall_use`](Self::item_mall_use) | `bItemMallUseSwitch` `bool` |
/// | 5 | [`village_war`](Self::village_war) | `bVillageWarSwitch` `bool` |
/// | 6 | [`bonus_time`](Self::bonus_time) | `bBonusTimeSwitch` `bool` |
/// | 7 | [`bonus2_time`](Self::bonus2_time) | `bBonus2TimeSwitch` `bool` |
/// | 8 | [`special_quest_bonus_time`](Self::special_quest_bonus_time) | `bSpecialQuestBonusTimeSwitch` `bool` |
/// | 9 | [`broadcast`](Self::broadcast) | `bBroadcastSwitch` `bool` |
/// | 10 | [`best_village_level`](Self::best_village_level) | `siBestVillageLevel` `SI08` |
/// | 11 | (pad) | align `siNPCPriceRate` to 4 |
/// | 12..16 | [`npc_price_rate`](Self::npc_price_rate) | `siNPCPriceRate` `SI32` |
/// | 16..24 | [`item_price_index`](Self::item_price_index) | `siItemPriceIndex` `GMONEY` (`SI64`) |
/// | 24..32 | [`event_switch`](Self::event_switch) | `bEventSwitch[`[`MAX_EVENT_NUMBER]`] |
/// | 32..36 | [`minimum_teacher_level`](Self::minimum_teacher_level) | `siMinimumTeacherLevel` `SI32` |
/// | 36..38 | [`service_age`](Self::service_age) | `siServiceAge` `SI16` |
/// | 38 | [`enable_pvp`](Self::enable_pvp) | `siEnablePVP` `SI08` |
/// | 39 | [`auto_unblock_mode`](Self::auto_unblock_mode) | `bAutoUnblockMode` `bool` |
/// | 40 | [`vote_campaign`](Self::vote_campaign) | `bVoteCampaign` `bool` |
/// | 41..43 | [`event_toggle_switch`](Self::event_toggle_switch) | `bEventToggleSwitch[`[`EVENT_TOGGLE_SWITCH_MAX]`] |
/// | 43..48 | (pad) | trailing struct pad |
///
/// The offsets are the server port's own, from the `ServerInfo::encode`
/// comments (`game.rs:990-1015`), which derive them from the same header
/// (`Server.cpp:6470-6487`); the names and types are the snapshot's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// `siServiceArea` at 0..2 — the region value the handler compares
    /// against the client's own before it will store anything
    /// (`Client-System.cpp:1682`).
    pub service_area: i16,
    /// `siServerUnique` at 2..4 — the server instance id.
    pub server_unique: i16,
    /// `bItemMallUseSwitch` at 4.
    pub item_mall_use: bool,
    /// `bVillageWarSwitch` at 5.
    pub village_war: bool,
    /// `bBonusTimeSwitch` at 6.
    pub bonus_time: bool,
    /// `bBonus2TimeSwitch` at 7.
    pub bonus2_time: bool,
    /// `bSpecialQuestBonusTimeSwitch` at 8.
    pub special_quest_bonus_time: bool,
    /// `bBroadcastSwitch` at 9.
    pub broadcast: bool,
    /// `siBestVillageLevel` at 10.
    pub best_village_level: i8,
    /// `siNPCPriceRate` at 12..16 — copied into the item statistics the
    /// quest-reward path reads (`Client-System.cpp:1702`).
    pub npc_price_rate: i32,
    /// `siItemPriceIndex` at 16..24 — `GMONEY`, an `SI64`.
    pub item_price_index: i64,
    /// `bEventSwitch` at 24..32.
    pub event_switch: [bool; MAX_EVENT_NUMBER],
    /// `siMinimumTeacherLevel` at 32..36.
    pub minimum_teacher_level: i32,
    /// `siServiceAge` at 36..38 — the SERVERINFO handler's write to it is
    /// commented out in the shipped client (`Client-System.cpp:1713`); the
    /// value arrives from another message instead (`Client-Winsock.cpp:359`),
    /// so this port parses it without acting on it either.
    pub service_age: i16,
    /// `siEnablePVP` at 38.
    pub enable_pvp: i8,
    /// `bAutoUnblockMode` at 39.
    pub auto_unblock_mode: bool,
    /// `bVoteCampaign` at 40.
    pub vote_campaign: bool,
    /// `bEventToggleSwitch` at 41..43.
    pub event_toggle_switch: [bool; EVENT_TOGGLE_SWITCH_MAX],
}

/// The byte-level SERVERINFO parser, callable with raw `(cmd, payload)` for
/// byte-exact testing. Opcode first, then exactly
/// [`SERVER_INFO_PAYLOAD_LEN`] bytes.
///
/// # Errors
///
/// [`WorldEntryError::UnexpectedOpcode`] for a command other than
/// [`RESPONSE_SERVERINFO`]; [`WorldEntryError::Truncated`] /
/// [`WorldEntryError::TrailingBytes`] for any other length (the legacy casts
/// `cData` and reads `sizeof` worth, `Client-System.cpp:1679`).
pub fn parse_server_info(cmd: u16, payload: &[u8]) -> Result<ServerInfo, WorldEntryError> {
    gate_frame(cmd, RESPONSE_SERVERINFO, SERVER_INFO_PAYLOAD_LEN, payload)?;
    Ok(ServerInfo {
        service_area: i16_at(payload, 0),
        server_unique: i16_at(payload, 2),
        item_mall_use: payload[4] != 0,
        village_war: payload[5] != 0,
        bonus_time: payload[6] != 0,
        bonus2_time: payload[7] != 0,
        special_quest_bonus_time: payload[8] != 0,
        broadcast: payload[9] != 0,
        best_village_level: payload[10] as i8,
        npc_price_rate: i32_at(payload, 12),
        item_price_index: i64_at(payload, 16),
        event_switch: [
            payload[24] != 0,
            payload[25] != 0,
            payload[26] != 0,
            payload[27] != 0,
            payload[28] != 0,
            payload[29] != 0,
            payload[30] != 0,
            payload[31] != 0,
        ],
        minimum_teacher_level: i32_at(payload, 32),
        service_age: i16_at(payload, 36),
        enable_pvp: payload[38] as i8,
        auto_unblock_mode: payload[39] != 0,
        vote_campaign: payload[40] != 0,
        event_toggle_switch: [payload[41] != 0, payload[42] != 0],
    })
}

/// `cltGameMsgResponse_VillageStruct` (`MsgType.h:3085-3102`) as it sits on the
/// wire — the opcode-1102 body.
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0..2 | [`village_unique`](Self::village_unique) | `VillageUnique` `SI16` |
/// | 2..22 | [`structure_info`](Self::structure_info) | `siVillageStructureInfo[`[`MAX_CONSTRUCTION_NUM]`] `SI16` |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VillageStruct {
    /// `VillageUnique` at 0..2 — the village this blueprint belongs to. The
    /// handler branches on its sign: negative is an error box
    /// (`Client-SystemNPC.cpp:880,898-900`).
    pub village_unique: i16,
    /// `siVillageStructureInfo` at 2..22 — the building slots, wiped and
    /// refilled one per index by the handler
    /// (`Client-SystemNPC.cpp:884-889`).
    pub structure_info: [i16; MAX_CONSTRUCTION_NUM],
}

/// The byte-level VILLAGESTRUCT parser, callable with raw `(cmd, payload)`
/// for byte-exact testing. Opcode first, then exactly
/// [`VILLAGE_STRUCT_PAYLOAD_LEN`] bytes.
///
/// # Errors
///
/// [`WorldEntryError::UnexpectedOpcode`] for a command other than
/// [`RESPONSE_VILLAGESTRUCT`]; [`WorldEntryError::Truncated`] /
/// [`WorldEntryError::TrailingBytes`] for any other length.
pub fn parse_village_struct(cmd: u16, payload: &[u8]) -> Result<VillageStruct, WorldEntryError> {
    gate_frame(
        cmd,
        RESPONSE_VILLAGESTRUCT,
        VILLAGE_STRUCT_PAYLOAD_LEN,
        payload,
    )?;
    let mut structure_info = [0i16; MAX_CONSTRUCTION_NUM];
    for (index, slot) in structure_info.iter_mut().enumerate() {
        *slot = i16_at(payload, 2 + index * 2);
    }
    Ok(VillageStruct {
        village_unique: i16_at(payload, 0),
        structure_info,
    })
}

/// The local player's world-entry identity record — the port's `cltCharClient`
/// for the character this connection controls.
///
/// Every field is copied from the opcode-5 [`MakeCharInfo`] whose
/// `uiMakeCharUnique` equals the connection's `MyCharUnique`
/// (`Client-Login.cpp:116`), which is the only record the legacy turns into
/// the *local* avatar: `SetChar` builds the `cltCharClient` from it
/// (`Client-SetChar.cpp:138`), and the README Step 4 gate — "player stats
/// (Level, HP, MP, coordinates) match wire payload exactly" (`README.md:97`) — is a comparison
/// against exactly these values for every field `MakeCharInfo` carries; `MP`
/// is the one it does not, as the paragraph below explains.
///
/// **What is deliberately not here:** `MakeCharInfo` carries no mana field, so
/// neither does this struct — MP arrives on later state-sync frames (the
/// opcode-215 HP/MP push and the opcode-248 `clBI` block), which are Step 5
/// scope. Inventing an unset `mana` field to satisfy the README's wording
/// would be a stub; the struct models the record the world-entry wire
/// actually carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharClient {
    /// `uiMakeCharUnique` — the session character id, which equals
    /// `MyCharUnique` for this record (`Client-Login.cpp:116`) and indexes
    /// `CharUniqueInfo[]`.
    pub char_unique: u16,
    /// `uiMakeCharKind` — the class/avatar id.
    pub kind: u16,
    /// `uiLevel` — the level (README gate: "Level").
    pub level: u8,
    /// `uiLife` — current hit points (README gate: "HP").
    pub life: u32,
    /// `uiX` — the map tile x (README gate: "coordinates"), copied into
    /// `clPos.siX` by the legacy (`Client-Login.cpp:193`).
    pub x: u16,
    /// `uiY` — the map tile y (README gate: "coordinates").
    pub y: u16,
    /// `uiMapIndex` — the `MapData` index, copied into
    /// `cltMapManager::CurrentMapIndex` (`Client-Login.cpp:243-245`).
    pub map_index: u16,
    /// `uiTotalMoveSpeed` — the effective movement speed.
    pub move_speed: u8,
    /// `siHomeVillageUnique` — the home village unique.
    pub home_village: i8,
    /// `siGameMasterLevel` — the GM level, forwarded from the account.
    pub gm_level: u8,
    /// `siMapUnique` — 0 lets the client resolve it from its own `MapData`
    /// (`Client-Login.cpp:243`).
    pub map_unique: i16,
}

impl CharClient {
    /// Builds the local character from the opcode-5 record the service sent
    /// for this connection — the port's `SetChar(1, &clinfo)`
    /// (`Client-SetChar.cpp:138`), field for field.
    #[must_use]
    pub fn from_make_char_info(info: &MakeCharInfo) -> Self {
        Self {
            char_unique: info.make_char_unique,
            kind: info.make_char_kind,
            level: info.level,
            life: info.life,
            x: info.x,
            y: info.y,
            map_index: info.map_index,
            move_speed: info.total_move_speed,
            home_village: info.home_village_unique,
            gm_level: info.game_master_level,
            map_unique: info.map_unique,
        }
    }
}

/// The port's model of the world-entry switches (`module docs`, "the three
/// switches that *are* the in-world condition"): the SERVERINFO banner, the
/// local [`CharClient`], and the outstanding village-blueprint wait.
///
/// One instance per connection, advanced by the three `apply_*` handlers as
/// the corresponding frames arrive. [`WorldEntryState::is_in_world`] is the
/// derived acceptance predicate the README Step 4 flow ends at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEntryState {
    /// `MyCharUnique` — this connection's session character id, set by the
    /// opcode-1 handshake (`Client-SystemNPC.cpp:698`). The opcode-5 record is
    /// only adopted when its `uiMakeCharUnique` matches
    /// (`Client-Login.cpp:116`).
    my_char_unique: i32,
    /// `bReceiveServerInfoSwitch` as `Option`: `Some` once a banner matched
    /// the region gate (`Client-System.cpp:1720`), `None` before that or
    /// after a dropped banner.
    server_info: Option<ServerInfo>,
    /// `bWaitVillageStructureInfoSwitch` (`Client-Login.cpp:288`) — set when a
    /// blueprint is requested, cleared by the answer
    /// (`Client-SystemNPC.cpp:896`). Starts clear: a map that is not a village
    /// never sets it (`Client-Login.cpp:279`).
    village_struct_pending: bool,
    /// The populated `cltCharClient` for `MyCharUnique`
    /// (`Client-SetChar.cpp:138`).
    char_client: Option<CharClient>,
}

impl WorldEntryState {
    /// A fresh connection's world-entry state: `MyCharUnique` from the
    /// handshake, no banner yet, no blueprint outstanding, no character
    /// populated.
    #[must_use]
    pub const fn new(my_char_unique: i32) -> Self {
        Self {
            my_char_unique,
            server_info: None,
            village_struct_pending: false,
            char_client: None,
        }
    }

    /// Applies a `RESPONSE_SERVERINFO` frame: parses it, then runs the
    /// handler's region gate (`Client-System.cpp:1682`).
    ///
    /// `client_service_area` is the client's own `siServiceArea` — the value
    /// [`SERVICE_AREA_ENGLISH`] names for an English build.
    ///
    /// Returns `Ok(true)` when the banner was accepted and stored (the switch
    /// is now set); `Ok(false)` when the region gate dropped it — the legacy's
    /// silent `return`, reproduced as a non-event rather than an error, with
    /// the state left exactly as it was.
    ///
    /// # Errors
    ///
    /// As [`parse_server_info`].
    pub fn apply_server_info(
        &mut self,
        cmd: u16,
        payload: &[u8],
        client_service_area: i16,
    ) -> Result<bool, WorldEntryError> {
        let info = parse_server_info(cmd, payload)?;
        if info.service_area != client_service_area {
            return Ok(false);
        }
        self.server_info = Some(info);
        Ok(true)
    }

    /// Applies a `RESPONSE_LOGIN` frame: parses the opcode-5 [`MakeCharInfo`]
    /// and populates the local [`CharClient`] — but only when the record is
    /// this connection's own, i.e. `uiMakeCharUnique == MyCharUnique`
    /// (`Client-Login.cpp:116`). A record for another character returns
    /// `Ok(false)` and changes nothing (the legacy returns before `SetChar`).
    ///
    /// # Errors
    ///
    /// As the Step 4.4 [`MakeCharInfo`] parser, surfaced through
    /// [`WorldEntryError::CharInfo`].
    pub fn apply_login(&mut self, cmd: u16, payload: &[u8]) -> Result<bool, WorldEntryError> {
        let info = parse_make_char_info(cmd, payload)?;
        if i32::from(info.make_char_unique) != self.my_char_unique {
            return Ok(false);
        }
        self.char_client = Some(CharClient::from_make_char_info(&info));
        Ok(true)
    }

    /// Records that a village blueprint has been requested — the port's
    /// `bWaitVillageStructureInfoSwitch = true` (`Client-Login.cpp:288`),
    /// which is what keeps [`WorldEntryState::is_in_world`] false until the
    /// answer arrives.
    ///
    /// Call this after [`request_village_struct`] succeeds. Ref 0 (a map that
    /// is not a village) is never requested and therefore never recorded, so
    /// it cannot stall entry (`Client-Login.cpp:279`).
    pub fn mark_village_struct_requested(&mut self) {
        self.village_struct_pending = true;
    }

    /// Applies a `RESPONSE_VILLAGESTRUCT` frame: parses it, refuses a negative
    /// village (the legacy's message-box branch,
    /// `Client-SystemNPC.cpp:880,898-900`), and on success clears the outstanding
    /// wait (`:896`).
    ///
    /// # Errors
    ///
    /// As [`parse_village_struct`]; then
    /// [`WorldEntryError::NegativeVillageUnique`] for a negative `VillageUnique`.
    pub fn apply_village_struct(
        &mut self,
        cmd: u16,
        payload: &[u8],
    ) -> Result<(), WorldEntryError> {
        let village = parse_village_struct(cmd, payload)?;
        if village.village_unique < 0 {
            return Err(WorldEntryError::NegativeVillageUnique {
                village_unique: village.village_unique,
            });
        }
        self.village_struct_pending = false;
        Ok(())
    }

    /// The derived in-world condition (module docs): the banner matched, the
    /// local character is populated, and no blueprint is still outstanding.
    /// These are precisely the switches that gate avatar creation —
    /// `bReceiveServerInfoSwitch` (`Client-SetChar.cpp:110`) and
    /// `bWaitVillageStructureInfoSwitch` (`Client-SetChar.cpp:66`).
    #[must_use]
    pub const fn is_in_world(&self) -> bool {
        self.server_info.is_some() && self.char_client.is_some() && !self.village_struct_pending
    }

    /// The accepted SERVERINFO banner, `None` while the region gate has not
    /// let one through.
    #[must_use]
    pub const fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    /// The populated local character, `None` until this connection's own
    /// opcode-5 record has been applied.
    #[must_use]
    pub const fn char_client(&self) -> Option<&CharClient> {
        self.char_client.as_ref()
    }

    /// Whether a village blueprint is still outstanding.
    #[must_use]
    pub const fn village_struct_pending(&self) -> bool {
        self.village_struct_pending
    }

    /// `MyCharUnique` — the session character id the opcode-5 record must
    /// match.
    #[must_use]
    pub const fn my_char_unique(&self) -> i32 {
        self.my_char_unique
    }
}

#[cfg(test)]
mod tests;
