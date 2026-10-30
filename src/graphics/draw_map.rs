//! Step 7.2: the tile-map draw loop (`WO-7.2.md` §6.4).
//!
//! Port of `cltMapClient::DrawMap`'s full redraw (`mapdraw.cpp:167-271`)
//! and its helpers, against Step 7.1's parsed [`MapFile`]:
//!
//! 1. **Near-edge clear** (`:233-237`): when the view center sits within
//!    `half_size` tiles of any map border, the whole screen rect is
//!    refilled with color-table index 0 - it wipes stale edge tiles the
//!    clipped view can no longer reach. The color resolves through the
//!    *current* (global) palette at draw time (`FillBox`,
//!    `Graphic_Capsule.cpp:856`).
//! 2. **Tile loops** (`:240-241`): `virtual_y` over
//!    `center_y - half .. center_y + half` and `virtual_x` likewise -
//!    **half-open**: the loop runs `2 * half` tiles per axis, not
//!    `2 * half + 1`. Stated, not "fixed" (P3-5).
//! 3. **Out-of-map skip** (`:244`): `IsInMapArea` is
//!    `x < 0 || x >= MapXsize || y < 0 || y >= MapYsize` (`map.cpp:224-228`).
//! 4. **Center** via [`draw_center_xy`] (`:247`).
//! 5. **Layer pick** (`:249-256`): draw layer 1, *unless*
//!    `GetDrawInfo_Unique(x, y) == 90` on layer 1, which draws layer 0.
//!    The legacy unique read is `pDrawInfo[...] >> 24` - the top byte of
//!    the layer-1 **draw_info DWORD**, not the separate `pUnique` array
//!    (`Map-Layor.cpp:79-86`). The magic 90 is a legacy constant -
//!    preserved and named ([`UNIQUE_LAYER0`]), not "fixed" (P3-5).
//! 6. **`draw_layer`** (`mapdraw.cpp:881-911`): decompose `draw_info`,
//!    bounds-check the tileset ref against [`MAX_TILESET_NUMBER`]
//!    (`:897`), fetch the tile sprite (`:900`), blit at
//!    `center - half_tile` (`:890-891`) through [`blit::put_mpr`]
//!    (`DrawTile`, `:914-928`, `PUT_SPR_SPR` with `cmmode = FALSE` outside
//!    map edit - `:927`).
//!
//! `draw_info` decomposition, verbatim (`Map-Layor.cpp`):
//! `ref = LOBYTE(HIWORD(v)) = (v >> 16) & 0xFF` (`:107`),
//! `file = LOWORD(v) >> 8 = (v >> 8) & 0xFF` (`:129`),
//! `font = LOWORD(v) & 0xFF` (`:151`), `unique = v >> 24` (`:85`);
//! the cell index is `x + YOff[y]` (`:85`, `YOff[y] = y * MapXsize`).
//!
//! Legacy quirks kept (P3-5): [`MAX_TILESET_NUMBER`] = 255 means ref 255
//! itself is skipped (`ref >= MAX_TILESET_NUMBER`, `TileSet.h:12`,
//! `mapdraw.cpp:897`); `font` is decomposed but 7.2 blits the image-0
//! decode that Step 7.1 produces (`spr.rs` - per-image/font frame slicing
//! is a later step, stated); the tile *file* lookup ignores layer, just
//! as `pclTileInfo[ref]->pSpr[file]` does (`mapdraw.cpp:917`).
//!
//! I/O-free by construction: tiles come in through the [`Tileset`] trait,
//! so gate tests inject synthetic tiles and no archive/file access exists
//! in the draw path. No `unsafe` (WO §7).

use super::blit;
use super::framebuffer::{ClipArea, Framebuffer};
use super::map::MapFile;
use super::palette::TPal;
use super::screen::{
    draw_center_xy, ScreenGeometry, ScreenRect, MAP_TILE_XSIZE_HALF, MAP_TILE_YSIZE_HALF,
};
use super::spr::LoadedSprite;

/// The legacy magic unique value that swaps layer 1 for layer 0 at a
/// cell: `if (GetDrawInfo_Unique(j, i) != 90) DrawLayer(..., 1, ...) else
/// DrawLayer(..., 0, ...)` (`mapdraw.cpp:249-256`). A quirk, preserved
/// verbatim (P3-5).
pub const UNIQUE_LAYER0: u8 = 90;

/// `MAX_TILESET_NUMBER` (`TileSet.h:12`); `DrawLayer` refuses
/// `ref >= 255` (`mapdraw.cpp:897`), so ref 255 - the last `u8` - is
/// never drawn. Stated, not "fixed" (P3-5).
pub const MAX_TILESET_NUMBER: u8 = 255;

/// Tile sprites for the map: `ref -> file -> sprite`, the shape of
/// `pclTileManager->pclTileInfo[ref]->pSpr[file]`
/// (`mapdraw.cpp:917`). Layer is intentionally absent - the legacy lookup
/// does not use it.
pub trait Tileset {
    /// The sprite for tileset ref `ref_index`, sheet `file`, or `None`
    /// when legacy would see a `NULL` slot (`mapdraw.cpp:898-901`).
    fn tile(&self, ref_index: u8, file: u8) -> Option<&LoadedSprite>;
}

/// `cltMapClient::DrawMap` - full redraw of the visible tile field into
/// `fb` (`mapdraw.cpp:167-271`).
///
/// `center_x`/`center_y` are the view's virtual-map position
/// (`VirtualMapX`/`VirtualMapY`), `global_palette` is the current color
/// table the near-edge clear resolves through
/// (`Graphic_Capsule.cpp:856`).
#[allow(clippy::too_many_arguments)] // legacy DrawMap reads 9+ globals; see draw_layer
pub fn draw_map(
    fb: &mut Framebuffer,
    clip: &ClipArea,
    map: &MapFile,
    tileset: &dyn Tileset,
    center_x: i32,
    center_y: i32,
    geometry: &ScreenGeometry,
    screen: &ScreenRect,
    global_palette: &TPal,
) {
    let map_x = i32::from(map.header.map_xsize);
    let map_y = i32::from(map.header.map_ysize);
    let half = geometry.half_size;

    // Near-edge full-screen clear (`mapdraw.cpp:233-237`): color-table
    // index 0 through the current (global) table.
    if center_x < half || center_x > map_x - half || center_y < half || center_y > map_y - half {
        let index0 = global_palette.pal[0];
        fb.fill_box(clip, 0, 0, geometry.xsize - 1, geometry.ysize - 1, index0);
    }

    // Tile loops - half-open, exactly `i < VirtualMapY + half` /
    // `j < VirtualMapX + half` (`mapdraw.cpp:240-241`).
    for tile_y in center_y - half..center_y + half {
        for tile_x in center_x - half..center_x + half {
            // IsInMapArea (`:244`, `map.cpp:224-228`).
            if tile_x < 0 || tile_x >= map_x || tile_y < 0 || tile_y >= map_y {
                continue;
            }
            let (center_px, center_py) = draw_center_xy(center_x, center_y, tile_x, tile_y, screen);
            let layer = pick_layer(map, map_x, tile_x, tile_y);
            draw_layer(
                fb,
                clip,
                map,
                tileset,
                layer,
                tile_x,
                tile_y,
                center_px,
                center_py,
                map_x,
                global_palette,
            );
        }
    }
}

/// Layer selection (`mapdraw.cpp:249-256`): layer 1 unless layer 1's
/// `draw_info` top byte is [`UNIQUE_LAYER0`] (`Map-Layor.cpp:79-86`).
fn pick_layer(map: &MapFile, map_x: i32, tile_x: i32, tile_y: i32) -> usize {
    let idx = (tile_x + tile_y * map_x) as usize; // `six + YOff[siy]` (Map-Layor.cpp:85)
    match map.layers.get(1).and_then(|layer| layer.draw_info.get(idx)) {
        // Out-of-area reads return 0 in legacy (`Map-Layor.cpp:82-83`),
        // which never equals 90 -> layer 1 stays the default.
        Some(&draw_info) if ((draw_info >> 24) as u8) == UNIQUE_LAYER0 => 0,
        _ => 1,
    }
}

/// `cltMapClient::DrawLayer` (`mapdraw.cpp:881-911`) + the `DrawTile`
/// blit (`:914-928`).
#[allow(clippy::too_many_arguments)]
fn draw_layer(
    fb: &mut Framebuffer,
    clip: &ClipArea,
    map: &MapFile,
    tileset: &dyn Tileset,
    layer: usize,
    tile_x: i32,
    tile_y: i32,
    center_px: i32,
    center_py: i32,
    map_x: i32,
    global_palette: &TPal,
) {
    let idx = (tile_x + tile_y * map_x) as usize;
    let Some(&draw_info) = map.layers.get(layer).and_then(|l| l.draw_info.get(idx)) else {
        return;
    };
    let ref_index = ((draw_info >> 16) & 0xFF) as u8; // LOBYTE(HIWORD) (Map-Layor.cpp:107)
    let file = ((draw_info >> 8) & 0xFF) as u8; // (LOWORD >> 8) (Map-Layor.cpp:129)
                                                // `ref >= MAX_TILESET_NUMBER` -> FALSE (mapdraw.cpp:897). Legacy's
                                                // `ref` is a `LOBYTE(HIWORD(draw_info))` (Map-Layor.cpp:107), i.e.
                                                // 0..=255, and `SHORT ref < 0` can never fire on it - so in `u8`
                                                // space the `>= 255` arm collapses to `== 255` (only ref 255 is
                                                // refused, exactly as legacy skips just that one slot).
    if ref_index == MAX_TILESET_NUMBER {
        return;
    }
    // `pclTileInfo[ref] == NULL` -> FALSE (`:898`); `pSpr[file] == NULL`
    // -> no draw (`:900-901`).
    let Some(sprite) = tileset.tile(ref_index, file) else {
        return;
    };
    // startx/starty = center - half tile (`mapdraw.cpp:890-891`).
    let start_x = center_px - MAP_TILE_XSIZE_HALF;
    let start_y = center_py - MAP_TILE_YSIZE_HALF;
    // Per-sprite table selection (v400 swap, Graphic_Capsule.cpp:550-554)
    // then the MPR blit (`DrawTile` -> `PutMpr PUT_SPR_SPR, cmmode=FALSE`,
    // mapdraw.cpp:924-927). A refused blit (P3-2 typed error) skips the
    // tile rather than panicking mid-frame.
    let table = blit::active_color_table(sprite, global_palette);
    let _ = blit::put_mpr(fb, clip, &sprite.image, start_x, start_y, table);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::blit::synthetic_sprite;
    use crate::graphics::map::{CltMapHeader, MapLayer};
    use crate::graphics::palette::TPal;
    use crate::graphics::spr::LoadedSprite;
    use std::cell::Cell;

    /// One small square map with both layers addressable; draw_info is
    /// assembled from the cited byte fields (ref << 16 | file << 8 |
    /// font | unique << 24 - `Map-Layor.cpp:85-151`).
    fn synthetic_map(map_x: i16, map_y: i16, layer0: &[u32], layer1: &[u32]) -> MapFile {
        MapFile {
            header: CltMapHeader {
                version: 1,
                unique: 0,
                text: [0; 128],
                reserved0: 0,
                map_xsize: map_x,
                map_ysize: map_y,
                map_type: 0,
                tile_set1: 0,
                b_weather_switch: false,
                tile_set2: 0,
                reserved: [0; 19],
            },
            id_map: vec![0; map_x as usize * map_y as usize],
            coll_map: vec![0; map_x as usize * map_y as usize],
            layers: [
                MapLayer {
                    draw_info: layer0.to_vec(),
                    unique: vec![0; map_x as usize * map_y as usize],
                },
                MapLayer {
                    draw_info: layer1.to_vec(),
                    unique: vec![0; map_x as usize * map_y as usize],
                },
            ],
            field_object_map: vec![0; map_x as usize * map_y as usize],
            field_object_para: vec![0; map_x as usize * map_y as usize],
            geo_info: None,
        }
    }

    /// `draw_info` per the cited decomposition (`Map-Layor.cpp:107`,
    /// `:129`, `:85`): ref in bits 16-23, file in bits 8-15, unique in
    /// bits 24-31 (font bits 0-7 are 0 here).
    fn draw_info(ref_index: u8, file: u8, unique: u8) -> u32 {
        ((ref_index as u32) << 16) | ((file as u32) << 8) | ((unique as u32) << 24)
    }

    fn global_pal() -> TPal {
        let mut pal = [0u16; 256];
        pal[0] = 0x0000; // index 0 -> black (the near-edge clear color)
        pal[1] = 0xF800; // red
        pal[2] = 0x07E0; // green
        TPal { pal }
    }

    /// A tileset of two solid 80x40 tiles (one legacy tile,
    /// `map.h:33,35`) in distinct colors, counting `tile()` lookups.
    struct FakeTileset {
        /// Ref 0 sheet 0 - solid palette index 1 (red).
        red: LoadedSprite,
        /// Ref 1 sheet 0 - solid palette index 2 (green).
        green: LoadedSprite,
        /// Legacy `pclTileInfo[ref]` lookups observed.
        lookups: Cell<usize>,
    }

    impl FakeTileset {
        fn new() -> Self {
            Self {
                red: synthetic_sprite(80, 40, vec![Some(1); 80 * 40]),
                green: synthetic_sprite(80, 40, vec![Some(2); 80 * 40]),
                lookups: Cell::new(0),
            }
        }
    }

    impl Tileset for FakeTileset {
        fn tile(&self, ref_index: u8, file: u8) -> Option<&LoadedSprite> {
            self.lookups.set(self.lookups.get() + 1);
            match (ref_index, file) {
                (0, 0) => Some(&self.red),
                (1, 0) => Some(&self.green),
                _ => None,
            }
        }
    }

    /// Mode-800x600 windowed geometry + its screen rect (citations live on
    /// the table test in `screen.rs`).
    fn geom() -> (ScreenGeometry, ScreenRect) {
        let g = crate::graphics::screen::ScreenMode::Mode800x600.backbuffer(false);
        let rect = g.screen_rect();
        (g, rect)
    }

    fn full_clip(fb: &Framebuffer) -> ClipArea {
        ClipArea::new(0, 0, fb.width as i32 - 1, fb.height as i32 - 1)
    }

    fn fb_for(g: &ScreenGeometry) -> Framebuffer {
        Framebuffer::new(g.xsize as usize, g.ysize as usize).expect("backbuffer allocates")
    }

    /// Probe (c) WO §8: `unique == 90` on layer 1 swaps in layer 0
    /// (`mapdraw.cpp:249-256`, `Map-Layor.cpp:79-86`). Swapping the layer
    /// selection must fail this test: at the center cell layer 1 (green)
    /// draws by default, and with unique 90 layer 0 (red) draws instead.
    #[test]
    fn unique_90_swaps_layer_1_for_layer_0() {
        const X: usize = 8;
        const Y: usize = 8;
        // Layer 0 everywhere red (ref 0), layer 1 everywhere green
        // (ref 1). The first map is the plain default: NO unique 90
        // anywhere, so layer 1 must draw at the sampled cell.
        let layer0 = vec![draw_info(0, 0, 0); X * Y];
        let layer1 = vec![draw_info(1, 0, 0); X * Y];
        let center = 4 + 4 * X;
        let map = synthetic_map(X as i16, Y as i16, &layer0, &layer1);
        let (g, rect) = geom();
        let pal = global_pal();

        // View centered on (4,4): the center tile's screen pixel sits at
        // the screen center (480, 340) (the `screen.rs` invariant test).
        // Sampling (470, 330) - inside the center tile's rect (440..519,
        // 320..359, `mapdraw.cpp:890-891`) but outside the later tile
        // (5,4) that would overwrite (480, 340) at (480..559, 340..379)
        // (loop order `:240-241`, y outer / x inner) - reads exactly the
        // center tile's layer color.
        let mut fb = fb_for(&g);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        draw_map(&mut fb, &clip, &map, &tiles, 4, 4, &g, &rect, &pal);
        assert_eq!(
            fb.get_pixel(470, 330),
            Some(0x07E0),
            "default: layer 1 (green) draws (mapdraw.cpp:249-251)"
        );

        // Same scene, center cell carries unique 90 -> layer 0 (red).
        let mut layer1_90 = vec![draw_info(1, 0, 0); X * Y];
        layer1_90[center] = draw_info(1, 0, UNIQUE_LAYER0);
        let map90 = synthetic_map(X as i16, Y as i16, &layer0, &layer1_90);
        let mut fb = fb_for(&g);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        draw_map(&mut fb, &clip, &map90, &tiles, 4, 4, &g, &rect, &pal);
        assert_eq!(
            fb.get_pixel(470, 330),
            Some(0xF800),
            "unique == 90: layer 0 (red) draws (mapdraw.cpp:249-256)"
        );
    }

    /// The tile loops are half-open: `2 * half` tiles per axis
    /// (`mapdraw.cpp:240-241`), so a fully in-map view performs exactly
    /// `(2 * half)^2` `tile()` lookups - 28 x 28 = 784 for half = 14.
    #[test]
    fn the_tile_loops_are_half_open() {
        const X: usize = 64;
        const Y: usize = 64;
        let layer = vec![draw_info(0, 0, 0); X * Y];
        let map = synthetic_map(X as i16, Y as i16, &layer, &layer);
        let (g, rect) = geom();
        assert_eq!(g.half_size, 14, "mode 800x600 half (Client.cpp:3208)");
        let pal = global_pal();
        let mut fb = fb_for(&g);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        draw_map(&mut fb, &clip, &map, &tiles, 32, 32, &g, &rect, &pal);
        let expected = (2 * 14) * (2 * 14); // 784, hand-computed from the half-open loop
        assert_eq!(
            tiles.lookups.get(),
            expected,
            "half-open loops (mapdraw.cpp:240-241): 28x28 cells, no +1"
        );
    }

    /// Out-of-map cells are skipped (`mapdraw.cpp:244`,
    /// `map.cpp:224-228`) and never panic or index out of the layers
    /// (P3-2), while the in-map cells still draw.
    #[test]
    fn out_of_map_cells_are_skipped() {
        const X: i16 = 8;
        const Y: i16 = 8;
        let layer0 = vec![draw_info(0, 0, 0); 64];
        let layer1 = vec![draw_info(1, 0, 0); 64];
        let map = synthetic_map(X, Y, &layer0, &layer1);
        let (g, rect) = geom();
        let pal = global_pal();
        let mut fb = fb_for(&g);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        // View at the top-left corner: most of the loop is off-map.
        draw_map(&mut fb, &clip, &map, &tiles, 0, 0, &g, &rect, &pal);
        // In-map part of the loop: the 8x8 map intersects x in 0..14 and
        // y in 0..14 only on 0..8, so 8 * 8 = 64 lookups (everything else
        // is skipped at `:244`).
        assert_eq!(
            tiles.lookups.get(),
            8 * 8,
            "only in-map cells reach the tileset (mapdraw.cpp:244)"
        );
        // The in-map corner tile: draw_center(0,0, 0,0) = (480, 340)
        // (mapdraw.cpp:39-49 with the screen center 480/340), blit origin
        // (440, 320) (`:890-891`) -> tile-local (40, 20) samples at
        // (480, 340) and shows layer 1's green.
        assert_eq!(
            fb.get_pixel(480, 340),
            Some(0x07E0),
            "in-map cell draws (layer 1 green)"
        );
    }

    /// Near-edge full-screen clear (`mapdraw.cpp:233-237`): a view within
    /// `half` tiles of the border wipes the backbuffer with color-table
    /// index 0 first.
    #[test]
    fn near_edge_views_clear_the_screen_first() {
        const X: i16 = 8;
        const Y: i16 = 8;
        let layer0 = vec![draw_info(0, 0, 0); 64];
        let layer1 = vec![draw_info(1, 0, 0); 64];
        let map = synthetic_map(X, Y, &layer0, &layer1);
        let (g, rect) = geom();
        let pal = global_pal();
        let mut fb = fb_for(&g);
        // Sentinel everywhere: only the clear can repaint the far corners.
        fb.clear(0x5A5A);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        // center_x = 7 < half = 14 fires the clear arm
        // (`mapdraw.cpp:233-234`).
        draw_map(&mut fb, &clip, &map, &tiles, 7, 4, &g, &rect, &pal);
        assert_eq!(
            fb.get_pixel(2, 2),
            Some(0x0000),
            "near-edge clear painted the whole screen rect (mapdraw.cpp:236)"
        );
    }

    /// The same cited branch, negative case: an interior view must *not*
    /// clear - with a tileset that serves no sprites at all, the
    /// sentinel survives everywhere (only the clear could repaint it).
    #[test]
    fn interior_views_do_not_clear() {
        struct EmptyTileset;
        impl Tileset for EmptyTileset {
            fn tile(&self, _ref_index: u8, _file: u8) -> Option<&LoadedSprite> {
                None
            }
        }
        const X: i16 = 64;
        const Y: i16 = 64;
        let layer = vec![draw_info(0, 0, 0); 64 * 64];
        let map = synthetic_map(X, Y, &layer, &layer);
        let (g, rect) = geom();
        let pal = global_pal();
        let mut fb = fb_for(&g);
        fb.clear(0x5A5A);
        let clip = full_clip(&fb);
        draw_map(&mut fb, &clip, &map, &EmptyTileset, 32, 32, &g, &rect, &pal);
        assert_eq!(
            fb.get_pixel(0, 0),
            Some(0x5A5A),
            "no near-edge clear for an interior view (mapdraw.cpp:233)"
        );
    }

    /// `ref >= 255` is refused (`mapdraw.cpp:897`, `TileSet.h:12`): a
    /// ref-255 cell draws nothing and never reaches the tileset (P3-5
    /// quirk: the 255th slot exists in `u8` space but is unreachable).
    /// The view is near-edge, so the screen first shows the index-0
    /// clear (`:233-237`); no tile may paint over it.
    #[test]
    fn ref_255_is_never_drawn() {
        const X: usize = 8;
        const Y: usize = 8;
        let layer0 = vec![draw_info(0, 0, 0); X * Y];
        // Layer 1 uses ref 255 everywhere (still `u8`-legal).
        let layer1 = vec![draw_info(255, 0, 0); X * Y];
        let map = synthetic_map(X as i16, Y as i16, &layer0, &layer1);
        let (g, rect) = geom();
        let pal = global_pal();
        let mut fb = fb_for(&g);
        fb.clear(0x5A5A);
        let clip = full_clip(&fb);
        let tiles = FakeTileset::new();
        // center 4 < half 14 -> the near-edge clear runs first
        // (`mapdraw.cpp:233-234`), painting palette index 0 = 0x0000.
        draw_map(&mut fb, &clip, &map, &tiles, 4, 4, &g, &rect, &pal);
        assert_eq!(
            fb.get_pixel(470, 330),
            Some(0x0000),
            "ref 255 draws nothing over the clear (mapdraw.cpp:897, TileSet.h:12)"
        );
        assert_eq!(
            tiles.lookups.get(),
            0,
            "the ref gate runs before the tileset lookup (mapdraw.cpp:897 vs :900)"
        );
    }
}
