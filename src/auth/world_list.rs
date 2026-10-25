//! Step 3.2 — Notice request, WorldList request, and the `WORLDINFO`
//! response (`CONTRIBUTING.md:63-66`).
//!
//! # The legacy flow this step ports
//!
//! After the front login, the legacy client walks a ping-pong dispatched by
//! `clUserState` in `cltClient::FrontServerOperation`
//! (`Client-Winsock.cpp:595-613`) and driven from the login state machine
//! (`LogIn.cpp:760-801`):
//!
//! 1. `FRONTMSG_REQUEST_NOTICE` while the state is `FRONT_AUTHOK`
//!    (`LogIn.cpp:763-779`); the response flips the state to
//!    `USERSTATE_FRONT_NOTICEOK` (`Client-Winsock.cpp:595-604`, flip at `:603`).
//! 2. `FRONTMSG_REQUEST_WORLDLIST` once the state is `FRONT_NOTICEOK`
//!    (`LogIn.cpp:783-801`); the response goes to `cltWorldList::SetWorldList`
//!    and flips to `USERSTATE_FRONT_SERVERSELECTING`
//!    (`Client-Winsock.cpp:607-613`).
//!
//! This step owns the two request senders and the byte-exact parsers for the
//! two responses. The state box itself belongs to a later step: this module
//! never filters frames — callers pull with [`FrontLink::next_frame`] and
//! dispatch by opcode, so refresh notifies and heartbeats can never be
//! swallowed here.
//!
//! # Opcodes — resolving the dispatch's "Opcode 5"
//!
//! `CONTRIBUTING.md:65` says "Send Opcode 5 (`RequestWorldlist`)". The wire
//! truth disagrees, and it is not ambiguous — every source agrees:
//!
//! | Opcode | Name | Legacy header | Server port |
//! |---|---|---|---|
//! | 2 | `FRONTMSG_REQUEST_NOTICE` | `FrontMsg.h:45` | `opcodes_front.rs:17` |
//! | 3 | `FRONTMSG_REQUEST_WORLDLIST` | `FrontMsg.h:46` | `opcodes_front.rs:19` |
//! | 5 | `FRONTMSG_REQUEST_PATCHTEXT` | `FrontMsg.h:48` | — |
//! | 1001 | `FRONTMSG_RESPONSE_NOTICE` | `FrontMsg.h:94` | `opcodes_front.rs:65` |
//! | 1002 | `FRONTMSG_RESPONSE_WORLDLIST` | `FrontMsg.h:95` | `opcodes_front.rs:67` |
//!
//! The enum starts at `FRONTMSG_REQUEST_LOGIN = 1` (`FrontMsg.h:44`), so 5
//! counts to `PATCHTEXT`, not `WORLDLIST`. The legacy client writes
//! `usCmd = FRONTMSG_REQUEST_WORLDLIST` (`LogIn.cpp:790`), the legacy server
//! switches on the enum (`FrontClientProtocol.cpp:774`), and the server port
//! matches `FrontMsg::RequestWorldlist` (`services/front/src/main.rs:196`).
//! Sending 5 would be read as a patch request and dropped as unhandled —
//! this port sends **3**, resolved from source rather than guessed (standing
//! escalation rule: resolve against wire truth first).
//!
//! # Request frames (client → server, always ciphered)
//!
//! Both requests travel through [`FrontLink::send`], enciphered with the
//! session cipher installed by the `SECRETKEY` handshake — the same path every
//! legacy front send takes (`SendFrontServerMsg`, `Client-Winsock.cpp:139-150`
//! with the key gate at `:141`). **There is deliberately no plaintext send
//! path**: the legacy's one raw-send quirk (`FRONTMSG_REQUEST_PINGTEST`
//! written without `Encode`, `PatchChina/FrontProcess.cpp:125-137`) was
//! rejected as a security lowering by QC decision F-04 — 100 % cipher
//! integrity on every outbound frame.
//!
//! | Request | `usSize` | payload | Cite |
//! |---|---|---|---|
//! | Notice | 5 | none — `sRequest_Notice` is header-only | `FrontMsg.h:477-480`; sent `LogIn.cpp:767-772` |
//! | WorldList | 25 | `szAccountID[20]`, NUL-padded | `FrontMsg.h:486-491` (field at `:490`); sent `LogIn.cpp:787-794` |
//!
//! Both structs sit under the file's `#pragma pack(1)` (`FrontMsg.h:117`,
//! popped at `:655`), so `sizeof(sRequest_WorldList)` is 5 + 20 = 25 on the
//! wire, never a padded 26.
//!
//! # The `WORLDINFO` response — why 428, not 421
//!
//! `sResponse_WorldList` (`FrontMsg.h:509-516`) is `sPacketHeader` + `usCount`
//! (`:513`) + a `WORLDINFO[]` tail. `WORLDINFO` and its three nested structs
//! are declared at `FrontMsg.h:6-41` — **before** the file's
//! `#pragma pack(1)` at `:117` — and the `#pragma pack()` inside the included
//! `itzbase.h` (`:58` … `:70`, included at `FrontMsg.h:4`) has already reset
//! packing to the MSVC default (`/Zp8`). The world-list structs are therefore
//! **default-packed** while everything from line 117 on is byte-packed:
//!
//! ```text
//! GAMESERVER_BASEINFO       123 raw + 2 pad (after worldname[20])
//!                            + 3 pad (struct tail)          = 128
//! GAMESERVER_REFRESHINFO      6 raw + 2 pad (struct tail)   =   8
//! GAMESERVER_GAMESERVERINFO 292 raw                          = 292
//! WORLDINFO                                              = 428 bytes
//! ```
//!
//! The server port derives the same number independently: `WORLDINFO_LEN =
//! 428` with all three pads spelled out (`gz-proto/src/front.rs:31,210-233`;
//! its header notes that dropping them would shrink every world-list packet
//! by 7 bytes per world). Frame arithmetic, from the builder the legacy server
//! actually runs (`cltWorldList::MakeWorldListPacket`, `WorldList.cpp:215-256`):
//!
//! - `usSize = sizeof(sResponse_WorldList) + (count - 1) * sizeof(WORLDINFO)`
//!   (`WorldList.cpp:236`) = `7 + 428 * count` on the wire;
//! - empty list: `usSize = 5 + 2`, `usCount = 0` (`WorldList.cpp:227-229`);
//! - server → client is plaintext on this link, so the frame arrives verbatim
//!   (`front.rs` module docs, asymmetry table).
//!
//! # Parsing rules — strict on purpose
//!
//! 1. The opcode must match (1002 for the world list, 1001 for the notice).
//! 2. `usCount` needs 2 payload bytes.
//! 3. `expected = 2 + 428 * count` is computed in `usize` (worst case
//!    28,048,982 — no overflow) **before** any slicing or allocation.
//! 4. A short payload is [`WorldListError::Truncated`]. The legacy reads
//!    blind: `SetWorldList` copies `usCount` entries with no length check
//!    (`WorldList.cpp:189-194`), walking off the buffer of a truncated frame;
//!    this port refuses instead of reading out of bounds.
//! 5. A long payload is [`WorldListError::TrailingBytes`]: bytes the count
//!    does not explain are **surfaced, never dropped** — Step 3.2's explicit
//!    acceptance criterion.
//! 6. Strings are C strings: cut at the first NUL, invalid UTF-8 replaced —
//!    byte-identical to the server port's reader (`gz-proto/src/wire.rs:239-243`);
//!    the legacy `TCHAR` fields are MBCS.
//! 7. The three pad regions are alignment, not data: skipped, never read,
//!    never validated — the legacy copies them inside uninitialized structs,
//!    so their wire value is indeterminate by construction.
//!
//! The notice response must be exactly the 16 payload bytes both known
//! front servers send (`usSize = sizeof(sPacketHeader) + 16`,
//! `FrontClientProtocol.cpp:760`; `NOTICE_PAYLOAD_LEN`,
//! `gz-proto/src/front.rs:26-28`). The legacy client tolerated any size up
//! to `sizeof(text)` (`Client-Winsock.cpp:599`); this port refuses drift
//! loudly instead of parsing past what either server actually writes.
//!
//! # Test addresses are synthetic (F-01)
//!
//! The suite runs on loopback with synthetic addresses — pending live
//! ingestion against the real Front (5700) and Game (4010) servers (QC
//! decision F-01 on CAR-2026-10-24-RETRO01). What the tests prove is byte
//! arithmetic, not reachability.

use crate::auth::front::{FrontError, FrontLink};
use crate::net::codec::Frame;

/// `FRONTMSG_REQUEST_NOTICE` (`FrontMsg.h:45`): notice request, opcode 2.
pub const REQUEST_NOTICE: u16 = 2;

/// `FRONTMSG_RESPONSE_NOTICE` (`FrontMsg.h:94`): notice response, opcode
/// 1001 — sent by both front servers (`FrontClientProtocol.cpp:761`,
/// `opcodes_front.rs:65`).
pub const RESPONSE_NOTICE: u16 = 1001;

/// `FRONTMSG_REQUEST_WORLDLIST` (`FrontMsg.h:46`): world-list request,
/// opcode **3** — not the "Opcode 5" in `CONTRIBUTING.md:65` (5 is
/// `FRONTMSG_REQUEST_PATCHTEXT`, `FrontMsg.h:48`; module docs, Opcodes).
pub const REQUEST_WORLDLIST: u16 = 3;

/// `FRONTMSG_RESPONSE_WORLDLIST` (`FrontMsg.h:95`): world-list response,
/// opcode 1002 — built by `WorldList.cpp:215-256`, consumed at
/// `Client-Winsock.cpp:607-613`.
pub const RESPONSE_WORLDLIST: u16 = 1002;

/// `sizeof(szAccountID)` (`FrontMsg.h:490`): the world-list request's
/// payload is exactly this many bytes, NUL-padded — `usSize` 25 total
/// (`FrontMsg.h:486-491` under `#pragma pack(1)`).
pub const ACCOUNT_ID_LEN: usize = 20;

/// `sizeof(WORLDINFO)` under the MSVC default packing that applies to
/// `FrontMsg.h:6-41` (module docs, *why 428, not 421*); independently
/// pinned by the server port as `WORLDINFO_LEN` (`gz-proto/src/front.rs:31`).
pub const WORLDINFO_LEN: usize = 428;

/// The notice response's payload: `usSize = sizeof(sPacketHeader) + 16`
/// (`FrontClientProtocol.cpp:760`) and the server port's
/// `NOTICE_PAYLOAD_LEN` (`gz-proto/src/front.rs:26-28`) — both known front
/// servers send exactly this.
pub const NOTICE_PAYLOAD_LEN: usize = 16;

/// Why a world-list / notice operation failed.
#[derive(Debug, thiserror::Error)]
pub enum WorldListError {
    /// The link layer failed — the send hit [`FrontError`] (e.g.
    /// [`FrontError::NotKeyed`] before the handshake, preserving the legacy's
    /// `bFrontServerSecretKeySwitch` gate), or the receive side surfaced a
    /// session/codec failure.
    #[error(transparent)]
    Front(#[from] FrontError),
    /// The frame's opcode is not the response this parser expects — dispatch
    /// drift or a frame this port's contract does not cover
    /// (`FrontMsg.h:94-95`).
    #[error(
        "front opcode {cmd} is not the response this parser expects (notice 1001, worldlist 1002)"
    )]
    UnexpectedOpcode {
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// Fewer than 2 payload bytes: `usCount` cannot be read
    /// (`FrontMsg.h:513`).
    #[error("worldlist payload of {len} bytes cannot hold the 2-byte usCount (FrontMsg.h:513)")]
    MissingCount {
        /// Payload length actually received.
        len: usize,
    },
    /// The frame is shorter than `usCount` entries explain. The legacy would
    /// read past the buffer (`WorldList.cpp:189-194`); this port refuses.
    #[error("worldlist declares {count} entries ({expected} bytes) but the frame carries {got}")]
    Truncated {
        /// `usCount` as declared on the wire.
        count: u16,
        /// `2 + count * 428` bytes the count explains.
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// The frame carries bytes `usCount` does not explain. Refused rather
    /// than consumed in silence: no trailing byte is ever dropped (Step 3.2
    /// acceptance criterion).
    #[error("worldlist payload of {got} bytes exceeds the {expected} explained by usCount — trailing bytes must not be silently dropped")]
    TrailingBytes {
        /// `2 + count * 428` bytes the count explains.
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// The account id does not fit `szAccountID[20]` with its NUL
    /// terminator (`FrontMsg.h:490`). The legacy ignored `StringCchCopy`'s
    /// failure and transmitted an indeterminate buffer (`LogIn.cpp:792`);
    /// this port refuses instead (module docs, request frames).
    #[error("account id of {len} bytes does not fit szAccountID[20] with its NUL terminator (FrontMsg.h:490)")]
    AccountIdTooLong {
        /// Account-id length that failed to fit.
        len: usize,
    },
    /// The notice payload is not the 16 bytes both known front servers send
    /// (`FrontClientProtocol.cpp:760`, `gz-proto/src/front.rs:26-28`).
    #[error(
        "notice payload of {len} bytes; both known front servers send exactly {NOTICE_PAYLOAD_LEN}"
    )]
    UnexpectedNoticeLength {
        /// Payload length actually received.
        len: usize,
    },
}

/// Builds the 20-byte `szAccountID` payload of a world-list request
/// (`sRequest_WorldList`, `FrontMsg.h:486-491`).
///
/// Semantics follow the legacy's `StringCchCopy(sendmsg.szAccountID, sizeof,
/// szID)` (`LogIn.cpp:792`): copy the C-string up to its first NUL, and
/// zero-fill the rest of the 20 bytes (the legacy's remainder is
/// uninitialized stack; zero-filling is observationally identical — the
/// server reads only up to the terminator, `gz-proto/src/wire.rs:239-243`).
///
/// # Errors
///
/// [`WorldListError::AccountIdTooLong`] when the id (up to its first NUL)
/// needs 20 or more bytes and therefore cannot fit a terminator — the legacy
/// ignored that failure and sent garbage; this port refuses (module docs,
/// request frames).
pub fn worldlist_request_payload(account_id: &str) -> Result<[u8; ACCOUNT_ID_LEN], WorldListError> {
    let raw = account_id.as_bytes();
    let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    if len >= ACCOUNT_ID_LEN {
        return Err(WorldListError::AccountIdTooLong { len });
    }
    let mut payload = [0u8; ACCOUNT_ID_LEN];
    payload[..len].copy_from_slice(&raw[..len]);
    Ok(payload)
}

/// Sends `FRONTMSG_REQUEST_NOTICE` — an empty payload, because
/// `sRequest_Notice` is header-only (`FrontMsg.h:477-480`); `usSize` 5 on
/// the wire. The legacy send is `LogIn.cpp:767-772`.
///
/// Enciphered like every other front send (module docs, request frames) —
/// QC decision F-04: no plaintext exception.
///
/// # Errors
///
/// [`WorldListError::Front`] — [`FrontError::NotKeyed`] before the
/// `SECRETKEY` handshake, then the [`FrontLink::send`] failure cases.
pub async fn request_notice(link: &mut FrontLink) -> Result<(), WorldListError> {
    link.send(REQUEST_NOTICE, &[]).await?;
    Ok(())
}

/// Sends `FRONTMSG_REQUEST_WORLDLIST` with `szAccountID` set from
/// `account_id` — `usSize` 25 on the wire. The legacy send is
/// `LogIn.cpp:787-794`.
///
/// Enciphered like every other front send (module docs, request frames) —
/// QC decision F-04: no plaintext exception.
///
/// # Errors
///
/// [`WorldListError::AccountIdTooLong`] when `account_id` cannot fit
/// (nothing is written); then [`WorldListError::Front`] as in
/// [`request_notice`].
pub async fn request_worldlist(
    link: &mut FrontLink,
    account_id: &str,
) -> Result<(), WorldListError> {
    let payload = worldlist_request_payload(account_id)?;
    link.send(REQUEST_WORLDLIST, &payload).await?;
    Ok(())
}

/// One `WORLDINFO` entry (`FrontMsg.h:36-41`), every field read from its
/// literal wire offset under the default packing documented in the module
/// docs. `String` fields are C strings (cut at the first NUL, lossy UTF-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldInfo {
    /// `baseinfo.worldId` (`FrontMsg.h:8`), offset 0.
    pub world_id: u16,
    /// `baseinfo.worldname[20]` (`FrontMsg.h:9`), offset 2 — the world's
    /// server name as the select screen shows it.
    pub world_name: String,
    /// `baseinfo.siServerID` (`FrontMsg.h:10`), offset 24.
    pub server_id: i32,
    /// `baseinfo.gameserverip[16]` (`FrontMsg.h:11`), offset 28 — where the
    /// client dials next (Step 4.1).
    pub game_server_ip: String,
    /// `baseinfo.gameserverport` (`FrontMsg.h:12`), offset 44 — the Game
    /// Server port (4010 in these deployments).
    pub game_server_port: u16,
    /// `baseinfo.opendate[15]` (`FrontMsg.h:13`), offset 46.
    pub open_date: String,
    /// `baseinfo.shortmessage[64]` (`FrontMsg.h:14`), offset 61 — the
    /// world's short banner.
    pub short_message: String,
    /// `refreshinfo.worldId` (`FrontMsg.h:19`), offset 128 — the refresh
    /// copy of the id, kept exactly as received; this port does not assert
    /// it equals [`WorldInfo::world_id`] (no legacy path compares them
    /// either — `SetWorldList` copies whole structs, `WorldList.cpp:189-194`).
    pub refresh_world_id: u16,
    /// `refreshinfo.state` (`FrontMsg.h:20-22`), offset 130 — the status
    /// field. Kept **raw**: the legacy comment lists four states (0..3) but
    /// is codepage-mojibake in this tree, and only two values are corroborated
    /// anywhere (`0` normal, `2` maintenance — `gz-proto/src/front.rs:186`),
    /// so no enum is invented here (escalation rule: do not guess).
    pub state: u16,
    /// `refreshinfo.currentConnections` (`FrontMsg.h:23`), offset 132.
    pub current_connections: u16,
    /// `gameserverinfo.goonzuname[20]` (`FrontMsg.h:28`), offset 136.
    pub goonzu_name: String,
    /// `gameserverinfo.goonzutext[256]` (`FrontMsg.h:29`), offset 156 — the
    /// world's long description.
    pub goonzu_text: String,
    /// `gameserverinfo.population` (`FrontMsg.h:30`), offset 412 — signed.
    pub population: i32,
    /// `gameserverinfo.currentPrice` (`FrontMsg.h:31`), offset 416 — signed.
    pub current_price: i32,
    /// `gameserverinfo.npcrate` (`FrontMsg.h:32`), offset 420 — signed.
    pub npc_rate: i32,
    /// `gameserverinfo.fixationmoney` (`FrontMsg.h:33`), offset 424 —
    /// signed.
    pub fixation_money: i32,
}

/// Little-endian `u16` at `off`.
fn le_u16(entry: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([entry[off], entry[off + 1]])
}

/// Little-endian `i32` at `off`.
fn le_i32(entry: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([entry[off], entry[off + 1], entry[off + 2], entry[off + 3]])
}

/// Fixed-width C string: `len` bytes from `off`, cut at the first NUL,
/// invalid UTF-8 replaced — byte-identical to the server port's reader
/// (`gz-proto/src/wire.rs:239-243`).
fn cstr(entry: &[u8], off: usize, len: usize) -> String {
    let field = &entry[off..off + len];
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

impl WorldInfo {
    /// Reads one entry from a slice of exactly [`WORLDINFO_LEN`] bytes —
    /// the caller ( [`parse_world_list`]) proves that length before slicing.
    fn from_entry(entry: &[u8]) -> Self {
        debug_assert_eq!(
            entry.len(),
            WORLDINFO_LEN,
            "caller slices exactly one entry"
        );
        Self {
            world_id: le_u16(entry, 0),
            world_name: cstr(entry, 2, 20),
            server_id: le_i32(entry, 24),
            game_server_ip: cstr(entry, 28, 16),
            game_server_port: le_u16(entry, 44),
            open_date: cstr(entry, 46, 15),
            short_message: cstr(entry, 61, 64),
            refresh_world_id: le_u16(entry, 128),
            state: le_u16(entry, 130),
            current_connections: le_u16(entry, 132),
            goonzu_name: cstr(entry, 136, 20),
            goonzu_text: cstr(entry, 156, 256),
            population: le_i32(entry, 412),
            current_price: le_i32(entry, 416),
            npc_rate: le_i32(entry, 420),
            fixation_money: le_i32(entry, 424),
        }
    }
}

/// A fully parsed `FRONTMSG_RESPONSE_WORLDLIST` payload: `usCount` entries,
/// nothing left over, nothing read out of bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldList {
    /// The entries in wire order; its length is the frame's `usCount`.
    pub worlds: Vec<WorldInfo>,
}

impl WorldList {
    /// Parses a received [`Frame`] — opcode and payload checked together.
    ///
    /// # Errors
    ///
    /// As [`parse_world_list`]: [`WorldListError::UnexpectedOpcode`] for any
    /// frame that is not [`RESPONSE_WORLDLIST`], then the length rules.
    pub fn from_frame(frame: &Frame) -> Result<Self, WorldListError> {
        parse_world_list(frame.cmd(), frame.payload())
    }
}

/// Parses a `FRONTMSG_RESPONSE_WORLDLIST` **payload** (`usCount` + N ×
/// `WORLDINFO`) under the module's strict rules: exact count arithmetic,
/// short frames refused as [`WorldListError::Truncated`], surplus bytes
/// surfaced as [`WorldListError::TrailingBytes`] — never dropped.
///
/// # Errors
///
/// [`WorldListError::UnexpectedOpcode`] unless `cmd` is
/// [`RESPONSE_WORLDLIST`]; [`WorldListError::MissingCount`] under 2 payload
/// bytes; [`WorldListError::Truncated`] / [`WorldListError::TrailingBytes`]
/// when `payload.len() != 2 + 428 * count`. All validation runs before any
/// slicing, so no input can panic or read out of bounds.
pub fn parse_world_list(cmd: u16, payload: &[u8]) -> Result<WorldList, WorldListError> {
    if cmd != RESPONSE_WORLDLIST {
        return Err(WorldListError::UnexpectedOpcode { cmd });
    }
    let Some(count_bytes) = payload.get(..2) else {
        return Err(WorldListError::MissingCount { len: payload.len() });
    };
    let count = u16::from_le_bytes([count_bytes[0], count_bytes[1]]);
    // usize arithmetic: worst case 2 + 65535 * 428 = 28,048,982 — no
    // overflow, and computed before any slice or allocation.
    let expected = 2 + usize::from(count) * WORLDINFO_LEN;
    let got = payload.len();
    if got < expected {
        return Err(WorldListError::Truncated {
            count,
            expected,
            got,
        });
    }
    if got > expected {
        return Err(WorldListError::TrailingBytes { expected, got });
    }
    let worlds = (0..count)
        .map(|i| {
            let start = 2 + usize::from(i) * WORLDINFO_LEN;
            WorldInfo::from_entry(&payload[start..start + WORLDINFO_LEN])
        })
        .collect();
    Ok(WorldList { worlds })
}

/// Parses a `FRONTMSG_RESPONSE_NOTICE` **payload**: exactly
/// [`NOTICE_PAYLOAD_LEN`] bytes of C-string text.
///
/// # Errors
///
/// [`WorldListError::UnexpectedOpcode`] unless `cmd` is
/// [`RESPONSE_NOTICE`]; [`WorldListError::UnexpectedNoticeLength`] for any
/// payload size other than the 16 bytes both known front servers send
/// (module docs, parsing rules).
pub fn parse_notice(cmd: u16, payload: &[u8]) -> Result<String, WorldListError> {
    if cmd != RESPONSE_NOTICE {
        return Err(WorldListError::UnexpectedOpcode { cmd });
    }
    if payload.len() != NOTICE_PAYLOAD_LEN {
        return Err(WorldListError::UnexpectedNoticeLength { len: payload.len() });
    }
    Ok(cstr(payload, 0, NOTICE_PAYLOAD_LEN))
}

#[cfg(test)]
mod tests;
