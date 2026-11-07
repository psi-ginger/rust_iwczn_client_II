//! Step 8.1 §7.1 — the screen→tile inverse (`WO-8.1` D9/D16): the exact
//! reverse of the frozen 7.2 forward transform
//! [`draw_center_xy`](crate::graphics::screen::draw_center_xy)
//! (`mapdraw.cpp:36-52`), built from the two-step legacy chain
//! `TransFromScreenToRealXY`
//! (`Map-Client.cpp:183-188`) → `GetRealTileByRealDot`
//! (`MapCommon.cpp:963-965`), exactly as `MouseOperation.cpp:537,540`
//! composes them.
//!
//! Coordinate space (D9): everything here lives in `ScreenRect`/backbuffer
//! space — the space `draw_center_xy`, `IsPtInMap` and
//! `TransFromScreenToRealXY` all operate in. The window→backbuffer
//! translation `+ MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY`
//! (`MouseOperation.cpp:232-233`, Q4 → `(80, 40)`) is 8.3's message decode,
//! not this module's. No camera term exists: `GetScreenRX/RY = 0`
//! (`CommonLogic.h:837-838`).
//!
//! Stated divergences (WO-8.1 §8, Haru/CAPA): legacy computes in `SHORT`;
//! this port uses `i32` to match the frozen forward transform — no value
//! differs where legacy's arithmetic stays in range.

use crate::graphics::screen::{ScreenRect, MAP_TILE_XSIZE_HALF, MAP_TILE_YSIZE_HALF};

/// The view a click is interpreted against — screen geometry + map bounds
/// (D10; 8.3 wires it, tests build it directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapView {
    /// The backbuffer `ScreenRect` (`screen.rs:66-77`, `Client.cpp:5713-5716`).
    pub screen: ScreenRect,
    /// View-center tile — `draw_center_xy`'s `(center_x, center_y)`
    /// (`screen.rs:147`) ≡ legacy `VirtualMapX/Y` (`Map-Client.cpp:185`).
    pub center: (i32, i32),
    /// Map tile size `(MapXsize, MapYsize)` (`map.cpp:226`) — D14's gate.
    pub map_size: (i32, i32),
}

/// `cltMapClient::TransFromScreenToRealXY` (`Map-Client.cpp:183-188`), term
/// for term, Q4 camera anchor `GetScreenRX/RY = 0` folded away — screen
/// pixel → real dot.
#[must_use]
pub fn screen_to_real(x: i32, y: i32, center: (i32, i32), screen: &ScreenRect) -> (i32, i32) {
    let mid_x = screen.left + (screen.right - screen.left + 1) / 2;
    let mid_y = screen.top + (screen.bottom - screen.top + 1) / 2;
    // :185 — minus the view-center offset (draw_center_xy's center terms, negated).
    let realx = x - mid_x - (-center.0 * MAP_TILE_XSIZE_HALF + center.1 * MAP_TILE_XSIZE_HALF);
    // :187 — the trailing `+ MAP_TILE_YSIZE_HALF` is legacy's, kept verbatim (E15).
    let realy = y - mid_y - (-center.1 * MAP_TILE_YSIZE_HALF - center.0 * MAP_TILE_YSIZE_HALF)
        + MAP_TILE_YSIZE_HALF;
    (realx, realy)
}

/// `cltMapCommon::GetRealTileByRealDot` (`MapCommon.cpp:963-965`): **both**
/// axes divide by `MAP_TILE_XSIZE_HALF` (legacy quirk kept), C-style
/// truncation toward zero (Rust's integer `/` matches C99).
#[must_use]
pub fn real_to_tile(realx: i32, realy: i32) -> (i32, i32) {
    (
        (realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
        (-realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
    )
}

/// The exact inverse of `draw_center_xy` (`screen.rs:147`): the two-step
/// legacy chain `TransFromScreenToRealXY` → `GetRealTileByRealDot`
/// (`MouseOperation.cpp:537,540`).
///
/// Round-trip identity (WO-8.1 §2.2-1, charter gate 1): for every
/// `tile ≥ 0` and any center/screen,
/// `screen_to_tile(draw_center_xy(cx, cy, tx, ty, screen), (cx, cy), screen)
/// == (tx, ty)` — T12 pins it, T13/T14 pin hand-computed goldens.
#[must_use]
pub fn screen_to_tile(x: i32, y: i32, center: (i32, i32), screen: &ScreenRect) -> (i32, i32) {
    let (realx, realy) = screen_to_real(x, y, center, screen);
    real_to_tile(realx, realy)
}

/// `cltMapClient::IsPtInMap` (`Map-Client.cpp:196-206`) — inclusive
/// screen-rect gate (legacy fires at `MouseOperation.cpp:534`; a miss
/// leaves `MapX/MapY = -1`, `:529-530`).
#[must_use]
pub fn is_pt_in_map(x: i32, y: i32, screen: &ScreenRect) -> bool {
    screen.left <= x && x <= screen.right && screen.top <= y && y <= screen.bottom
}

/// `cltMapCommon::IsInMapArea` (`map.cpp:224-228`) — half-open tile bounds
/// (D14's named refusal; legacy's gate at `UnderMouseObject.cpp:335`).
#[must_use]
pub fn is_in_map_area(tile_x: i32, tile_y: i32, map_x: i32, map_y: i32) -> bool {
    tile_x >= 0 && tile_x < map_x && tile_y >= 0 && tile_y < map_y
}
