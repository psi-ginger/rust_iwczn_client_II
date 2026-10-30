# Work Order - Step 7.2: Windowing & 2D Render Loop (window, backbuffer, present path, tick loop)

## 1. Provenance, lane hats, baseline

Filed 2026-10-30 by the Implementer session (`psi-ginger`) under the Custodian's Step 7.2
directive ("draft WO-7.2 … scope, API boundaries, acceptance criteria for integrating the 7.1
asset decoders into a game window/viewport and tick loop"). Scope and acceptance below are
governed by `CONTRIBUTING_PHASE3.md` (Step 7.2 row, `PENDING`); this work order does not amend
it. Governance hats: **Implementer** authors code in `src/graphics/**` (P3-6 single-writer
guard); the **Verifier** writes matrix status and `evidence/7.2/` (S3); the **Adversary** is
mandatory for the byte-exactness claims below (§8).

- **Baseline:** branch `step/7.2-windowing`, cut from `origin/step/7.1-assets` @ `1601b26`
  (delivery `2c0964e` + delivery note). **Not cut from `main`**: Step 7.1's code
  (`src/graphics/**`) is not merged to `main` yet (`main` @ `60d38dd` carries the sign-off docs
  only; merge-base `2602e13`). All five gates re-run exit-0 this session at `1601b26`; tests
  **264 lib + 11 asset_fixtures + 14 e2e = 289 passed**, 4 `live_integration` ignored.
- **Input artifact:** the Step 7.1 decoder API (`spr::{DecodedSprite, LoadedSprite, load_sprite,
  COLOR_TRANS}`, `palette::{TPal, rgb_to_565, RgbMode}`, `map::{parse_map, MapData}`,
  `archive::{AssetSource, resolve}`) — consumed read-only, not modified (P3-6: 7.2 owns new
  files in `src/graphics/**` only; `spr.rs`/`palette.rs`/`map.rs`/`archive.rs` stay 7.1's).
- **Legacy format of record:** `GoonZu_Source/` windowing + present + tick + tile-draw path,
  every load-bearing line cited in §6.

## 2. Scope (verbatim authority: `CONTRIBUTING_PHASE3.md` Step 7.2)

> "Windowing & 2D Render Loop — window creation, backbuffer, present path"

Concretely, 7.2 delivers a **headless-deterministic 2D render loop library** inside
`src/graphics/**` that composes the 7.1 decoders into frames, plus a **Win32 window/present
backend compiled but never exercised by gates** (P3-3):

1. **Backbuffer** — an RGB565 software framebuffer with the legacy pitch semantics, clip area,
   clear/fill primitives (port of `NGraphics` surface ops).
2. **Blit kernels** — the sprite (`PutSpr16T`) and tile (`PutMpr16T`) color-key blits, fed from
   7.1's `DecodedSprite` + palette, with the legacy clip-area dispatch (`PutSpr` full vs clipped).
3. **Viewport / screen geometry** — the four screen modes with their exact backbuffer sizes and
   half-sizes, `ScreenRect`, the diamond-isometric tile center transform, and the present
   source-rect crop.
4. **Tile-map draw** — the full-redraw `DrawMap` loop (tile window `± siClientScreenHalfSize`,
   layer-1-unless-`unique==90` selection, per-tile clip gate) as a pure function from
   `map::parse_map` output + a tileset table.
5. **Present seam** — a `Presenter` trait: a **headless presenter** (records presented frames —
   the only implementation gate tests may touch) and a **Win32 presenter** (client-rect
   `BitBlt` path) plus a minimal window-creation module, both `#[cfg(windows)]`, behind the
   same seam.
6. **Tick loop** — the legacy `main.cpp` loop shape: bounded message drain (≤10 per tick),
   clock read, network hook, game-update hook, `Sleep(10)` **only** when idle in game mode —
   with injected clock/sleeper so the whole loop is deterministic in tests.

## 3. Resolutions (legacy quirks and environment facts - state, do not "fix")

- **R1 - zero new crates for the Win32 path.** The window/present backend uses raw
  `extern "system"` declarations for the ~15 user32/gdi32 entry points it needs
  (`RegisterClassExW`, `CreateWindowExW`, `DefWindowProcW`, `PeekMessageW`, `TranslateMessage`,
  `DispatchMessageW`, `ShowWindow`, `GetDC`, `ReleaseDC`, `BitBlt`, `IsIconic`, `ClientToScreen`,
  `GetClientRect`, `BeginPaint`/`EndPaint`, `PostQuitMessage`), gated `#[cfg(windows)]`.
  Rationale: `Cargo.toml` sits outside the P3-6 write guard, and Step 1.1's dependency policy
  ("minimal, high-performance dependencies only") is the repo norm — a windowing crate for one
  present path does not earn its place. Declaring `windows-sys` is a Custodian decision, not an
  Implementer one; this work order assumes none.
- **R2 - present is a trait, headless is the default.** Every gate test presents through a
  headless recorder. The Win32 presenter and window creation compile under `cargo check`/
  `clippy` (they are code, gates cover them) but no gate test ever constructs a window, a DC,
  or a display surface (P3-3, inherited verbatim: "no GPU/window/display/network in any gate
  test"). A live windowed smoke path, if delivered, is `#[ignore]`d like the Step 5.x
  `live_integration` tests and is never part of a gate.
- **R3 - blit input is the decoded canvas, not a re-walk of the RLE stream.** 7.1's
  `DecodedSprite::pixels: Vec<Option<u8>>` is already the byte-exact result of the legacy
  `PutSpr16T`/`PutMpr16T` walk (`Graphics_PutSpr16T.cpp:15-48`,
  `Graphics_PutMpr16.cpp:5-53`, proven against fixtures in 7.1). The 7.2 kernel therefore maps
  `Some(idx)` → `pdest = palette[idx]` with the legacy per-row pitch advance, and `None` →
  transparent skip. This is observationally identical to re-walking the stream: the legacy
  kernels' only side effect is exactly that write/skip. Stated, per P3-5, not silently.
- **R4 - color key stays 254.** `COLOR_TRANS = 254` (`spr.rs`, legacy
  `Graphic_Capsule.h:21` region) is the transparency sentinel; palette resolution reuses 7.1's
  `rgb_to_565`. Embedded v400 palettes (`LoadedSprite::embedded_palette`) swap the active color
  table for that sprite only — the legacy `PutSpr` v400 swap-in/swap-out
  (`Graphic_Capsule.cpp:101-105,130-132`).
- **R5 - the clock is injected.** Legacy reads `CurrentClock = timeGetTime()`
  (`main.cpp:1625`). The Rust tick loop takes a millisecond clock (`FnMut() -> u32`) and a
  sleeper (`FnMut(u32)`); tests feed a fixed/stepping clock, the live path feeds
  `Instant`-derived milliseconds. No wall-clock reads in library code.
- **R6 - the 797×597 quirk is preserved, not fixed.** Windowed client size is 800×600
  (`main.cpp:884-885`), except the legacy Vista/Win7 branch shrinks it by 3 px to 797×597
  (`main.cpp:926-938`). The Rust window module models this as an explicit variant/flag with the
  citation; the default is 800×600 and the 797×597 variant is selectable in tests. We do not
  replicate `GetVersionEx` OS sniffing (it is deprecated and returns lies on modern Windows —
  stated as a quirk, P3-5).
- **R7 - frame skip is honored.** Legacy `GsGame` skips `GameDraw` when `bDrawSkipSwitch`
  (`Client-WinMain.cpp:211-214`) and `PutMpr16T` early-outs on `m_bSkipDraw`
  (`Graphics_PutMpr16.cpp:7`). The Rust loop exposes a draw-skip flag with the same
  semantics: process updates still run, the frame is not rendered/presented.
- **R8 - row 7.1 reads `PENDING` on this branch.** True on `origin/step/7.1-assets`; it is
  `PASSED` on `main` @ `60d38dd`. Matrix status is Verifier-only (S3) — the Implementer does
  not touch `TASK_MATRIX.md`.

## 4. Non-goals

- **No binary wiring.** `src/main.rs` is outside the P3-6 guard; 7.2 delivers the windowing/
  loop **library API** only. A windowed playable binary is a later Custodian-authorized step.
- **No networking in the loop.** The tick loop calls an injected "network hook"; the actual
  `client::` integration is not 7.2's (and live sockets stay out of gate tests, P3-3).
- **No character/object/interface drawing.** `DrawObj`, UI widgets, text/fonts, effects
  (`PutSprAlpha`/`Multiply`/`Light` family, `Graphic_Capsule.cpp:231+`) — future steps.
- **No scroll-optimized partial redraw.** The legacy `Scroll_North/East/…` line-blit fast paths
  (`mapdraw.cpp:940+`) are not ported; 7.2 delivers the full-redraw path only, which is the
  format of record for output bytes.
- **No fullscreen mode, no vsync wait, no web-dialog present variants**
  (`Graphic.cpp:305-308`, `Draw.cpp:1152-1154` params unused).
- **No DirectDraw.** The legacy `lpDDSPrimary->Blt` chain exists only to move pixels to the
  screen; the Rust Win32 present path goes straight to `BitBlt` into the client DC, which is
  what the legacy windowed path effectively does via its convert surface
  (`Graphic.cpp:298-310`). Stated as a deliberate structural simplification (P3-5).

## 5. File plan (ownership: `CONTRIBUTING_PHASE3.md` P3-6)

| File | Role | Status |
|---|---|---|
| `src/graphics/framebuffer.rs` | RGB565 backbuffer, clip area, clear/fill | new |
| `src/graphics/blit.rs` | `put_spr` / `put_mpr` color-key kernels, palette application | new |
| `src/graphics/screen.rs` | screen modes, viewport geometry, diamond transform, present crop | new |
| `src/graphics/draw_map.rs` | `DrawMap` full-redraw tile loop over `map::MapData` + tileset table | new |
| `src/graphics/present.rs` | pure present-crop math + `#[cfg(windows)]` direct Win32 `BitBlt` present path | new (AMENDED 2026-10-29, Directive Adjustment — §9) |
| `src/graphics/window.rs` | `#[cfg(windows)]` minimal Win32 window creation + message pump (R1) | new |
| `src/graphics/tick.rs` | tick loop: drain bound, clock/sleeper hooks, draw-skip, hooks | new |
| `src/graphics/mod.rs` | declare + re-export the new submodules | edit (7.2's guard area) |

Everything else — `src/**` outside `src/graphics/`, `tests/`, `Cargo.toml`, `CONTRIBUTING*`,
`TASK_MATRIX.md`, `docs/agents/WO-7.1.md`, `GoonZu_Source/` — is **READ-ONLY** for the
Implementer. Tests live in `tests/<name>.rs` integration files or `#[cfg(test)]` modules inside
the new files; whichever way, they are 7.2's new test files (in-guard).

## 6. Design (format of record - every constant cited from `GoonZu_Source/`)

### 6.1 Backbuffer - `framebuffer.rs`

- `Framebuffer { width: usize, height: usize, pixels: Vec<u16> }` — RGB565, row-major, 2
  bytes/pixel. The legacy `lPitch = m_lPitch >> 1` (`Graphics_PutSpr16T.cpp:15`,
  `Graphics_PutMpr16.cpp:14`) converts byte pitch to **pixel** pitch; our pixel pitch is
  simply `width`. That equivalence is the R3 foundation and is asserted by a test with the
  citation.
- `ClipArea { x1, y1, x2, y2 }` inclusive, mirroring `SetClipArea`/`GetClipArea`/
  `IsInClipArea`/`FitToClipArea` (`Graphic.h:157-165`); default = full screen
  (`SetClipFullScreen`, `Graphic.h:161`).
- `clear(color)`, `fill_box(x1,y1,x2,y2,color)` (legacy `FillBox`,
  `Graphic_Capsule.cpp:852`), `set_pixel`/`get_pixel` (test-facing), and the out-of-bounds
  contract: blits never write outside `0..width × 0..height` — enforced by construction
  (per-pixel bounds check in the clipped kernel), no panics (P3-2 typed refusal: a `BlitError`
  for a degenerate/negative geometry input, never a slice-index panic).

### 6.2 Blit kernels - `blit.rs`

- `put_spr(fb, sprite: &LoadedSprite, x: i32, y: i32, palette: &Palette)` — port of
  `cltGraphic_Capsule::PutSpr` (`Graphic_Capsule.cpp:99-133`): if the whole
  `x..x+w-1 × y..y+h-1` rect is inside the clip area (`IsInClipArea`, `:112`) use the fast
  unclipped write; else the clipped variant (`PutSpr16CT`,
  `Graphics_PutSpr16T.cpp:280`) writes only in-bounds pixels. v400 embedded-palette swap per
  R4. Reverse (`bReverse`) and rotated/scaled variants are out of scope (§4).
- `put_mpr(fb, sprite: &LoadedSprite, x: i32, y: i32, palette: &Palette)` — tile blit,
  same canvas mapping (R3); the legacy `m_bSkipDraw` guard (`Graphics_PutMpr16.cpp:7`) is
  modeled by the draw-skip flag (R7), not inside the kernel.
- Both kernels consume 7.1 `DecodedSprite::pixels` + a `&[u16; 256]` color table resolved from
  `palette::TPal` via `rgb_to_565` (or `embedded_palette` for v400). **The kernels never see
  encoded sprite bytes** — that is 7.1's contract, already byte-proven.

### 6.3 Screen geometry - `screen.rs`

Legacy table, `Client.cpp:3197-3251` (`CreateDirectDraw`), all sizes `80*(n+2)` × `40*(n+2)`:

| mode | fullscreen backbuffer | windowed backbuffer | half-size |
|---|---|---|---|
| 800×600 | 960×680 (`:3206-3207`) | same | 14 (`:3208`) |
| 1024×768 | 1200×880 (`:3220-3221`) | 1120×800 (`:3225-3226`) | 18 (`:3222,3227`) |
| 1024×600 | 1200×680 (`:3237-3238`) | 1120×680 (`:3242-3243`) | 16 (`:3239,3244`) |
| 1280×1024 | 1440×1000 (`:3249-3250`) | same | 20 (`:3251`) |

- `ScreenRect = (0, 0, xsize-1, ysize-1)` (`Client.cpp:5713-5718`); tile constants 80/40/40/20
  (`map.h:33-36`).
- `draw_center_xy(virtual_x, virtual_y)` — verbatim port of `GetDrawCenterXY`
  (`mapdraw.cpp:36-52`): the diamond offsets around the screen center, using
  `MAP_TILE_XSIZE_HALF`/`MAP_TILE_YSIZE_HALF`.
- **Present crop** — legacy presents the sub-rect `src = (MAP_TILE_XSIZE + GetScreenRX(),
  MAP_TILE_YSIZE + GetScreenRY())` with draw size `(xsize - 2*MAP_TILE_XSIZE,
  ysize - 2*MAP_TILE_YSIZE)` (`Draw.cpp:1145-1154`, the one-tile border each side). The Rust
  present seam exposes exactly these numbers as pure geometry, asserted directly by unit tests
  (AMENDED 2026-10-29 — no headless recorder, §9).
- The inverse transform asymmetry (`TransFromScreenToRealXY` adds `MAP_TILE_YSIZE_HALF`,
  `Map-Client.cpp:187`) is mouse-picking territory — not needed in 7.2, noted for the future
  input step.

### 6.4 Tile-map draw - `draw_map.rs`

Port of `cltMapClient::DrawMap` full redraw (`mapdraw.cpp:231-257`):

- Iterate `virtual_y in vy-half .. vy+half`, `virtual_x in vx-half .. vx+half`
  (`:240-241`), skip out-of-map coords (`IsInMapArea`, `:244`), compute the tile center via
  §6.3 (`:247`), then draw **layer 1 unless `GetDrawInfo_Unique(x,y) == 90`, which draws
  layer 0** (`:249-256`). `unique == 90` is a legacy magic number — preserved verbatim,
  named as a constant with its citation (P3-5).
- `draw_layer` (`mapdraw.cpp:881-911`): bounds-check the tileset ref against
  `MAX_TILESET_NUMBER = 255` (`TileSet.h:12`, `:897`), look up the tile sprite, `put_mpr` at
  `center - half_tile` (`:890-891`).
- The near-edge full-screen `FillBox` (`:233-237`) is part of the port (it clears stale edge
  tiles when the view window is clipped by the map border).
- Inputs: a `&map::MapData` (7.1) + a `&dyn Tileset` trait (`tile(layer, x, y) ->
  Option<&LoadedSprite>`) so tests inject synthetic tiles and no file IO exists in the draw
  path. Tileset *loading* from archives is a later step (archive seam is 7.1's).

### 6.5 Present seam - `present.rs` / window - `window.rs`

**AMENDED 2026-10-29 (Custodian Directive Adjustment, recorded in §9):** the `Presenter`
trait and the `HeadlessPresenter` recorder are removed. The backbuffer is wired directly into
the real Win32 window — running the client visibly pops up the actual game window on the
desktop. Gate tests stay on the pure math (crop geometry, framebuffer, kernels, map loop,
tick drain); the Win32 present path is verified interactively by the operator (PsiGinger /
พี่แมว) on screen.

- `present.rs` — pure `PresentCrop` math (src origin `(MAP_TILE_XSIZE + RX, MAP_TILE_YSIZE +
  RY)`, size `(xsize - 2*MAP_TILE_XSIZE, ysize - 2*MAP_TILE_YSIZE)`, `Draw.cpp:1145-1154`),
  unit-tested with no window involved. `#[cfg(windows)] present(hwnd, fb, crop)` — legacy
  windowed present (`Graphic.cpp:266-310`): `IsIconic` early-out (`:263`), `GetClientRect` +
  `ClientToScreen` (`:272-276`), `GetDC` → `BitBlt SRCCOPY` of the crop (`:298-302`) →
  `ReleaseDC`. No convert surface, no vsync wait (§4). The only `unsafe` here is the Win32
  extern glue (R1).
- `#[cfg(windows)] window.rs` — `WNDCLASS` shape (`main.cpp:852-862`: `CS_VREDRAW|CS_HREDRAW|
  CS_DBLCLKS`, black background brush, our `WindowProc`), windowed style
  `WS_OVERLAPPED|WS_SYSMENU|WS_CAPTION|WS_MINIMIZEBOX` at 800×600 (`:880-885`, R6 for the
  797×597 variant), `CreateWindowEx` + `ShowWindow` (`:952-965,:1006`). `WindowProc`
  (`mainfunction.cpp:116`) handles `WM_PAINT` as legacy does — `BeginPaint`/`EndPaint` only,
  then a present request (`:1580-1596`) — `WM_DESTROY` → `PostQuitMessage` (`:1598+`), and
  `DefWindowProc` fallthrough (`:1695`). The pump mirrors the legacy drain one-for-one.
  Also hosts `pub fn run_demo()` — the runnable on-screen scene (clear + real-fixture
  `Mask.Spr` blit + synthetic tile-map draw, pumped through `tick.rs`) that the operator
  launches via the `#[ignore]`-d `operator_game_window_opens_on_screen` test (`#[ignore]`d
  tests count toward no reported number, §8). **No gate test calls any of this** (R2/P3-3).

### 6.6 Tick loop - `tick.rs`

Port of the `main.cpp` main loop (`:1544-1636`), made injectable (R5):

- Per iteration: drain up to **10** messages (`for i in 0..10`, `:1549`), each
  `PeekMessage`-gated `GetMessage`/`TranslateMessage`/`DispatchMessage` (`:1551-1572`); if the
  queue was empty **and** game mode is active, `sleeper(10)` (`:1579`, `if(gamemode == 1)
  Sleep(10)`); then `clock()` → `current_clock` (`:1625`), `network_hook()` (`:1628`),
  `game_hook()` (`:1630`). Quit-message handling ends the loop (`:1553-1556`).
- `game_hook` receives the draw-skip flag (R7): when set, update runs, render+present do not
  (`Client-WinMain.cpp:211-214`).
- The message source is a trait (`MessageQueue`: `peek() -> Option<Message>`), so tests script
  exact sequences (quit on message 3, 15 queued messages → assert exactly 10 drained per tick,
  sleeper called only on empty+game-mode, etc.). No Win32 types cross this seam —
  `window.rs` adapts the real pump to it.

## 7. Forbidden shortcuts

- No `unsafe` outside `window.rs`/`present.rs` Win32 glue (R1's extern block + calls), and no
  `unsafe` at all in `framebuffer/blit/screen/draw_map/tick`.
- No gate test may construct a window, DC, or any Win32 object (P3-3; AMENDED 2026-10-29:
  there is no headless presenter — window/present code compiles under `check`/`clippy` but is
  exercised only by the operator on screen, via the `#[ignore]`-d demo). No `#[ignore]`-d test
  counts toward any reported number.
- No new dependency (`Cargo.toml` is out of guard, R1).
- No expectation in any test derived from the code under test (F-07): expected pixels, crop
  rects, drain counts, and geometry numbers are either transcribed from the cited legacy
  lines (with the citation next to the assertion) or hand-computed from them in the test
  comment (precedent: 7.1's independent `Mask.Spr` byte-walk, Step 6.3's `make_npc_record`).
- No "fixing" the quirks: `unique == 90`, the 797×597 variant, the `Sleep(10)`-when-idle-only
  rule, the one-tile present crop, and the layer-1 default stay as cited (P3-5).
- No partial results on malformed input (P3-2): degenerate geometry returns `BlitError`/
  `PresentError`, never a panic, never a silently clipped-in-half blit.
- Forward-only (no force-push, no history rewrite); commits via temp file + `git commit -F`.

## 8. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- Five gates exit-0 in the clean clone: `check`, `clippy -D warnings`, `fmt --check`, `test`,
  `doc --no-deps`; `evidence/7.2/{head,env,check,clippy,fmt,test,doc}.log` + `SHA256SUMS`
  (manifest over committed bytes, LF, forward-slash paths — precedent `evidence/7.1`).
- Test count copied from the `test.result:` log lines, no handwritten numbers.
- **Adversary mutation probe (mandatory - byte-exact/negative claims):** at minimum
  (a) flip the pitch advance in `put_spr` (write row+1 instead of row+pitch) — a named blit
  test must fail; (b) change `COLOR_TRANS` handling so `None` pixels write palette index 254 —
  a named blit/transparent test must fail; (c) swap the `unique == 90` layer selection — a
  named `draw_map` test must fail; (d) move the present crop by one tile — a named present test
  must fail; (e) drain bound 10 → 11 — a named tick test must fail. Each probe: exit-101 with
  the named test in output, recorded in `evidence/7.2/mutation-probes.md`.
- The frame-determinism claim (AMENDED 2026-10-29: no headless presenter exists): a test
  renders the same synthetic scene into two fresh `Framebuffer`s and asserts byte-identical
  results; plus a frame built from a real 7.1 fixture
  (`Mask.Spr` via `load_sprite`) asserting specific in-frame pixels against a hand-walked
  expectation (F-07: the walk is written from the fixture bytes + legacy kernel semantics, not
  from Rust output).

## 9. Return blocks (append; do not rewrite the sections above)

### Directive adjustment record - Implementer (2026-10-29)

Custodian directive, verbatim intent: skip the headless `Presenter`/recorder abstraction;
wire the backbuffer **directly** into the Win32 presenter/window so running the client
visibly pops up the real game window on the desktop; keep unit tests on the core
math/framebuffer logic (no synthetic headless-recording harnesses); the human operator
(PsiGinger / พี่แมว) performs interactive visual verification on screen.

Applied amendments (each marked inline above): §5 `present.rs` role; §6.3 present-crop test
claim; §6.5 rewritten (no `Presenter` trait, no `HeadlessPresenter`); §7 presenter bullet;
§8 determinism-claim wording; §10 DoD point 3. Everything else in this work order stands
unchanged: P3-1 citations, P3-2 typed refusal, P3-3 (gate tests remain window-free — the
window code compiles under `check`/`clippy` and is exercised only on screen by the
operator via the `#[ignore]`-d demo test), P3-5 quirks, P3-6 write guard, P3-7 second
source, the §8 mutation probes (probe (d) lands on the pure crop-math test), and S4 evidence.

### Delivery note - Implementer (2026-10-29)

Branch `step/7.2-windowing`, cut from `origin/step/7.1-assets` @ `1601b26`.

**Delivered files** (§5, P3-6 honored): new `src/graphics/{framebuffer,blit,screen,
present,draw_map,tick,window}.rs`; edited `src/graphics/mod.rs` only. Nothing outside
`src/graphics/**` + `evidence/7.2/` + this `docs/agents/WO-7.2.md` was written;
`git diff` against the 7.1 delivery shows `spr.rs`/`palette.rs`/`map.rs`/`archive.rs`
byte-identical (their 7.1 tests pass unchanged), and `git status` shows no uncommitted
path outside the guard. `Cargo.toml`, `tests/`, `main.rs`, `TASK_MATRIX.md` untouched —
zero new crates, raw `extern "system"` FFI for user32/gdi32/kernel32/winmm (R1).

**Gates at this SHA** (counts copied verbatim from `test result:` lines):

```
test result: ok. 299 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, `cargo test`, `cargo doc --no-deps` — all exit-0. The 1 ignored
lib test is `operator_game_window_opens_on_screen` (counts toward no reported number);
the 4 ignored `live_integration` tests are the 7.1 baseline, unchanged.

**Adversary probes**: all five run, each exit-101 with the WO-named test, each source
restored and the suite re-run green afterwards — recorded in
`evidence/7.2/mutation-probes.md`.

**Deviations / disclosures:**

1. **Blit kernel signature narrows the WO §6.2 prose** — `put_spr`/`put_mpr` take
   `&DecodedSprite` (Step 7.1's decoded canvas) + an explicit `&[u16; 256]` color
   table rather than a sprite object that resolves its own table. Rationale: 7.1's
   decode already normalizes SPR/MPR walks into one canvas shape (R3), so the kernel
   is canvas-only; the v400 embedded-palette swap decision lives in capsule-level
   `active_color_table(&LoadedSprite, &TPal)` (`Graphic_Capsule.cpp:550-554`), unit-
   tested. Observable per-pixel results are unchanged.
2. **Present seam is direct, per the Directive Adjustment** — no `Presenter` trait, no
   `HeadlessPresenter`; `present.rs` = pure `PresentCrop` math (unit-tested) +
   `#[cfg(windows)] present_to_window` doing one backbuffer→DIB memcpy then one
   `BitBlt SRCCOPY` of the crop. The legacy convert-surface hop and vsync wait are
   dropped per §4/§6.5 amendment (structural simplification, stated not hidden).
3. **Demo pacing**: `run_demo()` runs `TickConfig { server_mode: true }` so the
   verbatim legacy idle branch (`gamemode == 1` → `Sleep(10)`, `main.cpp:1579`) paces
   the window at ~10 fps instead of a busy spin. The rule itself is ported exactly as
   cited (a `server_mode: false` config never sleeps — tested); only the demo's choice
   of config is a presentation decision. Stated here per P3-5.
4. **`ShowWindow` is unconditional in the demo** — legacy calls it at `main.cpp:1077`
   only under `gamemode == 1`; the demo's purpose is to be visible on screen
   (Custodian directive), so it always shows. Window class/style/version quirk
   (797×597 on Vista/Win7, `main.cpp:926-938`) are verbatim legacy, including the
   `OSVERSIONINFOEX`→`OSVERSIONINFO` fallback (`:895-918`).
5. **Fixture hand-walk correction (F-07)**: `Mask.Spr`'s RLE stream starts at
   `TSPR_HEADER_LEN` = 7·4 + 8·4 + 4 + 256·12 = **3136** (`spr.rs:93`); row 0 is
   `FE 27` (39 transparent) + `A3 A3` (idx 163) + `FE 27` (39 transparent) = exactly
   80 cells — a symmetric diamond-mask row. The walk and its assertions are written
   from those fixture bytes + `Graphics_PutSpr16T.cpp:28-48` semantics, not from Rust
   output; `monarch.pal` entry 163 = RGB(57,38,19) → `0xE4C9` hand-packed per
   `rgb_to_565`.
6. **`ref >= 255` vs `ref == 255`**: clippy `absurd_extreme_comparisons` fires
   because our `ref_index` is `u8` (legacy's is `SHORT`, `mapdraw.cpp:897`).
   Since legacy reads `LOBYTE(HIWORD(draw_info))` (`Map-Layor.cpp:107`) — never
   negative — `>= 255` and `== 255` are the same predicate; code uses `==` with the
   legacy `>=` form cited at the line. Ref 255 still refuses before the tileset
   lookup (tested).

**For the operator (PsiGinger / พี่แมว)**: on-screen verification command is
`cargo test --release -- --ignored operator_game_window_opens_on_screen` — it opens
the real 800×600 (797×597 on Vista/Win7) game window with the legacy class styles,
draws the real `Mask.Spr` fixture over a synthetic 64×64 tile-map diamond, and pumps
the legacy tick loop until the window closes. No gate test constructs any Win32
object (P3-3).

### Operator-feedback fix record - Implementer (2026-10-30)

Operator report (PsiGinger / พี่แมว): the demo window opened at 800x600 but the whole
client area rendered as one flat orange/ochre rectangle - no tile grid, no sprite.

**Diagnosis** (headless frame dump of the exact demo render path; `render_and_present`
is window-free at `hwnd == 0`, so the demo scene was rendered into a fresh
`Framebuffer` and the present crop histogrammed + dumped to BMP):

- The crop was **100.0% a single color** `0xE4C9` (2,400,000 of 2,400,000 pixels over
  5 frames; `0xE4C9` = `monarch.pal` entry 163, RGB(57,38,19) in 565 - the exact
  "orange/ochre" the operator saw). This disproved all three suspected classes:
  palette packing (`0xE4C9` converts to precisely its source RGB), clipping (nothing
  was clipped - the crop was fully covered), and the color-key blit (the kernels are
  the 7.1-proven ones; their unit tests stayed green throughout).

- Root cause was **demo scene composition** in `window.rs`, two defects compounding:
  1. The synthetic 64x64 map pointed **every** cell at ref 0, and `DemoTileset`
     served the same `Mask.Spr` diamond for it - a **single-palette-index** solid
     diamond (the 7.1 fixture walk shows index 163 is its only literal). Isometric
     80x40 diamonds on the (+-40,+-20) lattice **tessellate the plane exactly**
     (each diamond's transparent corners are covered by its neighbors' tips), so
     784 identical ochre diamonds fused into one perfectly flat field.
  2. `render_and_present` drew the map **after** the overlay sprite, repainting the
     marching `Mask.Spr` every frame - legacy draws the map first
     (`GameDraw()` runs `GameDrawMap()` before any sprite layer, `Draw.cpp:925-940`).

**Fix** (demo scene only - `window.rs`; zero changes to blit/palette/present/draw_map
kernels, so no test or probe expectation moved):

1. The map is now a **checkerboard**: cells alternate ref 0 / ref 1 by `(x + y) % 2`
   (`ref << 16`, `Map-Layor.cpp:107`), and `DemoTileset` serves two tiles built from
   the **same real fixture diamond canvas** under two embedded color tables
   (dark/light green, the v400 embedded-table swap `Graphic_Capsule.cpp:550-554`).
   Alternating colors make every diamond edge legible.
2. Render order corrected to legacy: `draw_map` first, then the overlay sprite.
3. The real `Mask.Spr` (real `monarch.pal` table, ochre `0xE4C9`) draws **static at
   the dead center of the visible crop**, position computed from the crop
   ((80,40) 800x600 -> (440,320)), per the operator's acceptance criterion.

**Verification** (headless dump of the fixed scene, 5 frames): crop = 3 distinct
colors - dark green `0x640c` 50.0%, light green `0x8c91` 49.7%, ochre `0xe4c9`
exactly 1,600 px/frame (= the sprite's literal pixel count, at center). Rendered
BMP reviewed visually: diamond checkerboard with clear contrast, `Mask.Spr` diamond
visible at screen center. All five gates re-run green, counts unchanged (lib
299 passed; 0 failed; 1 ignored; asset_fixtures 11; e2e_loopback 14). The demo
remains a scene-presentation concern only - no gate test touches it (P3-3), and the
blit/palette/present kernels that the gates pin are untouched by this fix.

### Verdict - Verifier

(Not appended — the Step 7.2 Verifier session was interrupted by quota exhaustion after
completing technical verification but before filing this block. The verification substance
and the unfinished merge-topology reconciliation are recorded by the Custodian/Lead in the
reconciliation record below; evidence at `evidence/7.2/reconciliation/`.)

### Adversary record

All five WO-§8 mutation probes were run by the Implementer at the delivery SHA and
recorded in `evidence/7.2/mutation-probes.md` (each: named test fails at exit-101,
source restored byte-clean, suite re-run green). Post-merge, the Custodian/Lead
re-verified all five independently at `0e86e3a` — see the reconciliation record below.

### Reconciliation & merge record - Custodian/Lead (2026-10-30, under directive)

**Context.** The directive of 2026-10-30 ("Pull Step 7.2, Reconcile Topology & Merge to
Main") records that Step 7.1/7.2 were technically verified and visually confirmed
on-screen by the operator (PsiGinger), and that the Verifier session ran out of quota
before finishing the merge-topology reconciliation. This block files that reconciliation.

**Topology.** Divergence from merge-base `2602e13`: `origin/main` @ `60d38dd` (7.1 S4
sign-off only) vs `origin/step/7.2-windowing` @ `4005fcd` (full 7.1 + 7.2 lineage, never
touched the matrix). Reconciled by a `--no-ff` **merge** — not a rebase — because the
sign-off record cites SHAs (`2c0964e`, `4005fcd`) that a rebase would orphan; merge
commit **`0e86e3a`** (parents `60d38dd` + `4005fcd`), auto-merged without conflict, both
WO-7.1 §9 return blocks intact, row 7.1 (PASSED @ `2c0964e`) unmodified.

**Code identity.** `git diff 4005fcd 0e86e3a -- src tests Cargo.toml Cargo.lock` is
empty: the reconciled tree runs the exact delivered 7.2 code.

**Gates at `0e86e3a`** (logs in `evidence/7.2/reconciliation/`, SHA256SUMS over committed
bytes): all five exit-0 — `check`, `clippy -D warnings` (0 warnings), `fmt --check`
(0 bytes), `test` **324/324** = 299 lib + 11 asset_fixtures + 14 e2e (5 ignored: the
operator demo test + 4 live), `doc` (0 warnings).

**Adversary probes re-verified post-merge** (Custodian/Lead, independently of the
Implementer's original run): (a) pitch advance, (b) color-key removal, (c) unique-90
layer swap, (d) crop origin +80→+160, (e) drain bound 10→11 — each fails its WO-named
test at exit-101, each restored byte-clean, and the full suite re-runs 324/324 after
restoration (no `PROBE` markers remain in `src/`).

**Operator confirmation.** On-screen at `4005fcd` after the demo-scene fix recorded in
the operator-feedback block above.

**Matrix.** Rows 7.1 (already PASSED) and 7.2 (set to PASSED @ `4005fcd`, 324/324)
updated in a `docs(qc)` commit under this Custodian directive — status cells are
Verifier-owned per `CONTRIBUTING_PHASE3.md` P3-6; the Verifier session's quota
exhaustion and this directed override are disclosed in the commit message
(precedent `fc5ffb1`, 7.0's "Governance actor (under Custodian directive)").

**Merge executed** to `origin/main` per the same directive; full bundle in
`evidence/7.2/reconciliation/`.

## 10. Definition of done

1. All §5 files exist in `src/graphics/**`, gates green at the delivery SHA, S4 evidence
   committed under `evidence/7.2/`, Verifier verdict + Adversary probe record appended to §9,
   matrix row 7.2 updated by the Verifier to `PASSED` with the delivery SHA and the test count
   copied from the logs, and merge authorized by the Custodian (fast-forward, per
   `CONTRIBUTING_PHASE2.md`).
2. `spr.rs`/`palette.rs`/`map.rs`/`archive.rs` are byte-identical to the 7.1 delivery (7.1's
   tests still pass unchanged); nothing outside `src/graphics/**` + `evidence/7.2/` +
   `docs/agents/WO-7.2.md` §9 was written by the Implementer.
3. P3-3 holds: the full gate suite runs with zero window, DC, display, GPU, or network
   involvement; no test constructs any Win32 object (AMENDED 2026-10-29 — the on-screen
   present path is verified interactively by the operator, PsiGinger / พี่แมว).
