//! Step 7.2: screen modes, backbuffer geometry, and the tile-center
//! transform (`WO-7.2.md` §6.3).
//!
//! Format of record - `cltClient::CreateDirectDraw`
//! (`Client.cpp:3197-3251`). Every screen mode allocates its backbuffer as
//! `80 * (n + 2)` by `40 * (m + 2)` pixels - one legacy tile
//! ([`MAP_TILE_XSIZE`] x [`MAP_TILE_YSIZE`]) of border on every side - and
//! a tile half-size for the view loop:
//!
//! | mode | fullscreen | windowed | half |
//! |---|---|---|---|
//! | 800x600 | 960x680 (`:3206-3207`) | same (`:3210-3214`) | 14 (`:3208`) |
//! | 1024x768 | 1200x880 (`:3220-3221`) | 1120x800 (`:3225-3226`) | 18 (`:3222`, `:3227`) |
//! | 1024x600 | 1200x680 (`:3236-3237`) | 1120x680 (`:3241-3242`) | 16 (`:3238`, `:3243`) |
//! | 1280x1024 | 1440x1000 (`:3249-3250`) | same (no branch) | 20 (`:3251`) |
//!
//! Quirk kept verbatim (P3-5): the 1024x600 *windowed* row is
//! `40 * (15 + 2) = 680` tall while its `n` in `80 * (12 + 2) = 1120`
//! would suggest 640 - the legacy table is hard-coded and inconsistent
//! between the axes. We copy the numbers exactly as written.
//!
//! [`ScreenRect`] is the present/draw rectangle
//! (`Client.cpp:5713-5716`): `(0, 0, xsize - 1, ysize - 1)`.
//!
//! [`draw_center_xy`] is `cltMapClient::GetDrawCenterXY`
//! (`mapdraw.cpp:36-52`): the isometric diamond offset that maps a
//! virtual-map tile onto the screen around its center.

/// `MAP_TILE_XSIZE` (`map.h:33`).
pub const MAP_TILE_XSIZE: i32 = 80;
/// `MAP_TILE_XSIZE_HALF` (`map.h:34`).
pub const MAP_TILE_XSIZE_HALF: i32 = 40;
/// `MAP_TILE_YSIZE` (`map.h:35`).
pub const MAP_TILE_YSIZE: i32 = 40;
/// `MAP_TILE_YSIZE_HALF` (`map.h:36`).
pub const MAP_TILE_YSIZE_HALF: i32 = 20;

/// The four legacy screen modes (`MODE_GRAPHICS*`, selected by
/// `siScreenMode` - `Client.cpp:3202`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenMode {
    /// `MODE_GRAPHICS800_600`.
    Mode800x600,
    /// `MODE_GRAPHICS1024_768`.
    Mode1024x768,
    /// `MODE_GRAPHICS1024_600` (the KHY-1218 late addition,
    /// `Client.cpp:3232-3233`).
    Mode1024x600,
    /// `MODE_GRAPHICS1280_1024`.
    Mode1280x1024,
}

/// Backbuffer geometry for one mode: `siClientScreenXsize`,
/// `siClientScreenYsize`, `siClientScreenHalfSize`
/// (`Client.cpp:3206-3208`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenGeometry {
    /// Backbuffer width in pixels (`siClientScreenXsize`).
    pub xsize: i32,
    /// Backbuffer height in pixels (`siClientScreenYsize`).
    pub ysize: i32,
    /// View half-size in tiles (`siClientScreenHalfSize`).
    pub half_size: i32,
}

/// The present/draw rectangle (`ScreenRect`, `Client.cpp:5713-5716`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    /// Left, always 0 (`:5713`).
    pub left: i32,
    /// Top, always 0 (`:5715`).
    pub top: i32,
    /// `siClientScreenXsize - 1` (`:5714`).
    pub right: i32,
    /// `siClientScreenYsize - 1` (`:5716`).
    pub bottom: i32,
}

impl ScreenMode {
    /// Backbuffer geometry for this mode - the verbatim
    /// `CreateDirectDraw` table (`Client.cpp:3197-3251`). `fullscreen`
    /// is `g_bFullScreen`; the two modes without a fullscreen/windowed
    /// split use the same numbers on both paths.
    pub const fn backbuffer(self, fullscreen: bool) -> ScreenGeometry {
        match (self, fullscreen) {
            // Client.cpp:3206-3208 - one table for both paths (:3210-3214).
            (ScreenMode::Mode800x600, _) => ScreenGeometry {
                xsize: 80 * (10 + 2),
                ysize: 40 * (15 + 2),
                half_size: 14,
            },
            // Client.cpp:3220-3222.
            (ScreenMode::Mode1024x768, true) => ScreenGeometry {
                xsize: 80 * (13 + 2),
                ysize: 40 * (20 + 2),
                half_size: 18,
            },
            // Client.cpp:3225-3227.
            (ScreenMode::Mode1024x768, false) => ScreenGeometry {
                xsize: 80 * (12 + 2),
                ysize: 40 * (18 + 2),
                half_size: 18,
            },
            // Client.cpp:3236-3238.
            (ScreenMode::Mode1024x600, true) => ScreenGeometry {
                xsize: 80 * (13 + 2),
                ysize: 40 * (15 + 2),
                half_size: 16,
            },
            // Client.cpp:3241-3243 - quirk: windowed keeps the fullscreen
            // height (see module docs, P3-5).
            (ScreenMode::Mode1024x600, false) => ScreenGeometry {
                xsize: 80 * (12 + 2),
                ysize: 40 * (15 + 2),
                half_size: 16,
            },
            // Client.cpp:3249-3251 - no fullscreen/windowed split.
            (ScreenMode::Mode1280x1024, _) => ScreenGeometry {
                xsize: 80 * (16 + 2),
                ysize: 40 * (23 + 2),
                half_size: 20,
            },
        }
    }
}

impl ScreenGeometry {
    /// The `ScreenRect` for this backbuffer (`Client.cpp:5713-5716`).
    pub const fn screen_rect(self) -> ScreenRect {
        ScreenRect {
            left: 0,
            top: 0,
            right: self.xsize - 1,
            bottom: self.ysize - 1,
        }
    }
}

/// `cltMapClient::GetDrawCenterXY` (`mapdraw.cpp:36-52`): screen pixel
/// center of tile `(tile_x, tile_y)` when the view is centered on
/// virtual-map tile `(center_x, center_y)`.
///
/// The transform is the isometric diamond: x moves by
/// `MAP_TILE_XSIZE_HALF` per tile step in either axis (with opposite
/// signs), y moves by `MAP_TILE_YSIZE_HALF` per step in both axes
/// (`:39-49`), around the screen-rect center (`:43`, `:47`).
pub fn draw_center_xy(
    center_x: i32,
    center_y: i32,
    tile_x: i32,
    tile_y: i32,
    screen: &ScreenRect,
) -> (i32, i32) {
    // mapdraw.cpp:39-43, term for term.
    let center_px = -center_x * MAP_TILE_XSIZE_HALF
        + center_y * MAP_TILE_XSIZE_HALF
        + tile_x * MAP_TILE_XSIZE_HALF
        - tile_y * MAP_TILE_XSIZE_HALF
        + screen.left
        + (screen.right - screen.left + 1) / 2;
    // mapdraw.cpp:45-49, term for term.
    let center_py = -center_y * MAP_TILE_YSIZE_HALF - center_x * MAP_TILE_YSIZE_HALF
        + screen.top
        + (screen.bottom - screen.top + 1) / 2
        + tile_y * MAP_TILE_YSIZE_HALF
        + tile_x * MAP_TILE_YSIZE_HALF;
    (center_px, center_py)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The full legacy table, numbers transcribed from the cited lines
    /// (no-handwritten-numbers: each expected value carries its
    /// `Client.cpp` citation, none is computed by the code under test).
    #[test]
    fn backbuffer_table_matches_create_directdraw() {
        let cases = [
            (
                ScreenMode::Mode800x600,
                true,
                (960, 680, 14),
                "Client.cpp:3206-3208 (one table for both paths, :3210-3214)",
            ),
            (
                ScreenMode::Mode800x600,
                false,
                (960, 680, 14),
                "Client.cpp:3206-3208 (:3210-3214)",
            ),
            (
                ScreenMode::Mode1024x768,
                true,
                (1200, 880, 18),
                "Client.cpp:3220-3222",
            ),
            (
                ScreenMode::Mode1024x768,
                false,
                (1120, 800, 18),
                "Client.cpp:3225-3227",
            ),
            (
                ScreenMode::Mode1024x600,
                true,
                (1200, 680, 16),
                "Client.cpp:3236-3238",
            ),
            (
                // P3-5 quirk: windowed keeps the fullscreen height.
                ScreenMode::Mode1024x600,
                false,
                (1120, 680, 16),
                "Client.cpp:3241-3243 (40*(15+2), not 40*(12+2))",
            ),
            (
                ScreenMode::Mode1280x1024,
                true,
                (1440, 1000, 20),
                "Client.cpp:3249-3251",
            ),
            (
                ScreenMode::Mode1280x1024,
                false,
                (1440, 1000, 20),
                "Client.cpp:3249-3251 (no split)",
            ),
        ];
        for (mode, fullscreen, (x, y, half), cite) in cases {
            let g = mode.backbuffer(fullscreen);
            assert_eq!(
                (g.xsize, g.ysize, g.half_size),
                (x, y, half),
                "{mode:?} fullscreen={fullscreen} must match {cite}"
            );
        }
    }

    /// `ScreenRect = (0, 0, xsize - 1, ysize - 1)` and the clip reset that
    /// follows (`Client.cpp:5713-5718`).
    #[test]
    fn screen_rect_is_the_off_by_one_backbuffer_rect() {
        let g = ScreenMode::Mode800x600.backbuffer(false);
        assert_eq!(
            g.screen_rect(),
            ScreenRect {
                left: 0,
                top: 0,
                right: 959,
                bottom: 679
            },
            "Client.cpp:5713-5716: right/bottom are xsize-1 / ysize-1"
        );
    }

    /// Diamond invariant, hand-derived from `mapdraw.cpp:39-49`: a tile at
    /// the view center lands on the screen-rect center.
    ///
    /// For mode 800x600 the screen rect is `(0,0,959,679)`, so the center
    /// term is `(959 - 0 + 1) / 2 = 480` x `(679 - 0 + 1) / 2 = 340`. With
    /// `tile == center` every ±term cancels pair-wise (the `-cx*XH +
    /// cx*XH` groups of `:39-42`), leaving exactly `(480, 340)`.
    #[test]
    fn the_tile_at_the_view_center_lands_on_the_screen_center() {
        let g = ScreenMode::Mode800x600.backbuffer(false);
        let screen = g.screen_rect();
        assert_eq!(
            draw_center_xy(100, 100, 100, 100, &screen),
            (480, 340),
            "center tile maps to (xsize/2, ysize/2) = (480, 340)"
        );
    }

    /// Diamond offsets, hand-derived from `mapdraw.cpp:39-49`: one tile
    /// step +x from the view center moves the screen point by
    /// `(+MAP_TILE_XSIZE_HALF, +MAP_TILE_YSIZE_HALF) = (+40, +20)`; one
    /// step +y moves it by `(-MAP_TILE_XSIZE_HALF, +MAP_TILE_YSIZE_HALF)
    /// = (-40, +20)`.
    #[test]
    fn one_tile_steps_move_the_expected_diamond_offsets() {
        let g = ScreenMode::Mode800x600.backbuffer(false);
        let screen = g.screen_rect();
        let (cx, cy) = draw_center_xy(50, 50, 50, 50, &screen);
        let (px, py) = draw_center_xy(50, 50, 51, 50, &screen);
        assert_eq!(
            (px - cx, py - cy),
            (40, 20),
            "+x tile step: (XSIZE_HALF, YSIZE_HALF) (mapdraw.cpp:41-42,48-49)"
        );
        let (qx, qy) = draw_center_xy(50, 50, 50, 51, &screen);
        assert_eq!(
            (qx - cx, qy - cy),
            (-40, 20),
            "+y tile step: (-XSIZE_HALF, YSIZE_HALF) (mapdraw.cpp:42,49)"
        );
    }
}
