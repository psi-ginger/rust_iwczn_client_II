//! Framed packet codec: byte stream ⇄ complete frames — decoder (Step 2.2),
//! encoder (Step 2.3).
//!
//! The inbound half is the framing layer the WBS describes as "stream →
//! packets": feed it whatever [`session::Session::read`] handed you, pull whole
//! frames out with [`Decoder::next_frame`]. A frame split across any number of
//! chunks and several frames sharing one chunk are both handled — the legacy
//! has the same property, it just does it with a ring buffer and a
//! `PACKET_MAX_SIZE`-byte slack zone (`ITZRecvBuffer::CreateRecvBuffer`,
//! `ITZRecvBuffer.cpp:56-58`) instead of a deque.
//!
//! The outbound half, [`Encoder`] (Step 2.3), is the WBS's "packets → wire":
//! take an opcode and a payload, prepend the 5-byte header, apply the outbound
//! cipher if one is active, flush to the socket.
//!
//! It composes the primitives already landed:
//! - [`crate::protocol::header`] (Step 1.2): the 5-byte header layout and the
//!   one guard table (`validate_size`) — this module never re-derives size rules.
//! - [`crate::net::cipher`] (Step 1.3): the inbound **and outbound** ciphers,
//!   each applied **if active**.
//! - [`crate::net::session`] (Step 2.1): the transport chunks come off and
//!   frames are flushed to.
//!
//! # Rejection is sticky: a refusal is a disconnect, not a dropped packet
//!
//! `ITZRecvBuffer::Completion` returns `BOOL`; the IOCP worker turns `FALSE`
//! straight into `SESSION_STATE_DISCONNECTED` (`ITZNetIOCP.cpp:145-150`), and
//! the caller stops feeding the buffer — the session no longer exists. So when
//! [`Decoder::feed`] or [`Decoder::next_frame`] rejects a header, the decoder
//! stays rejected: every later call returns the original error instead of
//! resuming on the next plausible-looking byte. Ignoring the error is safe
//! because the intended reaction is always to drop the connection. [`Decoder::clear`]
//! discards buffered bytes (as `ITZRecvBuffer::ClearBuffer` does) but, like the
//! session it kills, does not resurrect a refusal.
//!
//! Headers are validated **as soon as their 2 size bytes have arrived**, not
//! when the whole frame has (`ITZRecvBuffer.cpp:155-180` reads `usPacketSize`,
//! runs the guards, and only then asks whether the body is in). That is what
//! makes a refusal a *whole-buffer* event: a legal frame ahead of a bad header
//! in the same read is never delivered — the legacy's `Completion` refuses the
//! buffer before committing any packet count, and the recorded fixture case
//! `good_then_over_limit` pins exactly that behaviour.
//!
//! # The cipher is applied at pull time, exactly once per frame
//!
//! The legacy framing layer never decrypts: `usSize` sits in the clear, so
//! `Completion` can cut frames before any cipher runs
//! (`ITZRecvBuffer.cpp:155`). The consumer decrypts one packet at a time, at
//! consumption — `GetFirstPacket` → `GetEncrypt2()->Decode(...)` → on `false`
//! `return false` (teardown) → `RemoveFirstPacket` only after success
//! (`Client-Winsock.cpp:866-916`; the front link does the same and calls
//! `CloseSocket()` on a failed `Decode`, `FrontClientProtocol.cpp:85-91`).
//!
//! [`Decoder::next_frame`] is that consumption point: it pulls the raw frame,
//! runs **one** [`Cipher::decode`] call, and only then hands out plaintext.
//! Two consequences of `Cipher`'s key-tick invariant (Step 1.3):
//! - **Exactly one key tick per pulled frame**, in frame order. A frame that
//!   was validated by `feed` but never pulled (connection died first) never
//!   ticks — matching the legacy, where a refused `Completion` means no
//!   `Decode` ever runs on the buffered bytes.
//! - A guard refusal never ticks either: the cipher lives at the pull site,
//!   after the framing guards.
//!
//! [`Decoder::new`] runs with no cipher (the front link's inbound side is
//! plaintext — Step 3.1); [`Decoder::with_cipher`] activates one (the game
//! link's fixed `0x12,0x34` link cipher). There is deliberately no way to
//! install a cipher mid-stream: no legacy link ever switches its inbound side
//! after establishment — unlike the *outbound* side, which gains its key
//! partway through the connection (see [`Encoder`] below).
//!
//! # The outbound half: packets → wire (Step 2.3)
//!
//! The legacy send path, in order (`SendFrontServerMsg`,
//! `Client-Winsock.cpp:139-150`; `SendMsgToServer`, `:155-211`):
//!
//! 1. The caller fills an `sPacketHeader`-headed message (`usSize`, `usCmd`),
//!    optionally LZF-compresses the body itself and sets `usCmd |= 0x8000`
//!    (`:178-196`). Compression therefore sits **above** this codec:
//!    [`Encoder`] passes `cmd` through untouched, flag bits included.
//! 2. `GetEncrypt()->Encode(frame, cSecretBuffer)` (`:146,195,200,242`) —
//!    [`Cipher::encode`] over the whole frame: keys ticked first, `usSize`
//!    read from the header, kept in the clear, byte 2 overwritten with the
//!    resync tag (`ITZEncrypt.cpp:30-67`).
//! 3. `GetSendBuffer()->Write(cSecretBuffer)` (`:147,203,243`) — on the
//!    client this is a plain ring copy: the factory is built with the default
//!    `bEncrypt = false` (`:92`, `ITZSessionFactory.h:15`), so `Write`'s own
//!    cipher branch (`ITZSendBuffer.cpp:122-126`) never runs. **The bytes
//!    `Encode` produced are the bytes on the wire.**
//! 4. The IOCP drains the ring with `send` (`GetSendParam` → `Completion`,
//!    `ITZSendBuffer.cpp:67-112`).
//!
//! [`Encoder::encode`] is steps 1–2; [`Encoder::send`] replaces steps 3–4
//! with one awaited `write_all` over [`Session::stream`] — the legacy's ring
//! exists only to feed that overlapped send loop, so a port with none needs
//! no equivalent. Whether a link ciphers at all is the **caller's** gate in
//! the legacy: `bFrontServerSecretKeySwitch`/`bGameServerSecretKeySwitch`
//! refuse the whole call before the cipher is reached
//! (`Client-Winsock.cpp:141,171`; `LogIn.cpp:310` notes nothing may be sent
//! to the front server before the key arrives) — that policy belongs to Steps
//! 3.1/4.2, not to framing.
//!
//! # No ring: no second ceiling (BUG-014)
//!
//! The legacy's per-link receive ring is a *second* limit its size guard never
//! sees: `GetRecvParam` wraps `m_pLastPtr` back to `m_pBasePtr` when the ring
//! is full (`ITZRecvBuffer.cpp:78-89`), overwriting a half-received frame. The
//! guard allows up to [`MAX_FRAME_SIZE`] (64992) while rings were built smaller
//! on some links — the game server's client-facing factories pass 30000/50000
//! (`GoonZuWorld/Server/GameProtocol/GameProtocol.cpp:25,27`), so a frame
//! larger than the ring corrupts there (recorded as BUG-014). The client's own
//! game link allocates 300000 (`Client-Winsock.cpp:92`), comfortably above the
//! guard.
//!
//! A deque-backed buffer has no ring, so it cannot inherit that corruption and
//! needs no slack-zone linearisation either (`ITZRecvBuffer::GetFirstPacket`,
//! `ITZRecvBuffer.cpp:250-257` exists only to copy a wrapped frame out).
//! Buffering is bounded by [`MAX_FRAME_SIZE`] *provided the caller drains
//! frames*; a caller that only ever `feed`s grows without limit, where the
//! legacy refused once its ring filled.
//!
//! # Deliberate divergences from the legacy
//!
//! - **BUG-013 (2..=4)**: the legacy accepts a declared size of 2..=4 and hands
//!   out a "packet" with no room for `usCmd`; [`validate_size`] refuses it
//!   (Step 1.2's decision, parity with `gz-net`).
//! - **BUG-014**: frames larger than a legacy ring are reassembled faithfully
//!   here instead of being corrupted (see above).
//! - **The reserved byte on a deciphered frame is 0**: `Cipher::decode` never
//!   writes `dest[2]` (BUG-012), so a ciphered link's [`Frame::integrity`] is
//!   this port's initialised value rather than whatever stale bytes the
//!   legacy's `m_encRecvBuf` happened to hold. No legacy consumer reads it.
//! - **A refused frame is dropped with the connection**: the legacy left the
//!   frame in its buffer (it returns before `RemoveFirstPacket`), but tears the
//!   session down immediately — `_closesocket`/`SetSocket` wipe the buffer via
//!   `ClearBuffer` (`ITZSession.cpp:305,181`) — so the difference is never
//!   observable, and this decoder is poisoned past that point anyway.
//! - **`headerType == 1`** (byte-swapped size + 8) has no call site in the tree
//!   and is not implemented; see `docs/rca/2026-10-23-step-1.2-header-spec-mismatch.md`.
//! - **An over-limit payload is refused before the cipher ticks** (encoder):
//!   [`Encoder::encode`] returns [`HeaderError::PayloadTooLarge`] with the
//!   keys untouched. The legacy had no encode-time ceiling at all — but no
//!   legacy client send could have reached one: its messages top out near
//!   `sizeof(cltMsg)` (`MAX_CLTMSG_DATA_SIZE` is 11000,
//!   `GoonZuWorld/CommonLogic/MsgType.h:2943`), far under the 64992 that the
//!   peer's receive guard disconnects on (`ITZRecvBuffer.cpp:172`). The port
//!   guards anyway instead of emitting a frame a peer would kill the
//!   connection for.
//! - **A full local buffer backpressures instead of dropping** (encoder):
//!   the legacy's fixed send ring refused `Write` once full
//!   (`ITZSendBuffer.cpp:141`) and the client ignored the `FALSE`
//!   (`Client-Winsock.cpp:147,203`) — the frame was silently lost. With no
//!   ring, [`Encoder::send`] awaits `write_all`; the bytes wait in the
//!   socket, never on a buffer that can overflow.
//!
//! # Thread safety
//!
//! Not shared. The legacy relied on the enclosing session's critical section
//! (and, on `_CHINA` builds, an extra send critical section,
//! `Client-Winsock.cpp:159`) for the same reason, and [`Cipher`] is not
//! `Sync` either. Give each connection exactly one [`Decoder`] and one
//! [`Encoder`]: the inbound and the outbound direction, never crossed.
//!
//! [`session::Session::read`]: crate::net::session::Session::read
//! [`Session::stream`]: crate::net::session::Session::stream
//! [`Cipher`]: crate::net::cipher::Cipher
//! [`Cipher::encode`]: crate::net::cipher::Cipher::encode
//! [`Cipher::decode`]: crate::net::cipher::Cipher::decode
//! [`MAX_FRAME_SIZE`]: crate::protocol::header::MAX_FRAME_SIZE

use bytes::BytesMut;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use super::cipher::{Cipher, CipherError};
use super::session::Session;
use crate::protocol::header::{validate_size, HeaderError, WireHeader, HEADER_SIZE};

/// Why a frame could not be decoded — or built for sending.
///
/// From [`Decoder`] every variant means **kill the connection**, not "drop
/// this packet" — the legacy turns both refusal paths into teardown: a bad
/// header is `Completion() == FALSE` → `SESSION_STATE_DISCONNECTED`
/// (`ITZNetIOCP.cpp:145-150`), a failed cipher check is `Decode() == false` →
/// `return false`/`CloseSocket()` (`Client-Winsock.cpp:889`,
/// `FrontClientProtocol.cpp:90`). Hence [`Decoder`]'s sticky failure state.
///
/// From [`Encoder::encode`] the reachable variant is [`CodecError::Header`]
/// (`PayloadTooLarge`): no frame was built, no byte was sent, and no key
/// ticked — the connection is fine, only the message is (the legacy had no
/// encode-time refusal at all; see the module docs' divergences).
/// [`CodecError::Integrity`] is decode-only; [`CodecError::Cipher`] is kept
/// for the encoder on the same unreachability argument its variant documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CodecError {
    /// A declared size failed the guard table — see [`HeaderError`] for the
    /// three sub-variants (below 2, 2..=4, above the receive limit).
    #[error(transparent)]
    Header(#[from] HeaderError),
    /// The inbound cipher could not process a frame that already passed the
    /// guard table.
    ///
    /// Unreachable by construction: framing hands `Cipher::decode` exactly the
    /// `declared` bytes it validated, so the buffer agrees with `usSize`
    /// (`LengthMismatch`/`DestTooSmall`/`Truncated` cannot fire) and
    /// `declared <= 64992` is below the cipher's own `PACKET_MAX_SIZE` ceiling
    /// (`TooLarge` cannot fire). Kept so a future guard-table change cannot
    /// silently turn a cipher refusal into a panic.
    #[error(transparent)]
    Cipher(#[from] CipherError),
    /// The cipher's integrity byte disagreed with the decoded plaintext —
    /// the legacy's `Decode(...) == FALSE` (`ITZEncrypt.cpp:111`).
    ///
    /// The fields reproduce the legacy's own failure log
    /// (`Client-Winsock.cpp:878-884`, `_DEBUG`): `size` is `usSize` (in the
    /// clear), `cmd` is `usCmd` **as it appeared on the wire** — i.e.
    /// ciphertext on an encrypted link, exactly what the legacy logged — and
    /// `key1`/`key2` are the key position *after* the failed call's tick
    /// (`Decode` advances both keys on entry, `ITZEncrypt.cpp:72-73`, so the
    /// legacy's trailing `GetKey1`/`GetKey2` read the same post-tick values).
    #[error(
        "inbound frame failed the cipher integrity check: \
         size {size}, wire cmd {cmd}, keys {key1:#04x}/{key2:#04x}"
    )]
    Integrity {
        /// `usSize` from the refused frame's header (clear on the wire).
        size: u16,
        /// `usCmd` from the refused frame's header, **raw wire bytes**.
        cmd: u16,
        /// Key 1 after the failed decode's tick.
        key1: u8,
        /// Key 2 after the failed decode's tick.
        key2: u8,
    },
}

/// Why a frame could not be flushed to the socket ([`Encoder::send`]).
///
/// Deliberately separate from [`CodecError`]: a flush fails the way I/O
/// fails, and `io::Error` is neither `Copy` nor `Eq`, while [`CodecError`]
/// must stay `Copy` (Step 2.2 stores a refusal by value to keep it sticky for
/// the connection's life).
#[derive(Debug, Error)]
pub enum SendError {
    /// The frame could not be built (payload past the guard ceiling) — sent
    /// nothing, so no byte reached the socket.
    #[error(transparent)]
    Encode(#[from] CodecError),
    /// The socket write failed **after** the frame was encoded: the outbound
    /// keys have already ticked once for it (see [`Encoder::send`]), exactly
    /// as the legacy ticked while filling its send buffer, before and
    /// independently of the socket write completing.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// One complete, validated packet: header bytes and payload exactly as they
/// came off the wire (plaintext links) or as [`Cipher::decode`] produced them
/// (ciphered links).
///
/// Fields are derived from `bytes` rather than stored alongside them, so there
/// is no way to build a `Frame` whose `size` disagrees with its own length —
/// the class of bug Step 1.2 exists to rule out (BUG-012/BUG-013). The
/// constructor is private to this module and only runs on bytes that passed
/// [`validate_size`] (or, later, on bytes built by this codec's encoder), so
/// `bytes.len() >= HEADER_SIZE` always holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    bytes: Vec<u8>,
}

impl Frame {
    /// Wraps bytes already validated against the guard table.
    fn from_validated_bytes(bytes: Vec<u8>) -> Self {
        debug_assert!(
            bytes.len() >= HEADER_SIZE,
            "a validated frame can never be shorter than its header"
        );
        Self { bytes }
    }

    /// `usSize` read straight out of the header: total frame length, header
    /// included. Always equals [`Frame::wire_len`].
    #[must_use]
    pub fn size(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }

    /// Offset 2, the legacy's `ucChk`.
    ///
    /// Deliberately **not** named `chk`: it is not a checksum (Step 1.2). On a
    /// plaintext link it is carried through untouched; on a ciphered link it is
    /// the cipher's resync tag on the wire, and `0` in a decoded frame because
    /// `Cipher::decode` never writes `dest[2]` (BUG-012). Do not build
    /// integrity on it.
    #[must_use]
    pub fn integrity(&self) -> u8 {
        self.bytes[2]
    }

    /// `usCmd`: the opcode, including bit 15 when the LZF compression flag is
    /// set — the legacy clears that bit only *after* decompressing
    /// (`GameProtocol.cpp:141`), so this decoder never masks it. See
    /// [`Frame::cmd_id`].
    ///
    /// Only meaningful once the frame has been deciphered, if the link is
    /// encrypted (Step 1.3 enciphers everything from offset 3 onwards).
    #[must_use]
    pub fn cmd(&self) -> u16 {
        u16::from_le_bytes([self.bytes[3], self.bytes[4]])
    }

    /// Bit 15 of [`Frame::cmd`]: the LZF compression flag.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.cmd() & crate::protocol::header::COMPRESSION_FLAG != 0
    }

    /// The opcode with the compression flag cleared — what dispatch tables
    /// expect, valid once the payload has been decompressed.
    #[must_use]
    pub fn cmd_id(&self) -> u16 {
        self.cmd() & !crate::protocol::header::COMPRESSION_FLAG
    }

    /// Bytes after the 5-byte header.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.bytes[HEADER_SIZE..]
    }

    /// The whole frame, header included.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Frame length in bytes, header included (equals [`Frame::size`]).
    #[must_use]
    pub fn wire_len(&self) -> usize {
        self.bytes.len()
    }

    /// Consumes the frame and returns its bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Stream → frame conversion.
///
/// Feed it whatever the socket handed you with [`Decoder::feed`]; pull whole
/// frames out with [`Decoder::next_frame`]. One per connection, inbound
/// direction only. See the module docs for the legacy citations behind every
/// decision.
#[derive(Debug)]
pub struct Decoder {
    /// Bytes received but not yet emitted as a frame.
    buf: BytesMut,
    /// The inbound cipher, when the link encrypts server → client.
    cipher: Option<Cipher>,
    /// The refusal that permanently disabled this decoder, if any.
    failure: Option<CodecError>,
}

impl Decoder {
    /// A plaintext decoder: no inbound cipher, as on the front link
    /// (Step 3.1: inbound from the front server is plaintext).
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: BytesMut::new(),
            cipher: None,
            failure: None,
        }
    }

    /// A decoder that decipheres every pulled frame with `cipher` — e.g. the
    /// game link's fixed `Cipher::link_cipher()` (`GetEncrypt2`, keyed
    /// `0x12,0x34` by `ITZSession::Init`).
    ///
    /// The decoder owns the inbound cipher instance: exactly one `Cipher` per
    /// direction per connection (Step 1.3's invariant), applied exactly once
    /// per pulled frame.
    #[must_use]
    pub fn with_cipher(cipher: Cipher) -> Self {
        Self {
            cipher: Some(cipher),
            ..Self::new()
        }
    }

    /// The inbound cipher, for diagnostics — the equivalent of the legacy's
    /// `GetKey1()`/`GetKey2()` failure log (`Client-Winsock.cpp:880-881`),
    /// which reads the key position at the moment a decode fails.
    #[must_use]
    pub fn cipher(&self) -> Option<&Cipher> {
        self.cipher.as_ref()
    }

    /// Adds received bytes and validates every header that is now readable —
    /// the framing half of `ITZRecvBuffer::Completion` (`ITZRecvBuffer.cpp:112-225`).
    ///
    /// A header is checked as soon as its 2 size bytes are in hand, not when
    /// the whole frame has, so a bad header anywhere in the stream refuses the
    /// connection immediately — and, like the legacy, hands out **nothing** from
    /// a refused read, even the good frames queued ahead of it.
    ///
    /// Feeding an empty chunk is a no-op (both directions of the recorded
    /// fixture: `empty_chunk_then_frame`, `frame_then_empty_chunk`). Feeding
    /// into a refused decoder returns the original error.
    ///
    /// # Errors
    ///
    /// Whatever [`validate_size`] returns for the first readable header that
    /// fails, after which the decoder is permanently refused.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), CodecError> {
        if let Some(err) = self.failure {
            return Err(err);
        }
        self.buf.extend_from_slice(chunk);
        self.scan()
    }

    /// Reads and guards the declared frame size at offset `at` — the **one**
    /// place the guard table runs on the decode path, shared by
    /// [`Decoder::feed`]'s walk and [`Decoder::next_frame`]'s pull.
    ///
    /// `Ok(None)` while fewer than 2 size bytes are buffered at `at`;
    /// `Err` (permanently poisoning the decoder, [`Decoder::failure`]) when
    /// the declared size fails [`validate_size`].
    fn declared_at(&mut self, at: usize) -> Result<Option<usize>, CodecError> {
        if self.buf.len() - at < 2 {
            return Ok(None);
        }
        let declared = u16::from_le_bytes([self.buf[at], self.buf[at + 1]]);
        match validate_size(declared) {
            Ok(()) => Ok(Some(declared as usize)),
            Err(err) => {
                let err = CodecError::Header(err);
                self.failure = Some(err);
                Err(err)
            }
        }
    }

    /// Walks the buffered frames the way `ITZRecvBuffer::Completion` walks its
    /// ring: read a header, run the guards, step forward a whole frame at a
    /// time, stop at the first incomplete body (`ITZRecvBuffer.cpp:128-214`).
    ///
    /// Runs from the front of the buffer on every `feed`; the cost is bounded
    /// by what is buffered, and re-checking an already-valid header is free of
    /// effect.
    fn scan(&mut self) -> Result<(), CodecError> {
        let mut at = 0usize;
        while let Some(declared) = self.declared_at(at)? {
            if self.buf.len() - at < declared {
                break;
            }
            at += declared;
        }
        Ok(())
    }

    /// Removes and returns the next whole frame, or `None` while the next one
    /// is still incomplete.
    ///
    /// This is the legacy's consumption point (`GetFirstPacket` → `Decode` →
    /// `RemoveFirstPacket`, `Client-Winsock.cpp:866-916`): the header guard is
    /// re-checked here as well as in [`Decoder::feed`] (`feed` is the only
    /// source of bytes, so this cannot currently fail — the guard call keeps
    /// the invariant independent of that), then the inbound cipher runs **once**
    /// before the frame is handed over.
    ///
    /// # Errors
    ///
    /// The same refusal as [`Decoder::feed`], sticky for the decoder's whole
    /// life; plus [`CodecError::Integrity`] when the inbound cipher rejects
    /// the frame — also sticky, because the keys have already ticked and the
    /// legacy would have torn the connection down.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, CodecError> {
        if let Some(err) = self.failure {
            return Err(err);
        }
        let Some(declared) = self.declared_at(0)? else {
            return Ok(None);
        };
        if self.buf.len() < declared {
            return Ok(None);
        }

        // Cut the raw frame first: usSize is in the clear, which is what lets
        // framing run ahead of any decryption (ITZRecvBuffer.cpp:155).
        let raw = self.buf.split_to(declared);

        let cipher = match &mut self.cipher {
            None => return Ok(Some(Frame::from_validated_bytes(raw.to_vec()))),
            Some(cipher) => cipher,
        };

        let mut plain = vec![0u8; declared];
        let result = cipher.decode(&raw, &mut plain);
        // Post-tick key position: Decode advances both keys on entry, even
        // when it goes on to refuse (ITZEncrypt.cpp:72-73).
        let key1 = cipher.key1();
        let key2 = cipher.key2();

        match result {
            Ok(true) => Ok(Some(Frame::from_validated_bytes(plain))),
            Ok(false) => {
                let err = CodecError::Integrity {
                    size: declared as u16,
                    cmd: u16::from_le_bytes([raw[3], raw[4]]),
                    key1,
                    key2,
                };
                self.failure = Some(err);
                Err(err)
            }
            Err(err) => {
                let err = CodecError::Cipher(err);
                self.failure = Some(err);
                Err(err)
            }
        }
    }

    /// Bytes held but not yet forming a whole frame — including whole frames
    /// waiting to be pulled (see the module docs' capacity note: bounded
    /// while the caller drains).
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// Whether nothing is held at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The refusal that permanently disabled this decoder, if any.
    #[must_use]
    pub const fn failure(&self) -> Option<CodecError> {
        self.failure
    }

    /// Discards buffered bytes, as `ITZRecvBuffer::ClearBuffer` does. Does
    /// **not** clear a refusal: the legacy clears buffers when a session is
    /// reset or reused (`ITZSession.cpp:181,305`), never to rescue one that
    /// was refused.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet → frame conversion: the outbound half of this codec (Step 2.3).
///
/// [`Encoder::encode`] takes an opcode and a payload, prepends the 5-byte
/// header through [`WireHeader::new`] — so the one guard table decides what
/// may be emitted — and applies the outbound cipher **if one is active**: the
/// bytes `GetEncrypt()->Encode` produced for the same message
/// (`Client-Winsock.cpp:146,195,200,242`), byte for byte. The module docs
/// carry the full legacy send-path walkthrough.
///
/// The cipher owns the send path symmetrically with [`Decoder`]'s receive
/// path: **one key tick per encoded frame**, and it happens at encode time,
/// before any flush. The legacy ticked inside `Encode` while filling its send
/// buffer (`ITZEncrypt.cpp:33-34`), decoupled from whether the socket write
/// later completed — a flush that fails here likewise leaves the keys
/// advanced for exactly that frame.
///
/// # The outbound side is the side that gains its key mid-connection
///
/// Unlike the inbound side (which never switches after establishment), the
/// outbound cipher arrives partway through: opcode 30000 delivers
/// `sResponse_SecretKey` → `GetEncrypt()->SetKey(...)`
/// (`Client-Winsock.cpp:228`), and the game link's
/// `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` does the same
/// (`Client-SystemNPC.cpp:703`). [`Encoder::set_cipher`] is that `SetKey` —
/// it is why this type holds `Option<Cipher>` at all (Steps 3.1 and 4.2 wire
/// it).
///
/// # No send timeout, no local ring
///
/// The legacy sets no `SO_SNDTIMEO` on the client's links (the only use in
/// the tree is an unrelated billing socket,
/// `NHanGameBillingProcess.cpp:453`), so [`Encoder::send`] adds none:
/// `write_all` waits exactly as the legacy's send loop waited. And where the
/// legacy's fixed ring could overflow into a silently dropped frame
/// (`ITZSendBuffer.cpp:141`, return ignored at `Client-Winsock.cpp:147`),
/// this encoder has no ring: bytes wait in the socket under backpressure
/// (module docs, divergences).
///
/// Not shared between tasks: one `Encoder` per connection, outbound
/// direction. `Cipher` is not `Sync`; the legacy guarded the same state with
/// its session critical section (plus a dedicated send critical section on
/// `_CHINA` builds, `Client-Winsock.cpp:159`).
#[derive(Debug)]
pub struct Encoder {
    /// The outbound cipher, while the link is keyed.
    cipher: Option<Cipher>,
}

impl Encoder {
    /// A plaintext encoder: no outbound cipher installed yet.
    ///
    /// Every keyed legacy client link *starts* unset and is barred from
    /// sending before its key arrives — the `...SecretKeySwitch` gates refuse
    /// the whole call before the cipher is ever reached
    /// (`Client-Winsock.cpp:141,171`), so nothing actually goes out in
    /// plaintext on those links; the state still exists, and
    /// [`Encoder::set_cipher`] is how a link acquires its cipher (Step 3.1).
    /// Programs that never negotiate a key (the legacy GM/log clients) stay
    /// in this mode for every frame they send.
    #[must_use]
    pub fn new() -> Self {
        Self { cipher: None }
    }

    /// An encoder that enciphers every frame with `cipher`, from the first
    /// frame on — for a link whose key is already in hand at construction.
    #[must_use]
    pub fn with_cipher(cipher: Cipher) -> Self {
        Self {
            cipher: Some(cipher),
        }
    }

    /// `SetKey` on the outbound side (`ITZEncrypt.cpp:14`), as the message
    /// handlers call it: opcode 30000 → `GetEncrypt()->SetKey(ucKey1, ucKey2)`
    /// (`Client-Winsock.cpp:228`), `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` → same
    /// (`Client-SystemNPC.cpp:703`).
    ///
    /// Replaces any installed cipher wholesale: frames encoded before this
    /// call went out as header-only bytes, frames after it are enciphered
    /// from the new keys' first tick.
    pub fn set_cipher(&mut self, cipher: Cipher) {
        self.cipher = Some(cipher);
    }

    /// The outbound cipher while one is active — the mirror of
    /// [`Decoder::cipher`], reading key position the way the legacy's
    /// `GetKey1()`/`GetKey2()` do.
    #[must_use]
    pub fn cipher(&self) -> Option<&Cipher> {
        self.cipher.as_ref()
    }

    /// Builds the exact wire bytes for `cmd` and `payload`: the 5-byte header
    /// (`usSize` little-endian, reserved byte **zeroed**, `usCmd`) followed by
    /// the payload — then the outbound cipher, if active, over the whole
    /// frame.
    ///
    /// `usSize` is the total frame length, header included: precisely the
    /// value `ITZSendBuffer::Write` reads back to size its copy
    /// (`ITZSendBuffer.cpp:132`). The reserved byte is zeroed the way Step
    /// 1.2's [`WireHeader::new`] and the server's `Frame::new` zero it — no
    /// legacy producer ever writes `ucChk` — and on a ciphered link it is
    /// overwritten by the resync tag anyway (`ITZEncrypt.cpp:44`).
    ///
    /// The `0x8000` compression flag travels through `cmd` untouched: the
    /// legacy sets it on the caller's side *before* `Encode`
    /// (`Client-Winsock.cpp:192`) and clears it only after decompressing the
    /// other way (`:911`).
    ///
    /// # Errors
    ///
    /// [`CodecError::Header`] when `HEADER_SIZE + payload.len()` exceeds
    /// [`crate::protocol::header::MAX_FRAME_SIZE`] (64992) — a frame no peer
    /// may accept
    /// (`ITZRecvBuffer.cpp:172`) is never built, and the refusal happens
    /// **before** the cipher runs, so the keys do not tick (module docs,
    /// divergences). [`CodecError::Cipher`] is unreachable by construction:
    /// the frame was just built with `usSize == len <= 64992`, below the
    /// cipher's own `PACKET_MAX_SIZE` (same argument as [`Decoder`] uses).
    pub fn encode(&mut self, cmd: u16, payload: &[u8]) -> Result<Frame, CodecError> {
        let header = WireHeader::new(cmd, payload.len())?;
        let mut bytes = Vec::with_capacity(usize::from(header.size()));
        bytes.extend_from_slice(&header.encode());
        bytes.extend_from_slice(payload);

        let bytes = match &mut self.cipher {
            None => bytes,
            Some(cipher) => cipher.encode(&bytes)?,
        };
        Ok(Frame::from_validated_bytes(bytes))
    }

    /// Encodes with [`Encoder::encode`], then flushes every byte to
    /// `session` — the WBS's "flush to socket".
    ///
    /// This replaces the legacy's ring copy plus overlapped send drain
    /// (`ITZSendBuffer::Write` → `GetSendParam` → `Completion`,
    /// `ITZSendBuffer.cpp:115-177,67-112`): `write_all` loops until the whole
    /// frame is written, which is exactly what that `Completion(siSentBytes)`
    /// loop advanced toward, minus the ring it fed.
    ///
    /// The tick order is the legacy's: the keys advance inside `encode`,
    /// i.e. **before** the flush. A write that fails after a partial send
    /// leaves the cipher advanced for that frame — its `Encode` had already
    /// run while filling the buffer, and nothing ever rolled it back.
    ///
    /// # Errors
    ///
    /// [`SendError::Encode`] before any byte is written (nothing was sent),
    /// then [`SendError::Io`] if the socket refuses or the connection dies
    /// mid-frame.
    pub async fn send(
        &mut self,
        session: &mut Session,
        cmd: u16,
        payload: &[u8],
    ) -> Result<(), SendError> {
        let frame = self.encode(cmd, payload)?;
        session.stream().write_all(frame.as_bytes()).await?;
        Ok(())
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod tests;
