//! Front Server connection (port 5700) and the `SECRETKEY` handshake — Step
//! 3.1.
//!
//! The client's first hop: dial the Front Server, receive the unsolicited
//! `FRONTMSG_RESPONSE_SECRETKEY` (30000) frame it sends on accept, extract
//! the two seed bytes, and key the **outbound** session cipher with them.
//! After that, [`FrontLink::send`] is the ciphered client→server channel the
//! rest of MOD-03 talks over (login, world list, notice).
//!
//! Legacy source: the server writes the frame in `cClient::Init`
//! (`FrontServer.h:95-118`) the moment the connection is accepted; the client
//! handles it in `cltClient::FrontServerOperation`
//! (`Client-Winsock.cpp:222-250`).
//!
//! # The wire on this link is asymmetric
//!
//! | Direction | Cipher | Source |
//! |---|---|---|
//! | server → client | **plaintext**, always | the legacy front's session factory uses the default `bEncrypt = false` (`FrontClientProtocol.cpp:21`, `ITZSessionFactory.h:15`), so `GetSendBuffer()->Write` never encodes; the client's receive path confirms it — `GetFirstPacket` → handler → `RemoveFirstPacket` with no `Decode` (`Client-Winsock.cpp:775-784`) |
//! | client → server | **ciphered** from the first keyed send | the server decodes every inbound frame with the session cipher (`GetEncrypt()->Decode`, `FrontClientProtocol.cpp:85`; the `FRONTMSG_REQUEST_PINGTEST` plaintext exception is server-side only and the game client never sends it), keyed by the very seeds this handshake extracts |
//!
//! The server port (`psi-ginger/rust_Iwcgz_server`, `services/front`) states
//! the same asymmetry from the other side: it encodes no outbound frame and
//! decodes every inbound one with `Cipher::session_cipher(key1, key2)`.
//!
//! # The handshake frame
//!
//! Sent **unsolicited** on accept, in the clear:
//!
//! | Bytes | Field | Value |
//! |---|---|---|
//! | 0..2 | `usSize` | 10 (`sizeof(sResponse_SecretKey)`, `FrontMsg.h:132-141`) |
//! | 2 | `ucChk` | written uninitialized by the legacy server; `0` by the server port |
//! | 3..5 | `usCmd` | 30000 (`FRONTMSG_RESPONSE_SECRETKEY`, `FrontMsg.h:114`) |
//! | payload\[0\] | `ucTemp1` | indeterminate — never read by the client |
//! | payload\[1\] | `ucTemp2` | indeterminate — never read by the client |
//! | **payload\[2\]** | **`ucKey1`** | first session-cipher seed |
//! | payload\[3\] | `ucTemp3` | indeterminate — never read by the client |
//! | **payload\[4\]** | **`ucKey2`** | second session-cipher seed |
//!
//! The client handler does `GetEncrypt()->SetKey(ucKey1, ucKey2)` on the
//! **front session's session cipher** (`Client-Winsock.cpp:226-228`) — the
//! client→server instance (`ITZSession::GetEncrypt`, `cipher.rs`'s
//! [`Cipher::session_cipher`] — its `set_key` also resets the counter
//! position, so the keys start their per-call ticking from the seeds
//! themselves). Here that is [`Encoder::set_cipher`] with
//! `Cipher::session_cipher(key1, key2)`, installing the outbound half before
//! the first keyed send.
//!
//! # Nothing may be sent before the key arrives
//!
//! The legacy refuses every front send while `bFrontServerSecretKeySwitch` is
//! false — the gate runs **before** the cipher is reached
//! (`Client-Winsock.cpp:141`, flag set at `:247`; `LogIn.cpp:310` notes the
//! same rule). [`FrontLink::send`] reproduces that gate as
//! [`FrontError::NotKeyed`]. The receive side has no gate: inbound frames are
//! plaintext whether or not the key has arrived.
//!
//! # Deliberate divergences from the legacy
//!
//! - **The first frame must be 30000.** The legacy handler is an event loop:
//!   it tolerates any opcode arriving in any order and reacts to each. This
//!   port sequences connect → handshake → keyed sends, so
//!   [`FrontLink::handshake`] refuses any other first opcode
//!   ([`FrontError::UnexpectedOpcode`]). Both known servers send 30000 first
//!   — the legacy front writes it in `Init`, before its first `SessionProc`
//!   tick can queue the world-refresh notify (`FrontServer.h:95-118`,
//!   `FrontClientProtocol.cpp:41-60`), and the server port sends it before
//!   entering its read loop — so the strictness costs nothing and catches
//!   protocol drift immediately. Frames the handshake does not consume stay
//!   buffered for the caller's next [`FrontLink::next_frame`].
//! - **A short payload is an error, not an out-of-bounds read.** The legacy
//!   casts blindly and reads `ucKey1`/`ucKey2` at fixed offsets; this port
//!   refuses a payload under 5 bytes ([`FrontError::ShortSecretKeyPayload`]),
//!   the same bounds-check-instead-of-UB deviation the cipher layer takes
//!   (`cipher.rs`, deviations).
//! - **A second 30000 is ignored, not re-keyed.** The legacy would `SetKey`
//!   again, resetting the counter mid-link; [`FrontLink::handshake`] returns
//!   the already-installed key instead. No server sends a second one, so the
//!   difference is unobservable on the wire.

use std::io;

use crate::net::cipher::Cipher;
use crate::net::codec::{CodecError, Decoder, Encoder, Frame, SendError};
use crate::net::session::{Session, SessionError, SessionTimeouts};
use std::net::SocketAddr;

/// The Front Server's port (`CONTRIBUTING.md` Step 3.1; the legacy client
/// dials 5700 the same way).
pub const DEFAULT_PORT: u16 = 5700;

/// `FRONTMSG_RESPONSE_SECRETKEY` (`FrontMsg.h:114`), the handshake opcode.
pub const RESPONSE_SECRETKEY: u16 = 30000;

/// `sizeof(sResponse_SecretKey) - sizeof(sPacketHeader)` (`FrontMsg.h:132-141`):
/// five payload bytes, of which only offsets 2 and 4 are ever read.
pub const SECRET_KEY_PAYLOAD_LEN: usize = 5;

/// The two seed bytes the Front Server hands out in
/// [`RESPONSE_SECRETKEY`].
///
/// They become the client→server session cipher's initial keys — the counter
/// position, not just the key material (`cipher.rs`: the keys *are* the
/// counter), so the same pair must be installed before the first keyed send
/// and never disturbed afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretKey {
    key1: u8,
    key2: u8,
}

impl SecretKey {
    /// The two seeds as delivered.
    #[must_use]
    pub const fn new(key1: u8, key2: u8) -> Self {
        Self { key1, key2 }
    }

    /// `ucKey1` — `payload[2]` (`FrontMsg.h:138`).
    #[must_use]
    pub const fn key1(&self) -> u8 {
        self.key1
    }

    /// `ucKey2` — `payload[4]` (`FrontMsg.h:140`).
    #[must_use]
    pub const fn key2(&self) -> u8 {
        self.key2
    }

    /// Reads the seeds from a [`RESPONSE_SECRETKEY`] **payload** at the
    /// legacy struct's offsets (`sResponse_SecretKey`: `ucTemp1 ucTemp2
    /// ucKey1 ucTemp3 ucKey2`).
    ///
    /// The `ucTemp*` slots are never read, here or in the legacy client —
    /// the legacy server leaves them uninitialized (`FrontServer.h:108-117`
    /// assigns only the two key bytes) and the server port zeroes them, so
    /// both wirings parse identically.
    ///
    /// # Errors
    ///
    /// [`FrontError::ShortSecretKeyPayload`] when the payload is shorter
    /// than [`SECRET_KEY_PAYLOAD_LEN`] — the legacy would index out of
    /// bounds instead.
    pub fn from_payload(payload: &[u8]) -> Result<Self, FrontError> {
        let (Some(&key1), Some(&key2)) = (payload.get(2), payload.get(4)) else {
            return Err(FrontError::ShortSecretKeyPayload { len: payload.len() });
        };
        Ok(Self { key1, key2 })
    }
}

/// Why a front-link operation failed.
#[derive(Debug, thiserror::Error)]
pub enum FrontError {
    /// The bounded session windows elapsed, the socket failed, or the peer
    /// closed (EOF mid-handshake is [`io::ErrorKind::UnexpectedEof`]).
    #[error(transparent)]
    Session(#[from] SessionError),
    /// The receive decoder refused the stream (sticky framing or integrity
    /// poison — the connection must go, `codec.rs`).
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// The send failed; see [`SendError`] for whether bytes already reached
    /// the socket (and its keys already ticked).
    #[error(transparent)]
    Send(#[from] SendError),
    /// The first frame off the wire was not [`RESPONSE_SECRETKEY`] —
    /// protocol drift, or a server this port's handshake contract does not
    /// match (module docs, divergences).
    #[error("front sent opcode {cmd} before FRONTMSG_RESPONSE_SECRETKEY (30000)")]
    UnexpectedOpcode {
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The handshake frame's payload cannot hold `ucKey1`/`ucKey2` at
    /// offsets 2/4.
    #[error("SECRETKEY payload of {len} bytes is too short to hold ucKey1/ucKey2 at offsets 2/4")]
    ShortSecretKeyPayload {
        /// Payload length actually received.
        len: usize,
    },
    /// [`FrontLink::send`] before the handshake completed — the legacy
    /// `bFrontServerSecretKeySwitch` gate (`Client-Winsock.cpp:141`).
    #[error("nothing may be sent to the front server before the SECRETKEY handshake")]
    NotKeyed,
}

/// One Front Server connection in the Step 3.1 shape: plaintext inbound
/// forever, outbound keyed by [`RESPONSE_SECRETKEY`]'s seeds.
///
/// Not shared between tasks: it owns its [`Session`] and its per-direction
/// cipher, one connection each — the same per-connection rule the codec and
/// cipher layers document.
#[derive(Debug)]
pub struct FrontLink {
    session: Session,
    /// Inbound framing — plaintext for the link's life (the asymmetry table
    /// above); there is deliberately no way to key it.
    decoder: Decoder,
    /// Outbound framing — gains the session cipher when the seeds arrive.
    encoder: Encoder,
    /// `bFrontServerSecretKeySwitch` (`Client-Winsock.cpp:247`): the send
    /// gate, plus the installed seeds for callers and diagnostics.
    key: Option<SecretKey>,
}

impl FrontLink {
    /// Dials `addr` with [`SessionTimeouts::default`] (5 s connect).
    ///
    /// # Errors
    ///
    /// [`SessionError::ConnectTimeout`] or [`SessionError::Io`] from the
    /// dial, as [`Session::connect`].
    pub async fn connect(addr: SocketAddr) -> Result<Self, FrontError> {
        Self::connect_with_timeouts(addr, SessionTimeouts::default()).await
    }

    /// [`FrontLink::connect`] with explicit windows — e.g.
    /// [`SessionTimeouts::legacy_connect`] for the legacy's 7 s dial.
    ///
    /// # Errors
    ///
    /// As [`FrontLink::connect`].
    pub async fn connect_with_timeouts(
        addr: SocketAddr,
        timeouts: SessionTimeouts,
    ) -> Result<Self, FrontError> {
        Ok(Self {
            session: Session::connect_with_timeouts(addr, timeouts).await?,
            decoder: Decoder::new(),
            encoder: Encoder::new(),
            key: None,
        })
    }

    /// Consumes frames until the [`RESPONSE_SECRETKEY`] handshake arrives,
    /// then keys the outbound session cipher with its seeds and returns them
    /// — the Step 3.1 core, ending at the legacy's
    /// `SetKey(ucKey1, ucKey2)` (`Client-Winsock.cpp:228`).
    ///
    /// Idempotent once keyed: a second call returns the installed seeds
    /// without touching the wire (module docs, divergences). Frames other
    /// than 30000 are refused ([`FrontError::UnexpectedOpcode`]); anything
    /// buffered behind the handshake frame stays in the decoder for the
    /// next [`FrontLink::next_frame`].
    ///
    /// # Errors
    ///
    /// [`FrontError::UnexpectedOpcode`] for any first frame that is not
    /// 30000; [`FrontError::ShortSecretKeyPayload`] for a malformed
    /// handshake frame; [`FrontError::Session`] on read timeout, socket
    /// error, or EOF before the frame arrives
    /// ([`io::ErrorKind::UnexpectedEof`]); [`FrontError::Codec`] on a
    /// refused stream (sticky poison — the connection is finished).
    pub async fn handshake(&mut self) -> Result<SecretKey, FrontError> {
        if let Some(key) = self.key {
            return Ok(key);
        }

        let frame = self.next_frame().await?.ok_or_else(|| {
            FrontError::Session(SessionError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "front closed before FRONTMSG_RESPONSE_SECRETKEY",
            )))
        })?;

        if frame.cmd() != RESPONSE_SECRETKEY {
            return Err(FrontError::UnexpectedOpcode { cmd: frame.cmd() });
        }

        let key = SecretKey::from_payload(frame.payload())?;
        // The legacy's SetKey on the front session's GetEncrypt(): the seeds
        // become the outbound cipher's initial keys AND counter position.
        self.encoder
            .set_cipher(Cipher::session_cipher(key.key1, key.key2));
        self.key = Some(key);
        Ok(key)
    }

    /// Reads the next complete frame — plaintext, inbound, always
    /// ([`Decoder`] is never keyed on this link).
    ///
    /// `Ok(None)` when the peer closes the connection cleanly; buffered
    /// complete frames are handed out before EOF is observed.
    ///
    /// # Errors
    ///
    /// [`FrontError::Session`] on read timeout or socket error;
    /// [`FrontError::Codec`] on a sticky framing/integrity refusal.
    pub async fn next_frame(&mut self) -> Result<Option<Frame>, FrontError> {
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

    /// Encodes `cmd`/`payload` with the keyed session cipher and flushes the
    /// wire bytes — the ciphered client→server half of the asymmetry.
    ///
    /// The legacy's `SendFrontServerMsg` (`Client-Winsock.cpp:139-150`):
    /// gate first, then `GetEncrypt()->Encode`, then write. The gate runs
    /// here; the encode+write is [`Encoder::send`] (cipher ticked at encode
    /// time, before the flush).
    ///
    /// # Errors
    ///
    /// [`FrontError::NotKeyed`] before the handshake (nothing reaches the
    /// socket, no key ticks); then the [`SendError`] cases — a refused
    /// payload sent nothing, a socket failure left the keys advanced for
    /// exactly the frame that was being sent.
    pub async fn send(&mut self, cmd: u16, payload: &[u8]) -> Result<(), FrontError> {
        if self.key.is_none() {
            return Err(FrontError::NotKeyed);
        }
        Ok(self.encoder.send(&mut self.session, cmd, payload).await?)
    }

    /// Whether the [`RESPONSE_SECRETKEY`] handshake has keyed this link —
    /// the `bFrontServerSecretKeySwitch` flag as a query.
    #[must_use]
    pub const fn is_keyed(&self) -> bool {
        self.key.is_some()
    }

    /// The installed seeds, once the handshake has run.
    #[must_use]
    pub const fn key(&self) -> Option<SecretKey> {
        self.key
    }
}

#[cfg(test)]
mod tests;
