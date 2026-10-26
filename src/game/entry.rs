//! Steps 4.1–4.2 — the Game Server link: the fixed inbound link cipher from
//! byte 0 (4.1), then the `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` (opcode 1)
//! handshake that parses `char_unique` and keys the outbound session cipher
//! (4.2) (`CONTRIBUTING.md:74-82`).
//!
//! # The dial
//!
//! The legacy dials `clSelectedWorldInfo.baseinfo.gameserverip` /
//! `.gameserverport` — the WORLDINFO the front server handed out in Step 3.2
//! — from `cltClient::GameServerAction` once the state machine reaches
//! `USERSTATE_GAME_CONNECTING` (`Client-Winsock.cpp:796-826`), flips to
//! `USERSTATE_GAME_CONNECTED` on success (`:822`), and clears
//! `bGameServerSecretKeySwitch` (`:825`). The port is **4010** in these
//! deployments (`CONTRIBUTING.md` Step 4.1; the server port's default listen
//! is `0.0.0.0:4010`, `services/game/src/config.rs:56`) — callers pass the
//! `SocketAddr` the world list carried, as on the front link.
//!
//! # Cipher pairing on this link — both directions encoded, different keys
//!
//! | Direction | Cipher | Cite |
//! |---|---|---|
//! | server → client (**inbound**) | **link cipher**, fixed `0x12, 0x34`, active from byte 0 for the connection's life | keyed `m_pEncrypt2->SetKey(0x12, 0x34)` in `ITZSession::Init` (`ITZSession.cpp:50-56`), consumed by `GetEncrypt2()->Decode` (`Client-Winsock.cpp:872`); the server side pairs it identically — one `outbound` cipher keyed from config encodes **every** server→client frame it sends (`services/game/src/main.rs:680`, used at `:687,834,847,...`, citing `char.cpp:5177-5219`, `ITZSendBuffer.cpp:122-124`) |
//! | client → server (outbound) | **session cipher** seeded by opcode 1's two bytes | the gate starts closed: `bGameServerSecretKeySwitch = false` (`Client-Winsock.cpp:825`); [`GameLink::handshake`] installs it (Step 4.2) and [`GameLink::send`] keeps it closed until then; the server decodes with `Cipher::session_cipher(key1, key2)` from the same bytes (`main.rs:704`) |
//!
//! **"Immediately, before reading the very first frame"** is not a race to
//! win: the server port writes the `ResponseClientcharunique` frame the
//! moment the connection is accepted, *before* entering its read loop
//! (`main.rs:687-699`), so a complete encrypted frame can be sitting in the
//! socket before this port has issued a single read. [`GameLink`]
//! therefore installs [`Decoder::with_cipher`] keyed with
//! [`Cipher::link_cipher`] **in the constructor** — there is no code path
//! that reads one game byte without the key already in place. Framing cuts
//! each frame on `usSize`
//! **in the clear** ahead of decryption (`ITZRecvBuffer.cpp:155`; the pull
//! path in `codec.rs`), and the cipher runs exactly once per pulled frame —
//! one `Cipher` per direction per connection (Step 1.3's invariant).
//!
//! # The handshake — `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` (Step 4.2)
//!
//! The server sends opcode **1** unsolicited at accept
//! (`GAMEMSG_RESPONSE_CLIENTCHARUNIQUE = 1`, `MsgType.h:22`; the server
//! port's `GameMsg::ResponseClientcharunique = 1`, `opcodes_game.rs:15`).
//! Its body is exactly [`CHAR_UNIQUE_PAYLOAD_LEN`] = 8 bytes
//! (`CHAR_UNIQUE_PAYLOAD_LEN`, `game.rs:16`):
//!
//! | Offset | Field | Legacy declaration | Meaning |
//! |---|---|---|---|
//! | 0..4 | [`char_unique`](ClientCharUnique::char_unique) | `SI32 siCharUnique` | this connection's session character id → `MyCharUnique` (`Client-SystemNPC.cpp:698`) |
//! | 4 | [`key1`](ClientCharUnique::key1) | `UI08 uiKey1` | outbound session-cipher key byte |
//! | 5 | [`key2`](ClientCharUnique::key2) | `UI08 uiKey2` | outbound session-cipher key byte |
//! | 6..8 | [`version`](ClientCharUnique::version) | `SI16 siVersion` | the server's advertised version |
//!
//! **Why those offsets are physical fact, not convention**: the struct is
//! declared as `class cltGameMsgResponse_ClientCharUnique { SI32; UI08;
//! UI08; SI16; }` (`MsgType.h:3013-3027`), and the handler reaches it by
//! casting `pclMsg->cData` (`Client-SystemNPC.cpp:695`) — while `cltMsg` is
//! `sPacketHeader clHead` immediately followed by the `cData[]` union
//! (`MsgType.h:2945-2952`). So payload byte 0 **is** `siCharUnique`'s first
//! byte. Under MSVC default packing `siVersion` sits at 6 (its 2-alignment
//! is satisfied) and the struct pads to its 4-byte alignment → 8 total;
//! `#pragma pack(1)` yields the same 8 bytes, so one layout holds either
//! way. The server port encodes precisely these eight bytes
//! (`game.rs:98-110`: `i32`, `u8`, `u8`, `i16`).
//!
//! **What the legacy handler does** (`Client-SystemNPC.cpp:693-717`):
//!
//! 1. `MyCharUnique = siCharUnique` (`:698`) — the id is allocated per
//!    session before any character is even known (`main.rs:681-686`);
//! 2. `GetEncrypt()->SetKey(uiKey1, uiKey2)` (`:701-703`) — the two bytes
//!    become the outbound cipher's keys **and** counter position;
//! 3. `bGameServerSecretKeySwitch = true` (`:706`) — the send gate opens;
//! 4. version gate: `siVersion > siVersion` (the local build number) →
//!    message box + `dwQuitTimer = 1` (`:709-717`).
//!
//! Steps 1–3 map one-to-one onto [`ClientCharUnique`], the encoder install
//! in [`GameLink::handshake`], and the [`GameError::NotHandshaken`] gate on
//! [`GameLink::send`]. Step 4 the port **records but does not act on**:
//! comparing an advertised version against a client *build number* and
//! quitting the UI is a product policy, not a wire rule (the server port
//! advertises `CLIENT_COMPAT_VERSION = 1`, `game.rs:81-85`, citing
//! `char.cpp:5180`). The raw `i16` travels out on
//! [`GameLink::negotiated`] for the caller to gate on — nothing in this
//! module is left half-implemented; the quit policy is deliberately out of
//! scope and recorded here.
//!
//! **Strict length, both directions of wrong**: the legacy casts and reads —
//! there is no length check at `Client-SystemNPC.cpp:695`, so a short body
//! would be an out-of-bounds read and a long one is silently ignored. This
//! port requires **exactly** [`CHAR_UNIQUE_PAYLOAD_LEN`] bytes: fewer →
//! [`GameError::Truncated`], more → [`GameError::TrailingBytes`] (the same
//! port-added strictness as Steps 3.2/3.3). The opcode is checked **first**:
//! any other command is [`GameError::UnexpectedOpcode`] regardless of its
//! payload length.
//!
//! **The server's half** (`services/game/src/main.rs`): at accept it draws
//! `(key1, key2) = random_key_bytes()` (`:676`), link-encodes and writes the
//! frame (`:687-699`), and from then on decodes inbound with
//! `Cipher::session_cipher(key1, key2)` (`:704`) — while continuing to
//! *encode* with the link cipher (`:680`). The client mirrors that pairing
//! exactly: inbound stays on the link cipher for the connection's life,
//! outbound starts at this handshake.
//!
//! ## Pre-handshake dispatch — intentional protocol decision (QC P-11)
//!
//! The server is being rewritten in Rust as a new implementation, so this
//! behavior is defined directly by the port rather than inherited from the
//! old C++ code. The decision, deliberately:
//!
//! - [`GameLink::handshake`] accepts **only** the expected handshake frame
//!   (opcode 1) — nothing else is a valid first frame;
//! - an unexpected pre-handshake frame is rejected with
//!   [`GameError::UnexpectedOpcode`], checked *before* its payload is even
//!   measured (`parse_client_char_unique`); the frame is consumed, never
//!   dispatched or acted on;
//! - the connection is terminated immediately: the error returns to the
//!   caller with the outbound gate still closed ([`GameError::NotHandshaken`]
//!   keeps [`GameLink::send`] shut and the negotiation slot empty), so no
//!   further state can change on this link and the caller closes it;
//! - this strictness is an **intentional protocol decision for the new Rust
//!   server/client pair** and is explicitly *not* required to imitate the old
//!   C++ implementation (QC direction, P-11).
//!
//! # Read timeouts are not teardowns (F-02)
//!
//! QC decision F-02 on the game link: a [`SessionError::ReadTimeout`] bounds
//! one *wait* only — no shutdown, no drop, no state change
//! (`net/session.rs:35-46`: "callers on the **game** link must treat
//! `ReadTimeout` as *nothing arrived yet — keep waiting*, never as a close
//! signal"). [`GameLink::next_frame`] surfaces that timeout as an ordinary
//! error with the socket open, the partial frame still buffered, and the
//! link cipher's counter untouched — the next call continues exactly where
//! the last one stopped. A sticky [`CodecError`] is the only fatal class:
//! framing or integrity refusal poisons the decoder, like the legacy's
//! `return false` out of the decode path (`Client-Winsock.cpp:872-890`).
//!
//! # Sending after the handshake
//!
//! [`GameLink::send`] is the legacy `SendMsgToServer`
//! (`Client-Winsock.cpp:155-171`) with the gate first —
//! `bGameServerSecretKeySwitch == false → return FALSE` (`:171`), the very
//! flag step 3 above flips — surfaced as [`GameError::NotHandshaken`] so
//! that before the handshake **nothing** reaches the socket and no key
//! ticks. Only then does `GetEncrypt()->Encode` + write run, which is
//! [`Encoder::send`] (keys tick at encode time, before the flush).
//!
//! The legacy additionally LZF-compresses a frame once `usSize >= 200`
//! (`Client-Winsock.cpp:178-186`) with the `0x8000` compression flag set in
//! `usCmd` by the caller before encoding. The flag path already travels
//! through this port's encoder untouched (`Encoder::encode`, `codec.rs`);
//! the compression itself is a payload policy beyond Steps 4.1–4.2 — this
//! module's job is the keyed channel, not which payloads ride it.
//!
//! # Test addresses are synthetic (F-01)
//!
//! The suite runs on loopback with synthetic addresses — pending live
//! ingestion against the real Game Server (4010) (QC decision F-01/F-02 on
//! CAR-2026-10-24-RETRO01; the 5 s windows are revalidated there). What the
//! tests prove is byte arithmetic, cipher state, and timeout behaviour — not
//! reachability.

use crate::net::cipher::Cipher;
use crate::net::codec::{CodecError, Decoder, Encoder, Frame, SendError};
use crate::net::session::{Session, SessionError, SessionTimeouts};
use std::io;
use std::net::SocketAddr;

/// The Game Server's port (`CONTRIBUTING.md` Step 4.1; the server port's
/// default listen `0.0.0.0:4010`, `services/game/src/config.rs:56`). The
/// legacy reads it from the selected WORLDINFO instead of a literal
/// (`Client-Winsock.cpp:805-806`), so callers pass the address; this
/// constant documents the deployment's default.
pub const DEFAULT_PORT: u16 = 4010;

/// `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` — the handshake opcode
/// (`MsgType.h:22`; the server port's `GameMsg::ResponseClientcharunique`,
/// `opcodes_game.rs:15`).
pub const RESPONSE_CLIENTCHARUNIQUE: u16 = 1;

/// The handshake body's exact length: `SI32 + UI08 + UI08 + SI16` = 8 bytes
/// (`MsgType.h:3013-3027`; the server port's `CHAR_UNIQUE_PAYLOAD_LEN`,
/// `game.rs:16`).
pub const CHAR_UNIQUE_PAYLOAD_LEN: usize = 8;

/// The negotiated handshake state — `cltGameMsgResponse_ClientCharUnique`
/// (`MsgType.h:3013-3027`) as it sits on the wire, plus nothing else: the
/// legacy stores these four values and derives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientCharUnique {
    /// `siCharUnique` — this connection's session character id, stored as
    /// `MyCharUnique` (`Client-SystemNPC.cpp:698`). Wire: payload 0..4,
    /// little-endian `i32`.
    pub char_unique: i32,
    /// `uiKey1` — outbound session-cipher key byte 1, fed to
    /// `SetKey(uiKey1, uiKey2)` (`Client-SystemNPC.cpp:703`). Wire:
    /// payload byte 4.
    pub key1: u8,
    /// `uiKey2` — outbound session-cipher key byte 2
    /// (`Client-SystemNPC.cpp:703`). Wire: payload byte 5.
    pub key2: u8,
    /// `siVersion` — the server's advertised version, stored raw: the
    /// legacy's `siVersion > siVersion` quit gate is product policy, not
    /// ported here (module docs, the handshake). Wire: payload 6..8,
    /// little-endian `i16`.
    pub version: i16,
}

impl ClientCharUnique {
    /// Parses a whole handshake frame: opcode first, then the body of
    /// exactly [`CHAR_UNIQUE_PAYLOAD_LEN`] bytes (module docs, "strict
    /// length").
    ///
    /// # Errors
    ///
    /// As [`parse_client_char_unique`].
    pub fn from_frame(frame: &Frame) -> Result<Self, GameError> {
        parse_client_char_unique(frame.cmd(), frame.payload())
    }
}

/// The byte-level handshake parser behind [`ClientCharUnique::from_frame`],
/// callable with raw `(cmd, payload)` for byte-exact testing.
///
/// Opcode **before** length: any command other than
/// [`RESPONSE_CLIENTCHARUNIQUE`] is refused without its payload ever being
/// measured. Then exactly [`CHAR_UNIQUE_PAYLOAD_LEN`] bytes — the layout in
/// the module docs, read as `i32` LE, `u8`, `u8`, `i16` LE
/// (`MsgType.h:3013-3027`; the server writes the mirror image,
/// `game.rs:98-110`).
///
/// # Errors
///
/// [`GameError::UnexpectedOpcode`] for a foreign command;
/// [`GameError::Truncated`] below [`CHAR_UNIQUE_PAYLOAD_LEN`] (the legacy
/// would read out of bounds at `Client-SystemNPC.cpp:695`);
/// [`GameError::TrailingBytes`] above it (the legacy would silently ignore
/// them).
pub fn parse_client_char_unique(cmd: u16, payload: &[u8]) -> Result<ClientCharUnique, GameError> {
    if cmd != RESPONSE_CLIENTCHARUNIQUE {
        return Err(GameError::UnexpectedOpcode { cmd });
    }
    match payload.len() {
        len if len < CHAR_UNIQUE_PAYLOAD_LEN => Err(GameError::Truncated {
            expected: CHAR_UNIQUE_PAYLOAD_LEN,
            got: len,
        }),
        len if len > CHAR_UNIQUE_PAYLOAD_LEN => Err(GameError::TrailingBytes {
            expected: CHAR_UNIQUE_PAYLOAD_LEN,
            got: len,
        }),
        _ => Ok(ClientCharUnique {
            char_unique: i32::from_le_bytes(payload[0..4].try_into().expect("4 bytes checked")),
            key1: payload[4],
            key2: payload[5],
            version: i16::from_le_bytes(payload[6..8].try_into().expect("2 bytes checked")),
        }),
    }
}

/// Why a game-link operation failed.
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// The bounded windows elapsed or the socket failed. A
    /// [`SessionError::ReadTimeout`] here is **not** a teardown — the link
    /// stays open, buffered, and keyed (module docs, F-02); the caller
    /// decides when to wait again.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// The decoder refused the stream — a sticky framing or integrity
    /// poison (`codec.rs`): the connection is finished, exactly as the
    /// legacy's decode failure returns out of the receive path
    /// (`Client-Winsock.cpp:872-890`).
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// The send failed; see [`SendError`] for whether bytes already reached
    /// the socket (and its keys already ticked).
    #[error(transparent)]
    Send(#[from] SendError),
    /// A frame other than [`RESPONSE_CLIENTCHARUNIQUE`] arrived where the
    /// handshake was expected — checked before the payload's length
    /// (module docs, "strict length").
    #[error("expected GAMEMSG_RESPONSE_CLIENTCHARUNIQUE (1), got opcode {cmd}")]
    UnexpectedOpcode {
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The handshake body was shorter than [`CHAR_UNIQUE_PAYLOAD_LEN`] —
    /// where the legacy would read out of bounds
    /// (`Client-SystemNPC.cpp:695`).
    #[error("CLIENTCHARUNIQUE payload truncated: expected exactly {expected} bytes, got {got}")]
    Truncated {
        /// Required length ([`CHAR_UNIQUE_PAYLOAD_LEN`]).
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// The handshake body ran past [`CHAR_UNIQUE_PAYLOAD_LEN`] — trailing
    /// bytes the legacy would silently ignore.
    #[error(
        "CLIENTCHARUNIQUE payload has trailing bytes: expected exactly {expected} bytes, got {got}"
    )]
    TrailingBytes {
        /// Required length ([`CHAR_UNIQUE_PAYLOAD_LEN`]).
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// [`GameLink::send`] before the handshake — the legacy
    /// `bGameServerSecretKeySwitch == false → FALSE` gate
    /// (`Client-Winsock.cpp:171`).
    #[error("nothing may be sent to the game server before the GAMEMSG_RESPONSE_CLIENTCHARUNIQUE handshake")]
    NotHandshaken,
}

/// One Game Server connection: bounded transport, an inbound decoder keyed
/// with [`Cipher::link_cipher`] since before the first byte
/// (`ITZSession.cpp:50-56`), and — after [`GameLink::handshake`] — an
/// outbound encoder keyed with the negotiated session seeds.
///
/// Not shared between tasks: it owns its [`Session`] and its per-direction
/// [`Cipher`] — one of each per connection, as everywhere else.
#[derive(Debug)]
pub struct GameLink {
    /// The TCP session — connect- and read-bounded, teardown-decoupled
    /// from timeouts (`net/session.rs`).
    session: Session,
    /// Inbound framing **and** the fixed link cipher — both live from the
    /// constructor, so no read can run ahead of the key.
    decoder: Decoder,
    /// Outbound framing — plaintext until [`GameLink::handshake`] installs
    /// the negotiated session cipher; the gate on [`GameLink::send`] keeps
    /// it from ever producing bytes before that.
    encoder: Encoder,
    /// `MyCharUnique` + `bGameServerSecretKeySwitch` in one slot
    /// (`Client-SystemNPC.cpp:698,706`): the handshake result, `None`
    /// while the send gate is closed.
    negotiated: Option<ClientCharUnique>,
}

impl GameLink {
    /// Dials `addr` with [`SessionTimeouts::default`] (5 s connect) and
    /// returns the link with [`Cipher::link_cipher()`] already installed in
    /// the inbound decoder.
    ///
    /// # Errors
    ///
    /// [`SessionError::ConnectTimeout`] or [`SessionError::Io`] from the
    /// dial, as [`Session::connect`].
    pub async fn connect(addr: SocketAddr) -> Result<Self, GameError> {
        Self::connect_with_timeouts(addr, SessionTimeouts::default()).await
    }

    /// [`GameLink::connect`] with explicit windows — e.g. a short read
    /// window in tests, or [`SessionTimeouts::legacy_connect`] for the
    /// legacy's 7 s dial.
    ///
    /// The decoder is constructed here, before any read is possible: the
    /// key precedes the byte (module docs, "immediately"). The outbound
    /// encoder starts unkeyed and the send gate closed; Step 4.2's
    /// [`GameLink::handshake`] opens both.
    ///
    /// # Errors
    ///
    /// As [`GameLink::connect`].
    pub async fn connect_with_timeouts(
        addr: SocketAddr,
        timeouts: SessionTimeouts,
    ) -> Result<Self, GameError> {
        Ok(Self {
            session: Session::connect_with_timeouts(addr, timeouts).await?,
            decoder: Decoder::with_cipher(Cipher::link_cipher()),
            encoder: Encoder::new(),
            negotiated: None,
        })
    }

    /// Consumes frames until [`RESPONSE_CLIENTCHARUNIQUE`] arrives, parses
    /// its eight bytes, and keys the outbound channel with
    /// `Encoder::with_cipher(Cipher::session_cipher(key1, key2))` — the
    /// Step 4.2 core, ending at the legacy's `SetKey(uiKey1, uiKey2)` +
    /// `bGameServerSecretKeySwitch = true` (`Client-SystemNPC.cpp:701-706`).
    ///
    /// Idempotent once negotiated: a second call returns the stored state
    /// **without touching the wire** (the `FrontLink::handshake`
    /// precedent). Frames buffered behind the handshake frame stay in the
    /// decoder for the next [`GameLink::next_frame`] — inbound remains on
    /// the link cipher throughout, because the server keeps encoding that
    /// way for the connection's life (`main.rs:680`).
    ///
    /// A successful completion (the first call only — not idempotent
    /// re-entry) emits the session-seed log line `client handshake
    /// succeeded: session_seed = (0xRR, 0xRR), char_unique = <id>` for
    /// connection verification (QC P-08; purpose and format in
    /// `CONTRIBUTING.md`, Step 4.2).
    ///
    /// # Errors
    ///
    /// [`GameError::UnexpectedOpcode`] for any first frame that is not
    /// opcode 1; [`GameError::Truncated`] / [`GameError::TrailingBytes`]
    /// for a body that is not exactly [`CHAR_UNIQUE_PAYLOAD_LEN`] bytes;
    /// [`GameError::Session`] on read timeout, socket error, or EOF before
    /// the frame arrives ([`io::ErrorKind::UnexpectedEof`]);
    /// [`GameError::Codec`] on a refused stream (sticky poison — the
    /// connection is finished). No failure keys anything: the encoder and
    /// the negotiated slot change only after a full parse succeeds.
    pub async fn handshake(&mut self) -> Result<ClientCharUnique, GameError> {
        if let Some(state) = self.negotiated {
            return Ok(state);
        }

        let frame = self.next_frame().await?.ok_or_else(|| {
            GameError::Session(SessionError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "game server closed before GAMEMSG_RESPONSE_CLIENTCHARUNIQUE",
            )))
        })?;
        let state = ClientCharUnique::from_frame(&frame)?;

        // SetKey on the session's GetEncrypt(): the two bytes are both the
        // outbound cipher's keys and its counter position from the first
        // frame on (Client-SystemNPC.cpp:701-703).
        self.encoder = Encoder::with_cipher(Cipher::session_cipher(state.key1, state.key2));
        self.negotiated = Some(state);
        // QC P-08: the real handshake-success path emits the session seed this
        // connection actually received, for connection verification and later
        // QC tracing. The pair prints exactly as the server console prints it
        // (`game: <peer>: charunique=<id> -> (0xRR, 0xRR)`), so the two lines
        // correlate directly. Purpose and format: `CONTRIBUTING.md`, Step 4.2
        // "Seed log". Emitted once per connection — the idempotent early
        // return above is not a completion.
        tracing::info!(
            "client handshake succeeded: session_seed = ({:#04x}, {:#04x}), char_unique = {}",
            state.key1,
            state.key2,
            state.char_unique
        );
        Ok(state)
    }

    /// Reads until the next complete inbound frame, deciphering it with the
    /// link cipher — the legacy's `GetFirstPacket` → `Decode` →
    /// `RemoveFirstPacket` (`Client-Winsock.cpp:866-916`).
    ///
    /// `Ok(None)` when the peer closes cleanly; buffered complete frames are
    /// handed out before EOF is observed.
    ///
    /// A [`SessionError::ReadTimeout`] is an ordinary answer here: nothing
    /// is torn down, the partial frame stays buffered, and the cipher
    /// counter stands still until a frame is actually pulled — the next
    /// call resumes exactly where this one stopped (module docs, F-02).
    ///
    /// # Errors
    ///
    /// [`GameError::Session`] on read timeout or socket error;
    /// [`GameError::Codec`] on a sticky framing/integrity refusal (the
    /// connection is finished).
    pub async fn next_frame(&mut self) -> Result<Option<Frame>, GameError> {
        loop {
            if let Some(frame) = self.decoder.next_frame()? {
                return Ok(Some(frame));
            }
            let mut buf = [0u8; 4096];
            let n = self.session.read(&mut buf).await?;
            if n == 0 {
                return Ok(None);
            }
            self.decoder.feed(&buf[..n])?;
        }
    }

    /// Encodes `cmd`/`payload` with the negotiated session cipher and
    /// flushes the wire bytes — the ciphered client→server half of the
    /// pairing, gated exactly like the legacy `SendMsgToServer`
    /// (`Client-Winsock.cpp:155-171`): the `bGameServerSecretKeySwitch`
    /// gate first (`:171`), then `GetEncrypt()->Encode` + write.
    ///
    /// # Errors
    ///
    /// [`GameError::NotHandshaken`] before [`GameLink::handshake`] —
    /// nothing reaches the socket and no key ticks (the gate refuses the
    /// call before the cipher is ever reached, as in the legacy); then the
    /// [`SendError`] cases — a refused payload sent nothing, a socket
    /// failure left the keys advanced for exactly the frame that was being
    /// sent.
    pub async fn send(&mut self, cmd: u16, payload: &[u8]) -> Result<(), GameError> {
        if self.negotiated.is_none() {
            return Err(GameError::NotHandshaken);
        }
        Ok(self.encoder.send(&mut self.session, cmd, payload).await?)
    }

    /// The negotiated handshake state — `char_unique`, the session seeds,
    /// and the advertised `version` — once [`GameLink::handshake`] has run;
    /// `None` while the send gate is closed.
    #[must_use]
    pub const fn negotiated(&self) -> Option<ClientCharUnique> {
        self.negotiated
    }

    /// The outbound cipher's current key position `(key1, key2)` — `None`
    /// until the handshake, then the negotiated pair ticking with every
    /// sent frame (the keys *are* the counter, `cipher.rs`), the mirror of
    /// [`GameLink::inbound_keys`].
    #[must_use]
    pub fn outbound_keys(&self) -> Option<(u8, u8)> {
        self.encoder.cipher().map(|c| (c.key1(), c.key2()))
    }

    /// The inbound cipher's current key position `(key1, key2)` — the
    /// diagnostic the legacy logs when a decode fails
    /// (`GetKey1()`/`GetKey2()`, `Client-Winsock.cpp:880-881`).
    ///
    /// Immediately after connect this is `Some((0x12, 0x34))`; each pulled
    /// frame advances it (the keys *are* the counter, `cipher.rs`).
    #[must_use]
    pub fn inbound_keys(&self) -> Option<(u8, u8)> {
        self.decoder.cipher().map(|c| (c.key1(), c.key2()))
    }
}

#[cfg(test)]
mod tests;
