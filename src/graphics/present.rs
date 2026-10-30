//! Step 7.2: the present seam - pure crop geometry plus the direct Win32
//! `BitBlt` (`WO-7.2.md` §6.5, amended 2026-10-29 by the Custodian's
//! Directive Adjustment: **no** `Presenter` trait, **no** headless
//! recorder - the backbuffer is wired straight into the real game window).
//!
//! ## Crop geometry (pure, unit-tested, no window)
//!
//! Legacy presents a sub-rectangle of the backbuffer - the one-tile border
//! is cropped off every side (`Draw.cpp:1145-1154`):
//!
//! - source origin: `(MAP_TILE_XSIZE + GetScreenRX(),
//!   MAP_TILE_YSIZE + GetScreenRY())` (`:1149`), and this build keeps
//!   `siRX = siRY = 0` (`Client.cpp:1005`), so the origin is `(80, 40)`;
//! - draw size: `siClientScreenXsize - (MAP_TILE_XSIZE * 2)` by
//!   `siClientScreenYsize - (MAP_TILE_YSIZE * 2)` (`:1146-1147`) - e.g.
//!   800x600 for the 960x680 backbuffer.
//!
//! ## On-screen path (Windows only)
//!
//! `present_to_window` is the legacy windowed present
//! (`Graphic.cpp:266-310`) with DirectDraw removed as a structural
//! simplification (`WO-7.2.md` §4): the legacy path locks a primary
//! surface, `GetDC`s both the source and the convert surface, `BitBlt`s
//! the crop into the convert surface (`:295-303`), then `Blt`s the convert
//! surface to the primary (`:310`). Here the convert surface's role is a
//! 16-bit `DIB` section selected into a memory DC (owned by
//! [`super::window`]), the primary's role is the window's client DC, and
//! the final `Blt` is a `BitBlt SRCCOPY` of exactly the crop. Kept from
//! legacy, verbatim (P3-5): the `IsIconic` early-out (`:263`) and the
//! 16-bit source crop; dropped and stated: the convert-surface hop, the
//! vsync `WaitForVerticalBlank` (`:305-308`, `WO-7.2.md` §4), and the
//! toolbar `xindent/yindent` offset (`Draw.cpp:1096-1101` - 7.2 has no
//! toolbar, indent 0, which is also the legacy default `:1092-1093`).
//!
//! `unsafe` exists in this module only behind `#[cfg(windows)]`, only as
//! the Win32 extern glue and its calls (R1 / `WO-7.2.md` §7). No gate test
//! touches it (P3-3); the operator runs it on screen.

use super::screen::{ScreenGeometry, MAP_TILE_XSIZE, MAP_TILE_YSIZE};

/// `GetScreenRX()` - the client always builds with `siRX = 0`
/// (`Client.cpp:1005`).
pub const SCREEN_RX: i32 = 0;
/// `GetScreenRY()` - `siRY = 0` (`Client.cpp:1005`).
pub const SCREEN_RY: i32 = 0;

/// The sub-rectangle of the backbuffer that reaches the screen
/// (`Draw.cpp:1145-1154`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentCrop {
    /// Source x inside the backbuffer: `MAP_TILE_XSIZE + GetScreenRX()`.
    pub src_x: i32,
    /// Source y inside the backbuffer: `MAP_TILE_YSIZE + GetScreenRY()`.
    pub src_y: i32,
    /// Width to present: `xsize - MAP_TILE_XSIZE * 2`.
    pub width: i32,
    /// Height to present: `ysize - MAP_TILE_YSIZE * 2`.
    pub height: i32,
}

/// The present crop for a backbuffer geometry - `UpdateScreen`'s
/// arguments as passed at `Draw.cpp:1145-1149` (see module docs for the
/// term-by-term citation).
pub const fn present_crop(geometry: &ScreenGeometry) -> PresentCrop {
    PresentCrop {
        src_x: MAP_TILE_XSIZE + SCREEN_RX,
        src_y: MAP_TILE_YSIZE + SCREEN_RY,
        width: geometry.xsize - MAP_TILE_XSIZE * 2,
        height: geometry.ysize - MAP_TILE_YSIZE * 2,
    }
}

/// Direct on-screen present (Windows only): mirror the backbuffer into the
/// 16-bit DIB bits, then `BitBlt SRCCOPY` the crop into the window's
/// client DC.
///
/// The mirror is one `copy_from_slice`: backbuffer and DIB are the same
/// `width * height` cell layout (both top-down, pitch = `xsize`), so the
/// legacy convert-surface hop (`Graphic.cpp:298-303`) collapses to a
/// single memcpy plus one `BitBlt`.
///
/// # Safety
///
/// `hwnd` must be a live window handle; `mem_dc` must be a memory DC with
/// the DIB section for `cells` `u16` cells selected into it; `dib_bits`
/// must point at that DIB's storage with room for `cells` cells. These
/// are exactly the invariants [`super::window`] establishes.
#[cfg(windows)]
pub unsafe fn present_to_window(
    hwnd: isize,
    mem_dc: isize,
    backbuffer: &[u16],
    dib_bits: *mut u16,
    cells: usize,
    crop: &PresentCrop,
) {
    use std::os::raw::c_int;

    #[link(name = "user32")]
    extern "system" {
        fn IsIconic(hwnd: isize) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> c_int;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn BitBlt(
            dest: isize,
            x: c_int,
            y: c_int,
            w: c_int,
            h: c_int,
            src: isize,
            sx: c_int,
            sy: c_int,
            rop: u32,
        ) -> i32;
    }

    const SRCCOPY: u32 = 0x00CC_0020;

    // IsIconic early-out: a minimized window is not presented
    // (Graphic.cpp:263).
    if IsIconic(hwnd) != 0 {
        return;
    }
    // Shape check before any raw write (P3-2): the mirror is in-bounds by
    // construction, not by hope.
    if backbuffer.len() != cells || cells == 0 {
        return;
    }
    // Mirror the whole backbuffer into the DIB (the convert-surface hop,
    // Graphic.cpp:298-303, as one memcpy).
    std::ptr::copy_nonoverlapping(backbuffer.as_ptr(), dib_bits, cells);
    // Present exactly the crop: GetDC -> BitBlt SRCCOPY -> ReleaseDC,
    // legacy shape (Graphic.cpp:295-302 with the primary Blt of :310
    // replaced by the window client DC - §4 structural simplification).
    let hdc = GetDC(hwnd);
    if hdc == 0 {
        return;
    }
    BitBlt(
        hdc,
        0,
        0,
        crop.width,
        crop.height,
        mem_dc,
        crop.src_x,
        crop.src_y,
        SRCCOPY,
    );
    ReleaseDC(hwnd, hdc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::screen::ScreenMode;

    /// Probe (d) WO §8: the crop origin is `(80, 40)` and the size is the
    /// backbuffer minus one tile border each side - numbers from
    /// `Draw.cpp:1145-1149` with `siRX = siRY = 0` (`Client.cpp:1005`).
    /// Moving the crop by one tile (160x80 shift / shrink) must fail
    /// these assertions.
    #[test]
    fn crop_is_the_one_tile_border_for_every_mode() {
        let cases = [
            (
                ScreenMode::Mode800x600.backbuffer(false),
                (80, 40, 800, 600),
                "960x680 - (160, 80) (Draw.cpp:1146-1147, Client.cpp:3206-3207)",
            ),
            (
                ScreenMode::Mode800x600.backbuffer(true),
                (80, 40, 800, 600),
                "same table (Client.cpp:3210-3214)",
            ),
            (
                ScreenMode::Mode1024x768.backbuffer(false),
                (80, 40, 960, 720),
                "1120x800 - (160, 80) (Client.cpp:3225-3226)",
            ),
            (
                ScreenMode::Mode1024x768.backbuffer(true),
                (80, 40, 1040, 800),
                "1200x880 - (160, 80) (Client.cpp:3220-3221)",
            ),
            (
                ScreenMode::Mode1024x600.backbuffer(false),
                (80, 40, 960, 600),
                "1120x680 - (160, 80) (Client.cpp:3241-3242)",
            ),
            (
                ScreenMode::Mode1280x1024.backbuffer(true),
                (80, 40, 1280, 920),
                "1440x1000 - (160, 80) (Client.cpp:3249-3250)",
            ),
        ];
        for (g, (sx, sy, w, h), cite) in cases {
            let crop = present_crop(&g);
            assert_eq!(
                (crop.src_x, crop.src_y, crop.width, crop.height),
                (sx, sy, w, h),
                "{cite}"
            );
        }
    }

    /// The crop must always sit inside its backbuffer (the present copy is
    /// a pure sub-slice - P3-2).
    #[test]
    fn crop_never_reaches_past_the_backbuffer() {
        for mode in [
            ScreenMode::Mode800x600,
            ScreenMode::Mode1024x768,
            ScreenMode::Mode1024x600,
            ScreenMode::Mode1280x1024,
        ] {
            for fullscreen in [false, true] {
                let g = mode.backbuffer(fullscreen);
                let crop = present_crop(&g);
                assert!(crop.src_x + crop.width <= g.xsize, "width fits");
                assert!(crop.src_y + crop.height <= g.ysize, "height fits");
                assert!(crop.src_x >= 0 && crop.src_y >= 0, "origin in bounds");
            }
        }
    }
}
