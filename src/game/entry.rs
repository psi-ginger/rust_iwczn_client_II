//! Step 4.1 — Game Server connection with the inbound link cipher installed
//! from byte 0 (`CONTRIBUTING.md:76-77`).
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
//! | server → client (**inbound**, this step) | **link cipher**, fixed `0x12, 0x34`, active from byte 0 | keyed `m_pEncrypt2->SetKey(0x12, 0x34)` in `ITZSession::Init` (`ITZSession.cpp:50-56`), consumed by `GetEncrypt2()->Decode` (`Client-Winsock.cpp:872`); the server side pairs it identically (`services/game/src/main.rs:13,18`, citing `char.cpp:5177-5219`, `ITZSendBuffer.cpp:122-124`) |
//! | client → server (outbound) | session cipher seeded by opcode 1's two bytes — **not this step** | the gate starts closed: `bGameServerSecretKeySwitch = false` (`Client-Winsock.cpp:825`); the server states it from the other side — "the client cannot send anything until it has this" (`main.rs:678`). Step 4.2 installs it from `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE`; Step 4.3 sends |
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
//! # No send path in this step
//!
//! This module deliberately has no `send`: outbound bytes are impossible
//! until the session cipher arrives with opcode 1 (Step 4.2), and until then
//! the legacy's own gate refuses every game send. Absence here is the
//! invariant, not a stub.
//!
//! # Test addresses are synthetic (F-01)
//!
//! The suite runs on loopback with synthetic addresses — pending live
//! ingestion against the real Game Server (4010) (QC decision F-01/F-02 on
//! CAR-2026-10-24-RETRO01; the 5 s windows are revalidated there). What the
//! tests prove is byte arithmetic, cipher state, and timeout behaviour — not
//! reachability.

use crate::net::cipher::Cipher;
use crate::net::codec::{CodecError, Decoder, Frame};
use crate::net::session::{Session, SessionError, SessionTimeouts};
use std::net::SocketAddr;

/// The Game Server's port (`CONTRIBUTING.md` Step 4.1; the server port's
/// default listen `0.0.0.0:4010`, `services/game/src/config.rs:56`). The
/// legacy reads it from the selected WORLDINFO instead of a literal
/// (`Client-Winsock.cpp:805-806`), so callers pass the address; this
/// constant documents the deployment's default.
pub const DEFAULT_PORT: u16 = 4010;

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
}

/// One Game Server connection: bounded transport plus an inbound decoder
/// that has been keyed with [`Cipher::link_cipher()`] since before the
/// first byte (`ITZSession.cpp:50-56`).
///
/// Not shared between tasks: it owns its [`Session`] and its one inbound
/// [`Cipher`] — per direction per connection, as everywhere else.
/// Outbound does not exist yet (module docs, no send path).
#[derive(Debug)]
pub struct GameLink {
    /// The TCP session — connect- and read-bounded, teardown-decoupled
    /// from timeouts (`net/session.rs`).
    session: Session,
    /// Inbound framing **and** the fixed link cipher — both live from the
    /// constructor, so no read can run ahead of the key.
    decoder: Decoder,
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
    /// key precedes the byte (module docs, "immediately").
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
        })
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
