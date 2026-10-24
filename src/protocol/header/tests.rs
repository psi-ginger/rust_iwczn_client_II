use super::*;

/// `FRONTMSG_RESPONSE_SECRETKEY` from `Server/FrontServer/FrontMsg.h`.
const SECRETKEY_CMD: u16 = 30000;

#[test]
fn header_is_five_bytes() {
    // C++: #pragma pack(1) over UI16(2) + UI08(1) + UI16(2) = 5 bytes.
    assert_eq!(HEADER_SIZE, 5);
    assert_eq!(WireHeader::new(0, 0).unwrap().encode().len(), 5);
}

#[test]
fn encode_is_bit_exact_little_endian() {
    // usSize = 42 = 0x002A -> LE [0x2A, 0x00]; ucChk = 0;
    // usCmd = 30000 = 0x7530 -> LE [0x30, 0x75].
    let header = WireHeader::new(SECRETKEY_CMD, 37).unwrap();
    assert_eq!(
        header.encode(),
        [0x2A, 0x00, 0x00, 0x30, 0x75],
        "wire layout must be [usSize LE][ucChk][usCmd LE]"
    );
}

#[test]
fn decode_reads_cpp_s_packet_header_sample() {
    // Sample bytes as the packed C++ struct lays them out on x86:
    // usSize = 42, ucChk = 0, usCmd = 30000.
    let bytes = [0x2A, 0x00, 0x00, 0x30, 0x75, 0xFF]; // trailing byte = payload
    let header = WireHeader::decode(&bytes).unwrap();
    assert_eq!(header.size(), 42);
    assert_eq!(header.integrity(), 0);
    assert_eq!(header.cmd(), SECRETKEY_CMD);
    assert_eq!(header.payload_len(), 37);
}

#[test]
fn encode_decode_round_trip_preserves_every_field() {
    let header = WireHeader::new(0x1234, 8).unwrap();
    let decoded = WireHeader::decode(&header.encode()).unwrap();
    assert_eq!(decoded, header);
}

#[test]
fn reserved_byte_is_carried_through_untouched() {
    // The cipher writes an arbitrary resync tag at offset 2 on encrypted
    // links; framing must round-trip it without interpreting it.
    let wire = [0x05, 0x00, 0xA5, 0x02, 0x00];
    let header = WireHeader::decode(&wire).unwrap();
    assert_eq!(header.integrity(), 0xA5);
    assert_eq!(header.encode(), wire);
}

#[test]
fn guard_table_matches_legacy_recv_path() {
    // ITZRecvBuffer.cpp:163 — the legacy's own `< 2` guard.
    assert_eq!(
        validate_size(0),
        Err(HeaderError::SizeBelowMinimum { declared: 0 })
    );
    assert_eq!(
        validate_size(1),
        Err(HeaderError::SizeBelowMinimum { declared: 1 })
    );
    // 2..=4 fit the legacy guard but cannot hold a header (BUG-013);
    // rejected here for parity with gz-net.
    for declared in 2..HEADER_SIZE as u16 {
        assert_eq!(
            validate_size(declared),
            Err(HeaderError::SizeBelowHeader { declared })
        );
    }
    // ITZRecvBuffer.cpp:172 — PACKET_MAX_SIZE(65000) - 8 = 64992.
    assert_eq!(MAX_FRAME_SIZE, 64992);
    assert_eq!(validate_size(5), Ok(()));
    assert_eq!(validate_size(MAX_FRAME_SIZE), Ok(()));
    assert_eq!(
        validate_size(MAX_FRAME_SIZE + 1),
        Err(HeaderError::SizeTooLarge {
            declared: MAX_FRAME_SIZE + 1
        })
    );
}

#[test]
fn decode_applies_the_guard_table() {
    // Declared size 4: readable bytes, but a frame that cannot hold a
    // header — rejected even though the buffer is long enough.
    assert_eq!(
        WireHeader::decode(&[0x04, 0x00, 0x00, 0x00, 0x00]),
        Err(HeaderError::SizeBelowHeader { declared: 4 })
    );
    // Declared size 64992 + 1 = 64993 = 0xFDE1 -> LE [0xE1, 0xFD]:
    // above the receive limit.
    assert_eq!(
        WireHeader::decode(&[0xE1, 0xFD, 0x00, 0x00, 0x00]),
        Err(HeaderError::SizeTooLarge { declared: 64993 })
    );
}

#[test]
fn decode_rejects_short_buffers() {
    assert_eq!(
        WireHeader::decode(&[0x05, 0x00, 0x00]),
        Err(HeaderError::BufferTooShort { len: 3 })
    );
    assert_eq!(
        WireHeader::decode(&[]),
        Err(HeaderError::BufferTooShort { len: 0 })
    );
}

#[test]
fn new_rejects_oversized_payloads() {
    let max_payload = MAX_FRAME_SIZE as usize - HEADER_SIZE;
    assert!(WireHeader::new(1, max_payload).is_ok());
    assert_eq!(
        WireHeader::new(1, max_payload + 1),
        Err(HeaderError::PayloadTooLarge {
            len: max_payload + 1
        })
    );
}

#[test]
fn declared_error_accessor_covers_every_variant() {
    assert_eq!(
        HeaderError::SizeTooLarge { declared: 7 }.declared(),
        Some(7)
    );
    assert_eq!(HeaderError::BufferTooShort { len: 3 }.declared(), None);
    assert_eq!(HeaderError::PayloadTooLarge { len: 9 }.declared(), None);
}

#[test]
fn compression_flag_is_exposed_but_never_masked() {
    // Legacy clears the flag only after decompression
    // (GameProtocol.cpp:141: usCmd -= 0x8000), so cmd() must keep it.
    let wire = [0x05, 0x00, 0x00, 0x05, 0x80]; // usCmd = 0x8005
    let header = WireHeader::decode(&wire).unwrap();
    assert!(header.is_compressed());
    assert_eq!(header.cmd(), 0x8005, "framing must not clear the flag");
    assert_eq!(header.cmd_id(), 5);
    assert!(!WireHeader::new(5, 0).unwrap().is_compressed());
}
