//! Unit tests for the shared little-endian byte readers (Step 7.0 C1–C4).

use super::*;

#[test]
fn le_readers_decode_known_byte_patterns() {
    // 8 bytes: u16=0x0201, i16=0x0403, u32=0x08070605, i32=0x0C0B0A09, i64=...
    let buf = [
        0x01, 0x02, // u16 = 513
        0x03, 0x04, // i16 = 1027
        0x05, 0x06, 0x07, 0x08, // u32
        0x09, 0x0A, 0x0B, 0x0C, // i32
        0x0D, 0x0E, 0x0F, 0x10, // i64 (low half)
    ];
    assert_eq!(u16_at(&buf, 0), 0x0201);
    assert_eq!(i16_at(&buf, 2), 0x0403);
    assert_eq!(u32_at(&buf, 4), 0x0807_0605);
    assert_eq!(i32_at(&buf, 8), 0x0C0B_0A09u32 as i32);
    assert_eq!(try_u16_at(&buf, 0), Some(0x0201));
    assert_eq!(try_i32_at(&buf, 8), Some(0x0C0B_0A09u32 as i32));
}

#[test]
fn try_readers_return_none_past_end() {
    let buf = [0u8; 3];
    assert_eq!(try_u16_at(&buf, 2), None); // needs 2..4, only 3 bytes
    assert_eq!(try_i32_at(&buf, 0), None); // needs 0..4, only 3 bytes
    assert_eq!(try_i64_at(&buf, 0), None);
    assert_eq!(try_u16_at(&buf, 0), Some(0)); // 0..2 fits
}

#[test]
fn cstr_lossy_cuts_at_first_nul() {
    assert_eq!(cstr_lossy(b"ABC\0DEF"), "ABC");
    assert_eq!(cstr_lossy(b"ABC"), "ABC"); // no NUL -> whole field
    assert_eq!(cstr_lossy(b"\0ABC"), "");
    assert_eq!(cstr_at(b"xxABC\0yy", 2, 5), "ABC");
}

#[test]
fn cstr_lossy_replaces_invalid_utf8() {
    // 0xFF is not valid UTF-8; lossy replacement must not panic.
    let s = cstr_lossy(&[0x41, 0xFF, 0x42]);
    assert_eq!(s, "A\u{FFFD}B");
}
