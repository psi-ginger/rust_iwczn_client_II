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
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
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
        self.key1 = self.key1.wrapping_add(1);
        self.key2 = self.key2.wrapping_add(1);

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
        self.key1 = self.key1.wrapping_add(1);
        self.key2 = self.key2.wrapping_add(1);

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
mod tests {
    use super::*;

    /// Fill the legacy harness used for its destination buffers: the legacy
    /// `Decode` never writes output offset 2, so recorded outputs carry
    /// whatever the destination already held.
    const SENTINEL: u8 = 0xAA;

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

    /// One recorded call against a cipher instance, as captured from the
    /// legacy C++.
    struct Op {
        /// `true` for `Decode`, `false` for `Encode`.
        decode: bool,
        /// `true` when the input is the previous op's output (round trip).
        follow: bool,
        /// Hex input; ignored when `follow`.
        input: &'static str,
        /// Hex bytes the call wrote (the `size` bytes the legacy defines).
        output: &'static str,
        /// `Decode`'s return value; always `true` for `Encode`.
        accepted: bool,
    }

    /// A recorded sequence against one cipher pair, `key1`/`key2` initial.
    struct Case {
        name: &'static str,
        key1: u8,
        key2: u8,
        ops: &'static [Op],
    }

    /// Bit-exact legacy vectors, recorded by the server repo's harness from
    /// the **byte-for-byte unmodified** `ITZEncrypt.cpp` compiled with g++
    /// (`rust_Iwcgz_server/crates/gz-cipher/reference/`, confirmed by an
    /// independent Python reference). Nothing here was derived from any Rust
    /// implementation, so a mismatch means this port is wrong.
    const CASES: &[Case] = &[
        Case {
            name: "A1_encode_passthrough_size0",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "0000", output: "", accepted: true }],
        },
        Case {
            name: "A2_encode_passthrough_size1",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "0100ab", output: "01", accepted: true }],
        },
        Case {
            name: "A3_encode_passthrough_size2",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "0200abcd", output: "0200", accepted: true }],
        },
        Case {
            name: "A4_encode_passthrough_size3",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "0300abcdef", output: "0300ab", accepted: true }],
        },
        // The passthrough branch still ticks the keys.
        Case {
            name: "A5_passthrough_ticks_keys",
            key1: 0,
            key2: 0,
            ops: &[
                Op { decode: false, follow: false, input: "0300abcdef", output: "0300ab", accepted: true },
                Op { decode: false, follow: false, input: "04000011", output: "04001315", accepted: true },
            ],
        },
        Case {
            name: "B1_encode_size4",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "04000011", output: "04001211", accepted: true }],
        },
        Case {
            name: "B2_encode_size5",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "0500001122", output: "0500231144", accepted: true }],
        },
        Case {
            name: "B3_encode_size8",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: false, follow: false, input: "08000011223344556677", output: "080056114435acbb", accepted: true }],
        },
        Case {
            name: "B4_encode_size64",
            key1: 0,
            key2: 0,
            ops: &[Op {
                decode: false,
                follow: false,
                input: "4000030a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d141b222930373e454c535a61686f767d848b9299a0a7ae",
                output: "4000af0c251e518e8fc8f37069e20592d34ca7342d26395657909bf8f1ea6d7a1b14cfdcb5aea19e9f98c3c0f9f2556263dcf784bd36892667606b88817abdca",
                accepted: true,
            }],
        },
        // Fixed link-cipher keys 0x12, 0x34.
        Case {
            name: "C1_enc2_encode_size8",
            key1: 18,
            key2: 52,
            ops: &[Op { decode: false, follow: false, input: "0800aabbccddeeff", output: "080012dd5475f02f", accepted: true }],
        },
        // Bytes past usSize are ignored.
        Case {
            name: "C8_trailing_byte_ignored",
            key1: 18,
            key2: 52,
            ops: &[Op { decode: false, follow: false, input: "0800aabbccddeeff99", output: "080012dd5475f02f", accepted: true }],
        },
        Case {
            name: "C2_enc2_encode_size16",
            key1: 18,
            key2: 52,
            ops: &[Op { decode: false, follow: false, input: "1000f0efeeedecebeae9e8e7e6e5e4e3", output: "1000f631361112e3e8bbbc858a555617", accepted: true }],
        },
        Case {
            name: "C3_enc2_counter_advances",
            key1: 18,
            key2: 52,
            ops: &[
                Op { decode: false, follow: false, input: "0800aabbccddeeff", output: "080012dd5475f02f", accepted: true },
                Op { decode: false, follow: false, input: "0800aabbccddeeff", output: "080013e5547bea37", accepted: true },
            ],
        },
        Case {
            name: "C4_enc2_roundtrip",
            key1: 18,
            key2: 52,
            ops: &[
                Op { decode: false, follow: false, input: "0800aabbccddeeff", output: "080012dd5475f02f", accepted: true },
                Op { decode: true, follow: true, input: "080012dd5475f02f", output: "0800aabbccddeeff", accepted: true },
            ],
        },
        Case {
            name: "C5_enc2_roundtrip_size16",
            key1: 18,
            key2: 52,
            ops: &[
                Op { decode: false, follow: false, input: "1000f0efeeedecebeae9e8e7e6e5e4e3", output: "1000f631361112e3e8bbbc858a555617", accepted: true },
                Op { decode: true, follow: true, input: "1000f631361112e3e8bbbc858a555617", output: "1000aaefeeedecebeae9e8e7e6e5e4e3", accepted: true },
            ],
        },
        Case {
            name: "C6_enc2_roundtrip_size64",
            key1: 18,
            key2: 52,
            ops: &[
                Op {
                    decode: false,
                    follow: false,
                    input: "4000030a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d141b222930373e454c535a61686f767d848b9299a0a7ae",
                    output: "4000c14e476077b0b10a19928b242bd4158ecd766f687f7899d2e11a332cb3bc3d56151ed7f0c7c0c1dae9e21b347ba4a51e1dc6ff78cf48a9a2b1aac3bc030c",
                    accepted: true,
                },
                Op {
                    decode: true,
                    follow: true,
                    input: "4000c14e476077b0b10a19928b242bd4158ecd766f687f7899d2e11a332cb3bc3d56151ed7f0c7c0c1dae9e21b347ba4a51e1dc6ff78cf48a9a2b1aac3bc030c",
                    output: "4000aa0a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d141b222930373e454c535a61686f767d848b9299a0a7ae",
                    accepted: true,
                },
            ],
        },
        Case {
            name: "C7_enc2_roundtrip_burst",
            key1: 18,
            key2: 52,
            ops: &[
                Op { decode: false, follow: false, input: "04000011", output: "04002437", accepted: true },
                Op { decode: true, follow: true, input: "04002437", output: "0400aa11", accepted: true },
                Op { decode: false, follow: false, input: "0700112233445566", output: "0700696c5dcce3", accepted: true },
                Op { decode: true, follow: true, input: "0700696c5dcce3", output: "0700aa22334455", accepted: true },
                Op {
                    decode: false,
                    follow: false,
                    input: "4000010e1b2835424f5c697683909daab7c4d1deebf805121f2c394653606d7a8794a1aebbc8d5e2effc091623303d4a5764717e8b98a5b2bfccd9e6f3000d1a",
                    output: "40002f524588a51c0f9acf66594c79b0a39e333aed100d04176257cee194a1584b665ba29598356cdf2a1f36295c8900f3aec38a7da09dd4e7f2679e31243128",
                    accepted: true,
                },
                Op {
                    decode: true,
                    follow: true,
                    input: "40002f524588a51c0f9acf66594c79b0a39e333aed100d04176257cee194a1584b665ba29598356cdf2a1f36295c8900f3aec38a7da09dd4e7f2679e31243128",
                    output: "4000aa0e1b2835424f5c697683909daab7c4d1deebf805121f2c394653606d7a8794a1aebbc8d5e2effc091623303d4a5764717e8b98a5b2bfccd9e6f3000d1a",
                    accepted: true,
                },
            ],
        },
        Case {
            name: "D1_encode_wrap_ff",
            key1: 255,
            key2: 255,
            ops: &[Op { decode: false, follow: false, input: "06000102030405", output: "06000402030a", accepted: true }],
        },
        Case {
            name: "D2_encode_wrap_across_calls",
            key1: 254,
            key2: 255,
            ops: &[
                Op { decode: false, follow: false, input: "050010203040", output: "05002fdf0f", accepted: true },
                Op { decode: false, follow: false, input: "050010203040", output: "0500302131", accepted: true },
            ],
        },
        Case {
            name: "D3_roundtrip_wrap",
            key1: 255,
            key2: 254,
            ops: &[
                Op { decode: false, follow: false, input: "06000102030405", output: "060004010209", accepted: true },
                Op { decode: true, follow: false, input: "06000102030405", output: "0600aa030201", accepted: true },
            ],
        },
        Case {
            name: "E1_decode_valid",
            key1: 0,
            key2: 0,
            ops: &[
                Op { decode: false, follow: false, input: "04000011", output: "04001211", accepted: true },
                Op { decode: true, follow: true, input: "04001211", output: "0400aa11", accepted: true },
            ],
        },
        Case {
            name: "E2_decode_corrupt_payload",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: true, follow: false, input: "0400001235", output: "0400aa10", accepted: false }],
        },
        Case {
            name: "E4_decode_passthrough_size3",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: true, follow: false, input: "0300abcdef", output: "0300ab", accepted: true }],
        },
        Case {
            name: "E5_decode_passthrough_size0",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: true, follow: false, input: "0000", output: "", accepted: true }],
        },
        Case {
            name: "E6_decode_wrong_key",
            key1: 153,
            key2: 153,
            ops: &[Op { decode: true, follow: false, input: "0400001234", output: "0400aae2", accepted: false }],
        },
        Case {
            name: "E7_decode_corrupt_checksum_slot",
            key1: 0,
            key2: 0,
            ops: &[Op { decode: true, follow: false, input: "0500ff001122", output: "0500aafeed", accepted: false }],
        },
    ];

    /// Replay every recorded vector: outputs must be byte-exact and both
    /// peer counters must land on the same position.
    #[test]
    fn every_vector_matches_the_legacy() {
        let mut checked = 0usize;
        for case in CASES {
            // Paired instances: the legacy runs one cipher per link end, and
            // the counters stay aligned because every call advances both.
            let mut tx = Cipher::session_cipher(case.key1, case.key2);
            let mut rx = Cipher::session_cipher(case.key1, case.key2);

            let mut last_out: Vec<u8> = Vec::new();
            for (index, op) in case.ops.iter().enumerate() {
                let input = if op.follow {
                    last_out.clone()
                } else {
                    unhex(op.input)
                };
                let expected = unhex(op.output);
                let where_ = format!("{} op {index}", case.name);

                if op.decode {
                    let mut dest = vec![SENTINEL; expected.len()];
                    let accepted = rx
                        .decode(&input, &mut dest)
                        .unwrap_or_else(|err| panic!("{where_}: unexpected {err}"));
                    assert_eq!(accepted, op.accepted, "{where_}: accepted");
                    assert_eq!(dest, expected, "{where_}: output");
                    last_out = expected;
                } else {
                    let out = tx
                        .encode(&input)
                        .unwrap_or_else(|err| panic!("{where_}: unexpected {err}"));
                    assert_eq!(out, expected, "{where_}: output");
                    last_out = out;
                }
                checked += 1;
            }

            // A case driving both directions must land both counters on the
            // same position, or the round trip could not have verified.
            if case.ops.iter().any(|op| op.decode) && case.ops.iter().any(|op| !op.decode) {
                assert_eq!(
                    (tx.key1(), tx.key2()),
                    (rx.key1(), rx.key2()),
                    "{}: peer counters diverged",
                    case.name
                );
            }
        }
        assert!(checked >= 39, "expected the full vector set, ran {checked}");
    }

    /// The 65 KB oversize frame cannot be checked in as hex, so drive it
    /// directly: `Decode` rejects it before touching the body, and the keys
    /// have already ticked (legacy `ITZEncrypt.cpp:72-77`).
    #[test]
    fn oversize_frame_is_rejected_without_writing_output() {
        let mut cipher = Cipher::new();
        let mut dest = [SENTINEL; 8];
        assert_eq!(
            cipher.decode(&[0xff, 0xff], &mut dest),
            Err(CipherError::TooLarge(0xffff))
        );
        assert_eq!(dest, [SENTINEL; 8], "no output may be written");
        assert_eq!((cipher.key1(), cipher.key2()), (1, 1), "keys still ticked");
    }

    /// `Encode` has no size ceiling in the legacy, so it builds frames
    /// `Decode` then refuses. Preserved deliberately: tidying it up would
    /// accept frames the shipped peer never sends.
    #[test]
    fn encode_has_no_size_ceiling_but_decode_does() {
        let mut tx = Cipher::new();
        let mut rx = Cipher::new();
        let mut frame = vec![0u8; 65001];
        frame[..2].copy_from_slice(&(65001u16).to_le_bytes());

        let wire = tx
            .encode(&frame)
            .expect("legacy Encode does not check the size");
        assert_eq!(&wire[..2], &65001u16.to_le_bytes());

        let mut dest = vec![SENTINEL; wire.len()];
        assert_eq!(rx.decode(&wire, &mut dest), Err(CipherError::TooLarge(65001)));
    }

    /// The link cipher is keyed 0x12, 0x34 by `ITZSession::Init`
    /// (`ITZSession.cpp:56`), and `PACKET_MAX_SIZE` is 65000
    /// (`itzbase.h:45`).
    #[test]
    fn link_cipher_and_size_constants_match_the_legacy() {
        assert_eq!((LINK_KEY1, LINK_KEY2), (0x12, 0x34));
        assert_eq!(PACKET_MAX_SIZE, 65000);
        let cipher = Cipher::link_cipher();
        assert_eq!((cipher.key1(), cipher.key2()), (0x12, 0x34));
    }

    /// A full link-cipher round trip on real frame geometry: 5-byte header +
    /// payload, both peers keyed 0x12/0x34, usSize readable in the clear.
    #[test]
    fn link_cipher_roundtrip_on_a_real_frame() {
        let mut tx = Cipher::link_cipher();
        let mut rx = Cipher::link_cipher();

        // usSize = 10, ucChk = 0 (overwritten anyway), usCmd = 30000, 5-byte
        // payload.
        let mut frame = vec![0u8; 10];
        frame[..2].copy_from_slice(&10u16.to_le_bytes());
        frame[3..5].copy_from_slice(&30000u16.to_le_bytes());
        frame[5..].copy_from_slice(&[1, 2, 3, 4, 5]);

        let wire = tx.encode(&frame).unwrap();
        assert_eq!(wire.len(), frame.len(), "output length equals input");
        assert_eq!(&wire[..2], &frame[..2], "usSize stays in the clear");
        assert_ne!(&wire[3..], &frame[3..], "opcode+payload are enciphered");

        let mut plain = vec![SENTINEL; wire.len()];
        assert!(rx.decode(&wire, &mut plain).expect("decodes"));
        assert_eq!(&plain[..2], &frame[..2]);
        assert_eq!(&plain[3..], &frame[3..]);
        assert_eq!((tx.key1(), tx.key2()), (rx.key1(), rx.key2()));
    }

    /// Reversed roles: session-cipher encode into link-cipher decode must
    /// fail the integrity check (wrong key), like the legacy's `FALSE`.
    #[test]
    fn mismatched_key_pair_fails_the_integrity_check() {
        let mut tx = Cipher::session_cipher(7, 9);
        let mut rx = Cipher::link_cipher();
        let mut frame = vec![0u8; 8];
        frame[..2].copy_from_slice(&8u16.to_le_bytes());

        let wire = tx.encode(&frame).unwrap();
        let mut plain = vec![SENTINEL; wire.len()];
        assert!(!rx.decode(&wire, &mut plain).unwrap(), "must not accept");
    }

    /// The legacy decrements its *source* buffer at offset 2 in place; this
    /// port deliberately does not (module docs, deviation 1).
    #[test]
    fn decode_leaves_the_source_untouched() {
        let mut tx = Cipher::link_cipher();
        let mut rx = Cipher::link_cipher();
        let frame = [0x08, 0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let wire = tx.encode(&frame).unwrap();
        let before = wire.clone();
        let mut plain = vec![0u8; frame.len()];
        assert!(rx.decode(&wire, &mut plain).unwrap());
        assert_eq!(wire, before, "src must not be rewritten");
        assert_eq!(&plain[..2], &frame[..2]);
        assert_eq!(&plain[3..], &frame[3..]);
    }

    /// A lying length field is surfaced, not read out of bounds like the
    /// legacy (`ITZEncrypt.cpp:36,75`).
    #[test]
    fn lying_length_fields_are_rejected() {
        let mut cipher = Cipher::new();
        assert_eq!(
            cipher.encode(&[0x08, 0x00, 0x01]),
            Err(CipherError::LengthMismatch {
                declared: 8,
                have: 3
            })
        );
        let mut dest = [0u8; 8];
        assert_eq!(
            cipher.decode(&[], &mut dest),
            Err(CipherError::Truncated)
        );
        assert_eq!(
            cipher.decode(&[0x04, 0x00, 0x01], &mut dest),
            Err(CipherError::LengthMismatch {
                declared: 4,
                have: 3
            })
        );
        assert_eq!(
            cipher.decode(&[0x04, 0x00, 0x00, 0x11], &mut dest[..3]),
            Err(CipherError::DestTooSmall {
                declared: 4,
                have: 3
            })
        );
    }
}
