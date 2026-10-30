//! `.Map` decoding: `cltMapHeader` plus the layered body, in the exact
//! `MapManager` read order.
//!
//! Format of record:
//!
//! - Header struct: `GoonZuWorld/Common/Map/map.h:176-194`.
//! - Tile/geo constants: `map.h:33-38` and `map.h:258-259`.
//! - **Body read order:** `MapManager/MapManager.cpp:687-703`.
//! - **Second source (P3-7):** the write order in `map-save.cpp:44-55`
//!   (`cltMapCommon::Save`) — independent of the read path; the two agree.
//! - Section widths: `map-save.cpp:63-118` (ID/coll/field-object maps),
//!   `MapLayor/Map-Layor.cpp:154-169` (per-layer draw info + unique),
//!   `map.cpp:193-209` (`cltMapGeoInfo::Write`/`Read`).
//!
//! ## R3 — `TCHAR` is one byte on disk
//!
//! `BeTheRich.vcproj` sets `CharacterSet="2"` (MBCS), so `Text[128]` and
//! `Reserved[19]` are **byte** arrays, not `wchar_t`, and `bool` is one
//! byte. With default (unpacked) MSVC alignment the header lays out as:
//!
//! ```text
//!   0   DWORD Version
//!   4   DWORD Unique
//!   8   TCHAR Text[128]        -> 8..136
//! 136   DWORD Reserved0
//! 140   SHORT MapXsize         142 SHORT MapYsize
//! 144   SI32  MapType
//! 148   SI32  TileSet1
//! 152   bool  bWeatherSwitch   153 pad (SI16 needs 2-alignment)
//! 154   SI16  TileSet2
//! 156   TCHAR Reserved[19]     -> 156..175
//! 175   pad                    -> round to the struct's 4-byte alignment
//! ```
//!
//! = **176 bytes**, and there is no `#pragma pack` anywhere in `map.h`
//! (checked). The layout is asserted field-by-field by a test, and the
//! whole body is then parsed against that 176-byte gate.
//!
//! ## Body order (load-bearing)
//!
//! 1. `cltMapHeader` (176)
//! 2. ID map — `SHORT × X × Y` (`map-save.cpp:73`)
//! 3. collision map — `SHORT × X × Y` (`map-save.cpp:87`)
//! 4. layer 0 — `DWORD × X × Y` draw info, then `UI08 × X × Y` unique
//!    (`Map-Layor.cpp:167-168`)
//! 5. layer 1 — same (`Map-Layor.cpp:167-168`)
//! 6. field-object map — `DWORD × X × Y` (`map-save.cpp:101`)
//! 7. field-object map para — `DWORD × X × Y` (`map-save.cpp:116`)
//! 8. if `Version >= 2` → GeoInfo: `SI32 siXsize`, `SI32 siYsize`,
//!    `GEO_TEXT_LENGTH × MAX_GEO_NUMBER` text bytes (64 × 256 = 16384),
//!    then `UI08 × siXsize × siYsize` (`map.cpp:204-207`)
//!
//! Step 8 carries one further legacy condition: `MapManager.cpp:701` reads
//! GeoInfo only when the service area is **not** Korea. That gate is a
//! runtime configuration value, not a file property, so a byte-level
//! decoder cannot observe it — this port keys solely on `Version >= 2`,
//! exactly as `map-save.cpp:55` writes it unconditionally.

use thiserror::Error;

use crate::protocol::bytes::{cstr_lossy, i16_at, i32_at, u32_at};

/// On-disk size of `cltMapHeader` under the MBCS (R3) layout derived above
/// (`map.h:176-194`).
pub const MAP_HEADER_LEN: usize = 176;

/// `MAP_TILE_XSIZE` (`map.h:33`).
pub const MAP_TILE_XSIZE: i16 = 80;
/// `MAP_TILE_YSIZE` (`map.h:35`).
pub const MAP_TILE_YSIZE: i16 = 40;
/// `MAX_MAP_LAYER` (`map.h:38`).
pub const MAX_MAP_LAYER: usize = 2;
/// `MAX_GEO_NUMBER` (`map.h:258`).
pub const MAX_GEO_NUMBER: usize = 256;
/// `GEO_TEXT_LENGTH` (`map.h:259`).
pub const GEO_TEXT_LENGTH: usize = 64;

/// Field offsets of [`CltMapHeader`], asserted by test (R3).
pub mod offset {
    /// `Version` (`map.h:178`).
    pub const VERSION: usize = 0;
    /// `Unique` (`map.h:179`).
    pub const UNIQUE: usize = 4;
    /// `Text[128]` (`map.h:180`).
    pub const TEXT: usize = 8;
    /// `Reserved0` (`map.h:181`).
    pub const RESERVED0: usize = 136;
    /// `MapXsize` (`map.h:182`).
    pub const MAP_XSIZE: usize = 140;
    /// `MapYsize` (`map.h:182`).
    pub const MAP_YSIZE: usize = 142;
    /// `MapType` (`map.h:183`).
    pub const MAP_TYPE: usize = 144;
    /// `TileSet1` (`map.h:184`).
    pub const TILE_SET1: usize = 148;
    /// `bWeatherSwitch` (`map.h:186`).
    pub const B_WEATHER_SWITCH: usize = 152;
    /// `TileSet2` (`map.h:188`).
    pub const TILE_SET2: usize = 154;
    /// `Reserved[19]` (`map.h:190`).
    pub const RESERVED: usize = 156;
}

/// `cltMapHeader` (`map.h:176-194`), fields at their literal MBCS offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CltMapHeader {
    /// `Version` — `>= 2` adds the GeoInfo tail (`map.h:178`).
    pub version: u32,
    /// `Unique` (`map.h:179`).
    pub unique: u32,
    /// `Text[128]` raw MBCS bytes, NUL-terminated by convention
    /// (`map.h:180`). Use [`CltMapHeader::text_str`] to decode.
    pub text: [u8; 128],
    /// `Reserved0` (`map.h:181`).
    pub reserved0: u32,
    /// Map width in tiles (`map.h:182`).
    pub map_xsize: i16,
    /// Map height in tiles (`map.h:182`).
    pub map_ysize: i16,
    /// `MapType` (`map.h:183`).
    pub map_type: i32,
    /// `TileSet1` — layer 1 tile set (`map.h:184`).
    pub tile_set1: i32,
    /// `bWeatherSwitch`, one byte on disk (R3) (`map.h:186`).
    pub b_weather_switch: bool,
    /// `TileSet2` — layer 2 tile set (`map.h:188`).
    pub tile_set2: i16,
    /// `Reserved[19]` raw MBCS bytes (`map.h:190`).
    pub reserved: [u8; 19],
}

impl CltMapHeader {
    /// `Text[128]` decoded as the MBCS client stores it — bytes up to the
    /// first NUL, lossily decoded (the shared reader behind
    /// [`crate::protocol::bytes::cstr_lossy`]).
    pub fn text_str(&self) -> String {
        cstr_lossy(&self.text)
    }
}

/// One map layer: `cltMapLayer::Read` (`Map-Layor.cpp:165-169`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapLayer {
    /// `pDrawInfo` — `DWORD × X × Y` (`Map-Layor.cpp:167`).
    pub draw_info: Vec<u32>,
    /// `pUnique` — `UI08 × X × Y` (`Map-Layor.cpp:168`).
    pub unique: Vec<u8>,
}

/// `cltMapGeoInfo::Read` (`map.cpp:202-209`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeoInfo {
    /// `siXsize` (`map.cpp:204`).
    pub si_xsize: i32,
    /// `si_ysize` (`map.cpp:205`).
    pub si_ysize: i32,
    /// `szGetText` — `GEO_TEXT_LENGTH × MAX_GEO_NUMBER` = 64 × 256 raw
    /// bytes (`map.cpp:206`). Decode a slot with [`GeoInfo::text_str`].
    pub text: Vec<u8>,
    /// `pclGeoMap` — `UI08 × siXsize × siYsize` (`map.cpp:207`).
    pub geo_map: Vec<u8>,
}

impl GeoInfo {
    /// One `MAX_GEO_NUMBER` slot of `szGetText`, NUL-cut and lossily
    /// decoded.
    ///
    /// # Panics
    ///
    /// When `index >= `[`MAX_GEO_NUMBER`] — the array bound `map.cpp:206`
    /// reads against.
    pub fn text_str(&self, index: usize) -> String {
        let at = index * GEO_TEXT_LENGTH;
        cstr_lossy(&self.text[at..at + GEO_TEXT_LENGTH])
    }
}

/// A fully parsed `.Map` file, in `MapManager.cpp:687-703` read order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapFile {
    /// Section 1: the fixed header.
    pub header: CltMapHeader,
    /// Section 2: ID map, `X × Y` tiles.
    pub id_map: Vec<i16>,
    /// Section 3: collision map, `X × Y` tiles.
    pub coll_map: Vec<i16>,
    /// Sections 4-5: the two fixed layers (`MAX_MAP_LAYER`).
    pub layers: [MapLayer; MAX_MAP_LAYER],
    /// Section 6: field-object map, `X × Y` tiles.
    pub field_object_map: Vec<u32>,
    /// Section 7: field-object map para, `X × Y` tiles.
    pub field_object_para: Vec<u32>,
    /// Section 8: GeoInfo, present iff `Version >= 2`.
    pub geo_info: Option<GeoInfo>,
}

/// The named body sections, so a refusal can point at the exact one that
/// ran out (P3-2, `WO-7.1.md` §10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapSection {
    /// The 176-byte header.
    Header,
    /// ID map (`ReadIDMap`).
    IdMap,
    /// Collision map (`ReadCollMap`).
    CollMap,
    /// Layer 0 `pDrawInfo`.
    Layer0DrawInfo,
    /// Layer 0 `pUnique`.
    Layer0Unique,
    /// Layer 1 `pDrawInfo`.
    Layer1DrawInfo,
    /// Layer 1 `pUnique`.
    Layer1Unique,
    /// Field-object map.
    FieldObjectMap,
    /// Field-object map para.
    FieldObjectPara,
    /// GeoInfo `siXsize` + `siYsize`.
    GeoInfoDims,
    /// GeoInfo `szGetText`.
    GeoInfoText,
    /// GeoInfo `pclGeoMap`.
    GeoInfoMap,
}

impl MapSection {
    /// The section's name as it appears in refusals.
    pub const fn name(self) -> &'static str {
        match self {
            MapSection::Header => "cltMapHeader",
            MapSection::IdMap => "ID map",
            MapSection::CollMap => "collision map",
            MapSection::Layer0DrawInfo => "layer 0 draw info",
            MapSection::Layer0Unique => "layer 0 unique",
            MapSection::Layer1DrawInfo => "layer 1 draw info",
            MapSection::Layer1Unique => "layer 1 unique",
            MapSection::FieldObjectMap => "field-object map",
            MapSection::FieldObjectPara => "field-object map para",
            MapSection::GeoInfoDims => "GeoInfo dimensions",
            MapSection::GeoInfoText => "GeoInfo text",
            MapSection::GeoInfoMap => "GeoInfo geo-map",
        }
    }
}

impl core::fmt::Display for MapSection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a `.Map` stream was refused (P3-2).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MapError {
    /// A section ran out of bytes — legacy `fread` would have short-read it
    /// without complaint (`MapManager.cpp:687-703`).
    #[error("map truncated at {section}: needs {needed} bytes, {got} available")]
    Truncated {
        /// The exact section that ran out.
        section: MapSection,
        /// Bytes this section requires.
        needed: usize,
        /// Bytes actually available.
        got: usize,
    },

    /// `MapXsize`/`MapYsize` negative — cannot size the fixed maps.
    #[error("map {field} {value} is negative (section sizes cannot be computed)")]
    NegativeDimension {
        /// The offending header field.
        field: &'static str,
        /// The offending value.
        value: i32,
    },

    /// `siXsize`/`si_ysize` in the GeoInfo tail negative.
    #[error("GeoInfo {field} {value} is negative (geo-map size cannot be computed)")]
    NegativeGeoDimension {
        /// The offending GeoInfo field.
        field: &'static str,
        /// The offending value.
        value: i32,
    },
}

/// Cursor that length-gates every section before any field read.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Takes exactly `needed` bytes for `section`, or refuses by name.
    fn take(&mut self, needed: usize, section: MapSection) -> Result<&'a [u8], MapError> {
        let got = self.bytes.len().saturating_sub(self.pos);
        if got < needed {
            return Err(MapError::Truncated {
                section,
                needed,
                got,
            });
        }
        let slice = &self.bytes[self.pos..self.pos + needed];
        self.pos += needed;
        Ok(slice)
    }

    /// Takes `count` little-endian `i16`s (`map-save.cpp:73`).
    fn take_i16s(&mut self, count: usize, section: MapSection) -> Result<Vec<i16>, MapError> {
        let bytes = self.take(count * 2, section)?;
        Ok((0..count).map(|k| i16_at(bytes, k * 2)).collect())
    }

    /// Takes `count` little-endian `u32`s (`map-save.cpp:95,110`,
    /// `Map-Layor.cpp:167`).
    fn take_u32s(&mut self, count: usize, section: MapSection) -> Result<Vec<u32>, MapError> {
        let bytes = self.take(count * 4, section)?;
        Ok((0..count).map(|k| u32_at(bytes, k * 4)).collect())
    }

    /// Takes `count` raw bytes.
    fn take_bytes(&mut self, count: usize, section: MapSection) -> Result<Vec<u8>, MapError> {
        Ok(self.take(count, section)?.to_vec())
    }
}

/// Parses a complete `.Map` file image in `MapManager.cpp:687-703` order.
///
/// Every section is length-gated before it is read, so a truncated stream
/// is refused **at the section that ran out**, by name.
///
/// # Errors
///
/// [`MapError::Truncated`] naming the section, or a
/// [`MapError::NegativeDimension`] / [`MapError::NegativeGeoDimension`]
/// when the header declares a size that cannot be addressed.
pub fn parse_map(bytes: &[u8]) -> Result<MapFile, MapError> {
    let mut cursor = Cursor { bytes, pos: 0 };

    // 1. cltMapHeader (map.h:176-194) — 176 bytes under the MBCS layout.
    let header = parse_header(cursor.take(MAP_HEADER_LEN, MapSection::Header)?)?;

    // Tiles are SHORT (map.h:182); a negative size cannot index anything.
    if header.map_xsize < 0 {
        return Err(MapError::NegativeDimension {
            field: "MapXsize",
            value: header.map_xsize as i32,
        });
    }
    if header.map_ysize < 0 {
        return Err(MapError::NegativeDimension {
            field: "MapYsize",
            value: header.map_ysize as i32,
        });
    }
    let tiles = header.map_xsize as usize * header.map_ysize as usize;

    // 2. ID map: SHORT x X x Y (map-save.cpp:73).
    let id_map = cursor.take_i16s(tiles, MapSection::IdMap)?;
    // 3. Collision map: SHORT x X x Y (map-save.cpp:87).
    let coll_map = cursor.take_i16s(tiles, MapSection::CollMap)?;

    // 4-5. Layers 0 and 1: DWORD draw-info, then UI08 unique
    // (Map-Layor.cpp:167-168; read in MapManager.cpp:692-693).
    let mut layers = core::array::from_fn(|_| MapLayer {
        draw_info: Vec::new(),
        unique: Vec::new(),
    });
    for (index, layer) in layers.iter_mut().enumerate() {
        let (draw_section, unique_section) = match index {
            0 => (MapSection::Layer0DrawInfo, MapSection::Layer0Unique),
            _ => (MapSection::Layer1DrawInfo, MapSection::Layer1Unique),
        };
        layer.draw_info = cursor.take_u32s(tiles, draw_section)?;
        layer.unique = cursor.take_bytes(tiles, unique_section)?;
    }

    // 6. Field-object map: DWORD x X x Y (map-save.cpp:101).
    let field_object_map = cursor.take_u32s(tiles, MapSection::FieldObjectMap)?;
    // 7. Field-object map para: DWORD x X x Y (map-save.cpp:116).
    let field_object_para = cursor.take_u32s(tiles, MapSection::FieldObjectPara)?;

    // 8. GeoInfo iff Version >= 2 (MapManager.cpp:698; written
    // unconditionally at that version by map-save.cpp:55).
    let geo_info = if header.version >= 2 {
        let dims = cursor.take(8, MapSection::GeoInfoDims)?;
        let si_xsize = i32_at(dims, 0);
        let si_ysize = i32_at(dims, 4);
        if si_xsize < 0 {
            return Err(MapError::NegativeGeoDimension {
                field: "siXsize",
                value: si_xsize,
            });
        }
        if si_ysize < 0 {
            return Err(MapError::NegativeGeoDimension {
                field: "siYsize",
                value: si_ysize,
            });
        }
        let geo_tiles = si_xsize as usize * si_ysize as usize;
        let text = cursor.take_bytes(GEO_TEXT_LENGTH * MAX_GEO_NUMBER, MapSection::GeoInfoText)?;
        let geo_map = cursor.take_bytes(geo_tiles, MapSection::GeoInfoMap)?;
        Some(GeoInfo {
            si_xsize,
            si_ysize,
            text,
            geo_map,
        })
    } else {
        None
    };

    Ok(MapFile {
        header,
        id_map,
        coll_map,
        layers,
        field_object_map,
        field_object_para,
        geo_info,
    })
}

/// Parses the 176-byte [`CltMapHeader`] at its literal offsets.
///
/// # Errors
///
/// [`MapError::Truncated`] when fewer than [`MAP_HEADER_LEN`] bytes are
/// offered.
pub fn parse_header(bytes: &[u8]) -> Result<CltMapHeader, MapError> {
    if bytes.len() < MAP_HEADER_LEN {
        return Err(MapError::Truncated {
            section: MapSection::Header,
            needed: MAP_HEADER_LEN,
            got: bytes.len(),
        });
    }

    let mut text = [0u8; 128];
    text.copy_from_slice(&bytes[offset::TEXT..offset::TEXT + 128]);
    let mut reserved = [0u8; 19];
    reserved.copy_from_slice(&bytes[offset::RESERVED..offset::RESERVED + 19]);

    Ok(CltMapHeader {
        version: u32_at(bytes, offset::VERSION),
        unique: u32_at(bytes, offset::UNIQUE),
        text,
        reserved0: u32_at(bytes, offset::RESERVED0),
        map_xsize: i16_at(bytes, offset::MAP_XSIZE),
        map_ysize: i16_at(bytes, offset::MAP_YSIZE),
        map_type: i32_at(bytes, offset::MAP_TYPE),
        tile_set1: i32_at(bytes, offset::TILE_SET1),
        b_weather_switch: bytes[offset::B_WEATHER_SWITCH] != 0,
        tile_set2: i16_at(bytes, offset::TILE_SET2),
        reserved,
    })
}

#[cfg(test)]
mod tests;
