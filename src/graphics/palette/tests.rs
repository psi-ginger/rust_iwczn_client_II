//! Unit tests for the `.pal` decoder.
//!
//! **Real fixture** (exists in the tree — `WO-7.1.md` R2/R4):
//! `GoonZu_Source/CharactorCombination/Pal/monarch.pal`, 768 bytes, SHA-256
//! prefix `574E44B1A8AFC1AE` (both in-tree copies share it, per WO §6.2).
//! Tracked in git, so it is present on a clean clone.
//!
//! **Independent expectations** (P3-7): the RGB triples were read straight
//! from the fixture bytes and the 565/555 words computed with a standalone
//! PowerShell transcription of `TSpr.cpp:183-197`, not by running this
//! decoder; the values below are copied from that log (re-verified with the
//! intermediate masks printed term by term):
//!
//! ```text
//! 565: idx0=0x0000 idx1=0xDEFB idx2=0xBDF7 idx3=0x9CF3
//! 555: idx0=0x0000 idx1=0x6F7B idx2=0x5EF7 idx3=0x4E73
//! ```

use super::*;

/// In-tree real fixture (`monarch.pal`, 768 bytes, tracked in git).
const MONARCH_PAL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/GoonZu_Source/CharactorCombination/Pal/monarch.pal"
);

/// Reads the real palette fixture.
fn monarch_pal() -> Vec<u8> {
    std::fs::read(MONARCH_PAL).expect("monarch.pal is tracked in-tree")
}

// ---------------------------------------------------------------------------
// DoD 10.3 - the real fixture, the exact-768 rule, byte-exact 565
// ---------------------------------------------------------------------------

#[test]
fn monarch_pal_is_exactly_768_and_accepted() {
    let bytes = monarch_pal();
    assert_eq!(bytes.len(), 768, "the fixture itself is the canonical size");

    // Fixture identity read from the bytes (independent of this decoder):
    // idx0 = (0,0,0), idx1 = (55,55,55), idx2 = (47,47,47), idx3 = (39,39,39).
    assert_eq!(
        &bytes[0..12],
        &[0, 0, 0, 55, 55, 55, 47, 47, 47, 39, 39, 39]
    );

    let palette =
        load_pal(&bytes, "monarch.pal", RgbMode::Rgb565).expect("exactly 768 bytes are accepted");
    assert_eq!(palette.pal.len(), PALETTE_ENTRIES);
}

#[test]
fn pal_767_and_769_bytes_are_refused_by_name() {
    let bytes = monarch_pal();

    // TSpr.cpp:159-164 refuses anything whose size is not exactly 768.
    let short = &bytes[..767];
    let err = load_pal(short, "monarch.pal", RgbMode::Rgb565).unwrap_err();
    assert_eq!(
        err,
        PaletteError::WrongSize {
            name: "monarch.pal".to_string(),
            expected: 768,
            got: 767,
        }
    );
    assert!(
        err.to_string().contains("monarch.pal"),
        "the refusal must name the file: {err}"
    );

    // A trailing byte is refused too - no padding is tolerated.
    let mut padded = bytes.clone();
    padded.push(0);
    let err = load_pal(&padded, "monarch.pal", RgbMode::Rgb565).unwrap_err();
    assert_eq!(
        err,
        PaletteError::WrongSize {
            name: "monarch.pal".to_string(),
            expected: 768,
            got: 769,
        }
    );
    assert!(err.to_string().contains("monarch.pal"), "{err}");

    // Every other wrong size is refused the same way.
    for size in [0usize, 1, 767, 769, 1024, 1536] {
        let scratch = vec![0u8; size];
        assert_eq!(
            load_pal(&scratch, "scratch.pal", RgbMode::Rgb555),
            Err(PaletteError::WrongSize {
                name: "scratch.pal".to_string(),
                expected: 768,
                got: size,
            })
        );
    }
}

#[test]
fn pal_565_conversion_of_known_indices_is_byte_exact() {
    let palette = load_pal(&monarch_pal(), "monarch.pal", RgbMode::Rgb565).unwrap();
    assert_eq!(palette.pal[0], 0x0000, "rgb(0,0,0)");
    assert_eq!(palette.pal[1], 0xDEFB, "rgb(55,55,55)");
    assert_eq!(palette.pal[2], 0xBDF7, "rgb(47,47,47)");
    assert_eq!(palette.pal[3], 0x9CF3, "rgb(39,39,39)");
}

#[test]
fn pal_555_conversion_of_known_indices_is_byte_exact() {
    let palette = load_pal(&monarch_pal(), "monarch.pal", RgbMode::Rgb555).unwrap();
    assert_eq!(palette.pal[0], 0x0000, "rgb(0,0,0)");
    assert_eq!(palette.pal[1], 0x6F7B, "rgb(55,55,55)");
    assert_eq!(palette.pal[2], 0x5EF7, "rgb(47,47,47)");
    assert_eq!(palette.pal[3], 0x4E73, "rgb(39,39,39)");
}

// ---------------------------------------------------------------------------
// Conversion rules transcribed from TSpr.cpp:183-197
// ---------------------------------------------------------------------------

#[test]
fn mode_selects_the_packing_of_the_cited_lines() {
    // The same triple packs differently in each mode; both are pinned to
    // the exact masked-OR result of TSpr.cpp:185-187 / :193-195.
    let (r, g, b) = (55u8, 55u8, 55u8);
    assert_eq!(rgb_to_565(r, g, b), 0xDEFB);
    assert_eq!(rgb_to_555(r, g, b), 0x6F7B);
    assert_ne!(rgb_to_565(r, g, b), rgb_to_555(r, g, b));

    // One channel at a time: red takes << 11 in 565 but << 10 in 555.
    assert_eq!(rgb_to_565(255, 0, 0), 0xF800);
    assert_eq!(rgb_to_555(255, 0, 0), 0x7C00);
    // Blue is `>> 1` into 5 bits in both.
    assert_eq!(rgb_to_565(0, 0, 255), 0x001F);
    assert_eq!(rgb_to_555(0, 0, 255), 0x001F);
    // Black is zero in both.
    assert_eq!(rgb_to_565(0, 0, 0), 0x0000);
    assert_eq!(rgb_to_555(0, 0, 0), 0x0000);
}

#[test]
fn r_green_channel_is_not_right_shifted_in_565() {
    // TSpr.cpp:186 packs 565 green as `MASK & (g << 5)` with NO `>> 1`,
    // while TSpr.cpp:194 shifts it for 555. That asymmetry is in the source
    // and must survive: correcting it changes every green value.
    assert_eq!(rgb_to_565(0, 55, 0), 0x06E0, "565: (55 << 5) & 0x07E0");
    assert_eq!(
        rgb_to_555(0, 55, 0),
        0x0360,
        "555: ((55 >> 1) << 5) & 0x03E0"
    );
    assert_ne!(
        rgb_to_565(0, 55, 0),
        rgb_to_555(0, 55, 0),
        "the two packings must differ for green"
    );
}

#[test]
fn masks_match_the_cited_graphics_header() {
    // Graphics.h:23-29.
    assert_eq!(RGB16_565_RMASK, 0xF800);
    assert_eq!(RGB16_565_GMASK, 0x07E0);
    assert_eq!(RGB16_565_BMASK, 0x001F);
    assert_eq!(RGB16_555_RMASK, 0x7C00);
    assert_eq!(RGB16_555_GMASK, 0x03E0);
    assert_eq!(RGB16_555_BMASK, 0x001F);

    // TSpr.cpp:159-164.
    assert_eq!(PALETTE_FILE_LEN, 768);
    assert_eq!(PALETTE_ENTRIES, 256);
    assert_eq!(PALETTE_ENTRIES * RGB_BYTES, PALETTE_FILE_LEN);
}

#[test]
fn every_entry_is_converted_not_just_the_first() {
    // A synthetic 768-byte file (no real fixture covers all 256 slots
    // distinctly): each entry = (i, i, i), so a loop that stopped early or
    // read the wrong stride would show at the anchors below.
    let mut raw = vec![0u8; PALETTE_FILE_LEN];
    for (index, triple) in raw.as_chunks_mut::<RGB_BYTES>().0.iter_mut().enumerate() {
        triple.fill(index as u8);
    }
    let palette = load_pal(&raw, "synthetic.pal", RgbMode::Rgb565).unwrap();
    let palette555 = load_pal(&raw, "synthetic.pal", RgbMode::Rgb555).unwrap();

    assert_eq!(palette.pal.len(), PALETTE_ENTRIES);

    // Anchors copied from the independent PowerShell transcription of
    // TSpr.cpp:185-187 / :193-195 — spread across the array so both an
    // early stop and a mis-strided read are caught.
    //
    // Note the last two: `i = 64` and `i = 128` collapse to 0x0000 because
    // the source's `((i >> 1) << 11) & 0xF800` shifts the red term clean
    // out of the 16-bit word. That is the source arithmetic, reproduced as
    // computed, not "fixed".
    let anchors = [
        (1usize, 0x0020u16, 0x0000u16),
        (2, 0x0841, 0x0421),
        (31, 0x7BEF, 0x3DEF),
        (32, 0x8410, 0x4210),
        (55, 0xDEFB, 0x6F7B),
        (63, 0xFFFF, 0x7FFF),
        (64, 0x0000, 0x0000),
        (128, 0x0000, 0x0000),
        (200, 0x2104, 0x1084),
        (255, 0xFFFF, 0x7FFF),
    ];
    for (index, expect565, expect555) in anchors {
        assert_eq!(palette.pal[index], expect565, "565 entry {index}");
        assert_eq!(palette555.pal[index], expect555, "555 entry {index}");
    }

    // Entry 0 is black, and the array is not uniformly one value - every
    // slot really did take its own triple.
    assert_eq!(palette.pal[0], 0x0000);
    let distinct = palette.pal.iter().collect::<std::collections::HashSet<_>>();
    assert!(
        distinct.len() > 8,
        "a stride or early-stop bug would flatten this: {:?}",
        &palette.pal[..8]
    );
}

#[test]
fn rgb_mode_names_are_stable() {
    // Used verbatim in SprError::PaletteTruncated messages.
    assert_eq!(RgbMode::Rgb565.name(), "RGB565");
    assert_eq!(RgbMode::Rgb555.name(), "RGB555");
    assert_eq!(RgbMode::Rgb565.to_string(), "RGB565");
}
