//! Step 7 Phase 1 — the outbound in-world action encoders (WO §4a,
//! `docs/agents/WO-step7-in-world-actions.md` §8, Rev 2.1).
//!
//! Six intents, modern lean payloads (§8.2 message table):
//!
//! | Intent | Const | Opcode | Payload |
//! |---|---|---:|---|
//! | Order: Move / Attack / SitDown | [`REQUEST_ORDER`] | 240 | `kind u8` + variant fields |
//! | Chat | [`REQUEST_CHAT`] | 117 | `mode u8` + UTF-8 text |
//! | Quit to title | [`REQUEST_QUIT`] | 107 | ∅ |
//! | Return to front | [`REQUEST_FRONT`] | 109 | ∅ |
//! | Warp to village | [`REQUEST_WARP`] | 146 | `village u16` |
//! | System-NPC position query | [`REQUEST_NPCPOS`] | 5206 | `kind u16` |
//!
//! Opcode identity is retained (D1: `MsgType.h` numbers are identifiers, not
//! bloat); only the *payloads* are modernized. The 5-byte frame header
//! (`usSize | ucChk | usCmd`, `src/protocol/header.rs:45`) is transport, not
//! bloat (D2) — every message below rides the existing `GameLink::send` gate
//! (`entry.rs:530-535`), which builds the frame; the builders here are pure
//! payload codecs, tested without I/O (§8.4).
//!
//! No in-payload self-descriptors (D2: no `siDataSize`), no padding, no
//! transmute (D4), coordinates as a clean LE word pair (D5), chat
//! frame-delimited UTF-8 with typed refusals that never clamp (D6/D7).
//! [`RESPONSE_ORDER`] (241) is declared here for the inbound half Phase 2
//! adds to `dispatcher` (D8); nothing parses it yet.

use crate::game::entry::{GameError, GameLink};

/// `GAMEMSG_REQUEST_USERORDER` — the one order channel for Move, Attack and
/// SitDown (`MsgType.h:289`; legacy send path `Client.cpp:3284-3304`, server
/// dispatch `Server-Person.cpp:2700`).
pub const REQUEST_ORDER: u16 = 240;
/// `GAMEMSG_RESPONSE_USERORDER` — the neighbour echo of an order
/// (`MsgType.h:290`; emitted from `char-order.cpp:118`). Inbound parsing is
/// Phase 2 (D8); declared now so the constant table is single-sourced (T1).
pub const RESPONSE_ORDER: u16 = 241;
/// `GAMEMSG_REQUEST_CHAT` — chat submit, mode + UTF-8 text (`MsgType.h:86`).
pub const REQUEST_CHAT: u16 = 117;
/// `HOPETOQUIT` — quit to the title screen (`MsgType.h:68`).
pub const REQUEST_QUIT: u16 = 107;
/// `HOPETOFRONT` — return to the front-login screen (`MsgType.h:71`).
pub const REQUEST_FRONT: u16 = 109;
/// `WARPVILLAGE` — warp to a village (`MsgType.h:144`).
pub const REQUEST_WARP: u16 = 146;
/// System-NPC position query (`MsgType.h:1420`, anchored at the `5200`
/// family on `:1411`).
pub const REQUEST_NPCPOS: u16 = 5206;

/// The chat text cap in **bytes** (§8.2/D6): over it is a typed refusal,
/// never a clamp — the deliberate divergence from the legacy
/// `StringCchCopy` silent truncation (`MsgType-System.h:1110`).
pub const MAX_CHAT_BYTES: usize = 255;

/// The wire kind tag of an order (§8.2/D3): `0 Move`, `1 Attack`,
/// `2 SitDown`; `3..=255` is reserved — **unrepresentable on send** (a
/// Rust enum cannot emit it) and a typed refusal on receive (Phase 2, D8).
/// Semantics inherited from the legacy `ORDER_*` table
/// (`CommonHeader.h:517-535`): Move ↔ `ORDER_MOVE`(4), Attack ↔
/// `ORDER_ATTACK`(5)/`ORDER_ATTACK_CLIENT`(18), SitDown ↔ `ORDER_SITDOWN`(17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderKind {
    /// Movement intent — the legacy order 4 (`CommonHeader.h:519`).
    Move = 0,
    /// Attack intent — the legacy orders 5/18 (`CommonHeader.h:521`).
    Attack = 1,
    /// Sit-down intent — the legacy order 17 (`CommonHeader.h:531`).
    SitDown = 2,
}

/// One outbound order (WO §8.4): a tagged enum whose payload is exactly the
/// variant's fields — replacing the legacy 24-byte struct that carried every
/// kind's fields at once (D3, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Walk to map coordinate `(x, y)` — a clean LE word pair (D5); the
    /// legacy superword `siPara1 = x | (y << 16)` (`order.cpp:148`) is
    /// retired.
    Move {
        /// Map x, little-endian (`order.cpp:148` semantics, range-checked
        /// by the server).
        x: u16,
        /// Map y, little-endian.
        y: u16,
    },
    /// Attack character `target_char` — 4 bytes LE (Q-4: `u32`/`i32` is
    /// notation only, the wire is the same 4 bytes either way).
    Attack {
        /// The target's unique id (`ORDER_ATTACK`, `CommonHeader.h:521`).
        target_char: u32,
    },
    /// Sit down — the kind tag alone (§8.3-C).
    SitDown,
}

impl Order {
    /// The wire kind tag of this order (§8.2/D3).
    #[must_use]
    pub const fn kind(&self) -> OrderKind {
        match self {
            Order::Move { .. } => OrderKind::Move,
            Order::Attack { .. } => OrderKind::Attack,
            Order::SitDown => OrderKind::SitDown,
        }
    }
}

/// Encodes `order` into its 240 payload: `kind u8` first, then only the
/// variant's fields — 5 bytes (Move/Attack) or 1 byte (SitDown), little
/// endian, field-wise (§8.1.5, no padding, no transmute).
#[must_use]
pub fn encode_order(order: &Order) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    payload.push(order.kind() as u8);
    match order {
        Order::Move { x, y } => {
            payload.extend_from_slice(&x.to_le_bytes());
            payload.extend_from_slice(&y.to_le_bytes());
        }
        Order::Attack { target_char } => payload.extend_from_slice(&target_char.to_le_bytes()),
        Order::SitDown => {}
    }
    payload
}

/// Sends an order through the negotiated link (`entry.rs:530-535`).
///
/// # Errors
///
/// [`GameError::NotHandshaken`] before the opcode 1 handshake — nothing is
/// written; then the send/socket cases.
pub async fn send_order(link: &mut GameLink, order: &Order) -> Result<(), GameError> {
    let payload = encode_order(order);
    link.send(REQUEST_ORDER, &payload).await
}

/// Chat modes (`CHATMODE_*`, `MsgType-System.h:1085-1097`; value 7 is not
/// in the table). `ChatMode::System` folds the legacy `siSystemTxt` field
/// away (D4, `MsgType-System.h:1091`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChatMode {
    /// Ordinary speech (`MsgType-System.h:1085`).
    Normal = 1,
    /// Whisper (`:1086`).
    Secret = 2,
    /// Town-wide shout (`:1087`).
    Town = 3,
    /// Head/announcement channel (`:1088`).
    Head = 4,
    /// Home/village channel (`:1089`).
    Home = 5,
    /// Question channel (`:1090`).
    Ask = 6,
    /// System message — carries the legacy `siSystemTxt` semantics (D4).
    System = 8,
    /// NPC/villager speech (`:1092`).
    Villager = 9,
    /// Party channel (`:1093`).
    Party = 10,
    /// King/governor channel (`:1094`).
    King = 11,
    /// Item-news channel (`:1095`).
    NewsItem = 12,
    /// Guild channel (`:1096`).
    Guild = 13,
}

/// Why an outbound action could not be encoded or sent (§8.4, D6).
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// The chat text is empty — refused, never clamped (D6; the legacy's
    /// empty guard was `Client.cpp:3315`).
    #[error("chat text is empty (D6: refusals are never truncation)")]
    EmptyChat,
    /// The chat text exceeds [`MAX_CHAT_BYTES`] — refused with the exact
    /// byte length (D6; the legacy's silent clamp was `StringCchCopy`,
    /// `MsgType-System.h:1110`).
    #[error("chat text of {len} bytes exceeds MAX_CHAT_BYTES ({max}) (D6: refusals are never truncation)")]
    ChatTooLong {
        /// The text's byte length (`str::len`).
        len: usize,
        /// [`MAX_CHAT_BYTES`] at the time of the refusal.
        max: usize,
    },
    /// The send gate refused (pre-handshake) or the socket failed —
    /// [`GameLink::send`] (`entry.rs:530-535`).
    #[error(transparent)]
    Send(#[from] GameError),
}

/// Encodes a chat payload: `mode u8` + the text's UTF-8 bytes — frame
/// delimited, no NUL, no `siDataSize` (D2/D6/D7).
///
/// # Errors
///
/// [`ActionError::EmptyChat`] for `""`; [`ActionError::ChatTooLong`] when
/// the text exceeds [`MAX_CHAT_BYTES`] bytes — a typed refusal, never a
/// clamp (D6).
pub fn encode_chat(mode: ChatMode, text: &str) -> Result<Vec<u8>, ActionError> {
    if text.is_empty() {
        return Err(ActionError::EmptyChat);
    }
    if text.len() > MAX_CHAT_BYTES {
        return Err(ActionError::ChatTooLong {
            len: text.len(),
            max: MAX_CHAT_BYTES,
        });
    }
    let mut payload = Vec::with_capacity(1 + text.len());
    payload.push(mode as u8);
    payload.extend_from_slice(text.as_bytes());
    Ok(payload)
}

/// Sends a chat through the negotiated link (`entry.rs:530-535`).
///
/// # Errors
///
/// The [`ActionError`] encode refusals first (nothing is written), then
/// [`ActionError::Send`] as [`send_order`].
pub async fn send_chat(link: &mut GameLink, mode: ChatMode, text: &str) -> Result<(), ActionError> {
    let payload = encode_chat(mode, text)?;
    link.send(REQUEST_CHAT, &payload).await?;
    Ok(())
}

/// The warp payload: `village u16` LE (§8.2/F).
#[must_use]
pub fn encode_warp(village: u16) -> [u8; 2] {
    village.to_le_bytes()
}

/// The system-NPC position-query payload: `kind u16` LE (§8.2/G).
///
/// `kind` is an **opaque, server-defined** value (Q-1): the WO carries no
/// enumerated valid set, so any `u16` encodes — neither endpoint may invent
/// a "known set" the other does not hold.
#[must_use]
pub fn encode_npc_pos(kind: u16) -> [u8; 2] {
    kind.to_le_bytes()
}

/// The empty body shared by quit and front (§8.4, D2: no redundant
/// headers — the frame header alone is the message, 5 bytes on the wire).
pub const EMPTY_BODY: [u8; 0] = [];

/// Sends the quit-to-title request (`MsgType.h:68`).
///
/// # Errors
///
/// [`GameError::NotHandshaken`] before the opcode 1 handshake — nothing is
/// written; then the send/socket cases.
pub async fn send_quit(link: &mut GameLink) -> Result<(), GameError> {
    link.send(REQUEST_QUIT, &EMPTY_BODY).await
}

/// Sends the return-to-front request (`MsgType.h:71`).
///
/// # Errors
///
/// As [`send_quit`].
pub async fn send_front(link: &mut GameLink) -> Result<(), GameError> {
    link.send(REQUEST_FRONT, &EMPTY_BODY).await
}

/// Sends the warp request (`MsgType.h:144`).
///
/// # Errors
///
/// As [`send_quit`].
pub async fn send_warp(link: &mut GameLink, village: u16) -> Result<(), GameError> {
    let payload = encode_warp(village);
    link.send(REQUEST_WARP, &payload).await
}

/// Sends the system-NPC position query (`MsgType.h:1420`; `kind` opaque,
/// Q-1).
///
/// # Errors
///
/// As [`send_quit`].
pub async fn send_npc_pos(link: &mut GameLink, kind: u16) -> Result<(), GameError> {
    let payload = encode_npc_pos(kind);
    link.send(REQUEST_NPCPOS, &payload).await
}

#[cfg(test)]
mod tests;
