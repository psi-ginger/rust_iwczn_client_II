//! Recorded legacy cipher vectors for the `cipher` tests.
//!
//! Extracted from the cipher test module by Step 2.4.1 (scan & refactor
//! MOD-01) so the implementation file carries implementation only. This
//! is recorded ground truth captured from the byte-for-byte unmodified
//! `ITZEncrypt.cpp` compiled with g++ (the server repo's
//! `gz-cipher/reference` harness); it must not be edited to make a test
//! pass - a disagreement means this port is wrong, not that the fixture
//! moved.

/// One recorded call against a cipher instance, as captured from the
/// legacy C++.
pub struct Op {
    /// `true` for `Decode`, `false` for `Encode`.
    pub decode: bool,
    /// `true` when the input is the previous op's output (round trip).
    pub follow: bool,
    /// Hex input; ignored when `follow`.
    pub input: &'static str,
    /// Hex bytes the call wrote (the `size` bytes the legacy defines).
    pub output: &'static str,
    /// `Decode`'s return value; always `true` for `Encode`.
    pub accepted: bool,
}

/// A recorded sequence against one cipher pair, `key1`/`key2` initial.
pub struct Case {
    pub name: &'static str,
    pub key1: u8,
    pub key2: u8,
    pub ops: &'static [Op],
}

/// Bit-exact legacy vectors, recorded by the server repo's harness from
/// the **byte-for-byte unmodified** `ITZEncrypt.cpp` compiled with g++
/// (`rust_Iwcgz_server/crates/gz-cipher/reference/`, confirmed by an
/// independent Python reference). Nothing here was derived from any Rust
/// implementation, so a mismatch means this port is wrong.
pub const CASES: &[Case] = &[
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
