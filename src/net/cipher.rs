//! GoonZu ITZ link cipher — byte-exact client port of
//! `NetworkLib/ITZNetLib/ITZEncrypt.cpp`.
//!
//! This is the rolling XOR/add stream cipher applied on top of the 5-byte
//! [`crate::protocol::header`] frame. Semantics verified bit-exact against the
//! server port (`rust_Iwcgz_server/crates/gz-cipher`), whose vectors were
//! recorded from the **byte-for-byte unmodified legacy C++ compiled with g++**
//! (`gz-cipher/reference/`).
//!
//! # Wire format
//!
//! | Offset | Field | On the wire |
//! |---|---|---|
//! | 0..2 | `usSize` | little-endian total frame length, **in the clear** |
//! | 2 | `ucChk` | one-byte integrity/resync value |
//! | 3..4 | `usCmd` | enciphered |
//! | 5.. | payload | enciphered |
//!
//! Output length always equals input length; `usSize` stays readable in the
//! clear so `ITZRecvBuffer` can cut frames before any decryption
//! (`ITZRecvBuffer.cpp:155`).
//!
//! # State — both keys tick on *every* call
//!
//! `ITZEncrypt.cpp:33-34,72-73`: `m_ucKey1++`/`m_ucKey2++` run **before any
//! check**, so even the `size <= 3` passthrough branch and frames `decode`
//! later rejects advance the counter. Two peers stay in step only if they make
//! the same calls in the same order — exactly one `Cipher` per direction per
//! connection, and `set_key` is how a desynchronised link is repaired (the
//! keys *are* the counter).
//!
//! # Two instances per session, one direction each
//!
//! `ITZSession.cpp:13-14` creates two `ITZEncrypt` instances per session, each
//! used in exactly one direction:
//!
//! | | [`Cipher::session_cipher`] (`GetEncrypt`) | [`Cipher::link_cipher`] (`GetEncrypt2`) |
//! |---|---|---|
//! | Key | negotiated `(key1, key2)` | fixed `0x12, 0x34` |
//! | Direction | client → server (`Client-Winsock.cpp:146,195,242`) | server → client (`Client-Winsock.cpp:872`, applied by `ITZSendBuffer.cpp:124`) |
//! | Keyed | client on opcode 30000 (`Client-Winsock.cpp:228`) | `ITZSession::Init` (`ITZSession.cpp:56`) |
//!
//! # Deliberate deviations from the C++
//!
//! - **`decode` leaves `src` untouched.** The legacy decrements `pSrc[2]` in
//!   place (`ITZEncrypt.cpp:85`); this port computes the same value in a local
//!   (`gz-cipher` does the same).
//! - **Bounds checks instead of out-of-bounds reads.** The legacy reads past
//!   the caller's buffer when `usSize` lies (`ITZEncrypt.cpp:36,75`); this
//!   port returns [`CipherError`]. A lie that survives the checks still fails
//!   the integrity test, like the legacy.
//! - **`decode` never writes `dest[2]`** — byte-*compatible* with the legacy,
//!   whose first loop write is `dest[3]` (BUG-012: the decoded `ucChk` is
//!   whatever the caller's buffer already held; no legacy consumer reads it).

/// `itzbase.h:45`. `decode` rejects any declared size above this; `encode`
/// has no ceiling in the legacy (`ITZEncrypt.cpp` checks only in `Decode`).
pub const PACKET_MAX_SIZE: usize = 65000;

/// Fixed link-cipher key byte 1, set by `ITZSession::Init` (`ITZSession.cpp:56`).
pub const LINK_KEY1: u8 = 0x12;

/// Fixed link-cipher key byte 2, set by `ITZSession::Init` (`ITZSession.cpp:56`).
pub const LINK_KEY2: u8 = 0x34;

/// Why a cipher call could not be completed.
///
/// The legacy returns `void` from `Encode` and `FALSE` from `Decode`, and reads
/// out of bounds when a length field lies; those cases are surfaced here
/// instead of being silently undefined.
/// `Clone, Copy` so the Step 2.2 decoder can keep a cipher refusal on file
/// (sticky rejection) and hand it back by value, like `HeaderError`.
#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum CipherError {
    /// Fewer than 2 bytes, so the `usSize` field cannot be read at all.
    #[error("frame shorter than the 2-byte usSize field")]
    Truncated,
    /// `usSize` claims more bytes than `src` holds — the legacy would read
    /// past the end of the caller's buffer (`ITZEncrypt.cpp:36,75`).
    #[error("frame declares {declared} bytes but source holds {have}")]
    LengthMismatch {
        /// Size taken from the frame header.
        declared: u16,
        /// Bytes actually available in `src`.
        have: usize,
    },
    /// Declared size above [`PACKET_MAX_SIZE`]; `Decode` returns `FALSE`
    /// (`ITZEncrypt.cpp:77`).
    #[error("declared size {0} exceeds PACKET_MAX_SIZE")]
    TooLarge(u16),
    /// The destination buffer is shorter than the declared size.
    #[error("destination holds {have} bytes, frame declares {declared}")]
    DestTooSmall {
        /// Size taken from the frame header.
        declared: u16,
        /// Bytes actually available in `dest`.
        have: usize,
    },
}

/// One `ITZEncrypt` instance: two key bytes that tick once per call.
///
/// Not `Sync`; the legacy relied on the enclosing session's critical section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cipher {
    key1: u8,
    key2: u8,
}

impl Cipher {
    /// Both keys at zero.
    ///
    /// The legacy constructor is empty, so its keys are whatever the fresh
    /// allocation held; zero is this port's defined choice. Prefer
    /// [`Cipher::link_cipher`] and [`Cipher::session_cipher`], the two
    /// instances the legacy actually configures.
    #[must_use]
    pub const fn new() -> Self {
        Self { key1: 0, key2: 0 }
    }

    /// The negotiated-key instance (`ITZSession::GetEncrypt`): client → server.
    ///
    /// Seeded from `sResponse_SecretKey` on opcode 30000
    /// (`Client-Winsock.cpp:228`); this is the "Dynamic Session Cipher"
    /// initialised from seed bytes.
    #[must_use]
    pub const fn session_cipher(key1: u8, key2: u8) -> Self {
        Self { key1, key2 }
    }

    /// The fixed-key instance (`ITZSession::GetEncrypt2`): server → client on
    /// the game link, keyed `0x12, 0x34` by `ITZSession::Init`.
    #[must_use]
    pub const fn link_cipher() -> Self {
        Self {
            key1: LINK_KEY1,
            key2: LINK_KEY2,
        }
    }

    /// `SetKey` (`ITZEncrypt.cpp:14`). Also resets the counter position,
    /// because the keys *are* the counter — the way to resynchronise a link.
    pub fn set_key(&mut self, key1: u8, key2: u8) {
        self.key1 = key1;
        self.key2 = key2;
    }

    /// `GetKey1` (`ITZEncrypt.cpp:20`): the value at the current call
    /// position, not the initial key.
    #[must_use]
    pub const fn key1(&self) -> u8 {
        self.key1
    }

    /// `GetKey2` (`ITZEncrypt.cpp:25`): the value at the current call
    /// position, not the initial key.
    #[must_use]
    pub const fn key2(&self) -> u8 {
        self.key2
    }

    /// Advances both keys one step — the **one** place the tick happens, on
    /// every [`Cipher::encode`] and [`Cipher::decode`] call.
    ///
    /// `ITZEncrypt.cpp:33-34,72-73`: the increments run **before any check**,
    /// so the `size <= 3` passthrough branch and frames that go on to be
    /// rejected all advance the counter. Two peers stay in step only if they
    /// make the same calls in the same order (module docs).
    #[inline]
    fn tick(&mut self) {
        self.key1 = self.key1.wrapping_add(1);
        self.key2 = self.key2.wrapping_add(1);
    }

    /// Little-endian `usSize` from the front of a frame.
    fn declared_len(src: &[u8]) -> Result<usize, CipherError> {
        let raw: [u8; 2] = src
            .get(..2)
            .ok_or(CipherError::Truncated)?
            .try_into()
            .expect("slice of length 2");
        Ok(u16::from_le_bytes(raw) as usize)
    }

    /// `ITZEncrypt::Encode` (`ITZEncrypt.cpp:30`). Returns the `size` bytes
    /// the legacy call writes; output length equals input length.
    ///
    /// Advances both keys first — including on the `size <= 3` passthrough
    /// branch (`ITZEncrypt.cpp:33-42`).
    ///
    /// # Errors
    ///
    /// [`CipherError::Truncated`] or [`CipherError::LengthMismatch`] when the
    /// length field is unreadable or overruns `src` (the legacy reads out of
    /// bounds instead).
    pub fn encode(&mut self, src: &[u8]) -> Result<Vec<u8>, CipherError> {
        self.tick();

        let size = Self::declared_len(src)?;
        if src.len() < size {
            return Err(CipherError::LengthMismatch {
                declared: size as u16,
                have: src.len(),
            });
        }

        // size <= 3: verbatim copy, no integrity byte written
        // (ITZEncrypt.cpp:38-42). Unreachable for real frames — the opcode at
        // offset 3 needs size >= 5 — but reproduced exactly.
        if size <= 3 {
            return Ok(src[..size].to_vec());
        }

        let mut dest = vec![0u8; size];
        // Offset 2: integrity slot = last plaintext byte + key1
        // (ITZEncrypt.cpp:44). The sender's own ucChk is never transmitted.
        dest[2] = src[size - 1].wrapping_add(self.key1);
        // usSize goes out in the clear, little-endian (ITZEncrypt.cpp:51).
        dest[..2].copy_from_slice(&(size as u16).to_le_bytes());

        // Rolling XOR/add over offsets 3.. (ITZEncrypt.cpp:55-65).
        let mut process1 = self.key1;
        let mut process2 = self.key2;
        for offset in 3..size {
            process1 ^= src[offset];
            dest[offset] = process1.wrapping_add(process2);
            process2 = process2.wrapping_add(src[offset]);
        }
        Ok(dest)
    }

    /// `ITZEncrypt::Decode` (`ITZEncrypt.cpp:69`). Returns whether the
    /// integrity byte matched — the legacy's `TRUE`/`FALSE`.
    ///
    /// Advances both keys first, like the legacy, **including on frames it
    /// later rejects**.
    ///
    /// `dest` must be at least `size` bytes. **`dest[2]` is left untouched**
    /// (`ITZEncrypt.cpp:96-100` writes its first byte at `dest[3]`); the
    /// decoded `ucChk` is whatever the caller's buffer already held — no
    /// legacy consumer ever reads it out of a decoded buffer (BUG-012).
    ///
    /// # Errors
    ///
    /// [`CipherError::Truncated`], [`CipherError::TooLarge`] (the legacy's
    /// `return FALSE`, `ITZEncrypt.cpp:77`), [`CipherError::LengthMismatch`],
    /// or [`CipherError::DestTooSmall`].
    pub fn decode(&mut self, src: &[u8], dest: &mut [u8]) -> Result<bool, CipherError> {
        self.tick();

        let size = Self::declared_len(src)?;
        // Check order matches the legacy: size ceiling (ITZEncrypt.cpp:77)
        // before the buffer checks that replace its out-of-bounds reads.
        if size > PACKET_MAX_SIZE {
            return Err(CipherError::TooLarge(size as u16));
        }
        if src.len() < size {
            return Err(CipherError::LengthMismatch {
                declared: size as u16,
                have: src.len(),
            });
        }
        if dest.len() < size {
            return Err(CipherError::DestTooSmall {
                declared: size as u16,
                have: dest.len(),
            });
        }

        // size <= 3: verbatim copy, always accepted (ITZEncrypt.cpp:79-83).
        if size <= 3 {
            dest[..size].copy_from_slice(&src[..size]);
            return Ok(true);
        }

        // The legacy rewrites src[2] in place (ITZEncrypt.cpp:85); computing
        // the same value in a local keeps `src` immutable (see module docs).
        let integrity = src[2].wrapping_sub(self.key1);

        // Rolling inverse over offsets 3.. (ITZEncrypt.cpp:98-109).
        let mut process1 = self.key1;
        let mut process2 = self.key2;
        dest[..2].copy_from_slice(&(size as u16).to_le_bytes());
        for offset in 3..size {
            let mut plain = src[offset].wrapping_sub(process2);
            plain ^= process1;
            dest[offset] = plain;
            process1 ^= plain;
            process2 = process2.wrapping_add(plain);
        }

        // The legacy's trailing check (ITZEncrypt.cpp:111): integrity byte vs
        // the last decoded plaintext byte.
        Ok(integrity == dest[size - 1])
    }
}

impl Default for Cipher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod tests;
