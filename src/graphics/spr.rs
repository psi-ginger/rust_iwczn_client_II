//! `.Spr` sprite decoding: `TSprHeader`, `TSprImageInfo`, and the two RLE
//! codecs (SPR = opaque sprite, MPR = tile sprite).
//!
//! Format of record:
//!
//! - Structs and constants: `GoonZu_Source/NLib/TSpr.h:11-47`.
//! - Load order: `NLib/TSpr.cpp:54-92` (`LoadTSprFromFile`) and
//!   `:94-127` (`LoadTSprFromMemory`) — the two agree.
//! - SPR RLE: `GoonZuWorld/Lib/NGraphics/Graphics_PutSpr16T.cpp:28-48`.
//! - MPR RLE: `Lib/NGraphics/Graphics_PutMpr16.cpp:27-53`.
//!
//! ## On-disk layout (the load-bearing fact)
//!
//! Legacy reads the header with `ReadFile(hFile, &header, sizeof(header),
//! …)` (`TSpr.cpp:54`) — i.e. the **whole `sizeof(TSprHeader)`**, which is a
//! fixed **3136 bytes**, because `ImageInfo[256]` is a fixed array member
//! (`TSpr.h:46`):
//!
//! ```text
//!   0.. 28   SI32 iVersion, iEncodeType, iXSize, iYSize, iHRes, iVRes,
//!            iImageDataSize              (7 x 4)
//!  28.. 60   SI32 reserved[8]            (8 x 4, ignored)
//!  60.. 64   SI32 iImageNum
//!  64..3136  TSprImageInfo ImageInfo[256] (256 x 12)
//! 3136..     image byte-stream of iImageDataSize bytes
//! ```
//!
//! Verified against the real `Mask.Spr`: `3136 + 1760 (iImageDataSize) =
//! 4896 = filesize`, with `iImageNum = 1`.
//!
//! > **Finding (WO-7.1 §6.1).** The work order's inline struct snippet
//! > transcribes `iImageNum` at offset 24 with `reserved[7]` and calls the
//! > header "28-byte-fixed". The cited source (`TSpr.h:32-47`) has
//! > `iImageDataSize` at 24, `reserved[8]`, `iImageNum` at 60 and a fixed
//! > `ImageInfo[256]`, so the on-disk header is 3136 bytes. The §10.2
//! > expected values are all consistent with the source reading. Per P3-1
//! > the source is canonical; this port implements the source.
//!
//! ## Load order
//!
//! 1. header — exactly `sizeof(TSprHeader)` bytes (`TSpr.cpp:54-59`);
//! 2. image data — exactly `iImageDataSize` bytes (`TSpr.cpp:63-70`);
//! 3. **only when `iVersion == TSPR_VERSION_16PAL (400)`** — a 512-byte
//!    skip in 555 mode, then a 512-byte palette read (`TSpr.cpp:74-87`).
//!
//! > **Finding (WO-7.1 R5).** R5 states the guard is `iVersion >= 400` and
//! > that the 512 palette bytes are skipped **before** the image data. Both
//! > statements contradict the cited source: the guard is `== 400`
//! > (`TSpr.cpp:74`, `:114`) and the palette is read **after** the image
//! > (`TSpr.cpp:66` precedes `:74`). The file therefore lays out
//! > `header | image | pal565(512) | pal555(512)`; in 565 mode the loader
//! > reads the first table, in 555 mode it skips the first and reads the
//! > second. This port implements the source; both quirks are pinned by
//! > failing-if-corrected tests.
//!
//! ## R1 — `iSize` is data, not a bound
//!
//! The blitter walks rows to `width`/`height` and never reads
//! `TSprImageInfo.iSize` (`Graphics_PutSpr16T.cpp:28-48`). Real `Mask.Spr`
//! declares `iSize = 224` while its stream is 1760 bytes and still decodes
//! to full coverage. This port exposes `i_size` as data only and never
//! truncates or rejects on it.

use thiserror::Error;

use crate::graphics::palette::RgbMode;
use crate::protocol::bytes::{i32_at, u16_at};

/// `TSPR_ENCODETYPE_SPR` — ordinary (opaque) sprite (`TSpr.h:11`).
pub const TSPR_ENCODETYPE_SPR: i32 = 0;
/// `TSPR_ENCODETYPE_MPR` — tile sprite (`TSpr.h:12`).
pub const TSPR_ENCODETYPE_MPR: i32 = 1;
/// `TSPR_VERSION_NOPAL` — header only, no embedded palette (`TSpr.h:14`).
pub const TSPR_VERSION_NOPAL: i32 = 10;
/// `TSPR_VERSION_16PAL` — embedded 512-byte palette table(s) (`TSpr.h:15`).
pub const TSPR_VERSION_16PAL: i32 = 400;
/// `COLOR_TRANS` — transparent-run sentinel (`Graphics.h:44`).
pub const COLOR_TRANS: u8 = 254;

/// `ImageInfo[256]` — the fixed array bound (`TSpr.h:46`).
pub const TSPR_IMAGE_NUM_MAX: usize = 256;
/// `sizeof(TSprImageInfo)` = 3 × `SI32` (`TSpr.h:21-26`).
pub const TSPR_IMAGE_INFO_LEN: usize = 12;
/// One embedded `RGB16 pal[256]` table = 512 bytes (`TSpr.h:74`,
/// `TSpr.cpp:81`).
pub const TSPR_PALETTE_LEN: usize = 512;

/// `sizeof(TSprHeader)` on disk — the fixed header region the legacy reads
/// in one `ReadFile` call (`TSpr.cpp:54`).
///
/// `7 × 4` fixed `SI32` + `reserved[8] × 4` + `iImageNum × 4` +
/// `ImageInfo[256] × 12` = **3136**.
pub const TSPR_HEADER_LEN: usize = 7 * 4 + 8 * 4 + 4 + TSPR_IMAGE_NUM_MAX * TSPR_IMAGE_INFO_LEN;

/// One `TSprImageInfo` entry (`TSpr.h:21-26`), fields at their literal
/// offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TSprImageInfo {
    /// Collision info (`TSpr.h:23`) — decode-time data only.
    pub i_coll_info: i32,
    /// Stream start position (`TSpr.h:24`) — decode-time data only.
    pub i_start_pos: i32,
    /// Declared per-image size (`TSpr.h:25`).
    ///
    /// **R1: never a decode bound.** The legacy blitter does not read this
    /// field; real `Mask.Spr` says 224 for a 1760-byte stream. Exposed as
    /// data, never used to truncate or reject.
    pub i_size: i32,
}

/// `TSprHeader` (`TSpr.h:32-47`) as it sits on disk, fields at their
/// literal offsets.
///
/// The header occupies a fixed [`TSPR_HEADER_LEN`] bytes regardless of
/// `i_image_num`, because `ImageInfo[256]` is a fixed array member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TSprHeader {
    /// `TSPR_VERSION_NOPAL = 10` | `TSPR_VERSION_16PAL = 400`
    /// (`TSpr.h:35`).
    pub i_version: i32,
    /// `TSPR_ENCODETYPE_SPR = 0` | `TSPR_ENCODETYPE_MPR = 1`
    /// (`TSpr.h:36`).
    pub i_encode_type: i32,
    /// Canvas width (`TSpr.h:37`).
    pub i_x_size: i32,
    /// Canvas height (`TSpr.h:38`).
    pub i_y_size: i32,
    /// Horizontal resolution field (`TSpr.h:39`) — data only.
    pub i_h_res: i32,
    /// Vertical resolution field (`TSpr.h:40`) — data only.
    pub i_v_res: i32,
    /// Length of the image byte-stream that follows the header
    /// (`TSpr.h:41`). This **is** a read bound: `TSpr.cpp:63-70` allocates
    /// and reads exactly this many bytes and refuses a short read.
    pub i_image_data_size: i32,
    /// `reserved[8]` — 32 ignored bytes (`TSpr.h:43` region), kept raw.
    pub reserved: [u8; 32],
    /// Count of meaningful `ImageInfo` entries (`TSpr.h:45`).
    pub i_image_num: i32,
    /// The first `i_image_num` entries of the fixed `ImageInfo[256]` array
    /// (`TSpr.h:46`), in file order.
    pub image_info: Vec<TSprImageInfo>,
}

/// One fully decoded frame: palette **indices** plus transparent spans —
/// no RGB, which is the palette's job (`WO-7.1.md` §6.1).
///
/// `pixels` is row-major, `width × height`, `None` = transparent (the
/// `COLOR_TRANS` runs). Every row is filled to exactly `width` cells, so
/// `pixels.len() == width * height` always holds — a run that would run past
/// the row end is clamped to the row, exactly as the legacy
/// `while (j < width)` loop ends the row (`Graphics_PutSpr16T.cpp:32`),
/// while the stream pointer still advances past the whole run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedSprite {
    /// Canvas width in pixels.
    pub width: usize,
    /// Canvas height in pixels.
    pub height: usize,
    /// Row-major indices; `None` = transparent.
    pub pixels: Vec<Option<u8>>,
    /// Stream bytes actually consumed by the RLE walk.
    pub bytes_consumed: usize,
    /// Pixels covered by literal/index bytes.
    pub literal_pixels: usize,
    /// Pixels covered by transparent runs.
    pub transparent_pixels: usize,
    /// Number of transparent runs (a `254, count` pair with `count > 0`).
    pub transparent_runs: usize,
}

impl DecodedSprite {
    /// Total pixels covered — literals plus transparent run pixels.
    #[inline]
    pub fn covered_pixels(&self) -> usize {
        self.literal_pixels + self.transparent_pixels
    }
}

/// A sprite file decoded end to end: header, frame, optional embedded
/// palette, and the file position the legacy loader would stop at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedSprite {
    /// The parsed fixed header.
    pub header: TSprHeader,
    /// The decoded frame (indices + transparent spans).
    pub image: DecodedSprite,
    /// The embedded `RGB16[256]` table when `iVersion == 400`, selected by
    /// `mode` (`TSpr.cpp:74-87`); `None` for every other version.
    pub embedded_palette: Option<[u16; TSPR_IMAGE_NUM_MAX]>,
    /// File bytes the loader consumes: header + declared image size + the
    /// mode-dependent palette span. Equals the whole file when nothing
    /// trails it.
    pub bytes_consumed: usize,
}

/// Why a sprite was refused (P3-2).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SprError {
    /// Fewer than [`TSPR_HEADER_LEN`] bytes — legacy refuses a short header
    /// read (`TSpr.cpp:56-59`).
    #[error("SPR header truncated: sizeof(TSprHeader) is {expected} bytes, got {got}")]
    HeaderTruncated {
        /// Fixed header size ([`TSPR_HEADER_LEN`]).
        expected: usize,
        /// Bytes actually available.
        got: usize,
    },

    /// `iImageNum` outside `0..=256` — it indexes the fixed
    /// `ImageInfo[256]` array (`TSpr.h:46`).
    #[error("SPR iImageNum {i_image_num} outside 0..=256 (indexes ImageInfo[256], TSpr.h:46)")]
    BadImageNum {
        /// The offending declared count.
        i_image_num: i32,
    },

    /// Negative `iImageDataSize` — cannot address a read of that length.
    #[error("SPR iImageDataSize {i_image_data_size} is negative")]
    NegativeImageDataSize {
        /// The offending declared size.
        i_image_data_size: i32,
    },

    /// Negative canvas dimension — cannot size a canvas.
    #[error("SPR {field} {value} is negative (canvas cannot be sized)")]
    NegativeDimension {
        /// The offending header field.
        field: &'static str,
        /// The offending value.
        value: i32,
    },

    /// `width × height` is not addressable — an arithmetic guard on the
    /// caller's canvas, not a format rule: the pixel count simply cannot be
    /// represented.
    #[error("SPR canvas {width} x {height} has no addressable pixel count")]
    CanvasTooLarge {
        /// Canvas width.
        width: usize,
        /// Canvas height.
        height: usize,
    },

    /// Neither `TSPR_ENCODETYPE_SPR` nor `TSPR_ENCODETYPE_MPR`
    /// (`TSpr.h:11-12`) — there is no third decoder.
    #[error("SPR iEncodeType {i_encode_type} is neither 0 (SPR) nor 1 (MPR) (TSpr.h:11-12)")]
    UnsupportedEncodeType {
        /// The offending declared encode type.
        i_encode_type: i32,
    },

    /// The file ended before the declared `iImageDataSize` — legacy refuses
    /// exactly here (`TSpr.cpp:67-70`).
    #[error("SPR image stream truncated: header declares {expected} bytes, {got} available")]
    ImageStreamTruncated {
        /// Declared `iImageDataSize`.
        expected: usize,
        /// Bytes actually available after the header.
        got: usize,
    },

    /// The RLE ran out before covering the canvas. Legacy would read past
    /// its buffer (undefined behaviour); the port refuses instead of
    /// emitting a partial frame.
    #[error(
        "SPR RLE exhausted at row {row}, column {col} (offset {offset} of a {canvas}-pixel canvas)"
    )]
    StreamExhausted {
        /// Row where the stream ran out.
        row: usize,
        /// Column where the stream ran out.
        col: usize,
        /// Byte offset into the image stream.
        offset: usize,
        /// Pixels the canvas still needed.
        canvas: usize,
    },

    /// A v400 file that ends before its embedded palette table — legacy
    /// refuses (`TSpr.cpp:82-85`).
    #[error("SPR v400 embedded palette truncated in {mode}: need {needed} bytes after the image, got {got}")]
    PaletteTruncated {
        /// Which packing the caller asked for.
        mode: RgbMode,
        /// Palette bytes this mode needs (512 for 565, 1024 for 555).
        needed: usize,
        /// Palette bytes actually present.
        got: usize,
    },
}

/// Parses the fixed header region (`TSpr.h:32-47`).
///
/// Length gate first: at least [`TSPR_HEADER_LEN`] bytes must be present,
/// mirroring the legacy `sizeof(header) != dwRead` refusal
/// (`TSpr.cpp:56-59`), then every field is read at its literal offset.
///
/// # Errors
///
/// [`SprError::HeaderTruncated`] when short, [`SprError::BadImageNum`]
/// when `i_image_num` cannot index the fixed `ImageInfo[256]` array.
pub fn parse_header(bytes: &[u8]) -> Result<TSprHeader, SprError> {
    if bytes.len() < TSPR_HEADER_LEN {
        return Err(SprError::HeaderTruncated {
            expected: TSPR_HEADER_LEN,
            got: bytes.len(),
        });
    }

    let i_image_num = i32_at(bytes, 60);
    if !(0..=TSPR_IMAGE_NUM_MAX as i32).contains(&i_image_num) {
        return Err(SprError::BadImageNum { i_image_num });
    }

    // ImageInfo[256] begins at 64 (TSpr.h:46); only the first iImageNum
    // entries are meaningful.
    let mut image_info = Vec::with_capacity(i_image_num as usize);
    for n in 0..i_image_num as usize {
        let at = 64 + n * TSPR_IMAGE_INFO_LEN;
        image_info.push(TSprImageInfo {
            i_coll_info: i32_at(bytes, at),
            i_start_pos: i32_at(bytes, at + 4),
            i_size: i32_at(bytes, at + 8),
        });
    }

    let mut reserved = [0u8; 32];
    reserved.copy_from_slice(&bytes[28..60]);

    Ok(TSprHeader {
        i_version: i32_at(bytes, 0),
        i_encode_type: i32_at(bytes, 4),
        i_x_size: i32_at(bytes, 8),
        i_y_size: i32_at(bytes, 12),
        i_h_res: i32_at(bytes, 16),
        i_v_res: i32_at(bytes, 20),
        i_image_data_size: i32_at(bytes, 24),
        reserved,
        i_image_num,
        image_info,
    })
}

/// Decodes the SPR (opaque-sprite) RLE stream.
///
/// Transcribed from `Graphics_PutSpr16T.cpp:28-48`: walk each row left to
/// right until `width`; byte `254` (`COLOR_TRANS`) is followed by a count
/// byte and covers that many transparent pixels **without** any index
/// bytes; any other byte is a literal palette index covering one pixel.
///
/// The stream is taken as "the bytes after the header"; exactly
/// `header.i_image_data_size` of them are the declared stream
/// (`TSpr.cpp:63-70`).
///
/// # Errors
///
/// [`SprError::ImageStreamTruncated`] when the file holds fewer than the
/// declared `iImageData_size` bytes, [`SprError::NegativeImageDataSize`] /
/// [`SprError::NegativeDimension`] on an unsizable header, and
/// [`SprError::StreamExhausted`] when the RLE ends before the canvas is
/// covered.
pub fn decode_spr(header: &TSprHeader, stream: &[u8]) -> Result<DecodedSprite, SprError> {
    decode_rle(header, stream, Rle::Spr)
}

/// Decodes the MPR (tile-sprite) RLE stream.
///
/// Transcribed from `Graphics_PutMpr16.cpp:27-53`: byte `254`
/// (`COLOR_TRANS`) + count = a transparent run; otherwise the byte **is** a
/// run length and the next `count` bytes are that many palette indices — a
/// solid run of one color.
///
/// SPR and MPR are deliberately not interchangeable: SPR emits one index
/// per non-transparent pixel, MPR emits a length plus repeated indices.
///
/// # Errors
///
/// As [`decode_spr`].
pub fn decode_mpr(header: &TSprHeader, stream: &[u8]) -> Result<DecodedSprite, SprError> {
    decode_rle(header, stream, Rle::Mpr)
}

/// Which of the two non-interchangeable decoders to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rle {
    /// `Graphics_PutSpr16T.cpp:28-48`.
    Spr,
    /// `Graphics_PutMpr16.cpp:27-53`.
    Mpr,
}

/// Shared row walker: length-gate the declared stream, then run the chosen
/// decoder over it.
fn decode_rle(header: &TSprHeader, stream: &[u8], rle: Rle) -> Result<DecodedSprite, SprError> {
    let declared = header.i_image_data_size;
    if declared < 0 {
        return Err(SprError::NegativeImageDataSize {
            i_image_data_size: declared,
        });
    }
    let declared = declared as usize;
    if stream.len() < declared {
        return Err(SprError::ImageStreamTruncated {
            expected: declared,
            got: stream.len(),
        });
    }
    // TSpr.cpp:66 reads exactly iImageDataSize bytes; anything past it is
    // not image stream (v400 palette, trailing data).
    let stream = &stream[..declared];

    if header.i_x_size < 0 {
        return Err(SprError::NegativeDimension {
            field: "iXSize",
            value: header.i_x_size,
        });
    }
    if header.i_y_size < 0 {
        return Err(SprError::NegativeDimension {
            field: "iYSize",
            value: header.i_y_size,
        });
    }
    let width = header.i_x_size as usize;
    let height = header.i_y_size as usize;
    let canvas = width
        .checked_mul(height)
        .ok_or(SprError::CanvasTooLarge { width, height })?;

    let mut pixels: Vec<Option<u8>> = Vec::new();
    let mut pos = 0usize;
    let mut literal_pixels = 0usize;
    let mut transparent_pixels = 0usize;
    let mut transparent_runs = 0usize;

    for row in 0..height {
        let mut col = 0usize;
        while col < width {
            let byte = *stream.get(pos).ok_or(SprError::StreamExhausted {
                row,
                col,
                offset: pos,
                canvas,
            })?;
            pos += 1;

            if byte == COLOR_TRANS {
                let count = *stream.get(pos).ok_or(SprError::StreamExhausted {
                    row,
                    col,
                    offset: pos,
                    canvas,
                })? as usize;
                pos += 1;

                // Legacy does `j += count` and lets `while (j < width)` end
                // the row (Graphics_PutSpr16T.cpp:35-37): reproduce that
                // control flow, but only materialise the pixels that fall
                // inside the row.
                let run = count.min(width - col);
                pixels.resize(pixels.len() + run, None);
                if count > 0 {
                    transparent_runs += 1;
                }
                transparent_pixels += run;
                col += count;
                continue;
            }

            match rle {
                Rle::Spr => {
                    // One literal index covers exactly one pixel
                    // (Graphics_PutSpr16T.cpp:41-44).
                    pixels.push(Some(byte));
                    literal_pixels += 1;
                    col += 1;
                }
                Rle::Mpr => {
                    // The byte is a run length; the next `count` bytes are
                    // that many indices (Graphics_PutMpr16.cpp:41-48).
                    let count = byte as usize;
                    for _ in 0..count {
                        let index = *stream.get(pos).ok_or(SprError::StreamExhausted {
                            row,
                            col,
                            offset: pos,
                            canvas,
                        })?;
                        pos += 1;
                        // Legacy writes `pdest[k]` for the whole run, which
                        // overruns the row when the run overshoots; the port
                        // consumes the byte identically but never writes out
                        // of the row.
                        if col < width {
                            pixels.push(Some(index));
                            literal_pixels += 1;
                            col += 1;
                        }
                    }
                }
            }
        }
    }

    debug_assert_eq!(
        pixels.len(),
        canvas,
        "every row fills exactly `width` cells"
    );

    Ok(DecodedSprite {
        width,
        height,
        pixels,
        bytes_consumed: pos,
        literal_pixels,
        transparent_pixels,
        transparent_runs,
    })
}

/// Loads a whole sprite file: header → image → (v400 only) embedded
/// palette, exactly as `TSpr.cpp:54-87`.
///
/// `mode` decides which embedded table a v400 file yields: 565 reads the
/// first 512 bytes after the image, 555 skips that table and reads the next
/// 512 (`TSpr.cpp:76-81`), so a 555 load spans 1024 palette bytes.
///
/// # Errors
///
/// Any [`SprError`] raised while parsing or decoding, including
/// [`SprError::PaletteTruncated`] when a v400 file ends before its table.
pub fn load_sprite(bytes: &[u8], mode: RgbMode) -> Result<LoadedSprite, SprError> {
    let header = parse_header(bytes)?;

    // TSpr.cpp:66 reads the stream straight after the fixed header.
    let stream = &bytes[TSPR_HEADER_LEN..];
    let image = match header.i_encode_type {
        TSPR_ENCODETYPE_SPR => decode_spr(&header, stream)?,
        TSPR_ENCODETYPE_MPR => decode_mpr(&header, stream)?,
        other => {
            return Err(SprError::UnsupportedEncodeType {
                i_encode_type: other,
            })
        }
    };

    // TSpr.cpp:74-87: only `iVersion == TSPR_VERSION_16PAL`, and only after
    // the image data has been read.
    let mut bytes_consumed = TSPR_HEADER_LEN + header.i_image_data_size as usize;
    let mut embedded_palette = None;
    if header.i_version == TSPR_VERSION_16PAL {
        let (palette_span, table_at) = match mode {
            RgbMode::Rgb565 => (TSPR_PALETTE_LEN, bytes_consumed),
            // 555 skips the first table (TSpr.cpp:78) then reads the second.
            RgbMode::Rgb555 => (2 * TSPR_PALETTE_LEN, bytes_consumed + TSPR_PALETTE_LEN),
        };
        let present = bytes.len().saturating_sub(bytes_consumed);
        if present < palette_span {
            return Err(SprError::PaletteTruncated {
                mode,
                needed: palette_span,
                got: present,
            });
        }
        embedded_palette = Some(core::array::from_fn(|i| {
            u16_at(bytes, table_at + i * core::mem::size_of::<u16>())
        }));
        bytes_consumed += palette_span;
    }

    Ok(LoadedSprite {
        header,
        image,
        embedded_palette,
        bytes_consumed,
    })
}

#[cfg(test)]
mod tests;
