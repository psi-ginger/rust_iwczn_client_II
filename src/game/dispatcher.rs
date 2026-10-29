//! README Step 5, items 2-3 — the message dispatcher: the inbound router for
//! the five in-world opcodes (`README.md:104-108`) and the delta reflection
//! into the character state (`README.md:109`).
//!
//! Step 4.5-4.7 delivered the *entry* conversation and the local
//! [`CharClient`]; this module delivers what runs after entry: every inbound
//! frame whose opcode is in scope is length-gated and parsed at its literal
//! offsets, and the resulting deltas are applied to a per-connection
//! [`DispatcherState`] that models the legacy's `CR[]` avatar table for these
//! five frames. It is a pure state machine — no sockets, no timers; the
//! non-blocking pump that will feed it is Step 5.1's [`crate::client::
//! event_loop`], whose handler receives each already-decifered [`Frame`].
//!
//! # The five opcodes and where each body length comes from
//!
//! | Opcode | Legacy message | Body | Size authority |
//! |---|---|---|---|
//! | 6 | `GAMEMSG_RESPONSE_LOGINMANY` | N x (592 or 68, by `uiType`) | binary-authoritative PC stride + wire- and source-pinned NPC stride (`char_info`, Step 4.4 + LIVE-01; port `game.rs:37-43`, `evidence/live/decode2.log`) |
//! | 11 | `GAMEMSG_RESPONSE_LOGOUT` | one bare `SI32` | the two-argument `cltMsg` constructor (`MsgType.h:2977-2983`) |
//! | 118 | `GAMEMSG_RESPONSE_CHAT` | 20 fixed bytes + the transmitted name/text region | the constructor's own `siDataSize` arithmetic (`MsgType-System.h:1139-1158`) |
//! | 200 | `GAMEMSG_RESPONSE_CHARUPDATE` | 24 | `sizeof(cltBasicCharUpdateInfo)` at the default `/Zp8` (`CommonLogic.h:383-410`, framed at `Server.cpp:1688`) — source-derived, see the caveat below |
//! | 215 | `GAMEMSG_RESPONSE_PERSON_IP` | 72 | binary-authoritative (`game.rs:426` `PERSON_IP_LEN = 8 + 64`, measured frame 77 = 5 + 72, `game.rs:1500`) |
//!
//! The opcode values are the legacy enum's (`MsgType.h:28` LOGINMANY = 6,
//! `:36` LOGOUT = 11, `:87` CHAT = 118, `:220` CHARUPDATE = 200, `:251-252`
//! PERSON_IP 214/215) and the port's own table (`opcodes_game.rs:25/35/89/
//! 221/251`). The router rows that send the client to these handlers are
//! `Client-Message.cpp:248` (215), `:309` (6), `:310` (11), `:330` (200) and
//! `:807` (118).
//!
//! # What each parser enforces, and on whose authority
//!
//! **Opcode 6 — nearby characters.** Delegated unchanged to
//! [`super::char_info::parse_make_char_info_many`] (Step 4.4, QC'd; LIVE-01
//! widened it): records are packed back to back with no length prefix and
//! each stride comes from the leading `uiType` bit
//! (`Client-Login.cpp:62-75`). A `MAKECHARTYPE_PC` record is 592 bytes
//! (binary-authoritative); any other type is its 68-byte
//! `cltMakeCharInfoCommon` head — dual-sourced at
//! [`super::char_info::MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN`] from the live
//! wire (`evidence/live/decode2.log`, 34 x 68) and the MSVC field sum
//! (`CommonHeader.h:1967-1995`). This dispatcher upserts only the PC
//! records; NPC records are counted into
//! [`DispatchOutcome::Entered::npcs`] and skipped (see
//! `apply_login_many` below).
//!
//! **Opcode 11 — logout.** The only in-scope packet that is not an explicit
//! struct: the two-argument `cltMsg` constructor sets `usSize` to header +
//! `sizeof(SI32)` and parks the id in `siData32[0]` (`MsgType.h:2977-2983`,
//! quoted verbatim in the port's `game.rs` `ResponseLogout` docs). The body
//! is therefore *exactly* 4 bytes. The legacy handler resolves the id through
//! `CharUniqueInfo[]`, returns early when it is the local slot (`quitid == 1`,
//! `Client-Login.cpp:402-405`) and deletes otherwise (`:408-411`).
//!
//! **Opcode 118 — chat.** `cltGameMsgResponse_Chat` is a 276-byte struct, but
//! the server frames only `clChat.siDataSize` bytes of it
//! (`Server-System.cpp:594` and every other chat send site), and the
//! constructor computes that size as 20 fixed bytes plus the *used* part of
//! the text buffer: `siDataSize += _tcslen(szChatText) + 1`
//! (`MsgType-System.h:1158`). The 20 are the four `SI16`s, `sizeof(BOOL)` and
//! the two `SI32`s (`:1139`); the text starts at 20 because `bGameMaster`
//! (`bool`, byte 8) is followed by three bytes of struct padding before the
//! 4-aligned `siSystemTxt` at 12. Name length is capped by the constructor
//! (`memcpy` of `MAX_PLAYER_NAME` = 20 with a NUL at `[19]`, `:1150-1151`)
//! and re-checked by the handler (`PlayerNameLength >= MAX_PLAYER_NAME`
//! returns, `Client-System.cpp:1473`), and the text copy is 100 bytes
//! (`MsgType-System.h:1155`), so the declared size can never legitimately
//! fall outside 21..=140. The parser refuses anything the constructor could
//! not have produced, then splits name and text exactly where the handler
//! does (`Client-System.cpp:1484-1488`).
//!
//! **Opcode 200 — state sync.** 24 bytes = the eleven declared fields
//! (`CommonLogic.h:385-394`: `SI16`, `SI08`, `bool`, three `SI32`, two
//! `SI16`, `UI08`, `bool`, `SI08` = 23) rounded to the type's 4-byte
//! alignment — the `sizeof` the legacy server frames (`Server.cpp:1688`) and
//! every field the handler reads (`Client-Person-BigInfo.cpp:1146-1156`).
//! **Caveat (stated, not buried):** unlike opcodes 6 and 215 this 24 has no
//! binary measurement behind it — it is `/Zp8` arithmetic on this snapshot's
//! declaration. If the shipped binary's struct were one byte different the
//! strict length gate fails loudly on the first frame instead of silently
//! mis-parsing, which is the strongest statement honest evidence supports.
//!
//! **Opcode 215 — HP/MP.** 72 = `SI16 siCharUnique` padded to the
//! 8-byte-aligned `cltPIPInfo` at 8, plus the 64-byte block itself: seven
//! ability `SI32`s, level/bonus `SI16`s, life/mana `SI32`s, `SI64` exp, then
//! the four fields `cltPIPInfo` adds (`parametabox.h:243-249`) — all at the
//! offsets the port's single `write_pip` encoder uses for both this frame
//! and the 248 block (`game.rs`, "one writer serves both"), under the
//! `cltPIPInfoCommon` prefix — the seven ability `SI32`s are inherited from
//! `cltBasicAbility` (`CommonHeader.h:2746-2752`) and level/bonus/life/mana/
//! exp are `parametabox.h:107-111` (the class starts at `:95`). The handler
//! replaces the target's whole parameta box (`clIP.Set`,
//! `Client-Person-BigInfo.cpp:243`).
//!
//! # How deltas are reflected (item 3)
//!
//! The legacy keeps one avatar object per character (`CR[id]`, self in
//! slot 1) and these five frames write into it. This port models that as a
//! [`DispatcherState`]: the local [`CharClient`] from Step 4.5-4.7 plus a
//! [`LocalDelta`] overlay for the fields the opcode-5 record does not carry
//! — `world_entry.rs` promised then that "MP arrives on later state-sync
//! frames … which are Step 5 scope"; this module is that arrival — and a
//! `char_unique`-keyed table of [`WorldChar`]s for everybody else, born from
//! opcode 6 exactly as the legacy's `PushChar`/`SetChar` chain is the only
//! thing that makes another player exist (`Client-Login.cpp:79`,
//! `Client-SetChar.cpp:138`).
//!
//! Each write mirrors its legacy writer, one for one:
//!
//! - 200 writes life (`SetLife`, `Client-Person-BigInfo.cpp:1228`), mana
//!   (`SetMana`, `:1238`), position (`MoveTo`, `:1218` — as the authoritative
//!   server coordinates; the `OrderMove`/`MoveInit` pathing around it drives
//!   animation this headless client does not model), `gm_mode`, `pc_room` and
//!   `make_ani_type` (`:1255-1258`), and ratchets reserved life — it only
//!   rises (`:1230-1236`). `move_speed` is written **only for other
//!   characters** (`:1241-1244`, `if(GetCharUnique() != MyCharUnique)`), and
//!   this port reproduces that exclusion rather than "fixing" it.
//! - 215 replaces the target's whole parameta box (`clIP.Set`,
//!   `Client-Person-BigInfo.cpp:243`) and in this port touches nothing
//!   else. The legacy aliasing is stated, not hidden: opcode 200's
//!   `SetLife`/`SetMana` store into that same `clIP` box, field by field
//!   (`Char-Common.cpp:1333` -> `parametabox.h:179`, an unclamped store;
//!   `Char-Common.cpp:1295-1298` -> `parametabox.h:194-198`, which clamps a
//!   negative mana to 0 and is mirrored here), so there the two frames'
//!   life/mana share one box and last writer wins. This port never lets a
//!   frame write a field its legacy writer did not write; reconciling the
//!   shared box for rendering is left to the consumer rather than invented.
//! - 11 removes a table entry, never the local avatar (the legacy's
//!   `quitid == 1` early return, `Client-Login.cpp:402-405`); an unknown id
//!   is a no-op (`IsValidID` false, `:408-411`). The legacy also clears its
//!   reserved-char queue (`DeleteReservedChar`, `:416`); this port has no
//!   queue to clear because opcode 6 upserts directly, which is stated here
//!   rather than left as a silent gap.
//! - 118 mutates nothing — the legacy pushes into chat dialogs and the
//!   overhead chat painter only (`Client-System.cpp:1468-1597`) — so the
//!   parsed [`ChatMessage`] *is* the outcome.
//!
//! Unknown targets for 200/215 are dropped exactly as the legacy drops them
//! (`charid <= 0` return, `Client-Person-BigInfo.cpp:1169`; `IsValidID`
//! false, `Client-Person-BigInfo.cpp:241`): reported as
//! [`ApplyTarget::Unmatched`], never an error, never a partial write.
//!
//! # Test posture (F-01/F-02/F-07)
//!
//! Everything here is pure, so the suite is pure: every payload is
//! hand-placed from the field tables above and asserted against values held
//! apart from both builder and parser. Length gates are proven in both
//! directions and proven to fire **before** any field is read and before any
//! state is touched — refused frames leave the state bit-identical.
use super::char_info::{
    parse_make_char_info_many, CharInfoError, MakeCharInfo, RESPONSE_LOGINMANY,
};
use super::world_entry::CharClient;
use crate::protocol::bytes::{cstr_lossy as cstring_lossy, i16_at, i32_at, i64_at};
use std::collections::BTreeMap;

/// `GAMEMSG_RESPONSE_LOGOUT` (`MsgType.h:36`, `opcodes_game.rs:35`).
pub const RESPONSE_LOGOUT: u16 = 11;
/// `GAMEMSG_RESPONSE_CHAT` (`MsgType.h:87`, `opcodes_game.rs:89`).
pub const RESPONSE_CHAT: u16 = 118;
/// `GAMEMSG_RESPONSE_CHARUPDATE` (`MsgType.h:220`, `opcodes_game.rs:221`).
pub const RESPONSE_CHARUPDATE: u16 = 200;
/// `GAMEMSG_RESPONSE_PERSON_IP` (`MsgType.h:252`, `opcodes_game.rs:251`).
pub const RESPONSE_PERSON_IP: u16 = 215;

/// `sizeof(SI32)` — the bare-`SI32` body of the two-argument `cltMsg`
/// (`MsgType.h:2977-2983`).
pub const LOGOUT_PAYLOAD_LEN: usize = 4;

/// The fixed prefix of a chat body: four `SI16`s + `sizeof(BOOL)` + two
/// `SI32`s (`MsgType-System.h:1139`), which is where `szChatText` starts.
pub const CHAT_FIXED_LEN: usize = 20;

/// The smallest constructor-produced chat body: the 20 fixed bytes plus the
/// one NUL `_tcslen` counts (`MsgType-System.h:1158`).
pub const CHAT_PAYLOAD_MIN: usize = CHAT_FIXED_LEN + 1;

/// The largest constructor-produced chat body: 20 fixed + the name
/// (`MAX_PLAYER_NAME` 20, NUL at `[19]`, `MsgType-System.h:1148-1150`) + the
/// 100-byte text copy (`:1155`) + the terminating NUL.
pub const CHAT_PAYLOAD_MAX: usize = CHAT_FIXED_LEN + (super::login::PLAYER_NAME_LEN - 1) + 100 + 1;

/// `sizeof(cltBasicCharUpdateInfo)` at the default `/Zp8` — see the module
/// docs' caveat: source-derived, not binary-measured.
pub const CHAR_UPDATE_PAYLOAD_LEN: usize = 24;

/// `sizeof(cltGameMsgResponse_IP)` — binary-authoritative
/// (`game.rs:426`: `8 + 64`; frame 77 = 5 + 72 at `game.rs:1500`).
pub const PERSON_IP_PAYLOAD_LEN: usize = 72;

/// Why a frame in scope was refused.
///
/// Every variant is a *refusal*, never a partial parse: the legacy reads
/// these bodies through a struct cast with no length check at all
/// (`Client-System.cpp:1471`, `Client-Person-BigInfo.cpp:1141,238`,
/// `Client-Login.cpp:62,398`), so a short frame there is an out-of-bounds
/// read; here it is an error that leaves state untouched.
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    /// A parser was called with a command it does not serve. Opcode gates
    /// run before any length gate, matching the world-entry convention.
    #[error("expected game opcode {expected}, got {cmd}")]
    UnexpectedOpcode {
        /// The opcode this parser accepts.
        expected: u16,
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The body was shorter than the contract: shorter than the fixed prefix,
    /// shorter than the declared `siDataSize`, or (opcode 6) shorter than one
    /// whole record.
    #[error("game opcode {opcode} body truncated: expected {expected} bytes, got {got}")]
    Truncated {
        /// The opcode whose gate fired.
        opcode: u16,
        /// The length the gate requires.
        expected: usize,
        /// The length that arrived.
        got: usize,
    },
    /// The body was longer than the contract: longer than the declared
    /// `siDataSize`, or (opcode 6) not a whole number of records.
    #[error("game opcode {opcode} body has {got} bytes, expected {expected}")]
    TrailingBytes {
        /// The opcode whose gate fired.
        opcode: u16,
        /// The length the gate requires.
        expected: usize,
        /// The length that arrived.
        got: usize,
    },
    /// A chat body declared a `siDataSize` the constructor cannot produce
    /// (`MsgType-System.h:1139-1158`): outside [`CHAT_PAYLOAD_MIN`]..=
    /// [`CHAT_PAYLOAD_MAX`], hence not the length the server framed.
    #[error("chat siDataSize {declared} outside the constructor range {min}..={max}")]
    ChatDeclaredSize {
        /// The declared size at 0..2.
        declared: i16,
        /// [`CHAT_PAYLOAD_MIN`].
        min: usize,
        /// [`CHAT_PAYLOAD_MAX`].
        max: usize,
    },
    /// A chat `PlayerNameLength` the handler refuses (`>= MAX_PLAYER_NAME`,
    /// `Client-System.cpp:1473`) or that cannot fit inside the transmitted
    /// name/text region (the name is a prefix of it, `Client-System.cpp:1484`).
    #[error(
        "chat PlayerNameLength {player_name_length} cannot fit the {region}-byte name/text region"
    )]
    ChatNameLength {
        /// The declared `PlayerNameLength` at 4..6.
        player_name_length: i16,
        /// `siDataSize - CHAT_FIXED_LEN`: the bytes actually transmitted for
        /// name + text + NUL.
        region: usize,
    },
    /// The opcode-6 walk failed — the Step 4.4 parser's strictness, surfaced
    /// unchanged (non-player record, partial record, empty body).
    #[error(transparent)]
    CharInfo(#[from] CharInfoError),
}

/// One parsed `cltGameMsgResponse_Chat` (`MsgType-System.h:1123-1160`).
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0..2 | [`declared_size`](Self::declared_size) | `siDataSize` `SI16` |
/// | 2..4 | [`char_unique`](Self::char_unique) | `siCharUnique` `SI16` |
/// | 4..6 | `PlayerNameLength` `SI16` | drives the name/text split, not kept |
/// | 6..8 | [`mode`](Self::mode) | `siMode` `SI16` |
/// | 8..9 | [`is_game_master`](Self::is_game_master) | `bGameMaster` `bool` |
/// | 9..12 | — | struct padding before the 4-aligned `siSystemTxt` |
/// | 12..16 | [`system_txt`](Self::system_txt) | `siSystemTxt` `SI32` |
/// | 16..20 | [`level`](Self::level) | `siLevel` `SI32` |
/// | 20.. | [`player_name`](Self::player_name) + [`text`](Self::text) | `szChatText` prefix, split like `Client-System.cpp:1484-1488` |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// The `siDataSize` the server framed with — the body's exact length.
    pub declared_size: i16,
    /// `siCharUnique` — the speaker.
    pub char_unique: i16,
    /// `siMode` — raw, as the handler matches it against the `CHATMODE_*`
    /// table (`CHATMODE_NORMAL` 1 .. `CHATMODE_GUILD` 13,
    /// `MsgType-System.h:1085-1097`); the mode branches themselves are UI.
    pub mode: i16,
    /// `bGameMaster` — the GM-flag the private-chat dialog consumes
    /// (`Client-System.cpp:1498`).
    pub is_game_master: bool,
    /// `siSystemTxt` — the system-message text id.
    pub system_txt: i32,
    /// `siLevel` — the speaker's level, forwarded to the chat dialogs
    /// (`Client-System.cpp:1498`).
    pub level: i32,
    /// The first `PlayerNameLength` bytes of `szChatText`, NUL-terminated by
    /// the constructor (`MsgType-System.h:1150-1151`) — the speaker name the
    /// handler extracts (`Client-System.cpp:1484-1485`).
    pub player_name: String,
    /// The rest of the transmitted region up to its NUL — `buffer` in the
    /// handler (`Client-System.cpp:1488`).
    pub text: String,
}

/// One parsed `cltBasicCharUpdateInfo` (`CommonLogic.h:383-410`).
///
/// | Offset | Field | Legacy declaration |
/// |---|---|---|
/// | 0..2 | [`char_unique`](Self::char_unique) | `siCharUnique` `SI16` |
/// | 2..3 | [`move_speed`](Self::move_speed) | `siMoveSpeed` `SI08` |
/// | 3..4 | [`urgent_mode`](Self::urgent_mode) | `bUrgentMode` `bool` |
/// | 4..8 | [`life`](Self::life) | `siLife` `SI32` |
/// | 8..12 | [`reserved_life`](Self::reserved_life) | `siReservedLife` `SI32` |
/// | 12..16 | [`mana`](Self::mana) | `siMana` `SI32` |
/// | 16..18 | [`x`](Self::x) | `siX` `SI16` |
/// | 18..20 | [`y`](Self::y) | `siY` `SI16` |
/// | 20..21 | [`gm_mode`](Self::gm_mode) | `uiGMMode` `UI08` |
/// | 21..22 | [`pc_room`](Self::pc_room) | `bPCRoom` `bool` |
/// | 22..23 | [`make_ani_type`](Self::make_ani_type) | `siMakeAniType` `SI08` |
/// | 23..24 | — | the alignment pad inside the 24-byte `sizeof` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharUpdate {
    /// `siCharUnique` — which character this delta is about.
    pub char_unique: i16,
    /// `siMoveSpeed` — applied to other characters only
    /// (`Client-Person-BigInfo.cpp:1241-1244`).
    pub move_speed: i8,
    /// `bUrgentMode` — the legacy's snap-the-avatar flag
    /// (`Client-Person-BigInfo.cpp:1179-1181`).
    pub urgent_mode: bool,
    /// `siLife` — current life, written by `SetLife`
    /// (`Client-Person-BigInfo.cpp:1228`).
    pub life: i32,
    /// `siReservedLife` — ratchets upward only
    /// (`Client-Person-BigInfo.cpp:1230-1236`).
    pub reserved_life: i32,
    /// `siMana` — current mana, written by `SetMana`
    /// (`Client-Person-BigInfo.cpp:1238`), whose store chain clamps a
    /// negative to 0 (`Char-Common.cpp:1295-1298` -> `parametabox.h:194-198`)
    /// — mirrored when applied.
    pub mana: i32,
    /// `siX` — the authoritative server x
    /// (`Client-Person-BigInfo.cpp:1218`).
    pub x: i16,
    /// `siY` — the authoritative server y.
    pub y: i16,
    /// `uiGMMode` (`Client-Person-BigInfo.cpp:1255`).
    pub gm_mode: u8,
    /// `bPCRoom` (`Client-Person-BigInfo.cpp:1256`).
    pub pc_room: bool,
    /// `siMakeAniType` (`Client-Person-BigInfo.cpp:1257`).
    pub make_ani_type: i8,
}

/// The 64-byte `cltPIPInfo` block — the "ImportantParameta" box
/// (`parametabox.h:243-249` on the `cltPIPInfoCommon` prefix: the seven
/// ability `SI32`s are inherited from `cltBasicAbility`
/// (`CommonHeader.h:2746-2752`) and level/bonus/life/mana/exp are at
/// `parametabox.h:107-111`).
///
/// Offsets are relative to the block; the opcode-215 body carries it at 8.
/// This is the same block the port's `write_pip` writes into both this frame
/// and the 248 char-info block (`game.rs`, "one writer serves both"), so the
/// two sides' stat layout cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipInfo {
    /// `siStr` at +0.
    pub str_: i32,
    /// `siDex` at +4.
    pub dex: i32,
    /// `siVit` at +8.
    pub vit: i32,
    /// `siMag` at +12.
    pub mag: i32,
    /// `siHnd` at +16.
    pub hnd: i32,
    /// `siLuk` at +20.
    pub luk: i32,
    /// `siWis` at +24.
    pub wis: i32,
    /// `siLevel` at +28 (`parametabox.h:107`).
    pub level: i16,
    /// `siBonus` at +30.
    pub bonus: i16,
    /// `Life` at +32 — the HP the README names (`README.md:108`).
    pub life: i32,
    /// `Mana` at +36 — the MP the README names.
    pub mana: i32,
    /// `siExp` at +40.
    pub exp: i64,
    /// `siFameLevel` at +48 (`parametabox.h:246`).
    pub fame_level: i16,
    /// `siSkillBonus` at +50 (`parametabox.h:247`).
    pub skill_bonus: i16,
    /// `siFameExp` at +52 (`parametabox.h:248`).
    pub fame_exp: i32,
    /// `siMoney` at +56 (`parametabox.h:249`).
    pub money: i64,
}

/// One parsed `cltGameMsgResponse_IP` (`MsgType-Person.h:16-27`).
///
/// | Offset | Field |
/// |---|---|
/// | 0..2 | [`char_unique`](Self::char_unique) `SI16` |
/// | 2..8 | — the `/Zp8` pad before the 8-byte-aligned block |
/// | 8..72 | [`pip`](Self::pip) `cltPIPInfo` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersonIp {
    /// `siCharUnique` — the character whose parameta box this is.
    pub char_unique: i16,
    /// The full `cltPIPInfo` block, exactly as the legacy `clIP.Set` copies
    /// it (`Client-Person-BigInfo.cpp:243`).
    pub pip: PipInfo,
}

/// One parsed `GAMEMSG_RESPONSE_LOGOUT`: the bare `SI32` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Logout {
    /// `siData32[0]` — the leaving character's unique
    /// (`Client-Login.cpp:398`).
    pub char_unique: i32,
}

/// Gate: opcode, then the exact (or declared) length — never a field read
/// first. Mirrors `world_entry::gate_frame`.
fn gate_frame(
    cmd: u16,
    expected_cmd: u16,
    expected_len: usize,
    payload: &[u8],
) -> Result<(), DispatchError> {
    if cmd != expected_cmd {
        return Err(DispatchError::UnexpectedOpcode {
            expected: expected_cmd,
            cmd,
        });
    }
    if payload.len() < expected_len {
        return Err(DispatchError::Truncated {
            opcode: expected_cmd,
            expected: expected_len,
            got: payload.len(),
        });
    }
    if payload.len() > expected_len {
        return Err(DispatchError::TrailingBytes {
            opcode: expected_cmd,
            expected: expected_len,
            got: payload.len(),
        });
    }
    Ok(())
}

/// Parses an opcode-118 body: opcode first, then the declared-size contract
/// (the server frames exactly `siDataSize` bytes, `Server-System.cpp:594`),
/// then the handler's name guard, then the fields.
///
/// # Errors
///
/// [`DispatchError::UnexpectedOpcode`], [`DispatchError::Truncated`] /
/// [`DispatchError::TrailingBytes`] for any length that disagrees with the
/// declared `siDataSize` (or a body shorter than [`CHAT_PAYLOAD_MIN`], which
/// cannot even hold it), [`DispatchError::ChatDeclaredSize`] for a
/// declaration the constructor cannot produce, [`DispatchError::ChatNameLength`]
/// for a name the handler refuses or that cannot fit the transmitted region.
pub fn parse_chat(cmd: u16, payload: &[u8]) -> Result<ChatMessage, DispatchError> {
    if cmd != RESPONSE_CHAT {
        return Err(DispatchError::UnexpectedOpcode {
            expected: RESPONSE_CHAT,
            cmd,
        });
    }
    if payload.len() < CHAT_PAYLOAD_MIN {
        return Err(DispatchError::Truncated {
            opcode: RESPONSE_CHAT,
            expected: CHAT_PAYLOAD_MIN,
            got: payload.len(),
        });
    }
    let declared_size = i16_at(payload, 0);
    let declared = usize::try_from(declared_size).map_err(|_| DispatchError::ChatDeclaredSize {
        declared: declared_size,
        min: CHAT_PAYLOAD_MIN,
        max: CHAT_PAYLOAD_MAX,
    })?;
    if !(CHAT_PAYLOAD_MIN..=CHAT_PAYLOAD_MAX).contains(&declared) {
        return Err(DispatchError::ChatDeclaredSize {
            declared: declared_size,
            min: CHAT_PAYLOAD_MIN,
            max: CHAT_PAYLOAD_MAX,
        });
    }
    if payload.len() < declared {
        return Err(DispatchError::Truncated {
            opcode: RESPONSE_CHAT,
            expected: declared,
            got: payload.len(),
        });
    }
    if payload.len() > declared {
        return Err(DispatchError::TrailingBytes {
            opcode: RESPONSE_CHAT,
            expected: declared,
            got: payload.len(),
        });
    }

    // `region` = the transmitted name + text + NUL bytes.
    let region = declared - CHAT_FIXED_LEN;
    let player_name_length = i16_at(payload, 4);
    let name_len = usize::try_from(player_name_length).ok().filter(|len| {
        // The handler's own guard: `PlayerNameLength >= MAX_PLAYER_NAME`
        // returns (`Client-System.cpp:1473`); and the name is a prefix of the
        // transmitted region, so at least the terminating NUL must follow it.
        *len < super::login::PLAYER_NAME_LEN && *len < region
    });
    let Some(name_len) = name_len else {
        return Err(DispatchError::ChatNameLength {
            player_name_length,
            region,
        });
    };

    Ok(ChatMessage {
        declared_size,
        char_unique: i16_at(payload, 2),
        mode: i16_at(payload, 6),
        is_game_master: payload[8] != 0,
        system_txt: i32_at(payload, 12),
        level: i32_at(payload, 16),
        player_name: cstring_lossy(&payload[CHAT_FIXED_LEN..CHAT_FIXED_LEN + name_len]),
        text: cstring_lossy(&payload[CHAT_FIXED_LEN + name_len..declared]),
    })
}

/// Parses an opcode-200 body: opcode, then exactly
/// [`CHAR_UPDATE_PAYLOAD_LEN`] bytes, then the eleven fields at the offsets
/// of the module docs' table.
///
/// # Errors
///
/// [`DispatchError::UnexpectedOpcode`] / [`DispatchError::Truncated`] /
/// [`DispatchError::TrailingBytes`].
pub fn parse_char_update(cmd: u16, payload: &[u8]) -> Result<CharUpdate, DispatchError> {
    gate_frame(cmd, RESPONSE_CHARUPDATE, CHAR_UPDATE_PAYLOAD_LEN, payload)?;
    Ok(CharUpdate {
        char_unique: i16_at(payload, 0),
        move_speed: payload[2] as i8,
        urgent_mode: payload[3] != 0,
        life: i32_at(payload, 4),
        reserved_life: i32_at(payload, 8),
        mana: i32_at(payload, 12),
        x: i16_at(payload, 16),
        y: i16_at(payload, 18),
        gm_mode: payload[20],
        pc_room: payload[21] != 0,
        make_ani_type: payload[22] as i8,
    })
}

/// Parses an opcode-215 body: opcode, then exactly
/// [`PERSON_IP_PAYLOAD_LEN`] bytes, `siCharUnique` at 0, the 64-byte
/// `cltPIPInfo` block at 8.
///
/// # Errors
///
/// [`DispatchError::UnexpectedOpcode`] / [`DispatchError::Truncated`] /
/// [`DispatchError::TrailingBytes`].
pub fn parse_person_ip(cmd: u16, payload: &[u8]) -> Result<PersonIp, DispatchError> {
    gate_frame(cmd, RESPONSE_PERSON_IP, PERSON_IP_PAYLOAD_LEN, payload)?;
    let b = 8; // the block's base inside the body
    Ok(PersonIp {
        char_unique: i16_at(payload, 0),
        pip: PipInfo {
            str_: i32_at(payload, b),
            dex: i32_at(payload, b + 4),
            vit: i32_at(payload, b + 8),
            mag: i32_at(payload, b + 12),
            hnd: i32_at(payload, b + 16),
            luk: i32_at(payload, b + 20),
            wis: i32_at(payload, b + 24),
            level: i16_at(payload, b + 28),
            bonus: i16_at(payload, b + 30),
            life: i32_at(payload, b + 32),
            mana: i32_at(payload, b + 36),
            exp: i64_at(payload, b + 40),
            fame_level: i16_at(payload, b + 48),
            skill_bonus: i16_at(payload, b + 50),
            fame_exp: i32_at(payload, b + 52),
            money: i64_at(payload, b + 56),
        },
    })
}

/// Parses an opcode-11 body: opcode, then exactly [`LOGOUT_PAYLOAD_LEN`] —
/// the bare `SI32` of the two-argument `cltMsg` (`MsgType.h:2977-2983`).
///
/// # Errors
///
/// [`DispatchError::UnexpectedOpcode`] / [`DispatchError::Truncated`] /
/// [`DispatchError::TrailingBytes`].
pub fn parse_logout(cmd: u16, payload: &[u8]) -> Result<Logout, DispatchError> {
    gate_frame(cmd, RESPONSE_LOGOUT, LOGOUT_PAYLOAD_LEN, payload)?;
    Ok(Logout {
        char_unique: i32_at(payload, 0),
    })
}

/// Where a 200/215 delta landed — the legacy's target resolution, reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyTarget {
    /// The local avatar: `charunique == MyCharUnique`
    /// (`Client-Person-BigInfo.cpp:1160-1163` routes it to `CR[1]` for 200;
    /// 215 resolves through the char-unique table, `:240-241`, and the local
    /// unique maps to the `CR[1]` slot the legacy gives it at
    /// `Client-SetChar.cpp:129-131`).
    Local,
    /// A character in the table — the legacy's `CharUniqueInfo[charunique]`
    /// resolving to a live `CR[id]`.
    Nearby,
    /// The legacy drops it silently: `charid <= 0` return
    /// (`Client-Person-BigInfo.cpp:1169`) for 200, `IsValidID` false
    /// (`:241`) for 215. State is untouched.
    Unmatched,
}

/// What one dispatched frame did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The opcode is not one of the five in scope. Not an error: the legacy
    /// `ClientMessageMap` routes hundreds of opcodes
    /// (`Client-Message.cpp`); this dispatcher owns five of them and claims
    /// nothing about the rest.
    Unhandled {
        /// The opcode that arrived.
        cmd: u16,
    },
    /// A parsed chat message — UI-bound, no state change
    /// (`Client-System.cpp:1468-1597`).
    Chat(ChatMessage),
    /// An opcode-6 body upserted these characters, in body order. NPC
    /// records ([`super::char_info::MAKECHARTYPE_NPC`], the 68-byte
    /// common-head form) are not
    /// upserted; they are counted here instead (LIVE-01).
    Entered {
        /// The `char_unique` of each upserted (player) record, body order.
        char_uniques: Vec<i32>,
        /// How many NPC records the body carried and the walk skipped.
        npcs: usize,
    },
    /// An opcode-11 body. `applied` is false for the local avatar (the
    /// legacy's `quitid == 1` early return, `Client-Login.cpp:402-405`) and
    /// for an unknown id (`IsValidID` false, `:408-411`).
    Removed {
        /// The unique the body named.
        char_unique: i32,
        /// Whether a table entry was actually deleted.
        applied: bool,
    },
    /// A 200/215 delta was applied (or dropped, [`ApplyTarget::Unmatched`]).
    Applied {
        /// The unique the body named.
        char_unique: i32,
        /// Where it landed.
        target: ApplyTarget,
    },
}

/// One nearby (or local-via-215) character as the five frames build it: the
/// opcode-6 birth record's fields plus every delta 200/215 can write.
///
/// `None` on a delta field means *no frame has carried it yet* — the port
/// never invents a starting value. The legacy's fields all live in zeroed
/// objects, which is a C++ allocation artifact, not wire evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldChar {
    /// `uiMakeCharUnique` (`CommonHeader.h:1971`), widened to `i32` for the
    /// key space shared with the other frames' `SI16`/`SI32` uniques.
    pub char_unique: i32,
    /// `uiMakeCharKind` — the class/avatar id (opcode 6).
    pub kind: u16,
    /// `uiLevel` — the birth level (opcode 6). Opcode 215 carries the parameta
    /// box's own `siLevel` inside [`WorldChar::pip`]; the two are not
    /// cross-written, matching the legacy's separate boxes.
    pub level: u8,
    /// `uiLife` (opcode 6) / `siLife` via `SetLife` (opcode 200).
    pub life: i32,
    /// `siReservedLife` — ratchets upward only (`Client-Person-BigInfo.cpp:
    /// 1230-1236`).
    pub reserved_life: Option<i32>,
    /// `siMana` via `SetMana` (opcode 200, clamped to >= 0 as the legacy
    /// store chain does, `Char-Common.cpp:1295-1298` ->
    /// `parametabox.h:194-198`).
    pub mana: Option<i32>,
    /// `uiX` (opcode 6) / `siX` via `MoveTo` (opcode 200).
    pub x: i16,
    /// `uiY` (opcode 6) / `siY` via `MoveTo` (opcode 200).
    pub y: i16,
    /// `uiMapIndex` (opcode 6).
    pub map_index: u16,
    /// `uiTotalMoveSpeed` (opcode 6) / `siMoveSpeed` (opcode 200, others
    /// only).
    pub move_speed: i8,
    /// `uiGMMode` (opcode 200).
    pub gm_mode: u8,
    /// `bPCRoom` (opcode 200).
    pub pc_room: bool,
    /// `siMakeAniType` (opcode 200).
    pub make_ani_type: i8,
    /// The parameta box opcode 215 replaced wholesale (`clIP.Set`,
    /// `Client-Person-BigInfo.cpp:243`).
    pub pip: Option<PipInfo>,
}

impl WorldChar {
    /// The birth record's fields, as the legacy's `SetChar` is the sole
    /// creator of an avatar (`Client-SetChar.cpp:138`). The record's fields
    /// are width-bounded bitfields (`uiLife` 20 bits, `uiX`/`uiY` 10,
    /// `uiTotalMoveSpeed` 6 — `CommonHeader.h:1969-1977`), so these widening
    /// casts are value-preserving.
    fn from_record(record: &MakeCharInfo) -> Self {
        Self {
            char_unique: i32::from(record.make_char_unique),
            kind: record.make_char_kind,
            level: record.level,
            life: record.life as i32,
            reserved_life: None,
            mana: None,
            x: record.x as i16,
            y: record.y as i16,
            map_index: record.map_index,
            move_speed: record.total_move_speed as i8,
            gm_mode: 0,
            pc_room: false,
            make_ani_type: 0,
            pip: None,
        }
    }

    /// The six birth-carried fields, refreshed from a re-received record
    /// (same width-bounded casts as [`Self::from_record`]).
    fn refresh_from_record(&mut self, record: &MakeCharInfo) {
        self.kind = record.make_char_kind;
        self.level = record.level;
        self.life = record.life as i32;
        self.x = record.x as i16;
        self.y = record.y as i16;
        self.map_index = record.map_index;
        self.move_speed = record.total_move_speed as i8;
    }
}

/// The local avatar's delta overlay: the fields the opcode-5 record does not
/// carry, each landed by the legacy writer that owns it.
///
/// `None` everywhere until a frame carries the field — step 4.5-4.7 left
/// `CharClient` without mana deliberately (`world_entry.rs`, "MP arrives on
/// later state-sync frames … Step 5 scope"); this is where it arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDelta {
    /// `siReservedLife`, ratcheting (`Client-Person-BigInfo.cpp:1230-1236`).
    pub reserved_life: Option<i32>,
    /// `siMana` via `SetMana` (`Client-Person-BigInfo.cpp:1238`), clamped to
    /// zero when negative, as the legacy store chain does
    /// (`Char-Common.cpp:1295-1298` -> `parametabox.h:194-198`).
    pub mana: Option<i32>,
    /// `uiGMMode` (`Client-Person-BigInfo.cpp:1255`).
    pub gm_mode: Option<u8>,
    /// `bPCRoom` (`Client-Person-BigInfo.cpp:1256`).
    pub pc_room: Option<bool>,
    /// `siMakeAniType` (`Client-Person-BigInfo.cpp:1257`).
    pub make_ani_type: Option<i8>,
    /// The parameta box opcode 215 replaced (`clIP.Set`,
    /// `Client-Person-BigInfo.cpp:243`).
    pub pip: Option<PipInfo>,
}

impl LocalDelta {
    /// Everything unarrived.
    const fn empty() -> Self {
        Self {
            reserved_life: None,
            mana: None,
            gm_mode: None,
            pc_room: None,
            make_ani_type: None,
            pip: None,
        }
    }
}

/// One connection's in-world character state — the port's model of the
/// `CR[]` table for the five frames in scope.
///
/// The local avatar is the Step 4.5-4.7 [`CharClient`] plus [`LocalDelta`];
/// everybody else is a [`WorldChar`] keyed by `char_unique` (the legacy
/// keys by `CharUniqueInfo[]` -> `CR[id]`; the key space is the same set of
/// characters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatcherState {
    /// `MyCharUnique` (`Client-SystemNPC.cpp:698`) — the local-avatar test
    /// every target resolution starts from.
    my_char_unique: i32,
    /// The local [`CharClient`], adopted after world entry
    /// (`Client-SetChar.cpp:138`).
    local_char: Option<CharClient>,
    /// The local avatar's delta overlay.
    local_delta: LocalDelta,
    /// Every other character, born from opcode 6.
    nearby: BTreeMap<i32, WorldChar>,
}

impl DispatcherState {
    /// A state for the connection's `MyCharUnique`, empty until frames
    /// arrive.
    #[must_use]
    pub fn new(my_char_unique: i32) -> Self {
        Self {
            my_char_unique,
            local_char: None,
            local_delta: LocalDelta::empty(),
            nearby: BTreeMap::new(),
        }
    }

    /// Adopts the world-entry [`CharClient`] as the local avatar — the port's
    /// `SetChar(1, &clinfo)` (`Client-SetChar.cpp:138`), after which 200's
    /// life/position writes have a home.
    pub fn adopt_local(&mut self, char_client: &CharClient) {
        self.local_char = Some(char_client.clone());
    }

    /// The local [`CharClient`], if adopted.
    #[must_use]
    pub fn local_char(&self) -> Option<&CharClient> {
        self.local_char.as_ref()
    }

    /// The local avatar's delta overlay.
    #[must_use]
    pub fn local_delta(&self) -> &LocalDelta {
        &self.local_delta
    }

    /// The nearby table, keyed by `char_unique`.
    #[must_use]
    pub fn nearby(&self) -> &BTreeMap<i32, WorldChar> {
        &self.nearby
    }

    /// Routes one already-deciphered frame: parse, then apply. Foreign
    /// opcodes are [`DispatchOutcome::Unhandled`]; refused frames return the
    /// error and touch nothing.
    ///
    /// # Errors
    ///
    /// As the individual [`parse_chat`] / [`parse_char_update`] /
    /// [`parse_person_ip`] / [`parse_logout`] parsers, and
    /// [`DispatchError::CharInfo`] for the opcode-6 walk.
    pub fn dispatch(&mut self, cmd: u16, payload: &[u8]) -> Result<DispatchOutcome, DispatchError> {
        match cmd {
            RESPONSE_LOGINMANY => {
                let records = parse_make_char_info_many(cmd, payload)?;
                let (char_uniques, npcs) = self.apply_login_many(&records);
                Ok(DispatchOutcome::Entered { char_uniques, npcs })
            }
            RESPONSE_LOGOUT => {
                let logout = parse_logout(cmd, payload)?;
                Ok(self.apply_logout(logout.char_unique))
            }
            RESPONSE_CHAT => Ok(DispatchOutcome::Chat(parse_chat(cmd, payload)?)),
            RESPONSE_CHARUPDATE => {
                let update = parse_char_update(cmd, payload)?;
                Ok(self.apply_char_update(update))
            }
            RESPONSE_PERSON_IP => {
                let ip = parse_person_ip(cmd, payload)?;
                Ok(self.apply_person_ip(ip))
            }
            other => Ok(DispatchOutcome::Unhandled { cmd: other }),
        }
    }

    /// Opcode 6: each **player** record upserts its entry, exactly as the
    /// legacy walk `PushChar`es every record it reads (`Client-Login.cpp:
    /// 53-85`, with no self-filter — a record naming the local unique is not
    /// special-cased here either). Returns the upserted uniques in body
    /// order, plus how many NPC records were skipped.
    ///
    /// The skip is the minimal-correct handling of an NPC record (LIVE-01,
    /// `docs/rca/2026-10-28-live-opcode6-npc-stride.md`): the legacy pushes
    /// one into its char list for the world renderer, but nothing this
    /// dispatcher models — chat, char-update deltas, person-IP, logout —
    /// acts on an NPC through this table, so upserting one would fabricate
    /// entries no later frame can maintain. The count keeps the skip
    /// observable: the live server's world-entry broadcast is NPC-led
    /// (34 x 68-byte records, `evidence/live/decode2.log`), so a zero count
    /// would mean the stride dispatch mis-read the batch.
    fn apply_login_many(&mut self, records: &[MakeCharInfo]) -> (Vec<i32>, usize) {
        let mut char_uniques = Vec::with_capacity(records.len());
        let mut npcs = 0;
        for record in records {
            if !record.is_player {
                npcs += 1;
                continue;
            }
            let key = i32::from(record.make_char_unique);
            match self.nearby.get_mut(&key) {
                Some(entry) => entry.refresh_from_record(record),
                None => {
                    self.nearby.insert(key, WorldChar::from_record(record));
                }
            }
            char_uniques.push(key);
        }
        (char_uniques, npcs)
    }

    /// Opcode 200: the legacy writer-for-writer, self and others alike.
    fn apply_char_update(&mut self, update: CharUpdate) -> DispatchOutcome {
        let key = i32::from(update.char_unique);
        if key == self.my_char_unique {
            // Client-Person-BigInfo.cpp:1160-1163 -> CR[1]. The legacy's
            // `SetLife(SI32)` chain is an unclamped store
            // (`Char-Common.cpp:1333` -> `parametabox.h:179`), and `MoveTo`
            // takes `SI32` into the signed `clPos`; `CharClient` carries the
            // opcode-5 record's unsigned fields, so these casts are
            // bit-preserving reinterpretations of the wire value, with no
            // clamping invented for them.
            if let Some(local) = self.local_char.as_mut() {
                local.life = update.life as u32; // SetLife, :1228
                local.x = update.x as u16; // MoveTo, :1218 (snap; pathing
                local.y = update.y as u16; // animation not modeled)
            }
            // :1241-1244 — move_speed is deliberately NOT written for self.
            ratchet(&mut self.local_delta.reserved_life, update.reserved_life);
            // SetMana, :1238: the store chain clamps a negative mana to 0
            // (`Char-Common.cpp:1295-1298` -> `parametabox.h:194-198`), and
            // that clamp is mirrored here — sourced, not invented.
            self.local_delta.mana = Some(update.mana.max(0));
            self.local_delta.gm_mode = Some(update.gm_mode);
            self.local_delta.pc_room = Some(update.pc_room);
            self.local_delta.make_ani_type = Some(update.make_ani_type);
            return DispatchOutcome::Applied {
                char_unique: key,
                target: ApplyTarget::Local,
            };
        }
        let Some(entry) = self.nearby.get_mut(&key) else {
            // :1169 — the legacy's silent `charid <= 0` return.
            return DispatchOutcome::Applied {
                char_unique: key,
                target: ApplyTarget::Unmatched,
            };
        };
        entry.life = update.life;
        ratchet(&mut entry.reserved_life, update.reserved_life);
        // Same chain as the self path: negative mana clamps to 0
        // (`Char-Common.cpp:1295-1298` -> `parametabox.h:194-198`).
        entry.mana = Some(update.mana.max(0));
        entry.x = update.x;
        entry.y = update.y;
        entry.move_speed = update.move_speed;
        entry.gm_mode = update.gm_mode;
        entry.pc_room = update.pc_room;
        entry.make_ani_type = update.make_ani_type;
        DispatchOutcome::Applied {
            char_unique: key,
            target: ApplyTarget::Nearby,
        }
    }

    /// Opcode 215: `clIP.Set` — the parameta box only, never the life/mana
    /// mirrors (`Client-Person-BigInfo.cpp:243`).
    fn apply_person_ip(&mut self, ip: PersonIp) -> DispatchOutcome {
        let key = i32::from(ip.char_unique);
        if key == self.my_char_unique {
            self.local_delta.pip = Some(ip.pip);
            return DispatchOutcome::Applied {
                char_unique: key,
                target: ApplyTarget::Local,
            };
        }
        let Some(entry) = self.nearby.get_mut(&key) else {
            // IsValidID false, :241.
            return DispatchOutcome::Applied {
                char_unique: key,
                target: ApplyTarget::Unmatched,
            };
        };
        entry.pip = Some(ip.pip);
        DispatchOutcome::Applied {
            char_unique: key,
            target: ApplyTarget::Nearby,
        }
    }

    /// Opcode 11: delete a table entry, never the local avatar
    /// (`Client-Login.cpp:403-411`). The legacy's `DeleteReservedChar`
    /// (`:416`) has no port model: opcode 6 upserts directly, so there is
    /// no reserved queue to clear.
    fn apply_logout(&mut self, char_unique: i32) -> DispatchOutcome {
        if char_unique == self.my_char_unique {
            return DispatchOutcome::Removed {
                char_unique,
                applied: false,
            };
        }
        let applied = self.nearby.remove(&char_unique).is_some();
        DispatchOutcome::Removed {
            char_unique,
            applied,
        }
    }
}

/// The reserved-life ratchet: only a strictly greater value lands
/// (`Client-Person-BigInfo.cpp:1230-1236`).
fn ratchet(slot: &mut Option<i32>, incoming: i32) {
    if slot.is_none_or(|current| incoming > current) {
        *slot = Some(incoming);
    }
}

#[cfg(test)]
mod tests;
