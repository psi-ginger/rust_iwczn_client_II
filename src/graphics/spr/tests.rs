//! Unit tests for the `.Spr` decoder.
//!
//! **Real fixtures** (they exist in the tree — `WO-7.1.md` R2/R4):
//! `GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr`, tracked in git so these
//! run on a clean clone.
//!
//! **Independent expectations** (P3-7 / `WO-7.1.md` §7): the consumption,
//! coverage and pixel figures below were computed by walking `Mask.Spr`
//! with a standalone PowerShell transcription of
//! `Graphics_PutSpr16T.cpp:28-48`, not by running this decoder — the
//! numbers were copied from that log:
//!
//! ```text
//! iImageDataSize     = 1760
//! bytes consumed     = 1760   (expect 1760 = iImageDataSize)
//! pixels (literals)  = 1600
//! pixels (skip runs) = 1600 in 80 runs
//! total pixels       = 3200   (expect 3200 = 80*40)
//! literal idx 0xA3   = 1600
//! stream fully consumed at offset 4896 (expect 4896)
//! ```

use super::*;

/// In-tree real fixture (`Mask.Spr`, 4896 bytes, tracked in git).
const MASK_SPR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr"
);

/// Reads the real sprite fixture.
fn mask_spr() -> Vec<u8> {
    std::fs::read(MASK_SPR).expect("Mask.Spr is tracked in-tree")
}

/// Writes one little-endian `i32` into a scratch header.
fn put_i32(bytes: &mut [u8], at: usize, value: i32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Builds a syntactically valid 3136-byte header with no `ImageInfo`
/// entries (`iImageNum = 0`), for decoder-level tests.
fn build_header(x: i32, y: i32, image_size: i32, encode_type: i32, version: i32) -> Vec<u8> {
    let mut header = vec![0u8; TSPR_HEADER_LEN];
    put_i32(&mut header, 0, version);
    put_i32(&mut header, 4, encode_type);
    put_i32(&mut header, 8, x);
    put_i32(&mut header, 12, y);
    put_i32(&mut header, 16, x); // iHRes
    put_i32(&mut header, 20, y); // iVRes
    put_i32(&mut header, 24, image_size);
    // reserved[8] (28..60) stays zero; iImageNum (60) stays 0.
    header
}

/// Full synthetic sprite file: header + image (+ optional palette tables).
fn build_file(header: &[u8], image: &[u8], tables: &[&[u8]]) -> Vec<u8> {
    let mut file = header.to_vec();
    file.extend_from_slice(image);
    for table in tables {
        file.extend_from_slice(table);
    }
    file
}

/// A 512-byte `RGB16[256]` table filled with one pattern word, so the two
/// v400 tables are distinguishable byte-for-byte.
fn palette_table(word: u16) -> Vec<u8> {
    let unit = word.to_le_bytes();
    (0..TSPR_PALETTE_LEN)
        .map(|i| unit[i % 2])
        .collect::<Vec<u8>>()
}

// ---------------------------------------------------------------------------
// DoD 10.2 - the real Mask.Spr, byte-exact
// ---------------------------------------------------------------------------

#[test]
fn mask_spr_header_fields_are_byte_exact() {
    let bytes = mask_spr();
    let header = parse_header(&bytes).expect("Mask.Spr is a valid sprite");

    // DoD 10.2 named expectations.
    assert_eq!(header.i_version, 10, "TSPR_VERSION_NOPAL");
    assert_eq!(header.i_encode_type, 0, "TSPR_ENCODETYPE_SPR");
    assert_eq!(header.i_x_size, 80);
    assert_eq!(header.i_y_size, 40);
    assert_eq!(header.i_image_num, 1);

    // The fixed header region + declared stream must equal the file size:
    // 3136 + 1760 = 4896 (TSpr.cpp:54,66 read exactly these two spans).
    assert_eq!(TSPR_HEADER_LEN, 3136, "sizeof(TSprHeader) is fixed");
    assert_eq!(header.i_image_data_size, 1760);
    assert_eq!(
        bytes.len(),
        TSPR_HEADER_LEN + 1760,
        "3136 header + 1760 stream = 4896-byte file"
    );
    assert_eq!(bytes.len(), 4896);
}

#[test]
fn mask_spr_stream_consumes_exactly_1760_and_covers_exactly_3200() {
    let bytes = mask_spr();
    let sprite = load_sprite(&bytes, RgbMode::Rgb565).expect("Mask.Spr decodes");

    let image = &sprite.image;
    // 1760/1760 stream bytes (iImageDataSize), 3200/3200 pixels (80 x 40).
    assert_eq!(image.bytes_consumed, 1760, "consumed == iImageDataSize");
    assert_eq!(image.width, 80);
    assert_eq!(image.height, 40);
    assert_eq!(image.pixels.len(), 3200, "80 x 40 canvas");
    assert_eq!(
        image.covered_pixels(),
        3200,
        "literal + transparent = canvas"
    );

    // 1600 literal idx 0xA3 + 1600 transparent px in 80 skip runs.
    assert_eq!(image.literal_pixels, 1600);
    assert_eq!(image.transparent_pixels, 1600);
    assert_eq!(image.transparent_runs, 80);
    assert!(
        image.pixels.iter().flatten().all(|&index| index == 0xA3),
        "every literal in Mask.Spr is palette index 0xA3"
    );

    // v10 carries no palette: the loader stops right after the stream
    // (TSpr.cpp:74 gate is `== 400`).
    assert_eq!(sprite.embedded_palette, None);
    assert_eq!(sprite.bytes_consumed, 4896, "header + stream = whole file");
    assert_eq!(sprite.bytes_consumed, bytes.len());
}

#[test]
fn mask_spr_transparent_runs_are_separate_from_literals() {
    // Structural cross-check of the RLE shape, from an independent walk of
    // Mask.Spr (standalone PowerShell transcription of
    // Graphics_PutSpr16T.cpp:28-48, log-verified): the mask is a
    // **diamond** — narrow transparent bands at top and bottom, nearly
    // solid in the middle, perfectly mirror-symmetric:
    //
    //   row  0 / 39: N39 L L N39   (2 literals,  skip runs [39, 39])
    //   row  1 / 38: N37 L*6 N37   (6 literals,  skip runs [37, 37])
    //   row 19 / 20: N1  L*78 N1   (78 literals, skip runs [1, 1])
    //
    // Totals: 1600 literals + 1600 transparent px in 80 runs (2 per row)
    // = 3200 pixels, 1760 stream bytes consumed. Mirror check: 0
    // mismatches over all 40 rows.
    let bytes = mask_spr();
    let sprite = load_sprite(&bytes, RgbMode::Rgb565).unwrap();
    let image = &sprite.image;

    assert_eq!(image.transparent_runs, 80, "2 skip runs x 40 rows");
    assert_eq!(
        image.transparent_pixels / image.transparent_runs,
        20,
        "20 px per run on average (1600 / 80)"
    );

    let rows: Vec<&[Option<u8>]> = image.pixels.chunks_exact(image.width).collect();
    assert_eq!(rows.len(), 40, "40 rows of 80 cells");

    // Pinned per-row literal counts from the independent walk.
    for (index, literals) in [(0usize, 2usize), (1, 6), (19, 78), (39, 2)] {
        let count = rows[index].iter().filter(|cell| cell.is_some()).count();
        assert_eq!(count, literals, "row {index} has {literals} literals");
    }

    // Row 0's exact shape: skip 39, two 0xA3 literals, skip 39.
    let row0 = rows[0];
    assert!(
        row0[..39].iter().all(|cell| cell.is_none()),
        "row 0 opens with a 39-px skip"
    );
    assert_eq!(row0[39], Some(0xA3));
    assert_eq!(row0[40], Some(0xA3));
    assert!(
        row0[41..].iter().all(|cell| cell.is_none()),
        "row 0 closes with a 39-px skip"
    );
    assert_eq!(none_spans(row0), vec![39, 39], "row 0 skip runs");

    // The diamond is mirror-symmetric (log: 0 mismatches over 40 rows).
    for index in 0..20 {
        assert_eq!(
            rows[index],
            rows[39 - index],
            "row {index} must mirror row {}",
            39 - index
        );
    }

    // Every literal is palette index 0xA3 (the mask has one ink).
    assert!(
        image.pixels.iter().flatten().all(|&index| index == 0xA3),
        "every literal is palette index 0xA3"
    );
}

/// Lengths of each maximal run of transparent (`None`) cells in a row.
fn none_spans(row: &[Option<u8>]) -> Vec<usize> {
    let mut spans = Vec::new();
    let mut run = 0usize;
    for cell in row {
        if cell.is_none() {
            run += 1;
        } else if run > 0 {
            spans.push(run);
            run = 0;
        }
    }
    if run > 0 {
        spans.push(run);
    }
    spans
}

// ---------------------------------------------------------------------------
// R1 - iSize is exposed as data, never used as a bound
// ---------------------------------------------------------------------------

#[test]
fn r1_isize_is_exposed_but_never_a_decode_bound() {
    let bytes = mask_spr();
    let header = parse_header(&bytes).unwrap();

    // Exposed as data (R1: "Expose iSize as data only").
    assert_eq!(header.image_info.len(), 1);
    assert_eq!(header.image_info[0].i_size, 224, "declared iSize");
    assert_eq!(header.image_info[0].i_coll_info, 0);
    assert_eq!(header.image_info[0].i_start_pos, 0);

    // ...but the decoder walks the real 1760-byte stream to canvas
    // coverage. Fails if anything truncates to iSize.
    let image = decode_spr(&header, &bytes[TSPR_HEADER_LEN..]).expect("decodes");
    assert_eq!(
        image.bytes_consumed, 1760,
        "stream, not iSize, bounds the walk"
    );
    assert_eq!(image.covered_pixels(), 3200);
    assert_ne!(
        image.bytes_consumed, header.image_info[0].i_size as usize,
        "224 must never act as a bound"
    );
}

#[test]
fn r1_zero_isize_still_decodes_the_whole_stream() {
    // Synthetic second case: iSize = 0 while the stream is longer. Built
    // from TSpr.h:25 (iSize is data) + Graphics_PutSpr16T.cpp:28-48 (the
    // blitter never reads it).
    let mut header = build_header(3, 1, 4, TSPR_ENCODETYPE_SPR, TSPR_VERSION_NOPAL);
    // iImageNum = 1, ImageInfo[0] = {coll=0, start=0, size=0} at offset 64.
    put_i32(&mut header, 60, 1);
    put_i32(&mut header, 64, 0);
    put_i32(&mut header, 68, 0);
    put_i32(&mut header, 72, 0);

    // Stream: 2 literals + a 1-px skip fills the 3-wide row in 4 bytes.
    let file = build_file(&header, &[0x11, 0x22, COLOR_TRANS, 0x01], &[]);
    let sprite = load_sprite(&file, RgbMode::Rgb565).expect("iSize=0 must not truncate");

    assert_eq!(sprite.header.image_info[0].i_size, 0);
    assert_eq!(sprite.image.bytes_consumed, 4, "walks past iSize=0");
    assert_eq!(sprite.image.covered_pixels(), 3);
}

// ---------------------------------------------------------------------------
// SPR and MPR are separately implemented and NOT interchangeable (§6.1, §7)
// ---------------------------------------------------------------------------

#[test]
fn spr_decoder_literal_plus_skip_covers_the_row() {
    // Graphics_PutSpr16T.cpp:41-44 (literal) and :34-38 (254 + count).
    let header = build_header(4, 1, 4, TSPR_ENCODETYPE_SPR, TSPR_VERSION_NOPAL);
    let header = parse_header(&header).unwrap();
    let image = decode_spr(&header, &[0x11, 0x22, COLOR_TRANS, 0x02]).unwrap();

    assert_eq!(
        image.pixels,
        vec![Some(0x11), Some(0x22), None, None],
        "SPR: one index per pixel, 254+count skips without index bytes"
    );
    assert_eq!(image.bytes_consumed, 4);
    assert_eq!(image.literal_pixels, 2);
    assert_eq!(image.transparent_pixels, 2);
    assert_eq!(image.transparent_runs, 1);
}

#[test]
fn mpr_decoder_count_plus_indices_covers_the_row() {
    // Graphics_PutMpr16.cpp:41-48 (count byte then `count` indices) and
    // :33-38 (254 + count).
    let header = build_header(4, 1, 5, TSPR_ENCODETYPE_MPR, TSPR_VERSION_NOPAL);
    let header = parse_header(&header).unwrap();
    let image = decode_mpr(&header, &[0x02, 0x33, 0x33, COLOR_TRANS, 0x02]).unwrap();

    assert_eq!(
        image.pixels,
        vec![Some(0x33), Some(0x33), None, None],
        "MPR: the byte is a run length, then that many index bytes"
    );
    assert_eq!(image.bytes_consumed, 5);
    assert_eq!(image.literal_pixels, 2);
    assert_eq!(image.transparent_runs, 1);
}

#[test]
fn r_spr_and_mpr_decoders_are_not_interchangeable() {
    // The two streams are each valid under exactly one decoder.
    let spr_stream = [0x11u8, 0x22, COLOR_TRANS, 0x02];
    let mpr_stream = [0x02u8, 0x33, 0x33, COLOR_TRANS, 0x02];

    let spr_header = parse_header(&build_header(4, 1, 4, TSPR_ENCODETYPE_SPR, 10)).unwrap();
    let mpr_header = parse_header(&build_header(4, 1, 5, TSPR_ENCODETYPE_MPR, 10)).unwrap();

    let as_spr = decode_spr(&spr_header, &spr_stream).unwrap();
    assert_eq!(as_spr.pixels, vec![Some(0x11), Some(0x22), None, None]);

    // Running the SPR stream through the MPR decoder misreads 0x11 as a
    // 17-pixel run length and runs out of stream: a typed refusal, never a
    // silent interchange.
    let err = decode_mpr(&spr_header, &spr_stream).expect_err("SPR stream is not MPR");
    assert!(matches!(err, SprError::StreamExhausted { .. }), "{err:?}");

    let as_mpr = decode_mpr(&mpr_header, &mpr_stream).unwrap();
    assert_eq!(as_mpr.pixels, vec![Some(0x33), Some(0x33), None, None]);

    // Running the MPR stream through the SPR decoder yields different
    // pixels: 0x02 is read as literal index 2, not as a run length.
    let spr_view_of_mpr = decode_spr(&mpr_header, &mpr_stream).unwrap();
    assert_eq!(
        spr_view_of_mpr.pixels,
        vec![Some(0x02), Some(0x33), Some(0x33), None]
    );
    assert_ne!(
        spr_view_of_mpr.pixels, as_mpr.pixels,
        "the decoders must not agree on a stream built for the other"
    );
}

#[test]
fn load_sprite_dispatches_on_encode_type() {
    let spr_stream = [0x11u8, 0x22, COLOR_TRANS, 0x02];
    let file = build_file(
        &build_header(4, 1, 4, TSPR_ENCODETYPE_SPR, 10),
        &spr_stream,
        &[],
    );
    let sprite = load_sprite(&file, RgbMode::Rgb565).unwrap();
    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x11), Some(0x22), None, None]
    );

    // A file that declares MPR but carries the SPR stream is refused at the
    // decode, not silently accepted as the other codec.
    let wrong = build_file(
        &build_header(4, 1, 4, TSPR_ENCODETYPE_MPR, 10),
        &spr_stream,
        &[],
    );
    assert!(matches!(
        load_sprite(&wrong, RgbMode::Rgb565),
        Err(SprError::StreamExhausted { .. })
    ));
}

// ---------------------------------------------------------------------------
// R5 - v400 palette: read AFTER the image, guarded by `== 400`
// ---------------------------------------------------------------------------

/// Synthetic v400 sprite: `header | image | pal565(512) | pal555(512)`.
///
/// **Provenance:** layout from `TSpr.cpp:66` (image read), `:74` (`== 400`
/// guard), `:76-81` (555 skip then 512-byte read). Synthetic — no real
/// v400 sprite exists in the tree (R2).
fn v400_file(version: i32, image: &[u8], tables: bool) -> Vec<u8> {
    let header = build_header(2, 1, image.len() as i32, TSPR_ENCODETYPE_SPR, version);
    if tables {
        build_file(
            &header,
            image,
            &[&palette_table(0xAAAA), &palette_table(0x5555)],
        )
    } else {
        build_file(&header, image, &[])
    }
}

#[test]
fn r5_v400_palette_is_read_after_the_image_not_before() {
    let file = v400_file(TSPR_VERSION_16PAL, &[0x11, 0x22], true);
    let sprite = load_sprite(&file, RgbMode::Rgb565).expect("v400 decodes");

    // The image came from the image bytes...
    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x11), Some(0x22)],
        "the 512-byte table must not be treated as image stream"
    );
    assert_eq!(sprite.image.bytes_consumed, 2, "exactly iImageDataSize");

    // ...and the FIRST 512 bytes after the image are the 565 table
    // (TSpr.cpp:81 reads immediately after the image read at :66).
    let palette = sprite.embedded_palette.expect("v400 has a palette");
    assert!(palette.iter().all(|&word| word == 0xAAAA), "first table");

    // header + declared image + 512 (TSpr.cpp:74-87).
    assert_eq!(sprite.bytes_consumed, TSPR_HEADER_LEN + 2 + 512);
    assert_eq!(sprite.bytes_consumed, file.len() - TSPR_PALETTE_LEN);
}

#[test]
fn r5_555_mode_skips_the_first_table_and_reads_the_second() {
    let file = v400_file(TSPR_VERSION_16PAL, &[0x11, 0x22], true);
    let sprite = load_sprite(&file, RgbMode::Rgb555).expect("v400 decodes");

    // Image unchanged - only the palette selection moves.
    assert_eq!(sprite.image.pixels, vec![Some(0x11), Some(0x22)]);

    // TSpr.cpp:76-79 skips 512 in 555 mode, then :81 reads 512.
    let palette = sprite.embedded_palette.expect("v400 has a palette");
    assert!(palette.iter().all(|&word| word == 0x5555), "second table");
    assert_eq!(
        sprite.bytes_consumed,
        TSPR_HEADER_LEN + 2 + 2 * TSPR_PALETTE_LEN
    );
}

#[test]
fn r5_guard_is_eq_400_not_ge_400() {
    // A version ABOVE 400 carries no palette table (TSpr.cpp:74 compares
    // `== TSPR_VERSION_16PAL`). Fails if the guard is corrected to `>= 400`,
    // which would demand 512 bytes this file does not have.
    let file = v400_file(401, &[0x11, 0x22], false);
    let sprite = load_sprite(&file, RgbMode::Rgb565).expect("v401 has no palette");

    assert_eq!(sprite.header.i_version, 401);
    assert_eq!(sprite.embedded_palette, None, "no table for version 401");
    assert_eq!(
        sprite.bytes_consumed,
        TSPR_HEADER_LEN + 2,
        "stops after image"
    );

    // Version 10 behaves the same way (the real Mask.Spr proves it too).
    let v10 = v400_file(TSPR_VERSION_NOPAL, &[0x11, 0x22], false);
    assert_eq!(
        load_sprite(&v10, RgbMode::Rgb565).unwrap().embedded_palette,
        None
    );
}

#[test]
fn r5_v400_palette_truncation_is_refused() {
    // TSpr.cpp:82-85 refuses when the 512-byte read comes up short.
    let file = v400_file(TSPR_VERSION_16PAL, &[0x11, 0x22], false);
    let err = load_sprite(&file, RgbMode::Rgb565).expect_err("no table present");
    assert_eq!(
        err,
        SprError::PaletteTruncated {
            mode: RgbMode::Rgb565,
            needed: 512,
            got: 0,
        }
    );

    // 555 mode needs both tables (skip + read).
    let half = v400_file(TSPR_VERSION_16PAL, &[0x11, 0x22], true);
    let truncated = &half[..half.len() - TSPR_PALETTE_LEN - 100];
    assert!(matches!(
        load_sprite(truncated, RgbMode::Rgb555),
        Err(SprError::PaletteTruncated {
            needed: 1024,
            got: 412,
            ..
        })
    ));
}

// ---------------------------------------------------------------------------
// P3-2 typed refusals
// ---------------------------------------------------------------------------

#[test]
fn short_header_is_refused_with_expected_and_got() {
    let bytes = mask_spr();
    assert_eq!(
        parse_header(&bytes[..TSPR_HEADER_LEN - 1]).unwrap_err(),
        SprError::HeaderTruncated {
            expected: 3136,
            got: 3135,
        }
    );
    assert!(matches!(
        parse_header(&[]),
        Err(SprError::HeaderTruncated { got: 0, .. })
    ));
}

#[test]
fn short_image_stream_is_refused_like_the_legacy_short_read() {
    // TSpr.cpp:67-70 refuses when fewer than iImageDataSize bytes arrive.
    let header = build_header(80, 40, 1760, TSPR_ENCODETYPE_SPR, TSPR_VERSION_NOPAL);
    let file = build_file(&header, &[0u8; 100], &[]);
    assert_eq!(
        load_sprite(&file, RgbMode::Rgb565).unwrap_err(),
        SprError::ImageStreamTruncated {
            expected: 1760,
            got: 100,
        }
    );
}

#[test]
fn rle_exhaustion_is_a_typed_refusal_not_a_panic() {
    // Legacy would walk off its buffer (Graphics_PutSpr16T.cpp:32 has no
    // end check); the port refuses instead of emitting a partial frame.
    let header = build_header(80, 40, 4, TSPR_ENCODETYPE_SPR, TSPR_VERSION_NOPAL);
    let header = parse_header(&header).unwrap();
    let err = decode_spr(&header, &[0x11, 0x22, COLOR_TRANS, 0x02]).unwrap_err();
    // 2 literals + a 2-px skip fill columns 0..4 of row 0, then the stream
    // is exhausted looking for column 4.
    assert!(
        matches!(
            err,
            SprError::StreamExhausted {
                row: 0,
                col: 4,
                offset: 4,
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn unsized_headers_are_refused() {
    let bad_num = build_header(4, 1, 0, TSPR_ENCODETYPE_SPR, 10);
    let mut bad_num = bad_num;
    put_i32(&mut bad_num, 60, 257);
    assert_eq!(
        parse_header(&bad_num).unwrap_err(),
        SprError::BadImageNum { i_image_num: 257 }
    );

    let mut negative_num = build_header(4, 1, 0, TSPR_ENCODETYPE_SPR, 10);
    put_i32(&mut negative_num, 60, -1);
    assert!(matches!(
        parse_header(&negative_num),
        Err(SprError::BadImageNum { i_image_num: -1 })
    ));

    let negative_stream = build_header(4, 1, -8, TSPR_ENCODETYPE_SPR, 10);
    let file = build_file(&negative_stream, &[], &[]);
    assert!(matches!(
        load_sprite(&file, RgbMode::Rgb565),
        Err(SprError::NegativeImageDataSize {
            i_image_data_size: -8
        })
    ));

    let negative_x = build_header(-5, 1, 1, TSPR_ENCODETYPE_SPR, 10);
    let file = build_file(&negative_x, &[0x11], &[]);
    assert!(matches!(
        load_sprite(&file, RgbMode::Rgb565),
        Err(SprError::NegativeDimension {
            field: "iXSize",
            value: -5
        })
    ));

    let negative_y = build_header(4, -3, 1, TSPR_ENCODETYPE_SPR, 10);
    let file = build_file(&negative_y, &[0x11], &[]);
    assert!(matches!(
        load_sprite(&file, RgbMode::Rgb565),
        Err(SprError::NegativeDimension {
            field: "iYSize",
            value: -3
        })
    ));
}

#[test]
fn unknown_encode_type_is_refused_by_name() {
    let header = build_header(4, 1, 2, 7, TSPR_VERSION_NOPAL);
    let file = build_file(&header, &[0x11, 0x22], &[]);
    assert_eq!(
        load_sprite(&file, RgbMode::Rgb565).unwrap_err(),
        SprError::UnsupportedEncodeType { i_encode_type: 7 }
    );
}

// ---------------------------------------------------------------------------
// Layout constants (P3-1 second source: the struct itself)
// ---------------------------------------------------------------------------

#[test]
fn header_layout_matches_the_struct_offsets() {
    let mut header = build_header(1, 2, 3, 4, 5);
    put_i32(&mut header, 16, 6); // iHRes
    put_i32(&mut header, 20, 7); // iVRes
    header[28] = 0xEE; // reserved[8] first byte
    header[59] = 0xEF; // reserved[8] last byte
    put_i32(&mut header, 60, 2); // iImageNum
    put_i32(&mut header, 64, 11); // ImageInfo[0].iCollInfo
    put_i32(&mut header, 68, 22); // ImageInfo[0].iStartPos
    put_i32(&mut header, 72, 33); // ImageInfo[0].iSize
    put_i32(&mut header, 76, 44); // ImageInfo[1].iCollInfo
    put_i32(&mut header, 80, 55); // ImageInfo[1].iStartPos
    put_i32(&mut header, 84, 66); // ImageInfo[1].iSize

    let parsed = parse_header(&header).unwrap();
    assert_eq!(parsed.i_version, 5);
    assert_eq!(parsed.i_encode_type, 4);
    assert_eq!(parsed.i_x_size, 1);
    assert_eq!(parsed.i_y_size, 2);
    assert_eq!(parsed.i_image_data_size, 3);
    assert_eq!(parsed.i_h_res, 6);
    assert_eq!(parsed.i_v_res, 7);
    assert_eq!(parsed.reserved[0], 0xEE);
    assert_eq!(parsed.reserved[31], 0xEF);
    assert_eq!(parsed.i_image_num, 2);
    assert_eq!(
        parsed.image_info,
        vec![
            TSprImageInfo {
                i_coll_info: 11,
                i_start_pos: 22,
                i_size: 33
            },
            TSprImageInfo {
                i_coll_info: 44,
                i_start_pos: 55,
                i_size: 66
            },
        ]
    );

    // The fixed array starts at 64 and is 256 x 12, so the header region is
    // exactly 3136 bytes (TSpr.h:46, TSpr.cpp:54).
    assert_eq!(
        64 + TSPR_IMAGE_NUM_MAX * TSPR_IMAGE_INFO_LEN,
        TSPR_HEADER_LEN
    );
    assert_eq!(TSPR_IMAGE_INFO_LEN, 12);
}

#[test]
fn constants_match_the_cited_header() {
    // TSpr.h:11-15.
    assert_eq!(TSPR_ENCODETYPE_SPR, 0);
    assert_eq!(TSPR_ENCODETYPE_MPR, 1);
    assert_eq!(TSPR_VERSION_NOPAL, 10);
    assert_eq!(TSPR_VERSION_16PAL, 400);
    // Graphics.h:44.
    assert_eq!(COLOR_TRANS, 254);
}

#[test]
fn run_past_row_end_is_clamped_to_the_row() {
    // Legacy `while (j < width)` ends the row as soon as the count takes j
    // to (or past) width, so an oversized skip still yields exactly
    // `width` cells per row while the stream pointer still advances past
    // the whole run (Graphics_PutSpr16T.cpp:32-38).
    let header = parse_header(&build_header(4, 1, 4, TSPR_ENCODETYPE_SPR, 10)).unwrap();
    let image = decode_spr(&header, &[COLOR_TRANS, 99, 0x11, 0x22]).unwrap();
    assert_eq!(
        image.pixels,
        vec![None, None, None, None],
        "skip of 99 fills the 4-wide row"
    );
    // The pair advances the stream by 2, then the row is complete and the
    // walk stops - the trailing 0x11 0x22 are simply never reached.
    assert_eq!(image.bytes_consumed, 2, "only the 254+count pair was read");
    assert_eq!(image.literal_pixels, 0);
    assert_eq!(image.transparent_pixels, 4, "clamped to the row");
    assert_eq!(image.covered_pixels(), 4, "never over-fills the canvas");
}
