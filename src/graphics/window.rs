//! Step 7.2: the Win32 window, the message pump, and the on-screen demo
//! (`WO-7.2.md` §6.5, amended 2026-10-29: direct presentation - no
//! presenter trait, no headless recorder).
//!
//! Format of record:
//!
//! - **Window class** (`main.cpp:852-862`): `CS_VREDRAW | CS_HREDRAW |
//!   CS_DBLCLKS`, our `WindowProc`, arrow cursor, black stock brush
//!   (`GetStockObject(BLACK_BRUSH)`), no menu. The legacy class/title
//!   strings are locale-provided (`CommonHeader.h:86-87` are commented
//!   out); this module uses its own class name so it registers
//!   independently of any legacy build.
//! - **Window styles** (`main.cpp:866-949`): windowed =
//!   `WS_OVERLAPPED | WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX` at
//!   800 x 600 (`:880-885`); **P3-5 quirk kept**: Vista (6.0 workstation)
//!   and Win7 (6.1 workstation) shrink the outer size to 797 x 597
//!   (`:926-938`). The version probe keeps the legacy
//!   `GetVersionExA` shape including its `OSVERSIONINFOEX` ->
//!   `OSVERSIONINFO` fallback (`:895-918`); on modern, unmanifested
//!   Windows the probe reports 6.2, neither branch fires, and the 800 x
//!   600 path stands - exactly the legacy outcome.
//! - **Show** (`main.cpp:1006` fullscreen, `:1077-1078` windowed-in-server
//!   mode): `ShowWindow` + `UpdateWindow`. The demo shows its window
//!   unconditionally (its whole point, per the Custodian's directive).
//! - **`WindowProc`** (`mainfunction.cpp:116`): `WM_PAINT` =
//!   `BeginPaint`/`EndPaint` first, then a present request
//!   (`:1580-1596`); `WM_DESTROY` -> `PostQuitMessage(0)` (`:1598`,
//!   `:1656`); everything else falls through to `DefWindowProc` (`:1695`).
//! - **Message pump**: `Win32Queue` adapts the Win32 queue to
//!   [`super::tick::MessageQueue`] - one `peek + take + translate + dispatch` per
//!   drain iteration, `WM_QUIT` reported as [`TickMessage::Quit`]
//!   (`main.cpp:1551-1573`).
//! - **Present**: the per-tick render writes [`Framebuffer`] pixels and
//!   [`super::present::present_to_window`] mirrors them into a 16-bit DIB section
//!   selected into a memory DC and `BitBlt`s the [`PresentCrop`] into the
//!   client (module docs in [`super::present`], legacy `Graphic.cpp:266-310`).
//!
//! `unsafe` exists only here and in the Win32 half of [`super::present`] (WO §7,
//! R1): raw `extern "system"` declarations and their calls - zero new
//! crates. **No gate test constructs a window, DC, or DIB** (P3-3); the
//! runnable path is the `#[ignore]`-d operator test at the bottom, which
//! the human operator (PsiGinger / พี่แมว) runs to verify on screen.

// Win32 ABI names stay in their C spelling (struct tags like `WNDCLASSA`,
// `BITMAPINFOHEADER`); clippy's acronym lint would mangle them.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use super::blit;
use super::draw_map::{draw_map, Tileset};
use super::framebuffer::{ClipArea, Framebuffer};
use super::map::MapFile;
use super::palette::{RgbMode, TPal};
use super::present::{present_crop, present_to_window, PresentCrop};
use super::screen::{ScreenGeometry, ScreenMode};
use super::spr::{load_sprite, LoadedSprite};
use super::tick::{run_tick, MessageQueue, TickConfig, TickMessage};

/// In-tree real sprite: 80x40 tile from the legacy MapTileDiv
/// (7.1 fixture, `tests/asset_fixtures.rs` DoD 10.2).
const MASK_SPR: &[u8] = include_bytes!("../../GoonZu_Source/MapTileDiv/MapTileDiv/Mask.Spr");
/// In-tree real palette: 768 B = 256 x RGB (7.1 fixture).
const MONARCH_PAL: &[u8] =
    include_bytes!("../../GoonZu_Source/CharactorCombination/Pal/monarch.pal");

// ---- Win32 ABI shapes (Microsoft Win32, x64 layout; `#[repr(C)]`) ----

#[repr(C)]
#[derive(Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}

/// The window-procedure pointer type behind `WNDCLASSA.lpfnWndProc`
/// (`main.cpp:853`).
type WindowProcFn = extern "system" fn(isize, u32, usize, isize) -> isize;

#[repr(C)]
struct WNDCLASSA {
    style: u32,
    lpfnWndProc: WindowProcFn,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: isize,
    hIcon: isize,
    hCursor: isize,
    hbrBackground: isize,
    lpszMenuName: *const u8,
    lpszClassName: *const u8,
}

#[repr(C)]
struct MSG {
    hwnd: isize,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: POINT,
    lPrivate: u32,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: isize,
    fErase: i32,
    rcPaint: RECT,
    fRestore: i32,
    fIncUpdate: i32,
    rgbReserved: [u8; 32],
}

#[repr(C)]
struct OSVERSIONINFOEXA {
    dwOSVersionInfoSize: u32,
    dwMajorVersion: u32,
    dwMinorVersion: u32,
    dwBuildNumber: u32,
    dwPlatformId: u32,
    szCSDVersion: [u16; 128],
    wServicePackMajor: u16,
    wServicePackMinor: u16,
    wSuiteMask: u16,
    wProductType: u8,
    wReserved: u8,
}

#[repr(C)]
struct BITMAPINFOHEADER {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: i32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: i32,
    biClrImportant: i32,
}

/// `BITMAPINFO` with room for the three BI_BITFIELDS channel masks that
/// follow the header for 16-bpp surfaces.
#[repr(C)]
struct BITMAPINFO16 {
    header: BITMAPINFOHEADER,
    masks: [u32; 3],
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassA(lpwcx: *const WNDCLASSA) -> u16;
    fn CreateWindowExA(
        dwExStyle: u32,
        lpClassName: *const u8,
        lpWindowName: *const u8,
        dwStyle: u32,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: isize,
        hMenu: isize,
        hInstance: isize,
        lpParam: *mut c_void,
    ) -> isize;
    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: isize) -> i32;
    fn DefWindowProcA(hWnd: isize, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn BeginPaint(hWnd: isize, lpPaint: *mut PAINTSTRUCT) -> isize;
    fn EndPaint(hWnd: isize, lpPaint: *const PAINTSTRUCT) -> i32;
    fn PeekMessageA(
        lpMsg: *mut MSG,
        hWnd: isize,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageA(lpMsg: *const MSG) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn LoadCursorA(hInstance: isize, lpCursorName: *const u8) -> isize;
    fn GetVersionExA(lpVersionInformation: *mut OSVERSIONINFOEXA) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(i: i32) -> isize;
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn CreateDIBSection(
        hdc: isize,
        pbmi: *const BITMAPINFO16,
        iUsage: u32,
        ppvBits: *mut *mut u8,
        hSection: isize,
        dwOffset: u32,
    ) -> isize;
    fn SelectObject(hdc: isize, h: isize) -> isize;
    fn DeleteObject(ho: isize) -> i32;
    fn DeleteDC(hdc: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(lpModuleName: *const u8) -> isize;
    fn GetLastError() -> u32;
    fn Sleep(dwMilliseconds: u32);
}

#[link(name = "winmm")]
extern "system" {
    fn timeGetTime() -> u32;
}

// Win32 constants (Microsoft Win32; the game-visible styles carry their
// legacy citations in the module docs).
const CS_VREDRAW: u32 = 0x0001;
const CS_HREDRAW: u32 = 0x0002;
const CS_DBLCLKS: u32 = 0x0008;
const WS_OVERLAPPED: u32 = 0x0000_0000;
const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const SW_SHOWNORMAL: i32 = 1;
const PM_REMOVE: u32 = 0x0001;
const WM_PAINT: u32 = 0x000F;
const WM_DESTROY: u32 = 0x0002;
const WM_QUIT: u32 = 0x0012;
const BLACK_BRUSH: i32 = 4;
/// `MAKEINTRESOURCE(IDC_ARROW)` - the standard arrow cursor loaded with a
/// NULL instance (`main.cpp:858` loads it from `hInstance`; either works
/// for the stock cursor).
fn idc_arrow() -> *const u8 {
    32512usize as *const u8
}
const BI_BITFIELDS: u32 = 3;
const DIB_RGB_COLORS: u32 = 0;
const VER_PLATFORM_WIN32_NT: u32 = 2;
const VER_NT_WORKSTATION: u8 = 1;

/// The demo's `Tileset`: two diamond tiles (refs 0 and 1) built from the
/// real `Mask.Spr` canvas, forming the checkerboard; every other ref is an
/// absent slot (`mapdraw.cpp:900-901`).
struct DemoTileset {
    /// `[0]` = ref 0 (dark green), `[1]` = ref 1 (light green).
    tiles: [LoadedSprite; 2],
}

impl Tileset for DemoTileset {
    fn tile(&self, ref_index: u8, file: u8) -> Option<&LoadedSprite> {
        if file != 0 {
            return None;
        }
        self.tiles.get(usize::from(ref_index))
    }
}

/// Builds a demo checkerboard tile: the real fixture's decoded diamond
/// canvas under an **embedded** color table (the `TSPR_VERSION_16PAL`
/// v400 swap, `Graphic_Capsule.cpp:550-554` - `active_color_table` picks
/// the embedded table over the global palette). The fixture's only
/// literal palette index is 163 (see the 7.1 fixture walk in
/// `blit.rs`), so only that entry needs a color; the RGB888 -> RGB565
/// packing is 7.1's cited `palette::rgb_to_565` (`TSpr.cpp:188-191`),
/// never hand-packed here.
fn checker_tile(base: &LoadedSprite, diamond_rgb: (u8, u8, u8)) -> LoadedSprite {
    let mut table = [0u16; 256];
    table[163] = super::palette::rgb_to_565(diamond_rgb.0, diamond_rgb.1, diamond_rgb.2);
    LoadedSprite {
        header: base.header.clone(),
        image: base.image.clone(),
        embedded_palette: Some(table),
        bytes_consumed: base.bytes_consumed,
    }
}

/// Everything the window and the tick loop share - the demo's analogue of
/// the legacy globals (`g_graphics`, `lpScreenMapBuffer`).
struct WindowState {
    hwnd: isize,
    fb: Framebuffer,
    geometry: ScreenGeometry,
    crop: PresentCrop,
    /// Memory DC holding the DIB (legacy convert surface,
    /// `Graphic.cpp:298`); 0 until the present path is built.
    mem_dc: isize,
    /// `HBITMAP` of the 16-bpp section.
    dib: isize,
    /// Raw bits of the DIB, `cells` `u16` cells, top-down.
    dib_bits: *mut u16,
    cells: usize,
    /// Frame counter (one increment per rendered frame; kept for future
    /// demo motion and for diagnostics).
    frame: u64,
    /// Overlay sprite (second load of the same fixture - independent
    /// ownership keeps the borrow graph trivial).
    overlay: LoadedSprite,
    pal: TPal,
    map: MapFile,
    tiles: DemoTileset,
}

/// The single demo state, reachable from `WindowProc` - the direct
/// analogue of the legacy module-level globals. An `AtomicPtr (not a
/// `static mut`) so no `static_mut_refs` lint can fire.
static DEMO_STATE: AtomicPtr<WindowState> = AtomicPtr::new(std::ptr::null_mut());

/// The Win32 message source behind [`MessageQueue`]: one
/// `PeekMessage(PM_REMOVE)` + translate + dispatch per drain iteration,
/// `WM_QUIT` surfaced as [`TickMessage::Quit`] (`main.cpp:1551-1573`).
/// Like legacy, it pumps the whole thread queue (hwnd = NULL filter,
/// `main.cpp:1551`), so it carries no per-window state.
struct Win32Queue;

impl MessageQueue for Win32Queue {
    fn poll(&mut self) -> Option<TickMessage> {
        unsafe {
            let mut msg = std::mem::MaybeUninit::<MSG>::zeroed();
            if PeekMessageA(msg.as_mut_ptr(), 0, 0, 0, PM_REMOVE) == 0 {
                return None; // no message waiting (main.cpp:1551)
            }
            let msg = msg.assume_init();
            if msg.message == WM_QUIT {
                // GetMessage returned 0 (main.cpp:1553-1555).
                return Some(TickMessage::Quit);
            }
            // main.cpp:1559-1573 (no accelerator table in 7.2, so the
            // translate/dispatch pair is unconditional).
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
            Some(TickMessage::Other)
        }
    }
}

/// Builds one demo frame into `state.fb`: clear, overlay blit, tile-map
/// draw - then presents it (the per-tick render of the legacy
/// `GameDrawUpdateScreen`, `Draw.cpp:959`).
///
/// # Safety
///
/// `state_ptr` must point at the live [`WindowState`] owned by
/// [`run_demo`]; single-threaded use only (the same invariant legacy
/// keeps over its globals).
unsafe fn render_and_present(state_ptr: *mut WindowState) {
    let state = &mut *state_ptr;
    state.frame = state.frame.wrapping_add(1);

    // Backbuffer + clip = the legacy per-frame reset
    // (Client.cpp:5718: SetClipArea(0, 0, xsize-1, ysize-1)).
    let clip = ClipArea::new(0, 0, state.geometry.xsize - 1, state.geometry.ysize - 1);
    state.fb.clear(0x0000);

    // 1) Map first - legacy order `GameDraw()` runs `GameDrawMap()` before
    // any sprite/interface layer (Draw.cpp:925-940). Interior view: no
    // near-edge clear (mapdraw.cpp:233).
    let screen = state.geometry.screen_rect();
    draw_map(
        &mut state.fb,
        &clip,
        &state.map,
        &state.tiles,
        32,
        32,
        &state.geometry,
        &screen,
        &state.pal,
    );

    // 2) The real Mask.Spr over the map, static at the dead center of the
    // visible crop (operator directive: the sprite must be clearly
    // visible in the center of the screen). Center placement is computed
    // from the crop, not hand-placed: crop (80, 40) 800x600 ->
    // (80 + (800 - 80) / 2, 40 + (600 - 40) / 2) = (440, 320). The sprite
    // keeps the real monarch.pal table (index 163 = ochre 0xE4C9), which
    // contrasts against both checkerboard greens.
    let sw = state.overlay.image.width as i32;
    let sh = state.overlay.image.height as i32;
    let ox = state.crop.src_x + (state.crop.width - sw) / 2;
    let oy = state.crop.src_y + (state.crop.height - sh) / 2;
    let table = blit::active_color_table(&state.overlay, &state.pal);
    let _ = blit::put_spr(&mut state.fb, &clip, &state.overlay.image, ox, oy, table);

    present_now(state_ptr);
}

/// Mirrors + presents the current backbuffer if the present path exists.
///
/// # Safety
///
/// As [`render_and_present`]; additionally the present path must have
/// been built (or its handles are skipped).
unsafe fn present_now(state_ptr: *mut WindowState) {
    let state = &*state_ptr;
    if state.hwnd == 0 || state.mem_dc == 0 || state.dib_bits.is_null() {
        return;
    }
    present_to_window(
        state.hwnd,
        state.mem_dc,
        &state.fb.pixels,
        state.dib_bits,
        state.cells,
        &state.crop,
    );
}

/// The demo `WindowProc` (`mainfunction.cpp:116`).
extern "system" fn window_proc(hwnd: isize, msg: u32, w_param: usize, l_param: isize) -> isize {
    unsafe {
        match msg {
            WM_PAINT => {
                // Legacy order: paint first, then redraw/present
                // (mainfunction.cpp:1585-1594).
                let mut ps = std::mem::MaybeUninit::<PAINTSTRUCT>::zeroed();
                BeginPaint(hwnd, ps.as_mut_ptr());
                let ps = ps.assume_init();
                EndPaint(hwnd, &ps);
                let state = DEMO_STATE.load(Ordering::Relaxed);
                if !state.is_null() {
                    present_now(state);
                }
                0
            }
            WM_DESTROY => {
                // mainfunction.cpp:1598, PostQuitMessage(0) at :1656.
                PostQuitMessage(0);
                0
            }
            // mainfunction.cpp:1695 - DefWindowProc fallthrough.
            _ => DefWindowProcA(hwnd, msg, w_param, l_param),
        }
    }
}

/// Legacy windowed outer size: 800 x 600, shrunk to 797 x 597 on Vista /
/// Win7 workstation (`main.cpp:880-940`, quirk P3-5 - module docs).
fn windowed_outer_size() -> (i32, i32) {
    let mut width = 800; // main.cpp:884-885
    let mut height = 600;
    unsafe {
        // The legacy probe, fallback dance included (main.cpp:895-918):
        // try OSVERSIONINFOEX first, then plain OSVERSIONINFO.
        let mut osvi = std::mem::MaybeUninit::<OSVERSIONINFOEXA>::zeroed();
        {
            let p = osvi.as_mut_ptr();
            (*p).dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXA>() as u32;
        }
        let mut got = GetVersionExA(osvi.as_mut_ptr()) != 0;
        let mut major = 0u32;
        let mut minor = 0u32;
        let mut platform = 0u32;
        let mut product_type = 0u8;
        if got {
            let v = osvi.assume_init();
            major = v.dwMajorVersion;
            minor = v.dwMinorVersion;
            platform = v.dwPlatformId;
            product_type = v.wProductType;
        } else {
            // OSVERSIONINFO (the smaller struct) retry (:907-913): the
            // same leading DWORDs, no product type - the legacy code
            // still reads those.
            let mut small = std::mem::MaybeUninit::<OSVERSIONINFOEXA>::zeroed();
            {
                let p = small.as_mut_ptr();
                (*p).dwOSVersionInfoSize = (std::mem::size_of::<u32>() * 5 + 128 * 2) as u32;
            }
            got = GetVersionExA(small.as_mut_ptr()) != 0;
            if got {
                let v = small.assume_init();
                major = v.dwMajorVersion;
                minor = v.dwMinorVersion;
                platform = v.dwPlatformId;
            }
        }
        // main.cpp:921-939: Vista (6.0) / Win7 (6.1), NT, workstation.
        if got && platform == VER_PLATFORM_WIN32_NT {
            let workstation = product_type == VER_NT_WORKSTATION || product_type == 0;
            if workstation && major == 6 && (minor == 0 || minor == 1) {
                width = 797; // :930-937
                height = 597;
            }
        }
    }
    (width, height)
}

/// Constructs the demo scene state (pure Rust - no window, no DC): the
/// real fixture sprite, the real fixture palette, and a synthetic 64x64
/// map whose cells all point at ref 0 / file 0 (`Map-Layor.cpp:107,129`)
/// - `FakeTileset`-style construction, but here served by [`DemoTileset`].
fn build_scene() -> Result<WindowState, String> {
    let geometry = ScreenMode::Mode800x600.backbuffer(false); // windowed table
    let crop = present_crop(&geometry);
    let fb = Framebuffer::new(geometry.xsize as usize, geometry.ysize as usize)
        .map_err(|e| format!("backbuffer: {e}"))?;
    let overlay = load_sprite(MASK_SPR, RgbMode::Rgb565).map_err(|e| format!("Mask.Spr: {e}"))?;
    let diamond = load_sprite(MASK_SPR, RgbMode::Rgb565).map_err(|e| format!("Mask.Spr: {e}"))?;
    let pal = load_pal_checked()?;
    // Two checkerboard tiles: the same real diamond canvas under two
    // embedded color tables (dark/light green). The fixture diamond is a
    // single palette index everywhere (7.1 fixture walk in blit.rs), so
    // with one uniform color the tessellating diamonds fuse into a flat
    // field; alternating colors make every diamond edge legible.
    let tiles = DemoTileset {
        tiles: [
            checker_tile(&diamond, (34, 100, 34)),
            checker_tile(&diamond, (88, 160, 88)),
        ],
    };
    let cells = 64usize * 64usize;
    // Checkerboard `draw_info` cells: ref 0 on even (x + y), ref 1 on odd
    // (file 0, font 0, unique 0 - decomposition `Map-Layor.cpp:85-151`).
    let layer1_cells: Vec<u32> = (0..cells)
        .map(|i| {
            let ref_index = ((i % 64 + i / 64) % 2) as u32;
            ref_index << 16 // LOBYTE(HIWORD) = ref (Map-Layor.cpp:107)
        })
        .collect();
    let map = MapFile {
        header: super::map::CltMapHeader {
            version: 1,
            unique: 0,
            text: [0; 128],
            reserved0: 0,
            map_xsize: 64,
            map_ysize: 64,
            map_type: 0,
            tile_set1: 0,
            b_weather_switch: false,
            tile_set2: 0,
            reserved: [0; 19],
        },
        id_map: vec![0; cells],
        coll_map: vec![0; cells],
        layers: [
            super::map::MapLayer {
                draw_info: vec![0; cells],
                unique: vec![0; cells],
            },
            super::map::MapLayer {
                draw_info: layer1_cells,
                unique: vec![0; cells],
            },
        ],
        field_object_map: vec![0; cells],
        field_object_para: vec![0; cells],
        geo_info: None,
    };
    Ok(WindowState {
        hwnd: 0,
        fb,
        geometry,
        crop,
        mem_dc: 0,
        dib: 0,
        dib_bits: std::ptr::null_mut(),
        cells: 0,
        frame: 0,
        overlay,
        pal,
        map,
        tiles,
    })
}

/// `load_pal` with the demo's fixture bytes (kept separate so the error
/// message names the palette, not the sprite).
fn load_pal_checked() -> Result<TPal, String> {
    super::palette::load_pal(MONARCH_PAL, "monarch.pal", RgbMode::Rgb565)
        .map_err(|e| format!("monarch.pal: {e}"))
}

/// Runs the on-screen demo: registers the legacy-shaped window class,
/// creates the real game window, builds the 16-bpp present path, and
/// pumps [`run_tick`] until the window closes.
///
/// Returns an error string instead of panicking so the operator gets a
/// readable failure (P3-2 discipline extends to the demo seam).
pub fn run_demo() -> Result<(), String> {
    // Scene first (pure Rust: fixture sprite + palette + synthetic map) -
    // nothing window-shaped exists yet, so a scene failure surfaces as a
    // plain error string.
    let state = build_scene()?;
    let geometry = state.geometry;
    let state_ptr: *mut WindowState = Box::into_raw(Box::new(state));
    // Re-entry guard: one demo window at a time.
    let taken = DEMO_STATE.compare_exchange(
        std::ptr::null_mut(),
        state_ptr,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );
    if taken.is_err() {
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
        return Err("a demo window is already running".into());
    }

    let result = unsafe { run_demo_inner(state_ptr, geometry) };

    // Reclaim: clear the global first so a late WM_PAINT sees nothing.
    DEMO_STATE.store(std::ptr::null_mut(), Ordering::SeqCst);
    unsafe {
        let mut state = Box::from_raw(state_ptr);
        if state.mem_dc != 0 {
            if state.dib != 0 {
                SelectObject(state.mem_dc, 0);
                DeleteObject(state.dib);
            }
            DeleteDC(state.mem_dc);
        }
        state.hwnd = 0;
    }
    result
}

/// # Safety
///
/// `state_ptr` must be the fresh `Box::into_raw` of the demo state; this
/// function owns it until it returns.
unsafe fn run_demo_inner(
    state_ptr: *mut WindowState,
    geometry: ScreenGeometry,
) -> Result<(), String> {
    let instance = GetModuleHandleA(std::ptr::null());
    if instance == 0 {
        return Err(format!("GetModuleHandleA failed ({})", GetLastError()));
    }

    // 16-bpp BI_BITFIELDS DIB in RGB565 - the masks are 7.1's published
    // constants (`palette::RGB16_565_*`), so the surface format is the
    // palette format by construction.
    let bmi = BITMAPINFO16 {
        header: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: geometry.xsize,
            // Negative height: top-down, matching the framebuffer's
            // row-major order exactly.
            biHeight: -geometry.ysize,
            biPlanes: 1,
            biBitCount: 16,
            biCompression: BI_BITFIELDS,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        masks: [
            super::palette::RGB16_565_RMASK as u32,
            super::palette::RGB16_565_GMASK as u32,
            super::palette::RGB16_565_BMASK as u32,
        ],
    };
    let mem_dc = CreateCompatibleDC(0);
    if mem_dc == 0 {
        return Err(format!("CreateCompatibleDC failed ({})", GetLastError()));
    }
    let mut bits: *mut u8 = std::ptr::null_mut();
    let dib = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, 0, 0);
    if dib == 0 || bits.is_null() {
        DeleteDC(mem_dc);
        return Err(format!("CreateDIBSection failed ({})", GetLastError()));
    }
    SelectObject(mem_dc, dib);
    {
        let state = &mut *state_ptr;
        state.mem_dc = mem_dc;
        state.dib = dib;
        state.dib_bits = bits as *mut u16;
        state.cells = geometry.xsize as usize * geometry.ysize as usize;
    }

    // Window class (main.cpp:852-862).
    let class = b"GoonZuRustWindow72\0";
    let wc = WNDCLASSA {
        style: CS_VREDRAW | CS_HREDRAW | CS_DBLCLKS, // :852
        lpfnWndProc: window_proc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: 0, // no resource icon in 7.2 (legacy loads IDI_ICON_GOONZU)
        hCursor: LoadCursorA(0, idc_arrow()), // :858
        hbrBackground: GetStockObject(BLACK_BRUSH), // :859
        lpszMenuName: std::ptr::null(),
        lpszClassName: class.as_ptr(),
    };
    // A leftover class from a previous in-process run (registration is
    // idempotent for the identical definition) is fine; any real problem
    // surfaces in CreateWindowExA below with its own error code.
    let _ = RegisterClassA(&wc);

    // Style + outer size (main.cpp:878-949).
    let style = WS_OVERLAPPED | WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX; // :880
    let (width, height) = windowed_outer_size();
    let hwnd = CreateWindowExA(
        WS_EX_APPWINDOW, // :955
        class.as_ptr(),
        c"GoonZu Rust Client - Step 7.2".as_ptr() as *const u8,
        style,
        0,
        0,
        width,
        height,
        0,
        0,
        instance,
        std::ptr::null_mut(),
    );
    if hwnd == 0 {
        let err = GetLastError();
        DeleteObject(dib);
        DeleteDC(mem_dc);
        return Err(format!("CreateWindowExA failed ({err})"));
    }
    (*state_ptr).hwnd = hwnd;

    // Show it (main.cpp:1077-1078 - legacy guards this with
    // gamemode == 1; the demo's purpose is to be visible, per the
    // Custodian directive 2026-10-29).
    ShowWindow(hwnd, SW_SHOWNORMAL);
    UpdateWindow(hwnd);

    // First frame so the window has content before any input.
    render_and_present(state_ptr);

    // The tick loop (main.cpp:1544-1636). The demo borrows the
    // legacy idle-sleep pacing branch (`gamemode == 1`, :1579) so the
    // operator sees a steady ~10 fps window instead of a busy loop; the
    // rule itself is ported verbatim either way (P3-5, tick.rs docs).
    let config = TickConfig { server_mode: true };
    let mut queue = Win32Queue;
    loop {
        let outcome = run_tick(
            &mut queue,
            &config,
            false,
            &mut |ms: u32| unsafe { Sleep(ms) },
            &mut || unsafe { u64::from(timeGetTime()) },
            &mut |_now: u64| {},
            &mut |_now: u64, _draw_skip: bool| {
                render_and_present(state_ptr);
            },
        );
        if outcome.quit {
            break;
        }
    }

    (*state_ptr).hwnd = 0;
    Ok(())
}

/// Operator verification entry (`WO-7.2.md` §6.5, amended): opens the
/// real game window on the desktop and runs the 7.2 render loop until it
/// is closed.
///
/// `#[ignore]`d so it is excluded from every gate count and never runs
/// during CI-style `cargo test` (P3-3): the human operator (PsiGinger /
/// พี่แมว) launches it on screen with
/// `cargo test --release -- --ignored operator_game_window_opens_on_screen`
/// and verifies the window visually.
#[test]
#[ignore = "opens a real window on screen - operator verification only (P3-3)"]
fn operator_game_window_opens_on_screen() {
    run_demo().expect("the demo window must open, render, and close cleanly");
}
