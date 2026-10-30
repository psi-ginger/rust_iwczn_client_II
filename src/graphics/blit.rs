//! Step 7.2: the color-keyed sprite blit kernels (`WO-7.2.md` §6.2).
//!
//! Legacy shape of record:
//!
//! - `cltGraphic_Capsule::PutSpr(TSpr*...)` (`Graphic_Capsule.cpp:99-133`)
//!   and `cltGraphic_Capsule::PutMpr(TSpr*...)` (`:548-563`) both pick the
//!   color table first: `TSPR_VERSION_16PAL` (400) sprites bring an
//!   embedded `RGB16[256]` table (`SetCurrentColorTable(pSpr->pal)`,
//!   `:550-554`) and every other version uses the global current table;
//!   after the blit the capsule restores the global table (`:560-562`).
//!   [`active_color_table`] is that decision, minus the global mutable.
//! - The capsule then dispatches to the canvas kernels - SPR via
//!   `PutSpr16CT` (`Graphics_PutSpr16T.cpp:280-394`), MPR via
//!   `PutMpr16CT`/`PutMpr16T` (`Graphic_Capsule.cpp:901-948`). Both walk a
//!   **color-keyed** stream: `COLOR_TRANS` (254) runs leave the
//!   destination untouched, other bytes resolve through the 256-entry
//!   table (`Graphics_PutSpr16T.cpp:333-337`, `Graphics.h:44`).
//!
//! R3 equivalence, why one kernel serves both: Step 7.1's decoders already
//! normalize the two non-interchangeable RLE walks (SPR literal-per-pixel,
//! MPR length-prefixed runs - `spr.rs` module docs) into one shape,
//! `DecodedSprite { width, height, pixels: Vec<Option<u8>> }`, row-major,
//! every row filled to exactly `width` cells, `None` = transparent. The
//! only difference between `PutSpr` and `PutMpr` on the *encoded* bytes is
//! therefore already behind us; on the decoded canvas both are the same
//! write: for each `Some(index)` cell inside the clip, store
//! `table[index]`. [`put_spr`] and [`put_mpr`] are that one kernel under
//! the two legacy entry-point names.
//!
//! Clip semantics (P3-5 stated, not "fixed"): `PutSpr16CT` rejects a
//! sprite whose rect misses the clip (`:297-298`), then trims rows
//! (`trim_y1`/`trim_y2`, `:300-302`) and columns (`trim_x`, `:323-325`,
//! `:361-362`) so exactly the sprite-rect *intersection* with the clip is
//! written. This module implements that intersection directly with
//! per-pixel gates - observationally identical for every write the legacy
//! form performs inside the buffer. One legacy branch is deliberately not
//! reproduced: when a sprite straddles **both** the left and the right
//! clip edges at once, the left branch (`:323-357`) forgets the right
//! trim and would scribble past `clip_x2`. That is unobservable in every
//! 7.2 configuration (the clip always equals the whole backbuffer,
//! `Client.cpp:5718`) and is anyway a write P3-2 forbids; the per-pixel
//! form keeps the observable pixels identical and the buffer safe.
//!
//! The kernels never see encoded sprite bytes - that is Step 7.1's
//! contract, already byte-proven. `m_bSkipDraw` (`Graphics_PutMpr16.cpp:7`)
//! is not a kernel concern; the draw-skip flag lives in the tick loop
//! (`WO-7.2.md` R7).
//!
//! No `unsafe` anywhere in this module (WO §7).

use super::framebuffer::{BlitError, ClipArea, Framebuffer};
use super::palette::TPal;
use super::spr::{DecodedSprite, LoadedSprite};

/// SPR entry point - port of the `PutSpr` capsule dispatch
/// (`Graphic_Capsule.cpp:99-133`) resolved onto the decoded canvas.
///
/// `table` is the 256-entry color table in effect, exactly as
/// `m_pCurrentColorTablePtr` was (`Graphic_Capsule.cpp:856`); resolve it
/// with [`active_color_table`].
///
/// # Errors
///
/// [`BlitError`] when the canvas dimensions do not fit the pixel buffer or
/// the origin/extent overflows `i32` (P3-2 typed refusal).
pub fn put_spr(
    fb: &mut Framebuffer,
    clip: &ClipArea,
    sprite: &DecodedSprite,
    x: i32,
    y: i32,
    table: &[u16; 256],
) -> Result<(), BlitError> {
    blit_canvas(fb, clip, sprite, x, y, table)
}

/// MPR (tile) entry point - port of the `PutMpr` capsule dispatch
/// (`Graphic_Capsule.cpp:548-563`, `:901-948`) resolved onto the decoded
/// canvas. See the module docs for why this is the same kernel as
/// [`put_spr`] (R3).
///
/// # Errors
///
/// As [`put_spr`].
pub fn put_mpr(
    fb: &mut Framebuffer,
    clip: &ClipArea,
    sprite: &DecodedSprite,
    x: i32,
    y: i32,
    table: &[u16; 256],
) -> Result<(), BlitError> {
    blit_canvas(fb, clip, sprite, x, y, table)
}

/// The color table a sprite blits with - the `SetCurrentColorTable`
/// decision at `Graphic_Capsule.cpp:550-554`: a `TSPR_VERSION_16PAL`
/// (400) sprite carries its own `RGB16[256]` table, every other version
/// uses the global current palette.
pub fn active_color_table<'a>(sprite: &'a LoadedSprite, global: &'a TPal) -> &'a [u16; 256] {
    match &sprite.embedded_palette {
        Some(embedded) => embedded,
        None => &global.pal,
    }
}

/// The shared color-keyed canvas write (`WO-7.2.md` §6.2).
fn blit_canvas(
    fb: &mut Framebuffer,
    clip: &ClipArea,
    sprite: &DecodedSprite,
    x: i32,
    y: i32,
    table: &[u16; 256],
) -> Result<(), BlitError> {
    let width = sprite.width;
    let height = sprite.height;
    let expected = width
        .checked_mul(height)
        .ok_or(BlitError::CoordinateOverflow {
            x,
            y,
            width,
            height,
        })?;
    if sprite.pixels.len() < expected {
        return Err(BlitError::CanvasMismatch {
            width,
            height,
            expected,
            got: sprite.pixels.len(),
        });
    }
    let w = i32::try_from(width).map_err(|_| BlitError::CoordinateOverflow {
        x,
        y,
        width,
        height,
    })?;
    let h = i32::try_from(height).map_err(|_| BlitError::CoordinateOverflow {
        x,
        y,
        width,
        height,
    })?;
    // Inclusive far corner, exactly `PutSpr16CT` (`:289-290`), but
    // overflow-checked instead of silently wrapping.
    let x2 = x.checked_add(w - 1).ok_or(BlitError::CoordinateOverflow {
        x,
        y,
        width,
        height,
    })?;
    let y2 = y.checked_add(h - 1).ok_or(BlitError::CoordinateOverflow {
        x,
        y,
        width,
        height,
    })?;
    // Fully clipped out: silent no-op (`:297-298`).
    if x > clip.x2 || x2 < clip.x1 || y > clip.y2 || y2 < clip.y1 {
        return Ok(());
    }
    for row in 0..height {
        let dest_y = y + row as i32;
        // trim_y gates (`:300-302`, `:329`, `:366`).
        if dest_y < clip.y1 || dest_y > clip.y2 {
            continue;
        }
        if dest_y < 0 || dest_y >= fb.height as i32 {
            continue;
        }
        let src = &sprite.pixels[row * width..row * width + width];
        let dest_at = dest_y as usize * fb.width;
        let dest = &mut fb.pixels[dest_at..dest_at + fb.width];
        for (col, index) in src.iter().enumerate() {
            // Color key: `None` never writes - the `COLOR_TRANS` run
            // leaves the destination untouched (`:333-337`).
            let Some(index) = index else {
                continue;
            };
            let dest_x = x + col as i32;
            // trim_x gates (`:323-325`, `:351`, `:361-362`, `:373`, `:388`).
            if dest_x < clip.x1 || dest_x > clip.x2 {
                continue;
            }
            if dest_x < 0 || dest_x >= fb.width as i32 {
                continue;
            }
            dest[dest_x as usize] = table[*index as usize];
        }
    }
    Ok(())
}

/// Test-facing builder: a `LoadedSprite` around an already-decoded canvas,
/// matching the public 7.1 field layout (`spr.rs` `LoadedSprite`,
/// `DecodedSprite`). Only the fields the blit path reads are meaningful;
/// header numbers are data-only placeholders.
#[cfg(test)]
pub(crate) fn synthetic_sprite(
    width: usize,
    height: usize,
    pixels: Vec<Option<u8>>,
) -> LoadedSprite {
    use super::spr::{TSprHeader, TSPR_ENCODETYPE_SPR, TSPR_VERSION_NOPAL};
    LoadedSprite {
        header: TSprHeader {
            i_version: TSPR_VERSION_NOPAL,
            i_encode_type: TSPR_ENCODETYPE_SPR,
            i_x_size: width as i32,
            i_y_size: height as i32,
            i_h_res: 0,
            i_v_res: 0,
            i_image_data_size: 0,
            reserved: [0; 32],
            i_image_num: 1,
            image_info: Vec::new(),
        },
        image: DecodedSprite {
            width,
            height,
            pixels,
            bytes_consumed: 0,
            literal_pixels: 0,
            transparent_pixels: 0,
            transparent_runs: 0,
        },
        embedded_palette: None,
        bytes_consumed: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::palette::{load_pal, RgbMode};
    use crate::graphics::spr::{load_sprite, TSPR_VERSION_16PAL};

    /// In-tree real sprite (7.1 fixture): 4896 B, `iVersion=10`, 80x40,
    /// stream 1760 B (`tests/asset_fixtures.rs` 7.1 facts, DoD 10.2).
    const MASK_SPR: &[u8] = include_bytes!("../../GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr");
    /// In-tree real palette: 768 B (256 x RGB) (`tests/asset_fixtures.rs`).
    const MONARCH_PAL: &[u8] =
        include_bytes!("../../GoonZu_Source/CharactorCombination/Pal/monarch.pal");

    fn table_565() -> [u16; 256] {
        let mut table = [0u16; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            *entry = i as u16;
        }
        table
    }

    /// Probe (a) WO §8: the destination advances by the pixel *pitch*
    /// (`pdest += lPitch`, `Graphics_BasicDraw.cpp:27`,
    /// `Graphics_PutSpr16T.cpp:340`). A mutation writing row+1 instead of
    /// row+pitch must fail this test.
    #[test]
    fn pitch_advance_lands_each_row_on_its_own_scanline() {
        let mut fb = Framebuffer::new(6, 5).expect("6x5 allocates");
        let clip = ClipArea::new(0, 0, 5, 4);
        let sprite = synthetic_sprite(2, 2, vec![Some(1), Some(2), Some(3), Some(4)]);
        put_spr(&mut fb, &clip, &sprite.image, 1, 1, &table_565()).expect("blit ok");
        assert_eq!(fb.get_pixel(1, 1), Some(1), "row 0 col 0");
        assert_eq!(fb.get_pixel(2, 1), Some(2), "row 0 col 1");
        assert_eq!(fb.get_pixel(1, 2), Some(3), "row 1 col 0 - pitch advance");
        assert_eq!(fb.get_pixel(2, 2), Some(4), "row 1 col 1 - pitch advance");
        assert_eq!(fb.get_pixel(1, 3), Some(0), "row 2 untouched");
    }

    /// Probe (b) WO §8: transparent cells never write - `COLOR_TRANS` runs
    /// leave the destination untouched (`Graphics_PutSpr16T.cpp:333-337`).
    /// A mutation writing palette index 254 into `None` cells must fail
    /// this test.
    #[test]
    fn transparent_cells_never_write() {
        let mut fb = Framebuffer::new(3, 1).expect("3x1 allocates");
        let clip = ClipArea::new(0, 0, 2, 0);
        // Prefill so an unwanted write is visible.
        fb.clear(0x7777);
        let sprite = synthetic_sprite(3, 1, vec![Some(5), None, Some(7)]);
        put_spr(&mut fb, &clip, &sprite.image, 0, 0, &table_565()).expect("blit ok");
        assert_eq!(fb.get_pixel(0, 0), Some(5), "opaque left pixel written");
        assert_eq!(
            fb.get_pixel(1, 0),
            Some(0x7777),
            "COLOR_TRANS cell keeps the old destination (probe b)"
        );
        assert_eq!(fb.get_pixel(2, 0), Some(7), "opaque right pixel written");
    }

    #[test]
    fn straddling_the_left_top_clip_edges_writes_only_the_intersection() {
        let mut fb = Framebuffer::new(6, 6).expect("6x6 allocates");
        let clip = ClipArea::new(2, 2, 5, 5);
        let sprite = synthetic_sprite(3, 3, vec![Some(9); 9]);
        put_spr(&mut fb, &clip, &sprite.image, 0, 0, &table_565()).expect("blit ok");
        // Visible intersection is (2..=2, 2..=2): only the sprite's
        // bottom-right pixel maps inside the clip.
        assert_eq!(fb.get_pixel(2, 2), Some(9), "intersection pixel written");
        assert_eq!(fb.get_pixel(1, 1), Some(0), "outside clip untouched");
        assert_eq!(fb.get_pixel(3, 3), Some(0), "sprite does not reach there");
    }

    #[test]
    fn straddling_the_right_bottom_clip_edges_writes_only_the_intersection() {
        let mut fb = Framebuffer::new(6, 6).expect("6x6 allocates");
        let clip = ClipArea::new(0, 0, 3, 3);
        let sprite = synthetic_sprite(3, 3, vec![Some(9); 9]);
        put_spr(&mut fb, &clip, &sprite.image, 2, 2, &table_565()).expect("blit ok");
        // Intersection is (2..=3, 2..=3); the sprite's bottom-right pixel
        // (4,4) is past the clip.
        assert_eq!(fb.get_pixel(2, 2), Some(9), "intersection written");
        assert_eq!(fb.get_pixel(3, 3), Some(9), "clip corner written");
        assert_eq!(fb.get_pixel(4, 4), Some(0), "past the clip untouched");
    }

    #[test]
    fn fully_clipped_out_sprites_are_a_noop() {
        let mut fb = Framebuffer::new(6, 6).expect("6x6 allocates");
        let clip = ClipArea::new(2, 2, 3, 3);
        let sprite = synthetic_sprite(2, 2, vec![Some(1); 4]);
        put_spr(&mut fb, &clip, &sprite.image, 5, 5, &table_565()).expect("blit ok");
        put_spr(&mut fb, &clip, &sprite.image, -3, -3, &table_565()).expect("blit ok");
        assert!(fb.pixels.iter().all(|&p| p == 0), "nothing written");
    }

    /// P3-2: a canvas larger than its buffer is a typed refusal, never a
    /// slice-index panic.
    #[test]
    fn short_pixel_buffer_is_refused_by_name() {
        let mut fb = Framebuffer::new(4, 4).expect("4x4 allocates");
        let clip = ClipArea::new(0, 0, 3, 3);
        let sprite = synthetic_sprite(2, 2, vec![Some(1); 3]); // one cell short
        let err = put_spr(&mut fb, &clip, &sprite.image, 0, 0, &table_565())
            .expect_err("short canvas must be refused");
        assert!(
            matches!(
                err,
                BlitError::CanvasMismatch {
                    expected: 4,
                    got: 3,
                    ..
                }
            ),
            "refusal names the mismatch: {err}"
        );
    }

    /// P3-2: an origin that overflows the far corner is refused, not
    /// wrapped.
    #[test]
    fn overflowing_origin_is_refused() {
        let mut fb = Framebuffer::new(4, 4).expect("4x4 allocates");
        let clip = ClipArea::new(-100, -100, 100, 100);
        let sprite = synthetic_sprite(2, 2, vec![Some(1); 4]);
        let err = put_spr(&mut fb, &clip, &sprite.image, i32::MAX, 0, &table_565())
            .expect_err("overflowing origin must be refused");
        assert!(
            matches!(err, BlitError::CoordinateOverflow { .. }),
            "refusal names the overflow: {err}"
        );
    }

    /// v400 embedded-palette swap (`Graphic_Capsule.cpp:550-554`): a
    /// `TSPR_VERSION_16PAL` sprite blits with its own table, every other
    /// version with the global one.
    #[test]
    fn active_color_table_follows_the_v400_swap() {
        let global_bytes = [0u8; 768];
        let global = load_pal(&global_bytes, "synthetic", RgbMode::Rgb565).expect("pal");
        // NOPAL (v10) sprite -> global table.
        let plain = synthetic_sprite(1, 1, vec![Some(1)]);
        assert_eq!(
            active_color_table(&plain, &global),
            &global.pal,
            "v10 sprite uses the global table (Graphic_Capsule.cpp:552-553)"
        );
        // v400 sprite -> embedded table.
        let mut v400 = synthetic_sprite(1, 1, vec![Some(1)]);
        let mut embedded = [0u16; 256];
        embedded[1] = 0xF800;
        v400.header.i_version = TSPR_VERSION_16PAL;
        v400.embedded_palette = Some(embedded);
        assert_eq!(
            active_color_table(&v400, &global)[1],
            0xF800,
            "v400 sprite swaps in its embedded table (Graphic_Capsule.cpp:550-551)"
        );
    }

    /// WO §8 determinism claim (amended 2026-10-29): the same synthetic
    /// scene rendered into two fresh framebuffers is byte-identical.
    #[test]
    fn the_same_scene_renders_byte_identically_twice() {
        let scene = |fb: &mut Framebuffer| {
            let clip = ClipArea::new(0, 0, fb.width as i32 - 1, fb.height as i32 - 1);
            fb.clear(0x0010);
            let checker = synthetic_sprite(2, 2, vec![Some(1), None, None, Some(2)]);
            for gy in 0..3 {
                for gx in 0..4 {
                    put_spr(
                        fb,
                        &clip,
                        &checker.image,
                        1 + gx * 3,
                        1 + gy * 3,
                        &table_565(),
                    )
                    .expect("scene blit");
                }
            }
        };
        let mut first = Framebuffer::new(16, 12).expect("16x12");
        let mut second = Framebuffer::new(16, 12).expect("16x12");
        scene(&mut first);
        scene(&mut second);
        assert_eq!(
            first.pixels, second.pixels,
            "deterministic: identical bytes both times"
        );
    }

    /// WO §8 real-fixture claim: a frame built from the real 7.1 fixture
    /// `Mask.Spr` through `load_sprite`, asserted against a **hand-walked**
    /// expectation (F-07: the walk below is written from the fixture bytes
    /// + the cited legacy kernel semantics, not from Rust output).
    ///
    /// Hand-walk of the stream (`Mask.Spr`: 4896 B total, header
    /// `TSPR_HEADER_LEN` = 7*4 + 8*4 + 4 + 256*12 = **3136** B
    /// (`spr.rs:93`, `TSpr.h:11-47`), leaving the 1760 B RLE stream;
    /// SPR RLE per `Graphics_PutSpr16T.cpp:28-48` - byte 254 + count =
    /// transparent run, any other byte = one literal palette index).
    /// First bytes of row 0, read from the fixture at offset 3136:
    ///
    /// ```text
    /// FE 27         -> transparent run of 39            (cols  0..=38)
    /// A3 A3         -> 2 literal pixels, index 0xA3=163 (cols 39..=40)
    /// FE 27         -> transparent run of 39            (cols 41..=79)
    /// ```
    ///
    /// 39 + 2 + 39 = 80 = exactly one row, so the diamond-shaped mask
    /// closes every row symmetrically without spilling into the next.
    /// Row 0 therefore holds transparency at cols 0-38, index 163 at
    /// cols 39-40 (the top tip of the mask's inner diamond), transparency
    /// again at cols 41-79. Row 1 opens `FE 25` (37 transparent), so its
    /// col 0 is transparent as well. Palette `monarch.pal` (768 B,
    /// `256 x RGB`) entry 163 = RGB(57,38,19) -> 565 packing per
    /// `palette::rgb_to_565` (`TSpr.cpp:188-191` transcription):
    /// `((57>>1)<<11) & 0xF800 = 0xE000`,
    /// `(38<<5) & 0x07E0 = 0x04C0`, `(19>>1) & 0x001F = 0x0009`,
    /// or'd = `0xE4C9`.
    #[test]
    fn real_mask_spr_blits_hand_walked_row_zero() {
        let sprite = load_sprite(MASK_SPR, RgbMode::Rgb565).expect("Mask.Spr decodes");
        assert_eq!(
            (sprite.header.i_x_size, sprite.header.i_y_size),
            (80, 40),
            "fixture canvas is one legacy tile (map.h:33,35)"
        );
        let pal = load_pal(MONARCH_PAL, "monarch.pal", RgbMode::Rgb565).expect("pal");
        let table = active_color_table(&sprite, &pal);
        assert_eq!(
            table[163], 0xE4C9,
            "hand-packed monarch.pal entry 163 (see fn docs)"
        );

        let mut fb = Framebuffer::new(100, 60).expect("100x60");
        fb.clear(0x5555); // sentinel for the transparent cells
        let clip = ClipArea::new(0, 0, 99, 59);
        put_spr(&mut fb, &clip, &sprite.image, 10, 20, table).expect("blit ok");

        let at = |col: i32| fb.get_pixel(10 + col, 20);
        assert_eq!(
            at(0),
            Some(0x5555),
            "col 0: the row opens transparent (FE 27 run) - destination kept"
        );
        assert_eq!(
            at(38),
            Some(0x5555),
            "col 38: last cell of the leading 39-transparent run"
        );
        assert_eq!(
            at(39),
            Some(0xE4C9),
            "col 39: literal index 163 -> pal[163]"
        );
        assert_eq!(
            at(40),
            Some(0xE4C9),
            "col 40: literal index 163 -> pal[163]"
        );
        assert_eq!(
            at(41),
            Some(0x5555),
            "col 41: the closing transparent run resumes"
        );
        assert_eq!(at(79), Some(0x5555), "col 79: last cell of row 0");
        assert_eq!(
            fb.get_pixel(10, 21),
            Some(0x5555),
            "row 1 col 0: row 1 opens with its own FE 25 run - stays transparent"
        );
        assert_eq!(
            fb.get_pixel(9, 20),
            Some(0x5555),
            "the sprite's left neighbour pixel is untouched"
        );
    }
}
