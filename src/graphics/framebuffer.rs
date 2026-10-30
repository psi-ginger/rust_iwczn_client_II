//! Step 7.2: the RGB565 backbuffer and its clip area (`WO-7.2.md` §6.1).
//!
//! The legacy client renders every 2D primitive into a locked 16-bit
//! DirectDraw surface. The format of record for that surface:
//!
//! - **Pixel format** — packed RGB565 (`RGB16`); the sprite/palette pipeline
//!   already emits 565 (`palette::rgb_to_565`).
//! - **Row stride** — the legacy kernels convert the byte pitch to a
//!   *pixel* pitch with `lPitch = m_lPitch >> 1` (`Graphics_PutSpr16T.cpp:15`,
//!   `Graphics_PutMpr16.cpp:14`, `Graphics_BasicDraw.cpp:16`) and advance the
//!   destination pointer by exactly that many `RGB16` cells per row
//!   (`Graphics_BasicDraw.cpp:27`, `Graphics_PutSpr16T.cpp:340`). Our buffer
//!   is a flat `Vec<u16>` of `width * height` cells, so the pixel pitch *is*
//!   `width` - that equivalence is the R3 foundation of this port and is
//!   asserted by the `pitch_is_the_pixel_width_row_major` unit test with its
//!   citation.
//! - **Clip area** — `m_rcClip`, inclusive on all four edges, manipulated by
//!   `SetClipArea`/`GetClipArea`/`IsInClipArea` (`Graphic.h:157-165`). The
//!   client resets it to the full backbuffer every frame
//!   (`Client.cpp:5718`).
//!
//! Standing rules that apply to everything in this module (P3-2): no input
//! is ever indexed without a gate; degenerate geometry is a typed
//! [`BlitError`], never a panic and never a partial write.
//!
//! Legacy quirks kept verbatim (P3-5), each stated rather than "fixed":
//!
//! - `FillBox`/`FillBox16` treat the rectangle as **inclusive** on every
//!   edge (`width = x2 - x1 + 1`, loops `i <= y2` - `Graphics_BasicDraw.cpp:20-28`).
//! - An **inverted** rectangle (`x1 > x2` or `y1 > y2`) is a silent no-op:
//!   the legacy width goes negative and both loops fall through
//!   (`Graphics_BasicDraw.cpp:20-28`). We reproduce the no-op.

use thiserror::Error;

/// Inclusive clip rectangle, mirroring `m_rcClip`
/// (`Graphic.h:157-165`).
///
/// All four bounds are inclusive: a single pixel `(x, y)` is inside when
/// `x1 <= x <= x2` and `y1 <= y <= y2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipArea {
    /// Left edge, inclusive.
    pub x1: i32,
    /// Top edge, inclusive.
    pub y1: i32,
    /// Right edge, inclusive.
    pub x2: i32,
    /// Bottom edge, inclusive.
    pub y2: i32,
}

impl ClipArea {
    /// Builds a clip rectangle (no validation; the legacy setters do none).
    pub const fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Inclusive containment of one pixel - the `IsInClipArea` predicate
    /// reduced to a point.
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && x <= self.x2 && y >= self.y1 && y <= self.y2
    }

    /// Inclusive containment of the whole rectangle
    /// `(x1, y1, x2, y2)`: the `IsInClipArea(x1, y1, x2, y2)` form the
    /// blitters use to pick the unclipped fast path
    /// (`Graphic_Capsule.cpp:112`, `Graphics_BasicDraw.cpp:37`).
    pub const fn contains_rect(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        x1 >= self.x1 && y1 >= self.y1 && x2 <= self.x2 && y2 <= self.y2
    }
}

/// Why a framebuffer or blit was refused (P3-2 typed refusal - never a
/// slice-index panic).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BlitError {
    /// `Framebuffer::new` got a zero dimension - a surface with no pixels
    /// is not a surface (`WO-7.2.md` §6.1).
    #[error("framebuffer degenerate: {width}x{height}")]
    DegenerateFramebuffer {
        /// Requested width in pixels.
        width: usize,
        /// Requested height in pixels.
        height: usize,
    },
    /// `width * height` overflowed - refuse rather than allocate nonsense
    /// (`WO-7.2.md` §6.1).
    #[error("framebuffer too large: {width}x{height} overflows")]
    FramebufferTooLarge {
        /// Requested width in pixels.
        width: usize,
        /// Requested height in pixels.
        height: usize,
    },
    /// A blit canvas whose declared `width * height` does not fit its
    /// pixel buffer.
    #[error("sprite canvas {width}x{height} needs {expected} cells, buffer has {got}")]
    CanvasMismatch {
        /// Canvas width in pixels.
        width: usize,
        /// Canvas height in pixels.
        height: usize,
        /// Cells required by the canvas.
        expected: usize,
        /// Cells actually present.
        got: usize,
    },
    /// A blit origin/extent that overflows `i32` - refuse instead of
    /// wrapping (the legacy C would silently wrap).
    #[error("blit coordinate overflow at ({x}, {y}) for {width}x{height}")]
    CoordinateOverflow {
        /// Blit origin x.
        x: i32,
        /// Blit origin y.
        y: i32,
        /// Sprite width.
        width: usize,
        /// Sprite height.
        height: usize,
    },
}

/// The RGB565 backbuffer: `width * height` packed pixels, row-major, one
/// `u16` per pixel.
///
/// This is the Rust form of the locked legacy surface (module docs). The
/// blit kernels in [`super::blit`] write here; the present path copies the
/// cropped region to the window (`WO-7.2.md` §6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    /// Width in pixels - also the pixel row stride (`lPitch >> 1`).
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Row-major `RGB16` cells; index `y * width + x`.
    pub pixels: Vec<u16>,
}

impl Framebuffer {
    /// Allocates a `width x height` backbuffer filled with `0`.
    ///
    /// # Errors
    ///
    /// [`BlitError::DegenerateFramebuffer`] for a zero dimension,
    /// [`BlitError::FramebufferTooLarge`] when `width * height` overflows.
    pub fn new(width: usize, height: usize) -> Result<Self, BlitError> {
        if width == 0 || height == 0 {
            return Err(BlitError::DegenerateFramebuffer { width, height });
        }
        let cells = width
            .checked_mul(height)
            .ok_or(BlitError::FramebufferTooLarge { width, height })?;
        Ok(Self {
            width,
            height,
            pixels: vec![0; cells],
        })
    }

    /// Fills the whole backbuffer with one color (the unclipped
    /// `FillBox16` over the full rect).
    pub fn clear(&mut self, color: u16) {
        self.pixels.fill(color);
    }

    /// Clipped solid rectangle - port of `FillBox16C` dispatch plus the
    /// `FillBox16` write loop (`Graphics_BasicDraw.cpp:32-63`, `:5-30`).
    ///
    /// Semantics, verbatim:
    ///
    /// 1. Fully inside the clip -> write the raw inclusive rect
    ///    (`:37-40`).
    /// 2. Fully outside the clip -> no-op (`:49-50`).
    /// 3. Otherwise clamp the rect to the clip (`:52-55`) and write the
    ///    clamped inclusive rect.
    /// 4. The write loop is inclusive on every edge
    ///    (`width = x2 - x1 + 1`, `for i in y1..=y2` - `:20-28`).
    ///
    /// An inverted rect is a no-op (module docs, P3-5). A final defensive
    /// clamp keeps every write inside the buffer: the legacy clip always
    /// sits inside the surface (`Client.cpp:5718`), but P3-2 forbids any
    /// out-of-buffer write regardless of caller input.
    pub fn fill_box(&mut self, clip: &ClipArea, x1: i32, y1: i32, x2: i32, y2: i32, color: u16) {
        let (x1, y1, x2, y2) = if clip.contains_rect(x1, y1, x2, y2) {
            (x1, y1, x2, y2)
        } else {
            // Rejection test (:49-50).
            if x1 > clip.x2 || x2 < clip.x1 || y1 > clip.y2 || y2 < clip.y1 {
                return;
            }
            // Clamp to the clip area (:52-55).
            (
                x1.max(clip.x1),
                y1.max(clip.y1),
                x2.min(clip.x2),
                y2.min(clip.y2),
            )
        };
        // Defensive buffer clamp (see fn docs).
        let x1 = x1.max(0);
        let y1 = y1.max(0);
        let x2 = x2.min(self.width as i32 - 1);
        let y2 = y2.min(self.height as i32 - 1);
        // Inverted or empty → no-op, matching the legacy negative width
        // falling through both loops (:20-28).
        if x1 > x2 || y1 > y2 {
            return;
        }
        let len = (x2 - x1 + 1) as usize;
        for y in y1..=y2 {
            let at = y as usize * self.width + x1 as usize;
            self.pixels[at..at + len].fill(color);
        }
    }

    /// Writes one pixel if it is inside the buffer; `false` when clipped
    /// out (test/draw-facing helper).
    pub fn set_pixel(&mut self, x: i32, y: i32, color: u16) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        self.pixels[y as usize * self.width + x as usize] = color;
        true
    }

    /// Reads one pixel; `None` outside the buffer (test-facing helper).
    pub fn get_pixel(&self, x: i32, y: i32) -> Option<u16> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(self.pixels[y as usize * self.width + x as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WO §6.1 / R3: index `y * width + x` *is* the legacy addressing
    /// `pdest = m_pImage + y * lPitch + x` with `lPitch = m_lPitch >> 1`
    /// (pixel pitch = width; `Graphics_PutSpr16T.cpp:15,313-314`).
    #[test]
    fn pitch_is_the_pixel_width_row_major() {
        let mut fb = Framebuffer::new(4, 3).expect("4x3 allocates");
        // Write via the public helper, read via the raw buffer: same cell.
        let (x, y) = (2usize, 1usize);
        assert!(
            fb.set_pixel(x as i32, y as i32, 0xBEEF),
            "in-bounds write succeeds"
        );
        assert_eq!(
            fb.pixels[y * 4 + x],
            0xBEEF,
            "row-major index y*width+x is the addressed cell \
             (Graphics_PutSpr16T.cpp:15,313)"
        );
    }

    #[test]
    fn new_rejects_degenerate_and_oversized() {
        assert_eq!(
            Framebuffer::new(0, 10),
            Err(BlitError::DegenerateFramebuffer {
                width: 0,
                height: 10
            }),
            "zero dimension is refused (P3-2)"
        );
        assert_eq!(
            Framebuffer::new(usize::MAX, 2),
            Err(BlitError::FramebufferTooLarge {
                width: usize::MAX,
                height: 2
            }),
            "cell-count overflow is refused (P3-2)"
        );
    }

    #[test]
    fn clear_fills_every_cell() {
        let mut fb = Framebuffer::new(3, 2).expect("3x2 allocates");
        fb.clear(0x1234);
        assert!(fb.pixels.iter().all(|&p| p == 0x1234), "all cells cleared");
    }

    /// Inclusive extents: `width = x2 - x1 + 1` and `i <= y2`
    /// (`Graphics_BasicDraw.cpp:20-28`).
    #[test]
    fn fill_box_writes_an_inclusive_rect() {
        let mut fb = Framebuffer::new(8, 8).expect("8x8 allocates");
        let clip = ClipArea::new(0, 0, 7, 7);
        fb.fill_box(&clip, 2, 3, 4, 5, 0x00AA);
        // 3x3 cells: (2..=4, 3..=5).
        for y in 3..=5 {
            for x in 2..=4 {
                assert_eq!(fb.get_pixel(x, y), Some(0x00AA), "cell ({x},{y}) filled");
            }
        }
        // Just outside the inclusive edges stays untouched.
        assert_eq!(fb.get_pixel(1, 3), Some(0), "left neighbour untouched");
        assert_eq!(fb.get_pixel(5, 3), Some(0), "right neighbour untouched");
        assert_eq!(fb.get_pixel(2, 2), Some(0), "top neighbour untouched");
        assert_eq!(fb.get_pixel(2, 6), Some(0), "bottom neighbour untouched");
    }

    /// Partially visible rect: clamped to the clip
    /// (`Graphics_BasicDraw.cpp:52-55`).
    #[test]
    fn fill_box_clamps_a_partially_clipped_rect() {
        let mut fb = Framebuffer::new(8, 8).expect("8x8 allocates");
        let clip = ClipArea::new(2, 2, 5, 5);
        // Straddles the left/top edges of the clip.
        fb.fill_box(&clip, 0, 0, 3, 3, 0x00BB);
        assert_eq!(fb.get_pixel(1, 1), Some(0), "outside clip untouched");
        for y in 2..=3 {
            for x in 2..=3 {
                assert_eq!(fb.get_pixel(x, y), Some(0x00BB), "clamped cell filled");
            }
        }
        // Straddles the right/bottom edges too.
        let mut fb = Framebuffer::new(8, 8).expect("8x8 allocates");
        fb.fill_box(&clip, 4, 4, 9, 9, 0x00CC);
        assert_eq!(fb.get_pixel(6, 6), Some(0), "beyond clip untouched");
        for y in 4..=5 {
            for x in 4..=5 {
                assert_eq!(fb.get_pixel(x, y), Some(0x00CC), "clamped cell filled");
            }
        }
    }

    /// Fully outside the clip: silent no-op
    /// (`Graphics_BasicDraw.cpp:49-50`).
    #[test]
    fn fill_box_fully_outside_the_clip_is_a_noop() {
        let mut fb = Framebuffer::new(8, 8).expect("8x8 allocates");
        let clip = ClipArea::new(2, 2, 5, 5);
        fb.fill_box(&clip, 6, 6, 9, 9, 0x00DD); // right of the clip
        fb.fill_box(&clip, 0, 0, 1, 1, 0x00EE); // above-left of the clip
        assert!(
            fb.pixels.iter().all(|&p| p == 0),
            "no cell written for a fully clipped-out rect"
        );
    }

    /// P3-2: even a clip wider than the buffer cannot produce an
    /// out-of-buffer write (legacy clip is always inside the surface,
    /// `Client.cpp:5718`).
    #[test]
    fn fill_box_never_writes_outside_the_buffer() {
        let mut fb = Framebuffer::new(4, 4).expect("4x4 allocates");
        let wide = ClipArea::new(-10, -10, 50, 50);
        fb.fill_box(&wide, -5, -5, 50, 50, 0x1111);
        assert_eq!(fb.pixels.len(), 16, "buffer size unchanged");
        assert!(
            fb.pixels.iter().all(|&p| p == 0x1111),
            "every in-buffer cell of the huge rect is filled"
        );
        // Negative half never wrapped into a valid index.
        assert_eq!(fb.get_pixel(-1, 0), None, "negative x reads as outside");
    }

    /// P3-5 quirk: an inverted rect is a silent no-op - legacy
    /// `width = x2 - x1 + 1` goes negative and both loops fall through
    /// (`Graphics_BasicDraw.cpp:20-28`). Stated, not "fixed".
    #[test]
    fn fill_box_inverted_rect_is_a_noop() {
        let mut fb = Framebuffer::new(8, 8).expect("8x8 allocates");
        let clip = ClipArea::new(0, 0, 7, 7);
        fb.fill_box(&clip, 5, 5, 2, 2, 0xFFFF);
        assert!(
            fb.pixels.iter().all(|&p| p == 0),
            "inverted rect writes nothing"
        );
    }

    #[test]
    fn set_and_get_pixel_gate_the_bounds() {
        let mut fb = Framebuffer::new(2, 2).expect("2x2 allocates");
        assert!(!fb.set_pixel(-1, 0, 1), "negative x refused");
        assert!(!fb.set_pixel(0, 2, 1), "y past height refused");
        assert!(fb.set_pixel(1, 1, 0xABCD), "corner written");
        assert_eq!(fb.get_pixel(1, 1), Some(0xABCD), "corner readable");
        assert_eq!(fb.get_pixel(2, 1), None, "x past width reads None");
    }
}
