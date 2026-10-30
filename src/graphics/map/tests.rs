//! Unit tests for the `.Map` decoder.
//!
//! **Synthetic fixtures** (R2: no real `.Map`/`.Mpr`/`.sz`/`.mz` exists in
//! the tree — verified by a repo-wide scan). Every builder below is
//! constructed from the cited legacy layout and carries its provenance:
//!
//! | Region | Built from |
//! |---|---|
//! | header (176 B, MBCS) | `map.h:176-194` + `map.h:33-38,258-259` |
//! | ID / collision maps | `map-save.cpp:73`, `:87` (`SHORT × X × Y`) |
//! | layers 0-1 | `Map-Layor.cpp:167-168` (`DWORD` then `UI08`, `X × Y`) |
//! | field-object maps | `map-save.cpp:101`, `:116` (`DWORD × X × Y`) |
//! | GeoInfo tail | `map.cpp:204-207` |
//! | **write order** (builder) | `map-save.cpp:44-55` |
//! | **read order** (parser) | `MapManager.cpp:687-703` |
//!
//! The builder lays sections down in **write** order and the parser must
//! consume them in **read** order; §6.3's second source is exactly that
//! agreement (`WO-7.1.md` R4), and the distinct sentinel values per section
//! make any transposition fail loudly.

use super::*;

/// Map width in tiles for the synthetic fixture.
const X: i16 = 4;
/// Map height in tiles for the synthetic fixture.
const Y: i16 = 3;
/// `X × Y` tiles.
const TILES: usize = 12;

/// Writes one little-endian `i32` into a scratch buffer.
fn put_i32(bytes: &mut [u8], at: usize, value: i32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Writes one little-endian `u32`.
fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Writes one little-endian `i16`.
fn put_i16(bytes: &mut [u8], at: usize, value: i16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// Builds the 176-byte `cltMapHeader` at the literal MBCS offsets of
/// `map.h:176-194`.
fn build_header(version: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; MAP_HEADER_LEN];

    put_u32(&mut bytes, offset::VERSION, version);
    put_u32(&mut bytes, offset::UNIQUE, 7);
    // Text[128] (map.h:180) - a 128-BYTE MBCS field, not 128 wchars (R3).
    let label = b"synthetic v2 map (provenance: map.h:176-194)\0";
    bytes[offset::TEXT..offset::TEXT + label.len()].copy_from_slice(label);
    put_u32(&mut bytes, offset::RESERVED0, 0x1234_5678);
    put_i16(&mut bytes, offset::MAP_XSIZE, X);
    put_i16(&mut bytes, offset::MAP_YSIZE, Y);
    put_i32(&mut bytes, offset::MAP_TYPE, 11);
    put_i32(&mut bytes, offset::TILE_SET1, 22);
    // bool is one byte on disk (map.h:186), then one pad byte before the
    // 2-aligned SI16 at 154 (R3).
    bytes[offset::B_WEATHER_SWITCH] = 1;
    bytes[offset::B_WEATHER_SWITCH + 1] = 0xFF; // the alignment pad byte
    put_i16(&mut bytes, offset::TILE_SET2, 33);
    // Reserved[19] (map.h:190).
    for (slot, byte) in bytes[offset::RESERVED..offset::RESERVED + 19]
        .iter_mut()
        .enumerate()
    {
        *byte = slot as u8 + 1;
    }

    debug_assert_eq!(offset::RESERVED + 19, 175, "fields end at 175");
    debug_assert_eq!(MAP_HEADER_LEN, 176, "padded to the 4-byte alignment");
    bytes
}

/// Appends `count` little-endian `u32`s with a visible pattern.
fn push_u32s(out: &mut Vec<u8>, base: u32, count: usize) {
    for i in 0..count {
        out.extend_from_slice(&(base + i as u32).to_le_bytes());
    }
}

/// Appends `count` little-endian `i16`s with a visible pattern.
fn push_i16s(out: &mut Vec<u8>, base: i16, count: usize) {
    for i in 0..count {
        out.extend_from_slice(&(base + i as i16).to_le_bytes());
    }
}

/// Builds the full synthetic v2 map in **`map-save.cpp:44-55` write
/// order**: header → IDMap → CollMap → layer0 → layer1 → FOMap → FOpara →
/// GeoInfo.
///
/// Provenance for each region is listed in the module docs. Synthetic by
/// necessity (R2).
fn build_map_v2() -> Vec<u8> {
    let mut out = build_header(2);

    // map-save.cpp:44 fwrite(&MH, sizeof(cltMapHeader), 1, fp)
    // map-save.cpp:46 WriteIDMap   -> SHORT x X x Y   (map-save.cpp:73)
    push_i16s(&mut out, 1000, TILES);
    // map-save.cpp:47 WriteCollMap -> SHORT x X x Y   (map-save.cpp:87)
    push_i16s(&mut out, 2000, TILES);
    // map-save.cpp:49 pclLayer[0]->Write -> DWORD then UI08 (Map-Layor.cpp:157,160)
    push_u32s(&mut out, 10_000, TILES);
    out.extend((0..TILES).map(|i| 0xA0 + i as u8));
    // map-save.cpp:50 pclLayer[1]->Write
    push_u32s(&mut out, 20_000, TILES);
    out.extend((0..TILES).map(|i| 0xB0 + i as u8));
    // map-save.cpp:52 WriteFieldObjectMap -> DWORD x X x Y (map-save.cpp:101)
    push_u32s(&mut out, 30_000, TILES);
    // map-save.cpp:53 WriteFieldObjectMapPara -> DWORD x X x Y (:116)
    push_u32s(&mut out, 40_000, TILES);
    // map-save.cpp:55 pclMapGeoInfo->Write -> SI32, SI32, text, geo-map
    // (map.cpp:195-198)
    push_i32s(&mut out, &[2, 2]); // siXsize, siYsize
    let mut text = vec![0u8; GEO_TEXT_LENGTH * MAX_GEO_NUMBER];
    let slot0 = b"geo-slot-0\0";
    text[..slot0.len()].copy_from_slice(slot0);
    out.extend_from_slice(&text);
    out.extend_from_slice(&[9, 8, 7, 6]); // UI08 x siXsize x siYsize

    out
}

/// Appends `count` little-endian `i32`s from a literal list.
fn push_i32s(out: &mut Vec<u8>, values: &[i32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Byte offsets of every section boundary in [`build_map_v2`]'s output,
/// used to pick truncation points that land inside each named section.
mod boundary {
    /// After the 176-byte header.
    pub const HEADER: usize = 176;
    /// After ID map (24 B).
    pub const ID_MAP: usize = HEADER + 24;
    /// After collision map (24 B).
    pub const COLL_MAP: usize = ID_MAP + 24;
    /// After layer 0 draw info (48 B).
    pub const LAYER0_DRAW: usize = COLL_MAP + 48;
    /// After layer 0 unique (12 B).
    pub const LAYER0_UNIQUE: usize = LAYER0_DRAW + 12;
    /// After layer 1 draw info (48 B).
    pub const LAYER1_DRAW: usize = LAYER0_UNIQUE + 48;
    /// After layer 1 unique (12 B).
    pub const LAYER1_UNIQUE: usize = LAYER1_DRAW + 12;
    /// After field-object map (48 B).
    pub const FIELD_OBJECT: usize = LAYER1_UNIQUE + 48;
    /// After field-object para (48 B).
    pub const FIELD_OBJECT_PARA: usize = FIELD_OBJECT + 48;
    /// After GeoInfo dimensions (8 B).
    pub const GEO_DIMS: usize = FIELD_OBJECT_PARA + 8;
    /// After GeoInfo text (16384 B).
    pub const GEO_TEXT: usize = GEO_DIMS + 16_384;
    /// End of file (GeoInfo map, 4 B).
    pub const END: usize = GEO_TEXT + 4;
}

// ---------------------------------------------------------------------------
// DoD 10.4 - the synthetic v2 map decodes in the MapManager body order
// ---------------------------------------------------------------------------

#[test]
fn synthetic_v2_map_decodes_in_mapmanager_body_order() {
    // Builder = map-save.cpp:44-55 write order; parser = MapManager.cpp:
    // 687-703 read order. The two must agree (R4 second source).
    let bytes = build_map_v2();
    assert_eq!(
        bytes.len(),
        boundary::END,
        "write order produces the whole file: 16836 bytes"
    );

    let map = parse_map(&bytes).expect("synthetic v2 map decodes");

    // 1. header (map.h:176-194).
    assert_eq!(map.header.version, 2);
    assert_eq!(map.header.unique, 7);
    assert!(map.header.text_str().starts_with("synthetic v2 map"));
    assert_eq!(map.header.reserved0, 0x1234_5678);
    assert_eq!(map.header.map_xsize, X);
    assert_eq!(map.header.map_ysize, Y);
    assert_eq!(map.header.map_type, 11);
    assert_eq!(map.header.tile_set1, 22);
    assert!(map.header.b_weather_switch);
    assert_eq!(map.header.tile_set2, 33);
    assert_eq!(&map.header.reserved[..3], &[1, 2, 3]);

    // 2. ID map (map-save.cpp:73) - base 1000 distinguishes it from #3.
    assert_eq!(map.id_map.len(), TILES);
    assert_eq!(map.id_map[0], 1000);
    assert_eq!(map.id_map[TILES - 1], 1000 + (TILES - 1) as i16);

    // 3. collision map (map-save.cpp:87) - base 2000.
    assert_eq!(map.coll_map.len(), TILES);
    assert_eq!(map.coll_map[0], 2000);
    assert_eq!(map.coll_map[TILES - 1], 2000 + (TILES - 1) as i16);

    // 4-5. layers 0 then 1 (Map-Layor.cpp:167-168).
    assert_eq!(map.layers[0].draw_info[0], 10_000);
    assert_eq!(
        map.layers[0].draw_info[TILES - 1],
        10_000 + (TILES - 1) as u32
    );
    assert_eq!(map.layers[0].unique[0], 0xA0);
    assert_eq!(map.layers[0].unique[TILES - 1], 0xA0 + (TILES - 1) as u8);
    assert_eq!(map.layers[1].draw_info[0], 20_000);
    assert_eq!(
        map.layers[1].draw_info[TILES - 1],
        20_000 + (TILES - 1) as u32
    );
    assert_eq!(map.layers[1].unique[0], 0xB0);
    assert_eq!(map.layers[1].unique[TILES - 1], 0xB0 + (TILES - 1) as u8);
    assert_eq!(map.layers[0].draw_info.len(), TILES);
    assert_eq!(map.layers[1].unique.len(), TILES);

    // 6-7. field-object map, then para (map-save.cpp:101, :116).
    assert_eq!(map.field_object_map[0], 30_000);
    assert_eq!(map.field_object_map[TILES - 1], 30_000 + (TILES - 1) as u32);
    assert_eq!(map.field_object_para[0], 40_000);
    assert_eq!(
        map.field_object_para[TILES - 1],
        40_000 + (TILES - 1) as u32
    );

    // 8. GeoInfo because Version == 2 (MapManager.cpp:698).
    let geo = map.geo_info.expect("version 2 carries the GeoInfo tail");
    assert_eq!(geo.si_xsize, 2);
    assert_eq!(geo.si_ysize, 2);
    assert_eq!(geo.text.len(), GEO_TEXT_LENGTH * MAX_GEO_NUMBER);
    assert_eq!(geo.text_str(0), "geo-slot-0");
    assert_eq!(geo.text_str(1), "");
    assert_eq!(geo.geo_map, vec![9, 8, 7, 6]);

    // No bytes were consumed beyond the declared sections.
    assert_eq!(bytes.len(), boundary::END);
}

#[test]
fn section_sizes_match_the_cited_write_and_read_methods() {
    // The arithmetic behind `boundary::*`, spelled out from both sources:
    // SHORT (2) per tile for ID/coll (map-save.cpp:73,87), DWORD (4) + UI08
    // (1) per tile per layer (Map-Layor.cpp:167-168), DWORD (4) per tile
    // for each field-object map (map-save.cpp:101,116), and GeoInfo
    // = 4 + 4 + 64*256 + x*y (map.cpp:204-207).
    assert_eq!(boundary::ID_MAP - boundary::HEADER, TILES * 2);
    assert_eq!(boundary::COLL_MAP - boundary::ID_MAP, TILES * 2);
    assert_eq!(boundary::LAYER0_DRAW - boundary::COLL_MAP, TILES * 4);
    assert_eq!(boundary::LAYER0_UNIQUE - boundary::LAYER0_DRAW, TILES);
    assert_eq!(boundary::LAYER1_DRAW - boundary::LAYER0_UNIQUE, TILES * 4);
    assert_eq!(boundary::LAYER1_UNIQUE - boundary::LAYER1_DRAW, TILES);
    assert_eq!(boundary::FIELD_OBJECT - boundary::LAYER1_UNIQUE, TILES * 4);
    assert_eq!(
        boundary::FIELD_OBJECT_PARA - boundary::FIELD_OBJECT,
        TILES * 4
    );
    assert_eq!(boundary::GEO_DIMS - boundary::FIELD_OBJECT_PARA, 8);
    assert_eq!(boundary::GEO_TEXT - boundary::GEO_DIMS, 16_384);
    assert_eq!(boundary::END - boundary::GEO_TEXT, 4);

    // Section-count constants from map.h.
    assert_eq!(MAX_MAP_LAYER, 2, "map.h:38");
    assert_eq!(MAP_TILE_XSIZE, 80, "map.h:33");
    assert_eq!(MAP_TILE_YSIZE, 40, "map.h:35");
    assert_eq!(MAX_GEO_NUMBER, 256, "map.h:258");
    assert_eq!(GEO_TEXT_LENGTH, 64, "map.h:259");
    assert_eq!(GEO_TEXT_LENGTH * MAX_GEO_NUMBER, 16_384);
}

// ---------------------------------------------------------------------------
// DoD 10.4 - truncation is refused at the NAMED section (P3-2)
// ---------------------------------------------------------------------------

#[test]
fn truncated_stream_is_refused_at_the_named_section() {
    let bytes = build_map_v2();

    // (cut point, section that must be named in the refusal)
    let cases: &[(usize, MapSection)] = &[
        (100, MapSection::Header),
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

    for &(cut, expected) in cases {
        let err = parse_map(&bytes[..cut]).expect_err("cut must be refused");
        assert_eq!(
            err,
            MapError::Truncated {
                section: expected,
                needed: expected_needed(expected),
                got: expected_got(expected, cut),
            },
            "cut at {cut} bytes must refuse at `{expected}`"
        );
        // The refusal names its section in prose too.
        assert!(
            err.to_string().contains(expected.name()),
            "message must name `{}`: {err}",
            expected.name()
        );
    }
}

/// Bytes this section requires (from the cited layout).
fn expected_needed(section: MapSection) -> usize {
    match section {
        MapSection::Header => MAP_HEADER_LEN,
        MapSection::IdMap | MapSection::CollMap => TILES * 2,
        MapSection::Layer0DrawInfo
        | MapSection::Layer0Unique
        | MapSection::Layer1DrawInfo
        | MapSection::Layer1Unique
        | MapSection::FieldObjectMap
        | MapSection::FieldObjectPara => {
            if matches!(section, MapSection::Layer0Unique | MapSection::Layer1Unique) {
                TILES
            } else {
                TILES * 4
            }
        }
        MapSection::GeoInfoDims => 8,
        MapSection::GeoInfoText => GEO_TEXT_LENGTH * MAX_GEO_NUMBER,
        MapSection::GeoInfoMap => 4,
    }
}

/// Bytes actually available at the moment that section is reached.
fn expected_got(section: MapSection, cut: usize) -> usize {
    let reached = match section {
        MapSection::Header => 0,
        MapSection::IdMap => boundary::HEADER,
        MapSection::CollMap => boundary::ID_MAP,
        MapSection::Layer0DrawInfo => boundary::COLL_MAP,
        MapSection::Layer0Unique => boundary::LAYER0_DRAW,
        MapSection::Layer1DrawInfo => boundary::LAYER0_UNIQUE,
        MapSection::Layer1Unique => boundary::LAYER1_DRAW,
        MapSection::FieldObjectMap => boundary::LAYER1_UNIQUE,
        MapSection::FieldObjectPara => boundary::FIELD_OBJECT,
        MapSection::GeoInfoDims => boundary::FIELD_OBJECT_PARA,
        MapSection::GeoInfoText => boundary::GEO_DIMS,
        MapSection::GeoInfoMap => boundary::GEO_TEXT,
    };
    cut.saturating_sub(reached)
}

// ---------------------------------------------------------------------------
// R3 - MBCS widths
// ---------------------------------------------------------------------------

#[test]
fn r3_header_is_176_bytes_with_the_cited_mbc_offsets() {
    // The UTF-16 reading would be 288+ bytes and would push every field
    // past these offsets; the MBCS reading must land exactly here.
    assert_eq!(MAP_HEADER_LEN, 176, "sizeof(cltMapHeader), MBCS, 4-aligned");
    assert_eq!(offset::VERSION, 0, "map.h:178");
    assert_eq!(offset::UNIQUE, 4, "map.h:179");
    assert_eq!(offset::TEXT, 8, "map.h:180 - 128 BYTES of TCHAR");
    assert_eq!(offset::RESERVED0, 136, "8 + 128, map.h:181");
    assert_eq!(offset::MAP_XSIZE, 140, "map.h:182");
    assert_eq!(offset::MAP_YSIZE, 142, "map.h:182");
    assert_eq!(offset::MAP_TYPE, 144, "map.h:183");
    assert_eq!(offset::TILE_SET1, 148, "map.h:184");
    assert_eq!(offset::B_WEATHER_SWITCH, 152, "map.h:186 - bool is 1 byte");
    assert_eq!(offset::TILE_SET2, 154, "map.h:188 - 2-aligned past the pad");
    assert_eq!(offset::RESERVED, 156, "map.h:190 - 19 BYTES of TCHAR");
    assert_eq!(offset::RESERVED + 19, 175, "fields end at 175");
    assert_eq!(MAP_HEADER_LEN, 176, "one pad byte to the 4-byte alignment");
}

#[test]
fn r3_text_field_is_128_bytes_not_128_wchars() {
    let mut header = build_header(1);
    // Fill all 128 text bytes with high (MBCS) values so a UTF-16 reader
    // would either overrun or land differently.
    for byte in header[offset::TEXT..offset::TEXT + 128].iter_mut() {
        *byte = 0x80;
    }
    header[offset::TEXT..offset::TEXT + 5].copy_from_slice(b"abc\0\x80");

    let parsed = parse_header(&header).unwrap();
    assert_eq!(parsed.text.len(), 128, "TCHAR is one byte here (R3)");
    assert_eq!(parsed.text[0], b'a');
    assert_eq!(parsed.text[4], 0x80, "byte 4 survives the NUL at index 3");
    assert_eq!(
        parsed.text[127], 0x80,
        "the last text byte is at offset 135"
    );
    assert_eq!(parsed.text_str(), "abc", "cut at the first NUL");

    // Reserved[19] is 19 bytes (map.h:190), not 19 wchars.
    assert_eq!(parsed.reserved.len(), 19);
    assert_eq!(parsed.reserved, {
        let mut expected = [0u8; 19];
        for (slot, byte) in expected.iter_mut().enumerate() {
            *byte = slot as u8 + 1;
        }
        expected
    });

    // The byte after the bool is padding, not data: TileSet2 reads its own
    // value from 154, proving bool occupies exactly one byte.
    assert!(parsed.b_weather_switch);
    assert_eq!(parsed.tile_set2, 33);
}

// ---------------------------------------------------------------------------
// Version gate for the GeoInfo tail
// ---------------------------------------------------------------------------

#[test]
fn version_below_2_has_no_geo_tail() {
    // map-save.cpp:34 always writes Version = 2 for new maps, but the read
    // path gates on `>= 2` (MapManager.cpp:698): a v1 file simply stops
    // after field-object para, and trailing bytes are not demanded.
    let mut bytes = build_header(1);
    bytes.extend(build_map_v2()[boundary::HEADER..boundary::FIELD_OBJECT_PARA].to_vec());

    let map = parse_map(&bytes).expect("v1 map decodes without GeoInfo");
    assert_eq!(map.header.version, 1);
    assert_eq!(map.geo_info, None, "no GeoInfo below version 2");
    assert_eq!(
        map.field_object_para[TILES - 1],
        40_000 + (TILES - 1) as u32
    );

    // ...and the same file with version 2 plus the tail is accepted.
    let mut v2 = build_header(2);
    v2.extend(build_map_v2()[boundary::HEADER..].to_vec());
    assert!(parse_map(&v2).unwrap().geo_info.is_some());
}

// ---------------------------------------------------------------------------
// P3-2 typed refusals on unsizable dimensions
// ---------------------------------------------------------------------------

#[test]
fn negative_dimensions_are_refused_by_name() {
    // Negative tile counts cannot size the fixed maps.
    let mut header = build_header(2);
    put_i16(&mut header, offset::MAP_XSIZE, -1);
    let mut bytes = header;
    bytes.extend(build_map_v2()[boundary::HEADER..].to_vec());
    assert_eq!(
        parse_map(&bytes).unwrap_err(),
        MapError::NegativeDimension {
            field: "MapXsize",
            value: -1
        }
    );

    let mut header = build_header(2);
    put_i16(&mut header, offset::MAP_YSIZE, -7);
    let mut bytes = header;
    bytes.extend(build_map_v2()[boundary::HEADER..].to_vec());
    assert!(matches!(
        parse_map(&bytes),
        Err(MapError::NegativeDimension {
            field: "MapYsize",
            value: -7
        })
    ));

    // A negative GeoInfo size cannot address the geo-map either
    // (map.cpp:207 reads siXsize * siYsize bytes).
    let mut bytes = build_map_v2();
    put_i32(&mut bytes, boundary::FIELD_OBJECT_PARA, -1);
    assert!(matches!(
        parse_map(&bytes),
        Err(MapError::NegativeGeoDimension {
            field: "siXsize",
            value: -1
        })
    ));
}

#[test]
fn header_shorter_than_176_bytes_is_refused_first() {
    // The header gate runs before anything else (MapManager.cpp:687).
    assert_eq!(
        parse_header(&[0u8; MAP_HEADER_LEN - 1]).unwrap_err(),
        MapError::Truncated {
            section: MapSection::Header,
            needed: MAP_HEADER_LEN,
            got: MAP_HEADER_LEN - 1,
        }
    );
    assert!(matches!(
        parse_map(&[]),
        Err(MapError::Truncated {
            section: MapSection::Header,
            got: 0,
            ..
        })
    ));
}

#[test]
fn section_names_are_stable_for_refusals() {
    // Each name is the label a refusal must carry (DoD 10.4).
    assert_eq!(MapSection::Header.name(), "cltMapHeader");
    assert_eq!(MapSection::IdMap.name(), "ID map");
    assert_eq!(MapSection::CollMap.name(), "collision map");
    assert_eq!(MapSection::Layer0DrawInfo.name(), "layer 0 draw info");
    assert_eq!(MapSection::Layer0Unique.name(), "layer 0 unique");
    assert_eq!(MapSection::Layer1DrawInfo.name(), "layer 1 draw info");
    assert_eq!(MapSection::Layer1Unique.name(), "layer 1 unique");
    assert_eq!(MapSection::FieldObjectMap.name(), "field-object map");
    assert_eq!(MapSection::FieldObjectPara.name(), "field-object map para");
    assert_eq!(MapSection::GeoInfoDims.name(), "GeoInfo dimensions");
    assert_eq!(MapSection::GeoInfoText.name(), "GeoInfo text");
    assert_eq!(MapSection::GeoInfoMap.name(), "GeoInfo geo-map");
    assert_eq!(MapSection::IdMap.to_string(), "ID map");
}
