// Step 7.1 — asset-fixture integration tests (WO-7.1 §10 DoD 2-5).
//
// These run against the crate's **public** `graphics` API only, at the
// integration boundary, and pair the two real in-tree fixtures with full
// synthetic builders for the formats that have no real fixture:
//
// - **Real** (tracked in git, present on a clean clone — WO-7.1 R2/R4):
//   `GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr` (4896 B) and
//   `GoonZu_Source/CharactorCombination/Pal/monarch.pal` (768 B). Their
//   expectations are independently computed (see the colocated suites in
//   `src/graphics/*/tests.rs` for the computation logs), never derived
//   from the decoder under test (F-07).
// - **Synthetic** (R2: no real `.Map`/`.Mpr`/`.sz`/`.mz` exists in the
//   tree): every builder below is constructed from the cited legacy layout
//   and carries its `file:line` provenance inline (P3-4).
//
// Headless and deterministic throughout (P3-3): no GPU, no window, no
// display, no network.

use goonzu_client::graphics::archive::{
    ArchiveError, AssetError, AssetSource, MemberBytes, Resolved,
};
use goonzu_client::graphics::map::{
    self, CltMapHeader, MapError, MapFile, MapSection, GEO_TEXT_LENGTH, MAP_HEADER_LEN,
    MAX_GEO_NUMBER,
};
use goonzu_client::graphics::palette::{self, load_pal, PaletteError, RgbMode, PALETTE_FILE_LEN};
use goonzu_client::graphics::spr::{self, load_sprite, SprError, TSPR_HEADER_LEN};

// ---------------------------------------------------------------------------
// Real fixtures (WO-7.1 R2/R4 — both tracked in git)
// ---------------------------------------------------------------------------

/// In-tree real sprite: 4896 B, `iVersion=10`, 80x40, stream 1760 B.
const MASK_SPR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr"
);

/// In-tree real palette: 768 B (exactly `256 x 3` RGB), SHA-256 prefix
/// `574E44B1A8AFC1AE` shared by both in-tree copies (WO-7.1 §6.2).
const MONARCH_PAL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/GoonZu_Source/CharactorCombination/Pal/monarch.pal"
);

fn mask_spr() -> Vec<u8> {
    std::fs::read(MASK_SPR).expect("Mask.Spr is tracked in-tree")
}

fn monarch_pal() -> Vec<u8> {
    std::fs::read(MONARCH_PAL).expect("monarch.pal is tracked in-tree")
}

// ---------------------------------------------------------------------------
// Synthetic builders (R2 — provenance cited per builder, P3-4)
// ---------------------------------------------------------------------------

fn put_i32(bytes: &mut [u8], at: usize, value: i32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_i16(bytes: &mut [u8], at: usize, value: i16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Synthetic `.Spr`/`.Mpr` file: fixed 3136-byte header + image stream.
///
/// Provenance: header fields at their literal offsets from
/// `GoonZu_Source/NLib/TSpr.h:32-47`; the fixed `sizeof(TSprHeader)` read
/// from `NLib/TSpr.cpp:54`; the stream read of exactly `iImageDataSize`
/// from `TSpr.cpp:63-70`. Synthetic (R2 — no real MPR fixture in tree).
fn build_sprite_file(
    version: i32,
    encode_type: i32,
    width: i32,
    height: i32,
    image: &[u8],
) -> Vec<u8> {
    let mut file = vec![0u8; TSPR_HEADER_LEN];
    put_i32(&mut file, 0, version); // TSpr.h:35  iVersion
    put_i32(&mut file, 4, encode_type); // TSpr.h:36  iEncodeType
    put_i32(&mut file, 8, width); // TSpr.h:37  iXSize
    put_i32(&mut file, 12, height); // TSpr.h:38  iYSize
    put_i32(&mut file, 16, width); // TSpr.h:39  iHRes
    put_i32(&mut file, 20, height); // TSpr.h:40  iVRes
    put_i32(&mut file, 24, image.len() as i32); // TSpr.h:41 iImageDataSize
                                                // reserved[8] (28..60) and iImageNum (60) stay zero: no ImageInfo.
    file.extend_from_slice(image);
    file
}

/// One 512-byte `RGB16 pal[256]` table (`TSpr.h:74`, read at
/// `TSpr.cpp:81`) filled with a pattern word so the two v400 tables are
/// byte-distinguishable.
fn palette_table(word: u16) -> Vec<u8> {
    let unit = word.to_le_bytes();
    (0..spr::TSPR_PALETTE_LEN)
        .map(|i| unit[i % 2])
        .collect::<Vec<u8>>()
}

/// Synthetic v400 sprite in the **source** layout
/// `header | image | pal565(512) | pal555(512)`.
///
/// Provenance: image read first (`TSpr.cpp:63-70`), `== 400` guard
/// (`TSpr.cpp:74`), 555-mode skip-512-then-read-512 (`TSpr.cpp:76-81`).
/// Synthetic (R2 — no real v400 sprite in tree).
fn build_v400_file(image: &[u8]) -> Vec<u8> {
    let mut file = build_sprite_file(
        spr::TSPR_VERSION_16PAL,
        spr::TSPR_ENCODETYPE_SPR,
        2,
        1,
        image,
    );
    file.extend_from_slice(&palette_table(0xAAAA));
    file.extend_from_slice(&palette_table(0x5555));
    file
}

/// Synthetic `.pal`: exactly `256 x 3` RGB bytes (`TSpr.cpp:159-164`
/// refuses any other length). Each entry is `(i, i, i)` so every slot is
/// independently checkable. Synthetic (R2).
fn build_palette_file() -> Vec<u8> {
    let mut raw = vec![0u8; PALETTE_FILE_LEN];
    for (index, triple) in raw
        .as_chunks_mut::<{ palette::RGB_BYTES }>()
        .0
        .iter_mut()
        .enumerate()
    {
        triple.fill(index as u8);
    }
    raw
}

// Map builder constants (X=4, Y=3 -> 12 tiles).
const MAP_X: i16 = 4;
const MAP_Y: i16 = 3;
const MAP_TILES: usize = 12;

/// Synthetic 176-byte `cltMapHeader` at the literal MBCS offsets.
///
/// Provenance: struct fields from
/// `GoonZuSource/GoonZuWorld/common/Map/map.h:176-194`; MBCS (1-byte
/// `TCHAR`) widths from `map.h:33-38` + the `CharacterSet="2"` project
/// setting; 4-byte trailing pad because the struct's max alignment is 4
/// and there is no `#pragma pack` in `map.h`. Synthetic (R2).
fn build_map_header(version: u32) -> Vec<u8> {
    use goonzu_client::graphics::map::offset;

    let mut bytes = vec![0u8; MAP_HEADER_LEN];
    put_u32(&mut bytes, offset::VERSION, version); // map.h:178
    put_u32(&mut bytes, offset::UNIQUE, 7); // map.h:179
    let label = b"synthetic integration v2 (map.h:176-194)\0"; // map.h:180 Text[128]
    bytes[offset::TEXT..offset::TEXT + label.len()].copy_from_slice(label);
    put_u32(&mut bytes, offset::RESERVED0, 0xCAFE_F00D); // map.h:181
    put_i16(&mut bytes, offset::MAP_XSIZE, MAP_X); // map.h:182
    put_i16(&mut bytes, offset::MAP_YSIZE, MAP_Y); // map.h:182
    put_i32(&mut bytes, offset::MAP_TYPE, 5); // map.h:183
    put_i32(&mut bytes, offset::TILE_SET1, 6); // map.h:184
    bytes[offset::B_WEATHER_SWITCH] = 1; // map.h:186 bool = 1 byte (R3)
    bytes[offset::B_WEATHER_SWITCH + 1] = 0xAA; // alignment pad before SI16
    put_i16(&mut bytes, offset::TILE_SET2, 7); // map.h:188
    for (slot, byte) in bytes[offset::RESERVED..offset::RESERVED + 19]
        .iter_mut()
        .enumerate()
    {
        *byte = 0xC0 + slot as u8; // map.h:190 Reserved[19]
    }
    bytes
}

/// Full synthetic v2 map in **`map-save.cpp:44-55` write order** (the
/// parser consumes it in `MapManager.cpp:687-703` read order; the two
/// sources agree — WO-7.1 §6.3 second source).
///
/// Provenance per section:
/// - header: `map-save.cpp:44` (`fwrite(&MH, sizeof(cltMapHeader), 1, fp)`)
/// - ID map: `map-save.cpp:46` + `:73` (`SHORT x X x Y`)
/// - collision map: `map-save.cpp:47` + `:87` (`SHORT x X x Y`)
/// - layer 0/1: `map-save.cpp:49-50`, widths from
///   `MapLayor/Map-Layor.cpp:167-168` (`DWORD` draw info, then `UI08`
///   unique, each `X x Y`)
/// - field-object map: `map-save.cpp:52` + `:101` (`DWORD x X x Y`)
/// - field-object para: `map-save.cpp:53` + `:116` (`DWORD x X x Y`)
/// - GeoInfo tail: `map-save.cpp:55` + `map.cpp:204-207`
///   (`SI32 siXsize, SI32 siYsize, GEO_TEXT_LENGTH*MAX_GEO_NUMBER` text
///   bytes, `UI08 x siXsize x siYsize`)
///
/// Synthetic (R2 — no real `.Map` in tree).
fn build_map_v2() -> Vec<u8> {
    let mut out = build_map_header(2);

    for i in 0..MAP_TILES {
        out.extend_from_slice(&(1000i16 + i as i16).to_le_bytes()); // map-save.cpp:73
    }
    for i in 0..MAP_TILES {
        out.extend_from_slice(&(2000i16 + i as i16).to_le_bytes()); // map-save.cpp:87
    }
    for i in 0..MAP_TILES {
        out.extend_from_slice(&(10_000u32 + i as u32).to_le_bytes()); // Map-Layor.cpp:167
    }
    for i in 0..MAP_TILES {
        out.push(0xA0 + i as u8); // Map-Layor.cpp:168
    }
    for i in 0..MAP_TILES {
        out.extend_from_slice(&(20_000u32 + i as u32).to_le_bytes()); // Map-Layor.cpp:167
    }
    for i in 0..MAP_TILES {
        out.push(0xB0 + i as u8); // Map-Layor.cpp:168
    }
    for i in 0..MAP_TILES {
        out.extend_from_slice(&(30_000u32 + i as u32).to_le_bytes()); // map-save.cpp:101
    }
    for i in 0..MAP_TILES {
        out.extend_from_slice(&(40_000u32 + i as u32).to_le_bytes()); // map-save.cpp:116
    }
    out.extend_from_slice(&2i32.to_le_bytes()); // map.cpp:204 siXsize
    out.extend_from_slice(&2i32.to_le_bytes()); // map.cpp:205 siYsize
    let mut text = vec![0u8; GEO_TEXT_LENGTH * MAX_GEO_NUMBER]; // map.cpp:206
    let slot0 = b"integration geo slot\0";
    text[..slot0.len()].copy_from_slice(slot0);
    out.extend_from_slice(&text);
    out.extend_from_slice(&[3, 2, 1, 0]); // map.cpp:207 UI08 x 2 x 2
    out
}

/// Byte offsets of each section boundary in [`build_map_v2`]'s output.
mod boundary {
    /// After the 176-byte header.
    pub const HEADER: usize = 176;
    /// After ID map (12 x 2 B).
    pub const ID_MAP: usize = HEADER + 24;
    /// After collision map (12 x 2 B).
    pub const COLL_MAP: usize = ID_MAP + 24;
    /// After layer 0 draw info (12 x 4 B).
    pub const LAYER0_DRAW: usize = COLL_MAP + 48;
    /// After layer 0 unique (12 x 1 B).
    pub const LAYER0_UNIQUE: usize = LAYER0_DRAW + 12;
    /// After layer 1 draw info (12 x 4 B).
    pub const LAYER1_DRAW: usize = LAYER0_UNIQUE + 48;
    /// After layer 1 unique (12 x 1 B).
    pub const LAYER1_UNIQUE: usize = LAYER1_DRAW + 12;
    /// After field-object map (12 x 4 B).
    pub const FIELD_OBJECT: usize = LAYER1_UNIQUE + 48;
    /// After field-object para (12 x 4 B).
    pub const FIELD_OBJECT_PARA: usize = FIELD_OBJECT + 48;
    /// After GeoInfo dims (4 + 4 B).
    pub const GEO_DIMS: usize = FIELD_OBJECT_PARA + 8;
    /// After GeoInfo text (64 x 256 B).
    pub const GEO_TEXT: usize = GEO_DIMS + 16_384;
    /// End of file (GeoInfo map 2 x 2 B).
    pub const END: usize = GEO_TEXT + 4;
}

/// A minimal valid `.Spr`: fixed header + three literal indices (one row).
fn small_sprite() -> Vec<u8> {
    build_sprite_file(
        spr::TSPR_VERSION_NOPAL,
        spr::TSPR_ENCODETYPE_SPR,
        3,
        1,
        &[0x11, 0x22, 0x33],
    )
}

// ---------------------------------------------------------------------------
// DoD 10.2 — real Mask.Spr through the public API
// ---------------------------------------------------------------------------

#[test]
fn do10_2_real_mask_spr_decodes_byte_exactly() {
    let bytes = mask_spr();
    let sprite = load_sprite(&bytes, RgbMode::Rgb565).expect("Mask.Spr decodes");

    // Named header expectations (WO-7.1 §10.2).
    assert_eq!(sprite.header.i_version, 10, "iVersion=10");
    assert_eq!(sprite.header.i_encode_type, 0, "iEncodeType=0");
    assert_eq!(sprite.header.i_x_size, 80, "iXSize=80");
    assert_eq!(sprite.header.i_y_size, 40, "iYSize=40");
    assert_eq!(sprite.header.i_image_num, 1, "iImageNum=1");

    // Stream consumption exactly 1760/1760, coverage exactly 3200/3200.
    assert_eq!(sprite.image.bytes_consumed, 1760, "1760/1760 consumed");
    assert_eq!(
        sprite.header.i_image_data_size, 1760,
        "declared stream length"
    );
    assert_eq!(sprite.image.pixels.len(), 3200, "3200/3200 covered");
    assert_eq!(sprite.image.covered_pixels(), 3200, "full coverage");

    // iSize=224 exposed as data, never as a bound (R1).
    assert_eq!(sprite.header.image_info[0].i_size, 224, "iSize exposed");
    assert_ne!(
        sprite.image.bytes_consumed, sprite.header.image_info[0].i_size as usize,
        "iSize must never act as a bound (R1)"
    );

    // v10 carries no embedded palette; the loader stops after the stream
    // (`TSpr.cpp:74` guard is `== 400`).
    assert_eq!(sprite.embedded_palette, None);
    assert_eq!(sprite.bytes_consumed, bytes.len(), "whole file consumed");

    // The fixed header region is 3136 bytes: 3136 + 1760 = 4896 (file
    // size), the fact that disproves the WO §6.1 "28-byte-fixed" snippet.
    assert_eq!(TSPR_HEADER_LEN, 3136);
    assert_eq!(bytes.len(), 4896);
}

// ---------------------------------------------------------------------------
// DoD 10.3 — real monarch.pal through the public API
// ---------------------------------------------------------------------------

#[test]
fn do10_3_real_monarch_pal_is_accepted_and_767_769_refused_by_name() {
    let bytes = monarch_pal();
    assert_eq!(bytes.len(), 768, "fixture is exactly 768 B");

    // Accepted at exactly 768 (TSpr.cpp:159-164).
    let pal =
        load_pal(&bytes, "monarch.pal", RgbMode::Rgb565).expect("exactly 768 bytes are accepted");
    assert_eq!(pal.pal.len(), palette::PALETTE_ENTRIES);

    // 767 and 769 refused **by name** (DoD 10.3).
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
    assert!(err.to_string().contains("monarch.pal"), "{err}");

    let mut long = bytes.clone();
    long.push(0);
    let err = load_pal(&long, "monarch.pal", RgbMode::Rgb565).unwrap_err();
    assert_eq!(
        err,
        PaletteError::WrongSize {
            name: "monarch.pal".to_string(),
            expected: 768,
            got: 769,
        }
    );
    assert!(err.to_string().contains("monarch.pal"), "{err}");

    // 565 conversion of known indices is byte-exact (DoD 10.3): the
    // fixture's first triples are (0,0,0), (55,55,55), (47,47,47),
    // (39,39,39) — independently converted with the TSpr.cpp:185-187
    // transcription (log-verified, term by term).
    assert_eq!(pal.pal[0], 0x0000, "rgb(0,0,0)");
    assert_eq!(pal.pal[1], 0xDEFB, "rgb(55,55,55)");
    assert_eq!(pal.pal[2], 0xBDF7, "rgb(47,47,47)");
    assert_eq!(pal.pal[3], 0x9CF3, "rgb(39,39,39)");
}

// ---------------------------------------------------------------------------
// Synthetic sprite/palette builders decode (R2 provenance)
// ---------------------------------------------------------------------------

#[test]
fn synthetic_spr_stream_decodes_one_index_per_pixel() {
    // Provenance: TSpr.h:32-47 header + Graphics_PutSpr16T.cpp:41-44
    // (literal = one index per pixel).
    let file = build_sprite_file(
        spr::TSPR_VERSION_NOPAL,
        spr::TSPR_ENCODETYPE_SPR,
        4,
        1,
        &[0x11, 0x22, spr::COLOR_TRANS, 0x02],
    );
    let sprite = load_sprite(&file, RgbMode::Rgb565).expect("synthetic SPR decodes");

    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x11), Some(0x22), None, None],
        "SPR: one index per literal, 254+count skips without index bytes"
    );
    assert_eq!(sprite.image.bytes_consumed, 4);
    assert_eq!(sprite.image.covered_pixels(), 4);
    assert_eq!(
        sprite.bytes_consumed,
        TSPR_HEADER_LEN + 4,
        "no palette for v10"
    );
}

#[test]
fn synthetic_mpr_stream_decodes_count_plus_indices() {
    // Provenance: Graphics_PutMpr16.cpp:41-48 (the byte is a run length,
    // then `count` index bytes) + :33-38 (254 + count transparent run).
    let file = build_sprite_file(
        spr::TSPR_VERSION_NOPAL,
        spr::TSPR_ENCODETYPE_MPR,
        4,
        1,
        &[0x02, 0x33, 0x33, spr::COLOR_TRANS, 0x02],
    );
    let sprite = load_sprite(&file, RgbMode::Rgb565).expect("synthetic MPR decodes");

    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x33), Some(0x33), None, None],
        "MPR: count byte then that many indices"
    );
    assert_eq!(sprite.image.bytes_consumed, 5);
    assert_eq!(sprite.image.literal_pixels, 2);
    assert_eq!(sprite.image.transparent_runs, 1);
}

#[test]
fn synthetic_spr_and_mpr_are_not_interchangeable() {
    // Each stream decodes under exactly one codec; running it through the
    // other yields a typed refusal or different pixels (WO-7.1 §7).
    let spr_file = build_sprite_file(
        spr::TSPR_VERSION_NOPAL,
        spr::TSPR_ENCODETYPE_SPR,
        4,
        1,
        &[0x11, 0x22, spr::COLOR_TRANS, 0x02],
    );
    let mpr_file = build_sprite_file(
        spr::TSPR_VERSION_NOPAL,
        spr::TSPR_ENCODETYPE_MPR,
        4,
        1,
        &[0x02, 0x33, 0x33, spr::COLOR_TRANS, 0x02],
    );

    let spr_sprite = load_sprite(&spr_file, RgbMode::Rgb565).unwrap();
    let mpr_sprite = load_sprite(&mpr_file, RgbMode::Rgb565).unwrap();
    assert_ne!(
        spr_sprite.image.pixels, mpr_sprite.image.pixels,
        "the two codecs must disagree on each other's streams"
    );

    // The SPR stream run through the MPR codec (encode type patched to
    // MPR) must be a typed refusal, never a silent interchange.
    let mut mislabeled = spr_file.clone();
    put_i32(&mut mislabeled, 4, spr::TSPR_ENCODETYPE_MPR);
    let err = load_sprite(&mislabeled, RgbMode::Rgb565).expect_err("not MPR data");
    assert!(
        matches!(err, SprError::StreamExhausted { .. }),
        "expected a typed exhaustion, got {err:?}"
    );
}

#[test]
fn synthetic_v400_sprite_loads_the_mode_selected_palette_after_the_image() {
    // Provenance: TSpr.cpp:66 (image), :74 (== 400), :76-81 (skip-512 in
    // 555 mode, then read 512) — layout `header | image | 565 | 555`.
    let file = build_v400_file(&[0x77, 0x88]);

    // 565 mode reads the FIRST table.
    let as565 = load_sprite(&file, RgbMode::Rgb565).expect("v400 565");
    assert_eq!(as565.image.pixels, vec![Some(0x77), Some(0x88)]);
    let pal565 = as565.embedded_palette.expect("v400 carries a palette");
    assert!(pal565.iter().all(|&w| w == 0xAAAA), "first table");
    assert_eq!(
        as565.bytes_consumed,
        TSPR_HEADER_LEN + 2 + spr::TSPR_PALETTE_LEN,
        "header + image + one 512-B table"
    );

    // 555 mode skips the first table and reads the SECOND.
    let as555 = load_sprite(&file, RgbMode::Rgb555).expect("v400 555");
    assert_eq!(as555.image.pixels, vec![Some(0x77), Some(0x88)]);
    let pal555 = as555.embedded_palette.expect("v400 carries a palette");
    assert!(pal555.iter().all(|&w| w == 0x5555), "second table");
    assert_eq!(
        as555.bytes_consumed,
        TSPR_HEADER_LEN + 2 + 2 * spr::TSPR_PALETTE_LEN,
        "555 spans both 512-B tables (TSpr.cpp:76-79 skip + :81 read)"
    );
}

#[test]
fn synthetic_palette_768_converts_every_slot() {
    // Provenance: TSpr.cpp:159-164 (exact-768 gate), :185-187 (565),
    // :193-195 (555). Synthetic entry i = (i, i, i).
    let raw = build_palette_file();
    let pal565 = load_pal(&raw, "synthetic.pal", RgbMode::Rgb565).unwrap();
    let pal555 = load_pal(&raw, "synthetic.pal", RgbMode::Rgb555).unwrap();

    // Log-verified anchors (independent PowerShell transcription).
    assert_eq!(pal565.pal[1], 0x0020, "565 grey 1");
    assert_eq!(pal555.pal[1], 0x0000, "555 grey 1");
    assert_eq!(pal565.pal[55], 0xDEFB, "565 grey 55");
    assert_eq!(pal555.pal[55], 0x6F7B, "555 grey 55");
    assert_eq!(pal565.pal[255], 0xFFFF, "565 grey 255");
    assert_eq!(pal555.pal[255], 0x7FFF, "555 grey 255");

    // The green asymmetry survives at the integration boundary too
    // (565 green is NOT right-shifted — TSpr.cpp:186 vs :194).
    assert_eq!(palette::rgb_to_565(0, 55, 0), 0x06E0);
    assert_eq!(palette::rgb_to_555(0, 55, 0), 0x0360);
}

// ---------------------------------------------------------------------------
// DoD 10.4 — synthetic v2 map, body order + named truncation (R2/R3)
// ---------------------------------------------------------------------------

#[test]
fn do10_4_synthetic_v2_map_decodes_in_mapmanager_body_order() {
    // Builder = map-save.cpp:44-55 write order; parser = MapManager.cpp:
    // 687-703 read order; the two must agree (WO-7.1 §6.3 second source).
    let bytes = build_map_v2();
    assert_eq!(bytes.len(), boundary::END, "16836-byte v2 file");

    let map: MapFile = map::parse_map(&bytes).expect("synthetic v2 map decodes");

    // 1. header (map.h:176-194, MBCS widths — R3).
    let CltMapHeader {
        version,
        unique,
        reserved0,
        map_xsize,
        map_ysize,
        map_type,
        tile_set1,
        b_weather_switch,
        tile_set2,
        ..
    } = &map.header;
    assert_eq!(*version, 2);
    assert_eq!(*unique, 7);
    assert!(map
        .header
        .text_str()
        .starts_with("synthetic integration v2"));
    assert_eq!(*reserved0, 0xCAFE_F00D);
    assert_eq!(*map_xsize, MAP_X);
    assert_eq!(*map_ysize, MAP_Y);
    assert_eq!(*map_type, 5);
    assert_eq!(*tile_set1, 6);
    assert!(*b_weather_switch, "bool is one byte at 152 (R3)");
    assert_eq!(*tile_set2, 7);

    // 2-3. ID + collision maps (map-save.cpp:73, :87) — distinct bases so
    // any transposition is loud.
    assert_eq!(map.id_map[0], 1000);
    assert_eq!(map.id_map[MAP_TILES - 1], 1011);
    assert_eq!(map.coll_map[0], 2000);
    assert_eq!(map.coll_map[MAP_TILES - 1], 2011);

    // 4-5. layer 0 then layer 1: DWORD draw info + UI08 unique each
    // (Map-Layor.cpp:167-168).
    assert_eq!(map.layers[0].draw_info[0], 10_000);
    assert_eq!(map.layers[0].draw_info[11], 10_011);
    assert_eq!(map.layers[0].unique[0], 0xA0);
    assert_eq!(map.layers[0].unique[11], 0xAB);
    assert_eq!(map.layers[1].draw_info[0], 20_000);
    assert_eq!(map.layers[1].draw_info[11], 20_011);
    assert_eq!(map.layers[1].unique[0], 0xB0);
    assert_eq!(map.layers[1].unique[11], 0xBB);

    // 6-7. field-object map then para (map-save.cpp:101, :116).
    assert_eq!(map.field_object_map[0], 30_000);
    assert_eq!(map.field_object_map[11], 30_011);
    assert_eq!(map.field_object_para[0], 40_000);
    assert_eq!(map.field_object_para[11], 40_011);

    // 8. GeoInfo because Version >= 2 (MapManager.cpp:698).
    let geo = map.geo_info.expect("version 2 carries the GeoInfo tail");
    assert_eq!(geo.si_xsize, 2);
    assert_eq!(geo.si_ysize, 2);
    assert_eq!(geo.text.len(), GEO_TEXT_LENGTH * MAX_GEO_NUMBER);
    assert_eq!(geo.text_str(0), "integration geo slot");
    assert_eq!(geo.geo_map, vec![3, 2, 1, 0]);
}

#[test]
fn do10_4_truncated_map_is_refused_at_the_named_section() {
    let bytes = build_map_v2();

    // One cut per section: (cut, section that must be named).
    let cases: &[(usize, MapSection)] = &[
        (boundary::HEADER - 1, MapSection::Header),
        (boundary::HEADER + 14, MapSection::IdMap),
        (boundary::ID_MAP + 10, MapSection::CollMap),
        (boundary::COLL_MAP + 36, MapSection::Layer0DrawInfo),
        (boundary::LAYER0_DRAW + 8, MapSection::Layer0Unique),
        (boundary::LAYER0_UNIQUE + 16, MapSection::Layer1DrawInfo),
        (boundary::LAYER1_DRAW + 8, MapSection::Layer1Unique),
        (boundary::LAYER1_UNIQUE + 16, MapSection::FieldObjectMap),
        (boundary::FIELD_OBJECT + 28, MapSection::FieldObjectPara),
        (boundary::FIELD_OBJECT_PARA + 4, MapSection::GeoInfoDims),
        (10_000, MapSection::GeoInfoText),
        (boundary::GEO_TEXT + 3, MapSection::GeoInfoMap),
    ];

    for &(cut, section) in cases {
        let err = map::parse_map(&bytes[..cut]).expect_err("cut must be refused");
        match &err {
            MapError::Truncated { section: got, .. } => {
                assert_eq!(*got, section, "cut at {cut} must refuse at `{section}`")
            }
            other => panic!("cut at {cut}: expected Truncated, got {other:?}"),
        }
        assert!(
            err.to_string().contains(section.name()),
            "message must name `{}`: {err}",
            section.name()
        );
    }

    // An empty stream is refused at the header gate first.
    assert!(matches!(
        map::parse_map(&[]),
        Err(MapError::Truncated {
            section: MapSection::Header,
            got: 0,
            ..
        })
    ));
}

#[test]
fn synthetic_map_negative_dimensions_are_refused() {
    // Negative tile counts cannot size the fixed maps (map-save.cpp:73
    // reads MapXsize * MapYsize elements).
    let mut bytes = build_map_header(2);
    put_i16(
        &mut bytes,
        goonzu_client::graphics::map::offset::MAP_XSIZE,
        -1,
    );
    bytes.extend_from_slice(&build_map_v2()[boundary::HEADER..]);
    assert_eq!(
        map::parse_map(&bytes).unwrap_err(),
        MapError::NegativeDimension {
            field: "MapXsize",
            value: -1
        }
    );
}

// ---------------------------------------------------------------------------
// DoD 10.5 — archive member-vs-loose, decoded through the §6.1 loader
// ---------------------------------------------------------------------------

#[test]
fn do10_5_archive_member_wins_then_loose_fallback_then_decode() {
    let member = small_sprite();
    let mut loose = small_sprite();
    // Tag the loose copy's first IMAGE byte (offset 0 is `iVersion`; the
    // image stream starts at TSPR_HEADER_LEN) so it is distinguishable
    // from the member bytes both raw and after decode.
    loose[TSPR_HEADER_LEN] = 0xEE;

    // Member present -> archive wins (XSPRObj.cpp:453, :477).
    let with_member =
        AssetSource::archive(vec![("HERO.SPR", MemberBytes::Data(&member))], Some(&loose));
    let (bytes, resolved) = with_member.resolve("hero.spr").expect("member");
    assert_eq!(resolved, Resolved::FromArchive, "case-insensitive match");
    assert_eq!(bytes, member.as_slice());

    let (sprite, resolved) = with_member
        .load_sprite("HERO.SPR", RgbMode::Rgb565)
        .expect("archive bytes decode through load_sprite");
    assert_eq!(resolved, Resolved::FromArchive);
    assert_eq!(sprite.header.i_x_size, 3);
    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x11), Some(0x22), Some(0x33)],
        "the extracted bytes reach the §6.1 loader unchanged"
    );

    // Member absent -> loose fallback (XSPRObj.cpp:487-503).
    let without_member = AssetSource::archive(
        vec![("OTHER.SPR", MemberBytes::Data(&member))],
        Some(&loose),
    );
    let (bytes, resolved) = without_member.resolve("HERO.SPR").expect("fallback");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(bytes[TSPR_HEADER_LEN], 0xEE, "the loose file's bytes");

    let (sprite, resolved) = without_member
        .load_sprite("HERO.SPR", RgbMode::Rgb565)
        .expect("loose bytes decode too");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(sprite.image.pixels[0], Some(0xEE));

    // Member found but unreadable -> error, **no** loose fallback even
    // though a good loose file is right there (XSPRObj.cpp:473-486).
    let unreadable =
        AssetSource::archive(vec![("HERO.SPR", MemberBytes::Unreadable)], Some(&loose));
    let err = unreadable.resolve("HERO.SPR").expect_err("no fallback");
    assert_eq!(
        err,
        ArchiveError::MemberReadFailed {
            member: "HERO.SPR".to_string()
        }
    );
    assert!(err.to_string().contains("HERO.SPR"), "{err}");

    // Neither side -> named error (XSPRObj.cpp:491-503).
    let nothing = AssetSource::archive(vec![], None);
    assert_eq!(
        nothing.resolve("HERO.SPR").expect_err("nothing to serve"),
        ArchiveError::MissingSource {
            member: "HERO.SPR".to_string()
        }
    );

    // The same seam carries the `.mz` field-object path
    // (FieldObject.cpp:342,345,368).
    let mz = AssetSource::archive(vec![("DUMMY.mz", MemberBytes::Data(&member))], None);
    let (bytes, resolved) = mz.resolve("DUMMY.MZ").expect("mz member");
    assert_eq!(resolved, Resolved::FromArchive);
    assert_eq!(bytes, member.as_slice());

    // Corrupt member bytes surface as a typed SprError, never a panic.
    let corrupt = vec![0u8; 64];
    let corrupt_source = AssetSource::archive(vec![("BAD.SPR", MemberBytes::Data(&corrupt))], None);
    let err = corrupt_source
        .load_sprite("BAD.SPR", RgbMode::Rgb565)
        .expect_err("not a sprite");
    assert!(
        matches!(
            err,
            AssetError::Sprite(SprError::HeaderTruncated { got: 64, .. })
        ),
        "{err:?}"
    );
}
