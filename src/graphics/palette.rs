//! `.pal` palette decoding — the exact-768 file rule and the RGB8 → RGB16
//! (565 / 555) conversion.
//!
//! Format of record: `GoonZu_Source/NLib/TSpr.cpp:138-203`
//! (`TPal::LoadPalFromFile`), mode/mask constants at
//! `GoonZuWorld/Lib/NGraphics/Graphics.h:15-29`.
//!
//! Rules pinned here:
//!
//! - **Exactly 768 bytes** — `256 × 3` RGB — or the file is refused
//!   (`TSpr.cpp:159-164`). One byte short or long is a refusal, not a
//!   partial load.
//! - **565 packing** (`TSpr.cpp:183-187`): `r = 0xF800 & ((r >> 1) << 11)`,
//!   `g = 0x07E0 & (g << 5)`, `b = 0x001F & (b >> 1)`. The green channel is
//!   **not** right-shifted — that asymmetry is in the source and is
//!   reproduced verbatim rather than "corrected" (`WO-7.1.md` §7).
//! - **555 packing** (`TSpr.cpp:193-195`): `r = 0x7C00 & ((r >> 1) << 10)`,
//!   `g = 0x03E0 & ((g >> 1) << 5)`, `b = 0x001F & (b >> 1)`.
//!
//! The three-term OR of the masked channels is the stored `RGB16`
//! (`TSpr.cpp:189`, `:197`).

use thiserror::Error;

/// A palette file is exactly `256 × 3` bytes of RGB (`TSpr.cpp:159-164`).
pub const PALETTE_FILE_LEN: usize = 768;

/// Number of palette entries (`TSpr.cpp:181`).
pub const PALETTE_ENTRIES: usize = 256;

/// Bytes per RGB triple in a `.pal` file.
pub const RGB_BYTES: usize = 3;

/// `RGB16_565_RMASK` (`Graphics.h:23`).
pub const RGB16_565_RMASK: u16 = 0xF800;
/// `RGB16_565_GMASK` (`Graphics.h:24`).
pub const RGB16_565_GMASK: u16 = 0x07E0;
/// `RGB16_565_BMASK` (`Graphics.h:25`).
pub const RGB16_565_BMASK: u16 = 0x001F;
/// `RGB16_555_RMASK` (`Graphics.h:27`).
pub const RGB16_555_RMASK: u16 = 0x7C00;
/// `RGB16_555_GMASK` (`Graphics.h:28`).
pub const RGB16_555_GMASK: u16 = 0x03E0;
/// `RGB16_555_BMASK` (`Graphics.h:29`).
pub const RGB16_555_BMASK: u16 = 0x001F;

/// Which 16-bit packing the client is currently drawing with
/// (`RGB16_MODE_565` / `RGB16_MODE_555`, `Graphics.h`).
///
/// The mode is not cosmetic: a v400 sprite file stores **both** tables and
/// the loader selects one by skipping the other (`TSpr.cpp:76-79`), which is
/// why the mode is threaded through the sprite loader too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RgbMode {
    /// 5-6-5 packing (`RGB16_MODE_565`) — the loader default
    /// (`TSpr.h:59`).
    Rgb565,
    /// 5-5-5 packing (`RGB16_MODE_555`).
    Rgb555,
}

impl RgbMode {
    /// The canonical name, for error messages.
    pub const fn name(self) -> &'static str {
        match self {
            RgbMode::Rgb565 => "RGB565",
            RgbMode::Rgb555 => "RGB555",
        }
    }
}

impl core::fmt::Display for RgbMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// One 16-bit color entry — `RGB16 pal[256]` (`TSpr.h:74`, `:88`).
///
/// This is `TPal`: 256 already-packed entries, exactly as the legacy struct
/// holds them after conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TPal {
    /// The 256 packed entries, index = sprite palette index.
    pub pal: [u16; PALETTE_ENTRIES],
}

/// Why a palette was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PaletteError {
    /// The file is not exactly [`PALETTE_FILE_LEN`] bytes — the single rule
    /// `TPal::LoadPalFromFile` enforces (`TSpr.cpp:159-164`). Carries the
    /// offending file's **name** so the refusal identifies its target.
    #[error(
        "palette file `{name}` refused: expected exactly {expected} bytes (256 x 3 RGB), got {got}"
    )]
    WrongSize {
        /// The file name, so the refusal names its target.
        name: String,
        /// The only accepted size ([`PALETTE_FILE_LEN`]).
        expected: usize,
        /// The size actually presented.
        got: usize,
    },
}

/// Converts three 8-bit channels to `RGB16` in 5-6-5 mode.
///
/// Byte-exact transcription of `TSpr.cpp:185-187`:
///
/// ```text
/// r = RGB16_565_RMASK & ((p[i*3    ] >> 1) << 11);
/// g = RGB16_565_GMASK & ((p[i*3 + 1])       <<  5);   // no >> 1
/// b = RGB16_565_BMASK &  (p[i*3 + 2] >> 1);
/// ```
///
/// The intermediate math runs in `u32` because the C source relies on `UI08`
/// promoting to `int` before the `<< 11` shift.
#[inline]
pub const fn rgb_to_565(r: u8, g: u8, b: u8) -> u16 {
    let r16 = (RGB16_565_RMASK as u32) & (((r as u32) >> 1) << 11);
    let g16 = (RGB16_565_GMASK as u32) & ((g as u32) << 5);
    let b16 = (RGB16_565_BMASK as u32) & ((b as u32) >> 1);
    (r16 | g16 | b16) as u16
}

/// Converts three 8-bit channels to `RGB16` in 5-5-5 mode.
///
/// Byte-exact transcription of `TSpr.cpp:193-195` — note that **both** the
/// red and the green channels are right-shifted here, unlike 565:
///
/// ```text
/// r = RGB16_555_RMASK & ((p[i*3    ] >> 1) << 10);
/// g = RGB16_555_GMASK & ((p[i*3 + 1] >> 1) <<  5);
/// b = RGB16_555_BMASK &  (p[i*3 + 2] >> 1);
/// ```
#[inline]
pub const fn rgb_to_555(r: u8, g: u8, b: u8) -> u16 {
    let r16 = (RGB16_555_RMASK as u32) & (((r as u32) >> 1) << 10);
    let g16 = (RGB16_555_GMASK as u32) & (((g as u32) >> 1) << 5);
    let b16 = (RGB16_555_BMASK as u32) & ((b as u32) >> 1);
    (r16 | g16 | b16) as u16
}

/// Loads a palette from an in-memory `.pal` file image.
///
/// `name` identifies the file in refusals (the legacy call site knows the
/// path it passed to `CreateFile`); `mode` selects 565 or 555 packing.
///
/// The size gate runs **before** any field read: the legacy reads the whole
/// 768-byte block only after `dwFileSize == 768` (`TSpr.cpp:159-172`), so a
/// 767- or 769-byte file never reaches the conversion loop.
///
/// # Errors
///
/// [`PaletteError::WrongSize`] naming `name`, whenever
/// `bytes.len() != `[`PALETTE_FILE_LEN`].
pub fn load_pal(bytes: &[u8], name: &str, mode: RgbMode) -> Result<TPal, PaletteError> {
    if bytes.len() != PALETTE_FILE_LEN {
        return Err(PaletteError::WrongSize {
            name: name.to_string(),
            expected: PALETTE_FILE_LEN,
            got: bytes.len(),
        });
    }

    let mut pal = [0u16; PALETTE_ENTRIES];
    for (entry, rgb) in pal.iter_mut().zip(bytes.as_chunks::<RGB_BYTES>().0) {
        let [r, g, b] = *rgb;
        *entry = match mode {
            RgbMode::Rgb565 => rgb_to_565(r, g, b),
            RgbMode::Rgb555 => rgb_to_555(r, g, b),
        };
    }
    Ok(TPal { pal })
}

#[cfg(test)]
mod tests;
