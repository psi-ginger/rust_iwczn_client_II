//! GoonZu wire frame header — 5 bytes, little-endian, `#pragma pack(1)`.
//!
//! C++ truth ([`GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h:63-68`], packed):
//!
//! ```c
//! struct sPacketHeader {
//!     UI16 usSize;   // total frame length in bytes, header included
//!     UI08 ucChk;    // reserved: never computed, never validated
//!     UI16 usCmd;    // opcode; bit 15 is the LZF compression flag
//! };
//! ```
//!
//! Wire order is little-endian: the default `headerType == 0` path reads
//! `*(UI16 *)ptr` directly with no `ntohs` (`ITZRecvBuffer.cpp:155`) and
//! copies bytes verbatim on send (`ITZSendBuffer.cpp:132`).
//!
//! [`ucChk` is not a checksum](#the-reserved-byte-is-not-a-checksum) and
//! there is no `param` field — the real header is 5 bytes, not 8. The only
//! 8-byte framing in the legacy (`headerType == 1`, `ntohs(size) + 8`) has no
//! call site in the entire tree and is not implemented by the server
//! (`rust_Iwcgz_server/crates/gz-net`). See
//! `docs/rca/2026-10-23-step-1.2-header-spec-mismatch.md`.
//!
//! # The reserved byte is not a checksum
//!
//! No legacy producer ever writes `ucChk` and no consumer validates it; the
//! server port documents offset 2 as "Deliberately *not* named `chk`, because
//! it is not a checksum ... Treat it as reserved: do not build integrity on
//! it" (`gz-net/src/lib.rs`). On the encrypted game link the cipher overwrites
//! it with a one-byte resync tag (`ITZEncrypt.cpp:44`), which is Step 1.3's
//! concern, not framing's. This module carries the byte through untouched.
//!
//! # Size guards
//!
//! Mirrors the legacy receive path (`ITZRecvBuffer.cpp:163-180`):
//! reject declared size `< 2` and `> PACKET_MAX_SIZE - 8` (64992), plus the
//! server's deliberate divergence: reject `2..=4` as too small to hold a
//! header (legacy BUG-013 accepts them; `gz-net` rejects them, and so do we).
//!
//! [`GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h:63-68`]: ../../GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h

use thiserror::Error;

/// Wire header size in bytes: `usSize`(2) + `ucChk`(1) + `usCmd`(2).
pub const HEADER_SIZE: usize = 5;

/// `PACKET_MAX_SIZE` from `itzbase.h:45`.
pub const PACKET_MAX_SIZE: u16 = 65000;

/// Largest accepted declared frame size: `PACKET_MAX_SIZE - 8`
/// (`ITZRecvBuffer.cpp:172`).
pub const MAX_FRAME_SIZE: u16 = PACKET_MAX_SIZE - 8;

/// LZF compression flag: bit 15 of `usCmd`. The legacy tests it before
/// decompressing and clears it only *afterwards* (`GameProtocol.cpp:109,141`,
/// `Client-Winsock.cpp:896`), so framing exposes it but never masks it.
pub const COMPRESSION_FLAG: u16 = 0x8000;

/// Errors produced while validating or decoding a wire header.
///
/// `Clone, Copy`: the Step 2.2 decoder stores a refusal by value to keep it
/// sticky for the connection's life (like `gz-net::FrameError`), so its error
/// type must be storable and re-returnable without a heap allocation.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum HeaderError {
    /// Declared size below the legacy's own `< 2` guard
    /// (`ITZRecvBuffer.cpp:163`).
    #[error("declared size {declared} is below the legacy minimum of 2")]
    SizeBelowMinimum { declared: u16 },
    /// Declared size in `2..=4`: fits the legacy guard but cannot hold a
    /// 5-byte header (legacy BUG-013; rejected here for parity with
    /// `gz-net`).
    #[error("declared size {declared} is too small to hold a {HEADER_SIZE}-byte header")]
    SizeBelowHeader { declared: u16 },
    /// Declared size above [`MAX_FRAME_SIZE`] (`ITZRecvBuffer.cpp:172`).
    #[error("declared size {declared} exceeds the {MAX_FRAME_SIZE}-byte receive limit")]
    SizeTooLarge { declared: u16 },
    /// The buffer handed to [`WireHeader::decode`] is shorter than
    /// [`HEADER_SIZE`], so there is no header to read.
    #[error("buffer of {len} bytes is too short for a {HEADER_SIZE}-byte header")]
    BufferTooShort { len: usize },
    /// A locally built frame's payload would push the declared size past
    /// [`MAX_FRAME_SIZE`]; a peer would refuse it, so we do not emit it.
    #[error("payload of {len} bytes does not fit a {MAX_FRAME_SIZE}-byte frame")]
    PayloadTooLarge { len: usize },
}

impl HeaderError {
    /// The declared size that caused the rejection, when the error carries one.
    #[must_use]
    pub const fn declared(&self) -> Option<u16> {
        match self {
            Self::SizeBelowMinimum { declared }
            | Self::SizeBelowHeader { declared }
            | Self::SizeTooLarge { declared } => Some(*declared),
            Self::BufferTooShort { .. } | Self::PayloadTooLarge { .. } => None,
        }
    }
}

/// The one place the size guards live, so encode and decode cannot drift.
///
/// # Errors
///
/// [`HeaderError::SizeBelowMinimum`] below 2 (the legacy guard),
/// [`HeaderError::SizeBelowHeader`] for `2..=4` (rejected here, accepted by
/// the legacy — BUG-013, parity with `gz-net::validate_size`), and
/// [`HeaderError::SizeTooLarge`] above [`MAX_FRAME_SIZE`].
#[inline]
pub const fn validate_size(declared: u16) -> Result<(), HeaderError> {
    if declared < 2 {
        return Err(HeaderError::SizeBelowMinimum { declared });
    }
    if declared < HEADER_SIZE as u16 {
        return Err(HeaderError::SizeBelowHeader { declared });
    }
    if declared > MAX_FRAME_SIZE {
        return Err(HeaderError::SizeTooLarge { declared });
    }
    Ok(())
}

/// One wire header, exactly as it goes on or comes off the wire.
///
/// Fields are kept in decoded form; [`WireHeader::decode`] validates the
/// declared size against [`validate_size`] so a caller can never observe a
/// header the receive path would have dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireHeader {
    /// `usSize`: total frame length in bytes, header included.
    size: u16,
    /// `ucChk`: reserved byte, carried through untouched (see module docs).
    integrity: u8,
    /// `usCmd`: opcode, with bit 15 as the compression flag.
    cmd: u16,
}

impl WireHeader {
    /// Builds a header for `cmd` and a payload of `payload_len` bytes, with
    /// the reserved byte zeroed (matching the server's `Frame::new`).
    ///
    /// # Errors
    ///
    /// [`HeaderError::PayloadTooLarge`] when `HEADER_SIZE + payload_len`
    /// exceeds [`MAX_FRAME_SIZE`] — bounding a locally built frame by the
    /// same limit the receive guards enforce keeps us from emitting a frame
    /// a peer would refuse.
    pub fn new(cmd: u16, payload_len: usize) -> Result<Self, HeaderError> {
        let size = HEADER_SIZE + payload_len;
        if size > MAX_FRAME_SIZE as usize {
            return Err(HeaderError::PayloadTooLarge { len: payload_len });
        }
        Ok(Self {
            size: size as u16,
            integrity: 0,
            cmd,
        })
    }

    /// Decodes a header from the front of `bytes` and validates its declared
    /// size against [`validate_size`].
    ///
    /// Does not require the full frame to be buffered — that is the framing
    /// layer's (Step 2.2) job; only the 5 header bytes and the declared-size
    /// guard are checked here.
    ///
    /// # Errors
    ///
    /// [`HeaderError::BufferTooShort`] when `bytes.len() < HEADER_SIZE`, then
    /// any error from [`validate_size`] for the declared `usSize`.
    pub fn decode(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < HEADER_SIZE {
            return Err(HeaderError::BufferTooShort { len: bytes.len() });
        }
        let size = u16::from_le_bytes([bytes[0], bytes[1]]);
        validate_size(size)?;
        Ok(Self {
            size,
            integrity: bytes[2],
            cmd: u16::from_le_bytes([bytes[3], bytes[4]]),
        })
    }

    /// Serializes to the 5-byte little-endian wire layout
    /// `[usSize LE][ucChk][usCmd LE]`.
    #[must_use]
    pub const fn encode(&self) -> [u8; HEADER_SIZE] {
        let size = self.size.to_le_bytes();
        let cmd = self.cmd.to_le_bytes();
        [size[0], size[1], self.integrity, cmd[0], cmd[1]]
    }

    /// `usSize`: total frame length in bytes, header included.
    #[must_use]
    pub const fn size(&self) -> u16 {
        self.size
    }

    /// The reserved byte (`ucChk`). **Not a checksum** — do not build
    /// integrity on it; on encrypted links it is the cipher's resync tag.
    #[must_use]
    pub const fn integrity(&self) -> u8 {
        self.integrity
    }

    /// `usCmd` as stored, including the compression flag bit when set.
    #[must_use]
    pub const fn cmd(&self) -> u16 {
        self.cmd
    }

    /// Payload length implied by `usSize` (frame minus header).
    ///
    /// Valid because [`Self::decode`] and [`Self::new`] both guarantee
    /// `size >= HEADER_SIZE`.
    #[must_use]
    pub const fn payload_len(&self) -> usize {
        self.size as usize - HEADER_SIZE
    }

    /// Whether the compression flag (bit 15) is set. Only meaningful once
    /// the frame has been deciphered, if the link is encrypted.
    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        self.cmd & COMPRESSION_FLAG != 0
    }

    /// The opcode with the compression flag cleared — what dispatch tables
    /// expect, valid once the payload has been decompressed.
    #[must_use]
    pub const fn cmd_id(&self) -> u16 {
        self.cmd & !COMPRESSION_FLAG
    }
}

#[cfg(test)]
mod tests;
