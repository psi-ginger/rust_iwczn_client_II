//! Framed packet decoder: byte stream → complete frames (Step 2.2).
//!
//! This is the framing layer the WBS describes as "stream → packets": feed it
//! whatever [`session::Session::read`] handed you, pull whole frames out with
//! [`Decoder::next_frame`]. A frame split across any number of chunks and
//! several frames sharing one chunk are both handled — the legacy has the same
//! property, it just does it with a ring buffer and a `PACKET_MAX_SIZE`-byte
//! slack zone (`ITZRecvBuffer::CreateRecvBuffer`, `ITZRecvBuffer.cpp:56-58`)
//! instead of a deque.
//!
//! It composes the three primitives already landed:
//! - [`crate::protocol::header`] (Step 1.2): the 5-byte header layout and the
//!   one guard table (`validate_size`) — this module never re-derives size rules.
//! - [`crate::net::cipher`] (Step 1.3): the inbound cipher, applied **if active**.
//! - [`crate::net::session`] (Step 2.1): the transport the chunks come off.
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
//! after establishment.
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
//!
//! # Thread safety
//!
//! Not shared. The legacy relied on the enclosing session's critical section
//! for the same reason; give each connection its own [`Decoder`] — one per
//! direction would be wrong anyway: only the *inbound* side is decoded.
//!
//! [`session::Session::read`]: crate::net::session::Session::read
//! [`Cipher::decode`]: crate::net::cipher::Cipher::decode
//! [`MAX_FRAME_SIZE`]: crate::protocol::header::MAX_FRAME_SIZE

use bytes::BytesMut;
use thiserror::Error;

use super::cipher::{Cipher, CipherError};
use crate::protocol::header::{validate_size, HeaderError, HEADER_SIZE};

/// Why a frame could not be decoded.
///
/// Every variant means **kill the connection**, not "drop this packet" — the
/// legacy turns both refusal paths into teardown: a bad header is
/// `Completion() == FALSE` → `SESSION_STATE_DISCONNECTED`
/// (`ITZNetIOCP.cpp:145-150`), a failed cipher check is `Decode() == false` →
/// `return false`/`CloseSocket()` (`Client-Winsock.cpp:889`,
/// `FrontClientProtocol.cpp:90`). Hence [`Decoder`]'s sticky failure state.
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

    /// Walks the buffered frames the way `ITZRecvBuffer::Completion` walks its
    /// ring: read a header, run the guards, step forward a whole frame at a
    /// time, stop at the first incomplete body (`ITZRecvBuffer.cpp:128-214`).
    ///
    /// Runs from the front of the buffer on every `feed`; the cost is bounded
    /// by what is buffered, and re-checking an already-valid header is free of
    /// effect.
    fn scan(&mut self) -> Result<(), CodecError> {
        let mut at = 0usize;
        while self.buf.len() - at >= 2 {
            let declared = u16::from_le_bytes([self.buf[at], self.buf[at + 1]]);
            let declared = match validate_size(declared) {
                Ok(()) => declared,
                Err(err) => {
                    let err = CodecError::Header(err);
                    self.failure = Some(err);
                    return Err(err);
                }
            };
            let declared = declared as usize;
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
        if self.buf.len() < 2 {
            return Ok(None);
        }

        let declared = u16::from_le_bytes([self.buf[0], self.buf[1]]);
        if let Err(err) = validate_size(declared) {
            let err = CodecError::Header(err);
            self.failure = Some(err);
            return Err(err);
        }

        let declared_usize = declared as usize;
        if self.buf.len() < declared_usize {
            return Ok(None);
        }

        // Cut the raw frame first: usSize is in the clear, which is what lets
        // framing run ahead of any decryption (ITZRecvBuffer.cpp:155).
        let raw = self.buf.split_to(declared_usize);

        let cipher = match &mut self.cipher {
            None => return Ok(Some(Frame::from_validated_bytes(raw.to_vec()))),
            Some(cipher) => cipher,
        };

        let mut plain = vec![0u8; declared_usize];
        let result = cipher.decode(&raw, &mut plain);
        // Post-tick key position: Decode advances both keys on entry, even
        // when it goes on to refuse (ITZEncrypt.cpp:72-73).
        let key1 = cipher.key1();
        let key2 = cipher.key2();

        match result {
            Ok(true) => Ok(Some(Frame::from_validated_bytes(plain))),
            Ok(false) => {
                let err = CodecError::Integrity {
                    size: declared,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::cipher::Cipher;
    use crate::net::session::Session;
    use crate::protocol::header::{WireHeader, MAX_FRAME_SIZE};
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    /// Recorded framing ground truth: the byte-for-byte unmodified legacy
    /// `ITZRecvBuffer.cpp` compiled with g++ and driven the way the IOCP drives
    /// it (chunked reads). Copied from
    /// `rust_Iwcgz_server/crates/gz-net/tests/data/vectors.rs` (generated by
    /// its `reference/emit_vectors.py`); every `completed`/`packets` pair was
    /// recorded from the legacy itself, so a disagreement here means this port
    /// is wrong, not that the fixture moved. See that repo's
    /// `reference/README.md` for the harness that produced it.
    mod legacy {
        /// One read: the bytes received, and what the legacy buffer did.
        pub struct Step {
            /// Hex bytes written into the buffer, i.e. the chunk read off the
            /// socket.
            pub chunk: &'static str,
            /// `Completion()`'s return value: `false` means the session is
            /// disconnected.
            pub completed: bool,
            /// Hex frames `GetFirstPacket()` handed out, in order.
            pub packets: &'static [&'static str],
        }

        /// A stream delivered to one buffer, in chunks.
        pub struct Case {
            pub name: &'static str,
            pub steps: &'static [Step],
        }

        /// Why this port deliberately answers a case differently.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Divergence {
            /// BUG-013: legacy accepts declared sizes 2..=4 (no room for
            /// `usCmd`), this port refuses.
            SubHeader,
            /// BUG-014: the legacy's ring wrapped over a half-received frame;
            /// this port, having no ring, delivers what was actually sent.
            RingCapacity,
        }

        /// Every deliberately divergent case, with its reason.
        pub static DIVERGENT: &[(&str, Divergence)] = &[
            ("declared_2", Divergence::SubHeader),
            ("declared_3", Divergence::SubHeader),
            ("declared_4", Divergence::SubHeader),
            ("ring_smaller_than_frame", Divergence::RingCapacity),
        ];

        /// Every case, in harness order.
        pub static CASES: &[Case] = &[
            Case {
                name: "single_frame",
                steps: &[Step {
                    chunk: "0d0000020101060b10151a1f24",
                    completed: true,
                    packets: &["0d0000020101060b10151a1f24"],
                }],
            },
            Case {
                name: "two_frames_one_chunk",
                steps: &[Step {
                    chunk: "05000001002d0000020001060b10151a1f24292e33383d42474c51565b60656a6f74797e83888d92979ca1a6abb0b5babfc4",
                    completed: true,
                    packets: &[
                        "0500000100",
                        "2d0000020001060b10151a1f24292e33383d42474c51565b60656a6f74797e83888d92979ca1a6abb0b5babfc4",
                    ],
                }],
            },
            Case {
                name: "split_across_three_chunks",
                steps: &[
                    Step {
                        chunk: "23000004030106",
                        completed: true,
                        packets: &[],
                    },
                    Step {
                        chunk: "0b10151a1f24292e33383d42",
                        completed: true,
                        packets: &[],
                    },
                    Step {
                        chunk: "474c51565b60656a6f74797e83888d92",
                        completed: true,
                        packets: &["230000040301060b10151a1f24292e33383d42474c51565b60656a6f74797e83888d92"],
                    },
                ],
            },
            Case {
                name: "byte_at_a_time",
                steps: &[
                    Step { chunk: "19", completed: true, packets: &[] },
                    Step { chunk: "00", completed: true, packets: &[] },
                    Step { chunk: "00", completed: true, packets: &[] },
                    Step { chunk: "06", completed: true, packets: &[] },
                    Step { chunk: "05", completed: true, packets: &[] },
                    Step { chunk: "01", completed: true, packets: &[] },
                    Step { chunk: "06", completed: true, packets: &[] },
                    Step { chunk: "0b", completed: true, packets: &[] },
                    Step { chunk: "10", completed: true, packets: &[] },
                    Step { chunk: "15", completed: true, packets: &[] },
                    Step { chunk: "1a", completed: true, packets: &[] },
                    Step { chunk: "1f", completed: true, packets: &[] },
                    Step { chunk: "24", completed: true, packets: &[] },
                    Step { chunk: "29", completed: true, packets: &[] },
                    Step { chunk: "2e", completed: true, packets: &[] },
                    Step { chunk: "33", completed: true, packets: &[] },
                    Step { chunk: "38", completed: true, packets: &[] },
                    Step { chunk: "3d", completed: true, packets: &[] },
                    Step { chunk: "42", completed: true, packets: &[] },
                    Step { chunk: "47", completed: true, packets: &[] },
                    Step { chunk: "4c", completed: true, packets: &[] },
                    Step { chunk: "51", completed: true, packets: &[] },
                    Step { chunk: "56", completed: true, packets: &[] },
                    Step { chunk: "5b", completed: true, packets: &[] },
                    Step { chunk: "60", completed: true, packets: &["190000060501060b10151a1f24292e33383d42474c51565b60"] },
                ],
            },
            Case {
                name: "header_split_from_body",
                steps: &[
                    Step { chunk: "1d", completed: true, packets: &[] },
                    Step { chunk: "000007", completed: true, packets: &[] },
                    Step {
                        chunk: "0601060b10151a1f24292e33383d42474c51565b60656a6f74",
                        completed: true,
                        packets: &["1d0000070601060b10151a1f24292e33383d42474c51565b60656a6f74"],
                    },
                ],
            },
            Case {
                name: "empty_chunk_then_frame",
                steps: &[
                    Step { chunk: "", completed: true, packets: &[] },
                    Step {
                        chunk: "090000080701060b10",
                        completed: true,
                        packets: &["090000080701060b10"],
                    },
                ],
            },
            Case {
                name: "frame_then_empty_chunk",
                steps: &[
                    Step {
                        chunk: "110000090001060b10151a1f24292e3338",
                        completed: true,
                        packets: &["110000090001060b10151a1f24292e3338"],
                    },
                    Step { chunk: "", completed: true, packets: &[] },
                ],
            },
            Case {
                name: "frame_then_partial",
                steps: &[Step {
                    chunk: "0f0000010001060b10151a1f24292e370000020001060b10151a1f24292e33383d42474c51565b",
                    completed: true,
                    packets: &["0f0000010001060b10151a1f24292e"],
                }],
            },
            Case {
                name: "declared_5_header_only",
                steps: &[Step {
                    chunk: "050011181f",
                    completed: true,
                    packets: &["050011181f"],
                }],
            },
            Case {
                name: "declared_64992_short_body",
                steps: &[Step {
                    chunk: "e0fd11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d14",
                    completed: true,
                    packets: &[],
                }],
            },
            Case {
                name: "declared_300_short_body",
                steps: &[Step {
                    chunk: "2c0111181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d14",
                    completed: true,
                    packets: &[],
                }],
            },
            Case {
                name: "declared_0",
                steps: &[Step {
                    chunk: "000011181f262d34",
                    completed: false,
                    packets: &[],
                }],
            },
            Case {
                name: "declared_1",
                steps: &[Step {
                    chunk: "010011181f262d34",
                    completed: false,
                    packets: &[],
                }],
            },
            Case {
                name: "declared_2",
                steps: &[Step {
                    chunk: "020011181f262d34",
                    completed: true,
                    packets: &["0200"],
                }],
            },
            Case {
                name: "declared_3",
                steps: &[Step {
                    chunk: "030011181f262d34",
                    completed: true,
                    packets: &["030011"],
                }],
            },
            Case {
                name: "declared_4",
                steps: &[Step {
                    chunk: "040011181f262d34",
                    completed: true,
                    packets: &["04001118"],
                }],
            },
            Case {
                name: "declared_5",
                steps: &[Step {
                    chunk: "050011181f262d34",
                    completed: true,
                    packets: &["050011181f"],
                }],
            },
            Case {
                name: "declared_64993_over_limit",
                steps: &[Step {
                    chunk: "e1fd11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d14",
                    completed: false,
                    packets: &[],
                }],
            },
            Case {
                name: "declared_65535",
                steps: &[Step {
                    chunk: "ffff11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d14",
                    completed: false,
                    packets: &[],
                }],
            },
            Case {
                name: "good_then_over_limit",
                steps: &[Step {
                    chunk: "0b0000010001060b10151ae1fd11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d14",
                    completed: false,
                    packets: &[],
                }],
            },
            Case {
                name: "ring_smaller_than_frame",
                steps: &[
                    Step {
                        chunk: "280011181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dc",
                        completed: true,
                        packets: &[],
                    },
                    Step {
                        chunk: "0500aabbcc040000",
                        completed: true,
                        packets: &["0500aabbcc"],
                    },
                ],
            },
        ];
    }

    use legacy::{Case, Divergence, CASES, DIVERGENT};

    fn unhex(hex: &str) -> Vec<u8> {
        assert!(hex.len() % 2 == 0, "odd-length hex {hex:?}");
        hex.as_bytes()
            .chunks(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).expect("hex is ascii");
                u8::from_str_radix(text, 16).expect("hex digit")
            })
            .collect()
    }

    /// A well-formed plaintext frame: 5-byte header + payload, reserved byte
    /// zeroed (the encoder Step 2.3 will formalise; built here from the Step
    /// 1.2 header type so size/guard logic stays in one place).
    fn plaintext_frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
        let header = WireHeader::new(cmd, payload.len()).expect("payload fits a frame");
        let mut bytes = header.encode().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }

    /// Pulls everything queued; `Err` only for a refused decoder.
    fn drain(decoder: &mut Decoder) -> Result<Vec<Vec<u8>>, CodecError> {
        let mut frames = Vec::new();
        while let Some(frame) = decoder.next_frame()? {
            frames.push(frame.into_bytes());
        }
        Ok(frames)
    }

    /// Replays one recorded case through this decoder, step by step, and
    /// compares both answers that matter: was the read refused (`feed` vs the
    /// legacy's `Completion` return) and which bytes came out (`next_frame`
    /// vs the legacy's `GetFirstPacket` reads). The two recorded divergences
    /// are asserted explicitly so neither can drift into agreement by
    /// accident. Adapted from `gz-net/tests/legacy_vectors.rs`.
    fn replay(case: &Case) -> Result<(), String> {
        let divergence = DIVERGENT
            .iter()
            .find(|(name, _)| *name == case.name)
            .map(|(_, kind)| *kind);
        let mut decoder = Decoder::new();
        let mut sent: Vec<u8> = Vec::new();

        for (index, step) in case.steps.iter().enumerate() {
            let at = format!("{} read {index}", case.name);
            let chunk = unhex(step.chunk);

            if divergence == Some(Divergence::SubHeader) {
                // BUG-013: the legacy handed out a frame with no room for
                // usCmd; this port refuses the connection. Asserted in both
                // directions, pinned in data rather than left to a comment.
                assert!(
                    decoder.feed(&chunk).is_err(),
                    "{at}: expected this port to refuse"
                );
                assert!(
                    step.completed,
                    "{at}: legacy is recorded as accepting a sub-header frame"
                );
                assert_eq!(step.packets.len(), 1, "{at}: expected one legacy frame");
                assert!(
                    unhex(step.packets[0]).len() < HEADER_SIZE,
                    "{at}: divergence case must be a sub-header frame"
                );
                assert!(
                    decoder.next_frame().is_err(),
                    "{at}: a refused decoder must stay refused"
                );
                return Ok(());
            }

            if divergence == Some(Divergence::RingCapacity) {
                // BUG-014: the legacy's ring wrapped and handed out a
                // different frame than the one sent. Having no ring, this port
                // must accept every read and reassemble what was written.
                decoder
                    .feed(&chunk)
                    .map_err(|e| format!("{at}: this port refused ({e})"))?;
                sent.extend_from_slice(&chunk);
                continue;
            }

            match decoder.feed(&chunk) {
                Ok(()) if !step.completed => {
                    return Err(format!(
                        "{at}: the legacy refused but this port accepted"
                    ));
                }
                Err(err) if step.completed => {
                    return Err(format!(
                        "{at}: this port refused ({err}) but the legacy accepted"
                    ));
                }
                Err(_) => {
                    // Both refused: a refusal hands out nothing.
                    if !step.packets.is_empty() {
                        return Err(format!("{at}: a refused read handed out a frame"));
                    }
                    assert!(
                        decoder.next_frame().is_err(),
                        "{at}: a refused decoder must stay refused"
                    );
                    return Ok(());
                }
                Ok(()) => {}
            }

            let got = drain(&mut decoder).map_err(|e| format!("{at}: {e}"))?;
            let want: Vec<Vec<u8>> = step.packets.iter().map(|p| unhex(p)).collect();
            if got != want {
                return Err(format!(
                    "{at}: frames differ\n  legacy: {want:x?}\n  this port: {got:x?}"
                ));
            }
        }

        if divergence == Some(Divergence::RingCapacity) {
            let got = drain(&mut decoder).expect("ring case must not be refused");
            assert_eq!(
                got,
                vec![sent.clone()],
                "this port must deliver exactly the frame the peer sent, reassembled"
            );
            let legacy: Vec<Vec<u8>> = case
                .steps
                .iter()
                .flat_map(|step| step.packets)
                .map(|p| unhex(p))
                .collect();
            assert_ne!(
                legacy,
                vec![sent],
                "the legacy no longer mangles this frame; re-check BUG-014"
            );
        }

        Ok(())
    }

    // ---------------------------------------------------------------- fixture

    #[test]
    fn fixture_is_intact_and_divergences_are_pinned() {
        // A ground-truth table must describe itself: every recorded packet's
        // length agrees with its own declared usSize, or the copy is corrupt.
        for case in CASES {
            for step in case.steps {
                for packet in step.packets {
                    let bytes = unhex(packet);
                    let declared = u16::from_le_bytes([bytes[0], bytes[1]]);
                    assert_eq!(
                        usize::from(declared),
                        bytes.len(),
                        "{}: packet length disagrees with its own header",
                        case.name
                    );
                }
            }
        }

        // Names unique and the fixture is not trivially small.
        let mut names: Vec<&str> = CASES.iter().map(|case| case.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate case names");
        assert!(CASES.len() >= 15, "the framing fixture shrank");

        // Exactly the reviewed divergences: BUG-013's three sub-header sizes
        // and BUG-014's over-ring frame. Nothing else may differ from the
        // legacy silently.
        assert_eq!(
            DIVERGENT,
            &[
                ("declared_2", Divergence::SubHeader),
                ("declared_3", Divergence::SubHeader),
                ("declared_4", Divergence::SubHeader),
                ("ring_smaller_than_frame", Divergence::RingCapacity),
            ],
            "the divergence list changed; re-verify it against the legacy"
        );

        // A refusal is the last read of its case: the legacy caller
        // disconnects and stops feeding, so later reads never happened.
        for case in CASES {
            let last_refusal = case
                .steps
                .iter()
                .rposition(|step| !step.completed)
                .unwrap_or(usize::MAX);
            for (index, step) in case.steps.iter().enumerate() {
                if !step.completed {
                    assert_eq!(
                        index, last_refusal,
                        "{}: reads recorded after a refusal",
                        case.name
                    );
                    assert!(
                        step.packets.is_empty(),
                        "{}: a refused read handed out a frame",
                        case.name
                    );
                }
            }
        }

        // Boundaries sit on this port's constants, measured not assumed.
        let by_name = |name: &str| {
            CASES
                .iter()
                .find(|case| case.name == name)
                .unwrap_or_else(|| panic!("missing case {name}"))
        };
        assert!(by_name("declared_5").steps[0].completed);
        assert!(by_name("declared_64992_short_body").steps[0].completed);
        assert!(by_name("declared_64992_short_body").steps[0].packets.is_empty());
        assert!(!by_name("declared_64993_over_limit").steps[0].completed);
        assert!(!by_name("declared_65535").steps[0].completed);
        assert_eq!(MAX_FRAME_SIZE, 64992);
    }

    #[test]
    fn decoder_agrees_with_the_compiled_legacy_on_every_recorded_read() {
        for case in CASES {
            if let Err(why) = replay(case) {
                panic!("{why}");
            }
        }
    }

    // ------------------------------------------------------------ plaintext

    #[test]
    fn single_frame_is_emitted_byte_exact() {
        let wire = plaintext_frame(30000, b"hello goonzu");
        let mut decoder = Decoder::new();

        decoder.feed(&wire).expect("valid stream");
        let frames = drain(&mut decoder).expect("valid stream");

        assert_eq!(frames, vec![wire.clone()]);
        assert!(decoder.next_frame().unwrap().is_none(), "queue drained");
        assert!(decoder.is_empty());
        assert_eq!(decoder.failure(), None);
        assert!(decoder.cipher().is_none(), "front-link style: no cipher");
    }

    #[test]
    fn a_frame_split_mid_header_and_mid_payload_reassembles() {
        let wire = plaintext_frame(0x0102, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let mut decoder = Decoder::new();

        // 1 byte: the first size byte alone — no header yet.
        decoder.feed(&wire[..1]).expect("valid");
        assert!(decoder.next_frame().unwrap().is_none());
        assert_eq!(decoder.buffered(), 1);

        // Through byte 3: usSize complete (guard already ran), ucChk in hand,
        // but usCmd is cut mid-field and no body has arrived.
        decoder.feed(&wire[1..4]).expect("valid");
        assert!(decoder.next_frame().unwrap().is_none());
        assert_eq!(decoder.buffered(), 4);

        // Header complete, half the payload.
        decoder.feed(&wire[4..8]).expect("valid");
        assert!(decoder.next_frame().unwrap().is_none());

        // The rest completes it.
        decoder.feed(&wire[8..]).expect("valid");
        let frames = drain(&mut decoder).expect("valid");
        assert_eq!(frames, vec![wire]);
        assert!(decoder.is_empty());
    }

    #[test]
    fn byte_by_byte_delivery_reassembles() {
        let wire = plaintext_frame(7, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let mut decoder = Decoder::new();

        let mut emitted = Vec::new();
        for (index, byte) in wire.iter().enumerate() {
            decoder.feed(&[*byte]).expect("valid");
            while let Some(frame) = decoder.next_frame().expect("valid") {
                assert_eq!(index, wire.len() - 1, "emitted before the last byte");
                emitted.push(frame.into_bytes());
            }
        }
        assert_eq!(emitted, vec![wire]);
    }

    #[test]
    fn multi_frame_bursts_drain_in_order() {
        let expected: Vec<Vec<u8>> = (0u16..5)
            .map(|i| {
                let payload = vec![i as u8; i as usize + 1];
                plaintext_frame(100 + i, &payload)
            })
            .collect();
        let mut burst: Vec<u8> = Vec::new();
        for frame in &expected {
            burst.extend_from_slice(frame);
        }

        // One burst, one feed: every frame must come out, in order.
        let mut decoder = Decoder::new();
        decoder.feed(&burst).expect("valid burst");
        assert_eq!(drain(&mut decoder).expect("valid"), expected);
        assert!(decoder.is_empty());

        // The same frames split across feeds at frame boundaries and inside
        // frames alike, drained between feeds. Frames end at 6, 13, 21, 30,
        // 40 of the 40-byte burst, so the cuts land inside frames, exactly on
        // one boundary, and at the end.
        let mut decoder = Decoder::new();
        let mut got = Vec::new();
        let cuts = [0, 7, 20, 21, 33, burst.len()];
        for window in cuts.windows(2) {
            decoder.feed(&burst[window[0]..window[1]]).expect("valid");
            got.extend(drain(&mut decoder).expect("valid"));
        }
        assert_eq!(got, expected, "chunk boundaries must not change framing");
    }

    #[test]
    fn a_legal_size_with_an_incomplete_body_is_held_not_refused() {
        // 64992 declared (inside the guard) with only part of the body
        // present: a slow peer must not be disconnected — the refusal
        // decision comes from the header, the wait from the body.
        let mut chunk = vec![0u8; 45];
        chunk[..2].copy_from_slice(&MAX_FRAME_SIZE.to_le_bytes());

        let mut decoder = Decoder::new();
        decoder.feed(&chunk).expect("valid, just incomplete");
        assert!(decoder.failure().is_none());
        assert!(decoder.next_frame().unwrap().is_none());
        assert_eq!(decoder.buffered(), 45);

        // Complete it: exactly MAX_FRAME_SIZE bytes on the wire, fully
        // delivered — the one boundary the recorded fixture cannot carry
        // (130 KB of hex), so it is checked end to end here.
        let body = vec![0x5A; MAX_FRAME_SIZE as usize - 45];
        decoder.feed(&body).expect("valid");
        assert_eq!(
            decoder.buffered(),
            MAX_FRAME_SIZE as usize,
            "45 buffered + body must equal the declared size exactly"
        );
        let frames = drain(&mut decoder).expect("valid");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), MAX_FRAME_SIZE as usize);
        assert!(decoder.is_empty(), "nothing may trail a full-size frame");
        // And its own header agrees, through the Step 1.2 decoder.
        let header = WireHeader::decode(&frames[0]).expect("validated frame");
        assert_eq!(header.size(), MAX_FRAME_SIZE);
        assert_eq!(header.payload_len(), MAX_FRAME_SIZE as usize - HEADER_SIZE);
    }

    #[test]
    fn the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks() {
        // (size bytes, expected refusal): the legacy's own <2 guard, BUG-013's
        // sub-header range, and the receive ceiling — all three refusal points
        // of ITZRecvBuffer.cpp:163,172 plus Step 1.2's deliberate divergence.
        let cases: [(u16, CodecError); 7] = [
            (0, CodecError::Header(HeaderError::SizeBelowMinimum { declared: 0 })),
            (1, CodecError::Header(HeaderError::SizeBelowMinimum { declared: 1 })),
            (2, CodecError::Header(HeaderError::SizeBelowHeader { declared: 2 })),
            (3, CodecError::Header(HeaderError::SizeBelowHeader { declared: 3 })),
            (4, CodecError::Header(HeaderError::SizeBelowHeader { declared: 4 })),
            (
                64993,
                CodecError::Header(HeaderError::SizeTooLarge { declared: 64993 }),
            ),
            (
                u16::MAX,
                CodecError::Header(HeaderError::SizeTooLarge { declared: u16::MAX }),
            ),
        ];

        for (declared, expected) in cases {
            let mut chunk = vec![0xFF; 16];
            chunk[..2].copy_from_slice(&declared.to_le_bytes());

            let mut decoder = Decoder::new();
            assert_eq!(decoder.feed(&chunk), Err(expected), "declared {declared}");
            // Sticky: every later call keeps the original refusal, and clear()
            // (which the legacy only ever uses to reset a live session,
            // ITZSession.cpp:181,305) does not resurrect it.
            assert_eq!(decoder.failure(), Some(expected));
            assert_eq!(decoder.feed(&[0x05, 0x00]), Err(expected));
            assert_eq!(decoder.next_frame(), Err(expected));
            decoder.clear();
            assert_eq!(decoder.next_frame(), Err(expected));
        }

        // ...while 5 (the smallest frame that describes itself) and 64992
        // (the ceiling) are accepted. Feed exact-length chunks: trailing zero
        // bytes after a *complete* frame would otherwise parse as the next
        // header's size 0 — a genuinely illegal header a real stream would
        // rightly be refused for.
        let mut header_only = vec![0u8; HEADER_SIZE];
        header_only[..2].copy_from_slice(&5u16.to_le_bytes());
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(&header_only), Ok(()));
        assert_eq!(decoder.failure(), None);
        let frame = decoder
            .next_frame()
            .expect("valid")
            .expect("a complete 5-byte frame comes out whole");
        assert_eq!(frame.size(), 5);

        // An in-limit size with an incomplete body is held, not refused.
        let mut at_ceiling = vec![0u8; 8];
        at_ceiling[..2].copy_from_slice(&MAX_FRAME_SIZE.to_le_bytes());
        let mut decoder = Decoder::new();
        assert_eq!(
            decoder.feed(&at_ceiling),
            Ok(()),
            "64992 sits exactly on the receive limit"
        );
        assert!(decoder.next_frame().unwrap().is_none(), "body incomplete");
    }

    #[test]
    fn a_good_frame_ahead_of_a_bad_header_is_never_delivered() {
        // The recorded `good_then_over_limit` case, spelled out: the legacy's
        // Completion refuses the whole read without committing the good frame
        // in front of it, so this must too. This is why headers are validated
        // eagerly in feed() rather than at pull time.
        let mut chunk = plaintext_frame(9, b"deliver me");
        chunk.extend_from_slice(&[0xE1, 0xFD, 0x00, 0x00, 0x00]); // 64993

        let mut decoder = Decoder::new();
        assert_eq!(
            decoder.feed(&chunk),
            Err(CodecError::Header(HeaderError::SizeTooLarge {
                declared: 64993
            }))
        );
        assert_eq!(
            decoder.next_frame(),
            Err(CodecError::Header(HeaderError::SizeTooLarge {
                declared: 64993
            })),
            "the queued good frame must die with the connection, not escape"
        );
    }

    #[test]
    fn clear_discards_bytes_but_keeps_a_refusal() {
        let wire = plaintext_frame(3, b"partial");

        // Healthy decoder: clear() resets the buffer like ClearBuffer does.
        let mut decoder = Decoder::new();
        decoder.feed(&wire[..4]).expect("valid");
        assert_eq!(decoder.buffered(), 4);
        assert!(!decoder.is_empty());
        decoder.clear();
        assert!(decoder.is_empty());

        // And the decoder is usable again after a clear...
        decoder.feed(&wire).expect("valid");
        assert_eq!(
            drain(&mut decoder).expect("valid"),
            vec![wire.clone()],
            "clear() on a healthy decoder must not lose later frames"
        );

        // ...but a refused one stays refused: the legacy only ever cleared
        // buffers to reset a live session (ITZSession.cpp:181,305), never to
        // rescue one that was refused.
        let mut refused = Decoder::new();
        assert!(refused.feed(&[0x00, 0x00, 0x00]).is_err());
        refused.clear();
        assert!(refused.next_frame().is_err(), "clear() must not resurrect a refusal");
        assert!(refused.failure().is_some());
    }

    #[test]
    fn compression_flag_and_reserved_byte_pass_through_the_decoder() {
        // usCmd bit 15 is the LZF flag the legacy clears only *after*
        // decompressing (GameProtocol.cpp:141): framing must carry it.
        let wire = [0x05, 0x00, 0x00, 0x05, 0x80];
        let mut decoder = Decoder::new();
        decoder.feed(&wire).expect("valid");
        let frame = decoder.next_frame().expect("valid").expect("one frame");
        assert!(frame.is_compressed());
        assert_eq!(frame.cmd(), 0x8005, "the flag must not be masked");
        assert_eq!(frame.cmd_id(), 5);
        assert!(frame.payload().is_empty());

        // The reserved byte is not a checksum: on a plaintext link (the front
        // link) it arrives arbitrary and must ride through untouched.
        let wire = [0x05, 0x00, 0xA5, 0x01, 0x00];
        let mut decoder = Decoder::new();
        decoder.feed(&wire).expect("valid");
        let frame = decoder.next_frame().expect("valid").expect("one frame");
        assert_eq!(frame.integrity(), 0xA5);
        assert_eq!(frame.as_bytes(), wire, "plaintext bytes pass verbatim");
    }

    // ---------------------------------------------------------------- cipher

    /// Four plaintext frames of assorted sizes/commands for cipher tests.
    fn cipher_frames() -> Vec<Vec<u8>> {
        vec![
            plaintext_frame(5, b""),
            plaintext_frame(0x000A, &[1, 2, 3]),
            plaintext_frame(30000, &[0xDE, 0xAD, 0xBE, 0xEF]),
            plaintext_frame(0x0042, &(0u8..64).collect::<Vec<u8>>()),
        ]
    }

    #[test]
    fn ciphered_frames_reassemble_across_irregular_splits_and_bursts() {
        let frames = cipher_frames();

        // The peer encodes each frame with its outbound link cipher...
        let mut tx = Cipher::link_cipher();
        let mut wire: Vec<u8> = Vec::new();
        for frame in &frames {
            wire.extend_from_slice(&tx.encode(frame).expect("encode"));
        }

        // ...and we decode with the paired inbound instance, feeding the
        // stream in deliberately ragged chunks (1,4,2,7,3,11,5,... bytes) so
        // every boundary lands somewhere different: mid-size-field, mid-cipher
        // body, on frame edges, and inside bursts.
        let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
        let steps = [1usize, 4, 2, 7, 3, 11, 5];
        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut at = 0usize;
        let mut round = 0usize;
        while at < wire.len() {
            let step = steps[round % steps.len()].min(wire.len() - at);
            round += 1;
            decoder.feed(&wire[at..at + step]).expect("valid stream");
            at += step;
            got.extend(drain(&mut decoder).expect("valid stream"));
        }

        assert_eq!(got, frames, "deciphered frames must be byte-exact");
        assert!(decoder.is_empty());

        // Peer counters ended in step: one cipher call per frame on each side
        // (Step 1.3's invariant, exercised through the framing layer).
        let rx = decoder.cipher().expect("cipher active");
        assert_eq!((rx.key1(), rx.key2()), (tx.key1(), tx.key2()));
        assert_eq!(rx.key1(), 0x12u8.wrapping_add(frames.len() as u8));
        assert_eq!(rx.key2(), 0x34u8.wrapping_add(frames.len() as u8));
    }

    #[test]
    fn cipher_keys_advance_exactly_once_per_pulled_frame() {
        let frames = cipher_frames();
        let mut tx = Cipher::link_cipher();
        let mut wire: Vec<Vec<u8>> = frames
            .iter()
            .map(|frame| tx.encode(frame).expect("encode"))
            .collect();

        let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
        let start = (0x12u8, 0x34u8);

        // Feed frame 1, do not pull: no consumption, no tick (the legacy only
        // calls Decode at GetFirstPacket time).
        decoder.feed(&wire[0]).expect("valid");
        assert_eq!(
            (decoder.cipher().unwrap().key1(), decoder.cipher().unwrap().key2()),
            start,
            "feed alone must not advance the keys"
        );

        // Pull it: exactly one tick.
        assert!(decoder.next_frame().unwrap().is_some());
        assert_eq!(
            (
                decoder.cipher().unwrap().key1(),
                decoder.cipher().unwrap().key2()
            ),
            (start.0.wrapping_add(1), start.1.wrapping_add(1))
        );

        // Pull the remaining three: three more ticks, ending in step with the
        // transmitting peer.
        wire.remove(0);
        let mut rest: Vec<u8> = Vec::new();
        for chunk in &wire {
            rest.extend_from_slice(chunk);
        }
        decoder.feed(&rest).expect("valid");
        assert_eq!(drain(&mut decoder).expect("valid"), frames[1..].to_vec());

        let rx = decoder.cipher().unwrap();
        assert_eq!((rx.key1(), rx.key2()), (tx.key1(), tx.key2()));
        assert_eq!(
            (rx.key1(), rx.key2()),
            (
                start.0.wrapping_add(frames.len() as u8),
                start.1.wrapping_add(frames.len() as u8)
            ),
            "N pulled frames = N ticks, no more, no less"
        );
    }

    #[test]
    fn a_tampered_frame_fails_integrity_and_reports_the_legacy_log_fields() {
        let plaintext = plaintext_frame(0x1234, &[0x11, 0x22, 0x33]);
        let mut tx = Cipher::link_cipher();
        let mut wire = tx.encode(&plaintext).expect("encode");

        // Corrupt one payload byte — the recorded cipher vectors' corrupt-
        // payload case (E2). Integrity must catch it.
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;

        let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
        decoder.feed(&wire).expect("framing is happy: usSize is clear");
        let err = decoder.next_frame().expect_err("integrity must fail");

        let (size, cmd, key1, key2) = match err {
            CodecError::Integrity {
                size,
                cmd,
                key1,
                key2,
            } => (size, cmd, key1, key2),
            other => panic!("expected Integrity, got {other:?}"),
        };
        assert_eq!(size as usize, wire.len(), "usSize logged from the header");
        // The legacy logged usCmd straight off the raw (ciphertext) buffer —
        // Client-Winsock.cpp:878 reads it before any decode succeeds.
        assert_eq!(cmd, u16::from_le_bytes([wire[3], wire[4]]));
        // Post-tick position: Decode advanced both keys on entry
        // (ITZEncrypt.cpp:72-73) before refusing.
        assert_eq!((key1, key2), (0x13, 0x35));

        // Sticky, like the legacy's `return false` teardown: the keys ticked,
        // so there is no way to continue.
        assert_eq!(decoder.failure(), Some(err));
        assert_eq!(decoder.feed(&wire), Err(err));
        assert_eq!(decoder.next_frame(), Err(err));
    }

    #[test]
    fn a_ciphered_decoder_refuses_a_plaintext_stream() {
        // Wrong bytes for the key in force: the integrity byte disagrees, so a
        // link wired up with the wrong cipher state dies instead of quietly
        // handing up garbage.
        let wire = plaintext_frame(0x1234, b"hi");
        let mut decoder = Decoder::with_cipher(Cipher::link_cipher());
        decoder.feed(&wire).expect("framing only checks usSize");

        let err = decoder.next_frame().expect_err("must not accept");
        let frame_len = wire.len();
        assert!(
            matches!(err, CodecError::Integrity { size, .. } if usize::from(size) == frame_len),
            "expected an integrity failure on this {frame_len}-byte frame, got {err:?}"
        );
        assert!(decoder.failure().is_some(), "the link must stay dead");
    }

    #[test]
    fn a_plaintext_decoder_never_touches_its_bytes_or_keys() {
        // Front-link parity: inbound is plaintext (Step 3.1), so decoding is
        // a pure framing pass — no cipher call, no integrity risk.
        let wire = plaintext_frame(30000, b"secret key incoming");
        let mut decoder = Decoder::new();
        decoder.feed(&wire).expect("valid");
        let frame = decoder.next_frame().expect("valid").expect("one");
        assert_eq!(frame.as_bytes(), &wire);
        assert!(decoder.cipher().is_none());
    }

    // ------------------------------------------------------- session transport

    #[tokio::test]
    async fn decoder_reassembles_a_dribbled_stream_read_off_the_session() {
        // Composition proof for Steps 2.1 + 2.2: frames written in four
        // dribbles over a real socket. TCP may coalesce or split them
        // arbitrarily on any given run — the decoder must reassemble the same
        // bytes every time either way.
        let frame_a = plaintext_frame(30000, b"front handshake");
        let frame_b = plaintext_frame(5, &[9, 8, 7, 6]);
        let mut wire = frame_a.clone();
        wire.extend_from_slice(&frame_b);

        let (listener, addr) = {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("addr");
            (listener, addr)
        };
        let writer = tokio::spawn(async move {
            let (mut stream, _addr) = listener.accept().await.expect("accept");
            let cuts = [0, 3, 9, wire.len() - 4, wire.len()];
            for window in cuts.windows(2) {
                stream
                    .write_all(&wire[window[0]..window[1]])
                    .await
                    .expect("write");
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            drop(stream);
        });

        let mut session = Session::connect(addr).await.expect("connect");
        let mut decoder = Decoder::new();
        let mut got: Vec<Vec<u8>> = Vec::new();

        loop {
            let mut buf = [0u8; 64];
            let read = session.read(&mut buf).await.expect("read");
            if read == 0 {
                break; // writer closed: EOF
            }
            decoder.feed(&buf[..read]).expect("valid stream");
            while let Some(frame) = decoder.next_frame().expect("valid stream") {
                got.push(frame.into_bytes());
            }
            if got.len() == 2 {
                break;
            }
        }
        writer.await.expect("writer task");

        assert_eq!(got, vec![frame_a, frame_b]);
        assert!(decoder.failure().is_none());
        assert!(decoder.is_empty());
    }
}
