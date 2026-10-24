use super::fixtures::CASES;
use super::*;

/// Fill the legacy harness used for its destination buffers: the legacy
/// `Decode` never writes output offset 2, so recorded outputs carry
/// whatever the destination already held.
const SENTINEL: u8 = 0xAA;

fn unhex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "odd-length hex {hex:?}");
    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex is ascii");
            u8::from_str_radix(text, 16).expect("hex digit")
        })
        .collect()
}

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
    assert_eq!(
        rx.decode(&wire, &mut dest),
        Err(CipherError::TooLarge(65001))
    );
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
    assert_eq!(cipher.decode(&[], &mut dest), Err(CipherError::Truncated));
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
