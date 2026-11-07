# Work Order - Step 8.1: Mouse — `screen_to_tile` Inverse, Click-to-Move & Target Selection

**Status:** `ACCEPTED` — definition-before-implementation: this WO is authored in the Scribe
lane per `TASK_MATRIX.md` row 8.1 ("`WO-8.1.md` (to author at opening)") and committed on
`main` **before** any `src/**` byte moves. The Implementer may pull and begin.
**Date:** 2026-11-07 · **Revision:** Rev 1 (initial, Scribe lane under directive).
**Branch:** `step/8.1-mouse-move`, to be cut from `main` at this WO's sign-off commit.
**Baseline (re-derived this session):** `main` @ `7d76490`, porcelain clean, all five mandatory
gates exit-0 — `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` under
`RUSTFLAGS=-D warnings`, `cargo fmt --check` 0 diffs, `cargo test` **371/371** (335 lib +
11 `asset_fixtures` + 25 `e2e_loopback`; 1 ignored lib + 4 ignored `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `TASK_MATRIX.md` Phase IV tracker (row 8.1 `PENDING`, est. +16–20, lanes
Implementer → Verifier + Adversary) · `CONTRIBUTING_PHASE4.md` (P4-C1 zero-queue, P4-C2 parity
line, P4-C3 latency, P4-C4 headless; verification gates 1–6) · `docs/agents/WO-8.0.md`
(D1–D8, E1–E12, T1–T11 — **this WO continues that numbering**: D9+, E13+, T12+, per the
cross-WO reference precedent of `WO-8.0` citing WO-7.4 D4/D7) · `docs/agents/WO-7.2.md` §6.3
(`draw_center_xy`, frozen) · `src/game/action.rs` (`Order`, `OrderKind`, `encode_order`) ·
`docs/agents/WO-6.3.md` §9 (mutation-probe precedent).

---

## 1. Provenance, governance, lane hats

- **Registration.** Row 8.1 was opened with "WO-8.1.md (to author at opening)"; this WO is the
  Scribe-lane authoring step, written under directive after the Step 8.0 sign-off merge
  (`7d76490`) — definition-before-implementation, before any code.
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every load-bearing claim here
  is "parity with legacy math" or "refuses/discards", dischargeable by verified mutation probes
  (§12), per `CONTRIBUTING_PHASE4.md` verification gates item 6.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs
  (including `WO-8.0.md`) and `docs/**` is READ-ONLY — a defect found there is a finding
  `F-8.1-NN`, never an edit (P3-6; F-7.4-01 disclosure pattern if the compiler ever forces one).
  `src/graphics/**` is READ-ONLY **but imported**: `draw_center_xy`, `ScreenRect` and the
  `MAP_TILE_*` constants are called, never edited or duplicated (D16).
- **Source-of-record split.** This step adds **no new wire bytes**: the `Order` layouts stay
  7.3-frozen (`action.rs:76-125`), `validate` stays 8.0-frozen (`WO-8.0` §7.4), the sending half
  stays `src/client/commands.rs` (7.4, frozen — `src/client/**` READ-ONLY for all of Phase IV).
  This WO owns only the **screen math contract** (§7.1), the **target-selection contract**
  (§7.2), the **click rows of `translate`** (§7.4), and the expectation table in §8, each row
  traced to a legacy `file:line` or a named frozen constant (F-07).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 The input core as shipped (Step 8.0) — what this step extends

| Piece | As of `7d76490` | Where |
|---|---|---|
| `translate` | 2-arg `(event, &mut UiState)`; `!in_world` guard checked first; `Move`/`LeftDown`/`LeftUp` all return `None` ("the clicks are 8.1's") | `src/input/keybinds.rs:30-36`, `:93-96` |
| `UiState` | `in_world`/`chat_open`/`line`, `#[derive(Debug, Clone, PartialEq, Eq, Default)]`, `Default` = `false`/`false`/empty | `src/input/intent.rs:41-49` |
| `Intent` surface | `MoveTo { x: i32, y: i32 }` and `AttackTarget { target_char: u32 }` **already declared for 8.1** ("8.1's hit-test", "8.1's screen math feeds this") — **no new variant needed** | `src/input/intent.rs:12-22` |
| `validate` | `MoveTo` → `u16::try_from` refuse-never-clamp (`:21-25`); `AttackTarget` → `Order::Attack` (`:26-28`) — consumed unchanged | `src/input/validate.rs` |
| 8.0 click expectations that must stay byte-identical | T5: default-state `LeftDown`/`LeftUp` → `None` + `UiState` unchanged (`tests.rs:142-157`); T6: pre-world clicks discarded (`:169-187`); T7: a `Move` stream mutates nothing (`:195-221`) | `src/input/tests.rs` |
| forward transform (frozen 7.2) | `draw_center_xy(center_x, center_y, tile_x, tile_y, &ScreenRect)` — isometric diamond around the screen-rect center | `src/graphics/screen.rs:147-168` (`mapdraw.cpp:36-52`) |
| screen types (frozen 7.2) | `ScreenRect { left, top, right, bottom }`, pub, `Copy`; 800x600 backbuffer `960×680` → rect `(0,0,959,679)`, mid `(480,340)` | `src/graphics/screen.rs:66-77`, `:87-91` |
| tile constants (frozen) | `MAP_TILE_XSIZE_HALF = 40`, `MAP_TILE_YSIZE_HALF = 20` | `src/graphics/screen.rs:30-36` (`map.h:34,36`) |
| frozen encoder (test pins only) | `encode_order` = `kind u8` + LE fields; `OrderKind::{Move = 0, Attack = 1, SitDown = 2}` | `src/game/action.rs:113-125`, `:63-69` |
| module wiring | `mod.rs` lists 4 submodules + re-exports + `#[cfg(test)] mod tests;` | `src/input/mod.rs:21-32` |
| VK / new-crate baseline | `Cargo.toml` untouched since 7.4; no `VK_*` beyond the three 8.0 constants | `Cargo.toml`, `keybinds.rs:18-22` |

### 2.2 Constraints discovered (each forces a design choice or a documented divergence)

1. **The round-trip identity (charter gate 1: "round-trip against `draw_center_xy`").**
   Forward (`draw_center_xy`, `screen.rs:155-166` ← `mapdraw.cpp:39-49`), with
   `mid = (left + (right-left+1)/2, top + (bottom-top+1)/2)`:

   ```text
   sx = -cx*40 + cy*40 + tx*40 - ty*40 + mid_x
   sy = -cx*20 - cy*20 + tx*20 + ty*20 + mid_y
   ```

   Inverse (`TransFromScreenToRealXY`, `Map-Client.cpp:185-187`; `VirtualMapX/Y ≡ (cx,cy)` —
   same view center the forward transform takes):

   ```text
   realx = sx - mid_x - (-cx*40 + cy*40)       = 40*(tx - ty)
   realy = sy - mid_y - (-cy*20 - cx*20) + 20  = 20*(tx + ty) + 20
   ```

   Tile (`GetRealTileByRealDot`, `MapCommon.cpp:965-966`, C truncation):

   ```text
   tx' = (realx/2 + realy) / 40 = (40*tx + 20) / 40
   ty' = (-realx/2 + realy) / 40 = (40*ty + 20) / 40
   ```

   In this direction `realx` is always a multiple of 40, so `realx/2` is exact. For
   `tx ≥ 0` and `ty ≥ 0` — every in-map tile (`map.cpp:226`) — `(40t + 20)/40 = t`
   exactly (remainder 20). **Boundary, kept and documented:** for a *negative* tile the `+20`
   crosses zero and truncation returns e.g. `(-40+20)/40 = 0 ≠ -1` — legacy behaves identically
   (C99 truncates toward zero, as does Rust's `/`), and such tiles are refused by the
   E16/E17/E19 gates before any order could carry them.
2. **Hand-computed goldens (F-07: never derived from the code under test).**
   `ScreenRect` for 800x600 (`screen.rs:87-91`: `960×680`) → `mid = (480, 340)`:

   | # | Center | Click | `realx, realy` | Division | Result |
   |---|---|---|---|---|---|
   | G1 | (100,100) | (480,340) | `0, 4020` | `(0+4020)/40 = 100 r20`, same | **(100,100)** |
   | G2 | (100,100) | (520,360) | `40, 4040` | `(20+4040)/40 = 101 r20`, `(-20+4040)/40 = 100 r20` | **(101,100)** |
   | G3 | (100,100) | (440,360) | `-40, 4040` | `(-20+4040)/40 = 100 r20`, `(20+4040)/40 = 101 r20` | **(100,101)** |
   | G4 | (0,0) | (10,10) | `-470, -310` | `(-235-310)/40 = -545/40 = -13.625 → -13`; `(235-310)/40 = -1.875 → -1` | **(-13,-1)** (floor would give (-14,-2)) |
   | G5 | — (`real_to_tile(-41,-19)`) | — | `-41/2 = -20` (floor: -21) | `(-20-19)/40 = -0.975 → 0`; `(20-19)/40 → 0` | **(0,0)** (floor paths would give (-1,0)) |

   G1 is itself the round-trip of tile (100,100); G4 pins truncation of both the quotient and
   the sign; G5 isolates the `realx/2` truncation.
3. **Coordinate space of `InputEvent` (D9).** `draw_center_xy`, `IsPtInMap` and
   `TransFromScreenToRealXY` all live in **backbuffer/`ScreenRect` space**. Legacy reaches it by
   `GameMouse = window_mouse + MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY`
   (`MouseOperation.cpp:232-233`), where Q4 pins `GetScreenRX/RY = 0`
   (`CommonLogic.h:837-838`, "no camera scrolling in Phase IV"). That window→backbuffer
   translation is message-decode machinery → **8.3's `wm_to_input`** (P4-C2); `InputEvent`'s
   mouse coordinates are therefore `ScreenRect`-space, which makes `screen_to_tile` *literally*
   the exact inverse of `draw_center_xy` (charter P4-C2 wording).
4. **8.0 test compatibility is a hard constraint (D10).** T5 (`tests.rs:145-146`), T6
   (`:171-172`) and T7 (`:215`) feed clicks/hovers with `..UiState::default()` and assert
   `None` + `UiState` byte-identical. Therefore: the click context lives in
   `UiState.view: Option<MapView>` defaulting to `None` (clicks discarded until wired, same
   lifecycle as `in_world`), `Move`/`LeftUp` stay inert unconditionally, and the click arm
   **never mutates `UiState`**. T1–T11 remain byte-identical with zero test edits.
5. **The input layer owns no world state (D13 input side).** Legacy's classification
   (`IsEnemy`/`IsNPC`, `MouseOperation.cpp:3259-3261/:3267/:3275-3277/:3283`) and char filters
   (self/dead/horse, `UnderMouseObject.cpp:160-162`) need world state that does not exist in
   `src/input/**`. Candidates therefore arrive **pre-classified** (`CharTarget.class`) from the
   caller (8.3's world snapshot); `pick_target` implements only the *math* core. Machinery
   split, stated per P4-C2 — not a silent shortcut.
6. **`FindLandChar` is two passes with different selection rules (D13).** Pass 1 (char
   enumeration, `:150-190`): first qualifying candidate becomes the winner (`:168-175`,
   `if(*newid == 0)`), while `min_length` tracks the smallest length seen; pass 2 (tile-ring
   scan, `:192-225`) replaces it only when **strictly** nearer (`:212-215`). The enumeration is
   insertion order (`IDManager.cpp:159-160`, `:194-198`) — not draw order. Net legacy effect:
   first-arrived candidate wins among overlapping chars *unless* a char absent from the
   enumeration is strictly nearer. **Port decision D13:** implement what the `min_length`
   machinery encodes — nearest center wins, ties → first in slice order — and state the
   difference (§8 divergences; Haru/CAPA: documented, never silent).
7. **Off-map tiles (D14).** Legacy computes the out-of-map tile (the screen gate passed),
   skips target lookup (`UnderMouseObject.cpp:335`, `IsInMapArea`) and still encodes
   `ORDER_MOVE` (`:5532-5537`) — the server drops it. Verification gate 2 requires an
   **off-map tile refused by name**, so `translate` gates with `is_in_map_area` against
   `MapView.map_size`. Same UX (nothing moves), no wire noise; the wire-behavior difference is
   stated (§8).
8. **Chat focus does not gate the pointer (D15).** The legacy `WM_LBUTTONDOWN` arm has no
   focus check (`mainfunction.cpp:232-249` — only `GetFrame() > 40` at `:236`): keyboard input
   is focus-driven (8.0's `chat_open` rows stand unchanged), mouse input is position-driven.
   Clicks therefore act while the chat line is open (E29 pins it).
9. **No new enum variants → F-7.4-01 is a pre-check, not a risk.** `Intent` already carries
   `MoveTo`/`AttackTarget` (8.0 declared the full surface); `UiState` gains *fields*, and the
   only structural matches over it use functional update (`..UiState::default()`), which new
   fields cannot break. The whole-tree grep for the new identifiers
   (`MapView|CharTarget|pick_target|screen_to_tile|is_pt_in_map|is_in_map_area`) is recorded in
   `evidence/8.1/` at verification; expected: matches confined to `src/input/**`.
10. **Arithmetic width.** Legacy computes in `SHORT` (`Map-Client.cpp:183` params); the 7.2
    forward port already uses `i32` (`screen.rs:147`). The inverse uses `i32` to match — no
    value differs anywhere legacy's `SHORT` arithmetic stays in range; removing a wrap risk is
    stated here rather than silently (§8 divergences).

### 2.3 Legacy semantics inherited (why the math and the click table exist)

| Legacy behavior | Citation |
|---|---|
| Left click dispatched on **press**, frame-gated, no chat-focus check | `mainfunction.cpp:232-249` (`WM_LBUTTONDOWN`, `GetFrame() > 40` at `:236`, `LeftPressSwitch` at `:248`) |
| Release arm pushes a no-flag key — no action | `mainfunction.cpp:251-258` |
| Hover is position tracking only (never an order) | `mainfunction.cpp:208` → 8.0 D8 (unchanged) |
| Window mouse → game mouse: `+ MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY` (Q4 ≡ 0) | `MouseOperation.cpp:232-233`; `CommonLogic.h:837-838` |
| Screen gate `IsPtInMap` (inclusive `ScreenRect`); miss → `MapX/MapY = -1` | `MouseOperation.cpp:529-534`; `Map-Client.cpp:196-206` |
| Screen → real dot | `MouseOperation.cpp:537` ← `Map-Client.cpp:183-188` (trailing `+ MAP_TILE_YSIZE_HALF` at `:187`) |
| Real dot → tile: **both** axes ÷ `MAP_TILE_XSIZE_HALF`, C truncation | `MouseOperation.cpp:540` ← `MapCommon.cpp:963-965` |
| Target lookup skipped when the click tile is out of map | `UnderMouseObject.cpp:335` (`IsInMapArea`) |
| Tile bounds half-open (`0 ≤ x < MapXsize`) | `map.cpp:224-228` |
| Hit-test: Chebyshev ≤ 3 tiles, filters, first-hit + `min_length`, pass-2 strict `<` | `UnderMouseObject.cpp:150-190` (`:156-159`, `:160-162`, `:168-175`, `:212-215`) |
| Strict `RealRect` open-interval containment; `length = max(|cx-x|, |cy-y|)` | `Char-Client.cpp:1012-1023` |
| Candidate enumeration = insertion order (not draw order) | `IDManager.cpp:159-160`, `:194-198` |
| Classification: enemy → `MOUSEMODE_ATTACK`; non-enemy NPC → `MOUSEMODE_CHARNPC`; non-enemy player → `MOUSEMODE_OTHERUSER` | `MouseOperation.cpp:3259-3261`, `:3267`, `:3275-3277`, `:3283` |
| Left-order branches: sit state-check (server), NPC **dialogs** (Q3 out), move, attack | `:4123` · `:4138+` (dialog `CreateInterface` arms) · `:5409` → dest `:5532`, `ORDER_MOVE` `:5537` · `:5544` → `ORDER_ATTACK` `:5607-5631` |
| Forward transform (frozen in 7.2, this WO only calls it) | `screen.rs:147` ← `mapdraw.cpp:36-52` |

## 3. Scope decision — this delivery

**In:**

1. `src/input/screen.rs` — NEW: `MapView` + `screen_to_real` / `real_to_tile` /
   `screen_to_tile` / `is_pt_in_map` / `is_in_map_area` (§7.1).
2. `src/input/target.rs` — NEW: `TargetClass` + `RealRect` + `CharTarget` + `pick_target`
   (§7.2).
3. `src/input/intent.rs` — EDIT: `UiState` gains `view` and `targets` (§7.3); everything else
   untouched.
4. `src/input/keybinds.rs` — EDIT: the `LeftDown`/`LeftUp` arms replace the 8.0 placeholder
   comment (`:93-96`); all key arms untouched (§7.4).
5. `src/input/mod.rs` — EDIT: two `pub mod` lines + two re-export lines + one doc sentence.
6. `src/input/tests.rs` — EDIT: **pure append** — new `use` lines and T12–T27 (§9);
   `git diff` shows 0 deletions, T1–T11 byte-identical.
7. Sixteen tests: baseline **371 → 387 expected** (+16 = T12–T27, all lib; the delivered
   number is copied from `test.log`).

**Out:** see §5.

## 4. Architectural decisions (binding for this WO; D9–D16 continue WO-8.0's D1–D8)

- **D9 — `InputEvent` mouse coordinates are `ScreenRect`/backbuffer space.** The window→
  backbuffer translation `+ MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY`
  (`MouseOperation.cpp:232-233`, Q4 → `(80, 40)`) belongs to 8.3's decode; with it outside,
  `screen_to_tile` *is* the exact inverse of `draw_center_xy` (§2.2-1/3, charter P4-C2).
- **D10 — the click context lives in `UiState`; defaults swallow clicks.**
  `view: Option<MapView>` = `None` and `targets: Vec<CharTarget>` = `[]` under
  `#[derive(Default)]` — clicks and hovers are discarded until 8.3 wires the view (the same
  lifecycle as `in_world`), which is exactly what keeps 8.0's T5/T6/T7 byte-identical. The
  target list is a **world snapshot field, not a queue** (P4-C1 targets queue machinery;
  declared here so §11's audit expectation matches this §7 text). `translate` reads both fields
  and never mutates them.
- **D11 — one intent per left press; release and hover stay inert.** `LeftDown` fires
  (`mainfunction.cpp:232-249`), `LeftUp` is accepted and ignored (`:251-258`), `Move` remains
  `None` (8.0 D8 — no hover classification machinery is ported; the click carries its own
  coordinates, so legacy's per-frame `GetMouseMode` state is unnecessary, P4-C2). No drag, no
  auto-repeat, no double-click (deferred with right-click since 8.0 §5).
- **D12 — the three-way click classification.** `Enemy` → `Intent::AttackTarget` (frozen
  `Order::Attack`); non-enemy `Npc` → **`None`** — legacy's dialog branch (`:4138+`) is
  Q3-excluded, and discarding is the declared outcome (no fall-through move: legacy sets
  `bdoneswitch`, so no move happens either); `OtherUser` and **no candidate at all** →
  `Intent::MoveTo { tile }` (the move branch `:5409` handles `MOUSEMODE_MOVE |
  MOUSEMODE_OTHERUSER` alike). Stated divergence, never silent (§8).
- **D13 — `pick_target` = `FindLandChar`'s math core: Chebyshev ≤ 3, strict `RealRect`,
  nearest-center, first-on-tie.** Pre-classified candidates arrive from the caller (§2.2-5);
  selection implements the `min_length` semantics (`:168-175`/`:212-215`), which deviates from
  legacy's pass-1 first-arrived effect exactly when ≥ 2 candidates overlap at different
  distances — documented in §8 (Haru/CAPA). World filters (self/dead/horse, `:160-162`) are the
  caller's, like `in_world` (8.0 D4 precedent: world facts enter `UiState`, never computed here).
- **D14 — off-map-tile clicks are refused before enqueue, by name.** `translate` gates the
  computed tile with `is_in_map_area(tile, view.map_size)` → typed `None`. Legacy encodes the
  order and the server drops it (`:5532-5537` with an out-of-range tile); the port refuses
  (verification gate 2; same UX, no wire noise). Stated in §8.
- **D15 — clicks are not swallowed by `chat_open`.** Parity with the legacy arm, which has no
  focus check (`mainfunction.cpp:232-249`); the keyboard focus model (8.0 §7.3 Insert/typing
  rows) is untouched. E29 pins both halves of the split.
- **D16 — zero new wire surface; frozen parts reused, never duplicated.** No new `Intent`
  variant, no `validate` change, no new opcode/crate/constant: the math imports
  `ScreenRect` + `MAP_TILE_*_HALF` from `graphics::screen` (READ-ONLY, called only), byte pins
  use the frozen `encode_order` (`action.rs:113-125`), and `translate` keeps its 2-arg
  signature so 8.0's tests compile and pass unmodified.

## 5. Non-goals

- **No queue machinery of any kind** (P4-C1): no `VecDeque`, ring, capacity constant or
  coalescing type in `src/input/**`; `UiState.targets` is the §7.2-declared snapshot field, the
  only collection added (the tokio mpsc remains the only queue).
- No `Move`/hover classification, no cursor rendering (`GAME_CURSOR_*`, `SetMouseOverSwitch`),
  no right-click menus, no double-click (`OperateAutoDoubleLeftOrder`), no dialogs of any kind
  (Q3 — including the NPC dialog arms and the right-mouse user list), no camera scroll (Q4).
- No window/`WM_*` decode, no `+80/+40` border translation (8.3's `wm_to_input`, D9), no
  `in_world`/`view` production wiring (8.3 sets both; here they are test-built).
- No pathfinding/waypoint queues: one click → one order, exactly legacy (`ORDER_MOVE` carries a
  single destination tile).
- No edits to `src/client/**`, `src/game/**`, `src/graphics/**` (called only), `src/auth/**`,
  `src/net/**`, `src/protocol/**`, `tests/**`, `Cargo.toml`/`Cargo.lock`.
- No new opcodes, no `Order`/`encode_order` changes, no chat behavior changes (8.0 rows stand),
  no live-server runs as gates (8.4's evidence lane).

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change |
|---:|---|---|
| 1 | `docs/agents/WO-8.1.md` | NEW — this document, committed **before** any code |
| 2 | `src/input/screen.rs` | NEW — `MapView` + the five math functions (§7.1) |
| 3 | `src/input/target.rs` | NEW — `TargetClass`, `RealRect`, `CharTarget`, `pick_target` (§7.2) |
| 4 | `src/input/intent.rs` | EDIT — `UiState` += `view`, `targets` (§7.3); two added fields only |
| 5 | `src/input/keybinds.rs` | EDIT — `LeftDown`/`LeftUp` arms replace the `:93-96` placeholder (§7.4); key arms untouched |
| 6 | `src/input/mod.rs` | EDIT — `pub mod screen; pub mod target;` (alphabetical) + two re-export lines + one doc sentence (insertions only) |
| 7 | `src/input/tests.rs` | EDIT — **pure append**: new `use` lines + T12–T27 (§9); T1–T11 byte-identical, 0 deletions |
| 8 | `evidence/8.1/` | NEW at verification, written by the Verifier only (§11) |

**READ-ONLY:** all of `src/client/**`, `src/game/**`, `src/graphics/**` (imported, never
edited), `src/auth/**`, `src/net/**`, `src/protocol/**`, `src/main.rs`, `src/input/event.rs`,
`src/input/validate.rs`, all of `tests/**`, `Cargo.toml`/`Cargo.lock`, `TASK_MATRIX.md`,
`CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, every other file in `docs/**` (incl.
`WO-8.0.md`), every existing `evidence/**` directory. A defect found there is a finding, never
an edit (P3-6). No `src/lib.rs` change is needed (`pub mod input;` already exists).

## 7. API & wiring spec (the shared contract)

### 7.1 `src/input/screen.rs`

```rust
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
/// for term, Q4 camera anchor `GetScreenRX/RY = 0` folded away
/// (`CommonLogic.h:837-838`) — screen pixel → real dot.
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
pub fn real_to_tile(realx: i32, realy: i32) -> (i32, i32) {
    (
        (realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
        (-realx / 2 + realy) / MAP_TILE_XSIZE_HALF,
    )
}

/// The exact inverse of `draw_center_xy` (`screen.rs:147`): the two-step
/// legacy chain `TransFromScreenToRealXY` → `GetRealTileByRealDot`
/// (`MouseOperation.cpp:537,540`).
pub fn screen_to_tile(x: i32, y: i32, center: (i32, i32), screen: &ScreenRect) -> (i32, i32) {
    let (realx, realy) = screen_to_real(x, y, center, screen);
    real_to_tile(realx, realy)
}

/// `cltMapClient::IsPtInMap` (`Map-Client.cpp:196-206`) — inclusive
/// screen-rect gate (legacy fires at `MouseOperation.cpp:534`).
pub fn is_pt_in_map(x: i32, y: i32, screen: &ScreenRect) -> bool {
    screen.left <= x && x <= screen.right && screen.top <= y && y <= screen.bottom
}

/// `cltMapCommon::IsInMapArea` (`map.cpp:224-228`) — half-open tile bounds
/// (D14's named refusal; legacy's gate at `UnderMouseObject.cpp:335`).
pub fn is_in_map_area(tile_x: i32, tile_y: i32, map_x: i32, map_y: i32) -> bool {
    tile_x >= 0 && tile_x < map_x && tile_y >= 0 && tile_y < map_y
}
```

Round-trip obligation (charter gate 1): for every `tile ≥ 0` and any center/screen,
`screen_to_tile(draw_center_xy(cx, cy, tx, ty, screen), (cx, cy), screen) == (tx, ty)` — the
§2.2-1 identity, pinned by T12; hand-computed goldens G1–G5 pin the constants and the
truncation (T13/T14).

### 7.2 `src/input/target.rs`

```rust
/// How the caller's world classifies a candidate — legacy `GetMouseMode`'s
/// bits, computed from world state the input layer does not own (D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetClass {
    /// `IsEnemy` → `MOUSEMODE_ATTACK` (`MouseOperation.cpp:3259-3261`,
    /// `:3275-3277`) → `ORDER_ATTACK` (`:5544`, `:5607-5631`).
    Enemy,
    /// Non-enemy NPC → `MOUSEMODE_CHARNPC` (`:3267`) → legacy dialog
    /// (`:4138+`, Q3-excluded) — the port **discards** the click (D12).
    Npc,
    /// Non-enemy player → `MOUSEMODE_OTHERUSER` (`:3283`) → legacy's move
    /// branch (`:5409`); the port emits `MoveTo` (D12).
    OtherUser,
}

/// `RealRect` of a candidate sprite (`Char-Client.cpp:1015`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealRect {
    /// Strict left bound (`< x`, `:1015`).
    pub left: i32,
    /// Strict top bound (`< y`, `:1016`).
    pub top: i32,
    /// Strict right bound (`x <`, `:1015`).
    pub right: i32,
    /// Strict bottom bound (`y <`, `:1016`).
    pub bottom: i32,
}

/// One clickable character candidate — a world snapshot supplied by the
/// caller (D10/D13); `translate` never mutates the list it sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharTarget {
    /// Unique id → `Intent::AttackTarget.target_char` (frozen `Order::Attack`).
    pub id: u32,
    /// The candidate's map tile (`GetX/GetY`, `FindLandChar:156`).
    pub tile: (i32, i32),
    /// Real-space sprite rect — strict containment (`Char-Client.cpp:1015-1016`).
    pub rect: RealRect,
    /// Real-space center (`siCenterX/Y`, `Char-Client.cpp:1018`).
    pub center: (i32, i32),
    /// The caller's classification (D13).
    pub class: TargetClass,
}

/// `FindLandChar`'s math core (`UnderMouseObject.cpp:150-190`): the
/// Chebyshev ≤ 3-tile filter (`:156-159`), strict `RealRect` containment
/// (`Char-Client.cpp:1015-1016`), length = Chebyshev distance to the
/// candidate center (`:1018`); nearest wins, ties → first in slice order
/// (D13, the stated §8 divergence from legacy's pass-1 first-hit).
pub fn pick_target<'a>(
    candidates: &'a [CharTarget],
    click_tile: (i32, i32),
    click_real: (i32, i32),
) -> Option<&'a CharTarget> {
    let mut best: Option<(&CharTarget, i32)> = None;
    for candidate in candidates {
        // :156-159 — Chebyshev tile distance; beyond 3 tiles: not a candidate.
        let tile_len = (candidate.tile.0 - click_tile.0).abs()
            .max((candidate.tile.1 - click_tile.1).abs());
        if tile_len > 3 {
            continue;
        }
        // Char-Client.cpp:1015-1016 — strict open interval, both axes.
        let inside = candidate.rect.left < click_real.0
            && click_real.0 < candidate.rect.right
            && candidate.rect.top < click_real.1
            && click_real.1 < candidate.rect.bottom;
        if !inside {
            continue;
        }
        // Char-Client.cpp:1018 — length to the sprite center.
        let length = (candidate.center.0 - click_real.0)
            .abs()
            .max((candidate.center.1 - click_real.1).abs());
        // D13: strictly nearer replaces; a tie keeps the earlier candidate
        // (`:212-215` strict `<` + `:168-175` first-hit on ties).
        match best {
            Some((_, best_len)) if length >= best_len => {}
            _ => best = Some((candidate, length)),
        }
    }
    best.map(|(candidate, _)| candidate)
}
```

### 7.3 `src/input/intent.rs` — `UiState` extension only

```rust
pub struct UiState {
    pub in_world: bool,      // 8.0, unchanged
    pub chat_open: bool,     // 8.0, unchanged
    pub line: String,        // 8.0, unchanged
    /// 8.1: the view a click is interpreted against; `None` until 8.3 wires
    /// it — clicks are discarded until then (D10). `Default` = `None`, so
    /// every 8.0 test's `..UiState::default()` builds unchanged state.
    pub view: Option<MapView>,
    /// 8.1: the candidate snapshot `pick_target` scans (D10 — a snapshot
    /// field, not a queue; `translate` never mutates it).
    pub targets: Vec<CharTarget>,
}
```

Derives stay `Debug, Clone, PartialEq, Eq, Default` (both new fields are defaulted); `Intent`,
`Default for UiState` semantics and `translate`'s signature are unchanged. `event.rs`,
`validate.rs` and `keybinds`' key arms are untouched.

### 7.4 `src/input/keybinds.rs` — the click arms

The 8.0 placeholder comment (`:93-96`) is replaced by the arms below; the guard
(`:34-36`) still runs first for every event (D4/E28), and the key/char arms are byte-identical.

```rust
InputEvent::LeftDown { x, y } => {
    let Some(view) = ui.view else { return None; };              // D10 — no view: discarded
    if !is_pt_in_map(x, y, &view.screen) { return None; }        // E18 — MouseOperation.cpp:534
    let (tile_x, tile_y) = screen_to_tile(x, y, view.center, &view.screen);
    if !is_in_map_area(tile_x, tile_y, view.map_size.0, view.map_size.1) {
        return None;                                             // E19 — D14, named refusal
    }
    let (real_x, real_y) = screen_to_real(x, y, view.center, &view.screen);
    match pick_target(&ui.targets, (tile_x, tile_y), (real_x, real_y)) {
        // D12: Enemy → attack · Npc → discard (Q3 dialog not ported) ·
        // OtherUser and no-hit → the move branch (`:5409`, `:5532-5537`).
        Some(target) if target.class == TargetClass::Enemy => {
            Some(Intent::AttackTarget { target_char: target.id })
        }
        Some(target) if target.class == TargetClass::Npc => None,
        _ => Some(Intent::MoveTo { x: tile_x, y: tile_y }),
    }
}
InputEvent::LeftUp { .. } => None,                               // E27 — :251-258
InputEvent::Move { .. } => None,                                 // E27 — 8.0 D8 unchanged
```

`translate` semantics for the mouse rows — exhaustive, every row tested (§8/§9). `UiState` is
never mutated by any mouse row:

| `InputEvent` (after the `!in_world → None` guard, D4) | Conditions | Result |
|---|---|---|
| `LeftDown { x, y }` | `ui.view == None` | `None` (D10: geometry not wired yet) |
| `LeftDown { x, y }` | `!is_pt_in_map(x, y, view.screen)` | `None` (E18) |
| `LeftDown { x, y }` | tile fails `is_in_map_area` | `None` (E19, D14) |
| `LeftDown { x, y }` | nearest candidate, class `Enemy` | `Some(AttackTarget { target_char: id })` (E21) |
| `LeftDown { x, y }` | nearest candidate, class `Npc` | `None` (E22, D12 — no fall-through) |
| `LeftDown { x, y }` | nearest candidate, class `OtherUser` | `Some(MoveTo { tile })` (E23) |
| `LeftDown { x, y }` | no candidate | `Some(MoveTo { tile })` (E20) |
| `LeftDown { x, y }`, chat open | passes all gates | identical rows — chat state untouched (E29, D15) |
| `LeftUp { .. }` | any | `None` (E27) |
| `Move { .. }` | any | `None` (E27, 8.0 D8) |
| **any mouse**, `!in_world` | — | `None` (checked first, E28) |

### 7.5 `src/input/validate.rs` — unchanged

The 8.0 §7.4 rows stand and are consumed as-is: `MoveTo` → `u16::try_from` refuse-never-clamp
(`:21-25`, E9), `AttackTarget` → `Order::Attack` (`:26-28`). No edit; byte pins in §9 go
through the frozen `encode_order` (`action.rs:113-125`), never through new encoding code.

## 8. Expectation table (the F-07 source for §9)

Tests assert **these rows**; no expectation may be derived from `screen_to_real`,
`real_to_tile`, `screen_to_tile`, `pick_target` or `translate` under test. Each row is a legacy
`file:line` plus, where applicable, the hand-computed value from §2.2 or the frozen constant
named.

| # | Behavior (as tested) | Provenance |
|---|---|---|
| **E13** | `screen_to_tile(draw_center_xy(c, t)) == t` for every non-negative tile tested (grid incl. `(0,0)`, `(399,399)`; three view centers; two screen rects) | Round-trip identity §2.2-1; `screen.rs:147` ← `mapdraw.cpp:36-52` + `Map-Client.cpp:183-188` + `MapCommon.cpp:963-965` (**charter gate 1**) |
| **E14** | Goldens: `(480,340)→(100,100)`, `(520,360)→(101,100)`, `(440,360)→(100,101)` at center `(100,100)`, rect `960×680` | Hand-computed G1–G3 (§2.2-2) from `Map-Client.cpp:185-187` + `MapCommon.cpp:965-966`; rect from `screen.rs:87-91` (`Client.cpp:3206-3208`) |
| **E15** | Negative truncation: `(10,10)` @ center `(0,0)` → `(-13,-1)` (not floor `(-14,-2)`); `real_to_tile(-41,-19)` → `(0,0)` (not `(-1,0)`) | Hand-computed G4–G5 (§2.2-2); C99 trunc-toward-zero in `MapCommon.cpp:965-966` (Rust `/` matches) |
| **E16** | `is_pt_in_map`: `(0,0)` and `(959,679)` true; `(-1,0)`, `(960,679)`, `(0,680)`, `(959,680)` false | `Map-Client.cpp:199-200` (inclusive `ScreenRect`) |
| **E17** | `is_in_map_area(…, 100, 100)`: `(0,0)`, `(99,99)` true; `(100,99)`, `(99,100)`, `(-1,0)`, `(0,-1)` false | `map.cpp:226` (half-open `0 ≤ x < MapXsize`) |
| **E18** | Off-screen clicks — `(1000,340)`, `(960,0)`, `(-1,-1)`, `(0,680)` — → `None`, `UiState` unchanged | `MouseOperation.cpp:534` gate; `Map-Client.cpp:196-206` |
| **E19** | In-screen click whose tile leaves the map (`(900,600)` @ center `(100,100)`, `map_size (100,100)` → tile `(112,101)`) → `None` by name | `UnderMouseObject.cpp:335` + `map.cpp:226` → D14 (**charter gate 2**: off-map refused) |
| **E20** | Click on empty land → `MoveTo { 100, 100 }` → `Order::Move` → `encode_order` = `[0, 100, 0, 100, 0]` | `MouseOperation.cpp:529-540` (tile), `:5532`/`:5537` (`ORDER_MOVE`); frozen `action.rs:65` (`Move = 0`), `:113-125` (**charter gate 1**: click-to-move bytes) |
| **E21** | `Enemy` under the cursor → `AttackTarget { 7 }` → `Order::Attack` → `encode_order` = `[1, 7, 0, 0, 0]` | `MouseOperation.cpp:3259-3261`/`:3275-3277` (enemy → `MOUSEMODE_ATTACK`), `:5544`, `:5607-5631`; frozen `action.rs:67` (`Attack = 1`), `:121` |
| **E22** | `Npc` under the cursor → `None` — **no** intent, no fall-through move, `UiState` unchanged | `MouseOperation.cpp:3267` (`MOUSEMODE_CHARNPC`) + `:4138+` dialog arms (Q3-excluded) → D12 |
| **E23** | `OtherUser` under the cursor → `MoveTo { tile }` (falls into the move branch) | `MouseOperation.cpp:3283` (non-enemy player) + `:5409` (`MOUSEMODE_MOVE \|\| MOUSEMODE_OTHERUSER`) → D12 |
| **E24** | Strict `RealRect`: a point on the edge (`x == rect.right`) misses; the same point with `right` one greater hits | `Char-Client.cpp:1015-1016` (strict `<` both axes) |
| **E25** | A candidate 4 tiles away (Chebyshev) is ignored even though its rect contains the point | `UnderMouseObject.cpp:156-159` (`length > 3` skipped) |
| **E26** | Nearest center wins (nearer candidate *after* an earlier farther one); equal length → first in slice order | `Char-Client.cpp:1018` (`length`) + `UnderMouseObject.cpp:168-175` (first-hit), `:212-215` (strict `<`) → D13 (stated divergence below) |
| **E27** | With view and targets wired: `LeftUp` and `Move` → `None`, `UiState` unchanged | `mainfunction.cpp:251-258` (release is a no-flag push), `:208` (hover is position-only) → 8.0 D8 + D11 |
| **E28** | With view and targets wired, `in_world = false`: `LeftDown` → `None`, `UiState` byte-identical (guard precedes all 8.1 logic) | `mainfunction.cpp:236` (`GetFrame() > 40`) → 8.0 D4 |
| **E29** | With the chat line open (line buffer holds text): a valid click still yields `MoveTo`, chat state untouched | `mainfunction.cpp:232-249` (arm has no focus check) → D15 |

**Stated divergences (Haru/CAPA — documented, never silent):**
(a) the window→backbuffer translation `+80/+40 (+RX/RY ≡ 0)` is 8.3's decode, not this step's
(D9, §2.2-3); (b) clicking a friendly NPC **discards** the click instead of opening legacy's
dialog (Q3-excluded, D12); (c) target selection is nearest-center/first-on-tie rather than
legacy's pass-1 first-arrived effect (D13, §2.2-6); (d) an in-screen click on an out-of-map
tile is refused before enqueue where legacy encodes `ORDER_MOVE` and the server drops it
(D14, §2.2-7); (e) real-coordinate arithmetic uses `i32` where legacy uses `SHORT` (§2.2-10,
matches the 7.2 forward port); (f) `GetMouseMode`'s per-frame hover/classification state, cursor
updates and the right-mouse user-list counting (`FindLandChar:177-184`) are machinery not
ported — classification arrives pre-computed on the candidate (D13, §2.2-5).

## 9. Test invariants (T12–T27) and the baseline

Pattern authorities: colocated unit tests (`src/input/tests.rs` from 8.0), pure-function tables
(`src/game/action/tests.rs`), hand-computed goldens (`src/graphics/screen.rs` 7.2 tests). F-07:
expectations from §8 only. All tests build `InputEvent`/`MapView`/`CharTarget` values directly —
no window, no Win32, no network (P4-C4). Every assertion message cites its E-row.

| # | Invariant | File | Phase |
|---|---|---|---|
| **T12** | `screen_to_tile_round_trips_draw_center_xy` — E13: for centers `(0,0)`, `(100,100)`, `(37,91)` × screens `960×680` and `1200×880` × an 11-tile grid (incl. `(0,0)`, `(1,0)`, `(100,100)`, `(399,399)`, `(799,0)`), `screen_to_tile(draw_center_xy(...)) == tile`; doc comment records the §2.2-1 identity and why non-negative tiles only | `src/input/tests.rs` | unit |
| **T13** | `screen_to_tile_matches_hand_computed_goldens` — E14: G1–G3 exactly, each asserted against the §2.2-2 hand value (never recomputed by helpers) | `src/input/tests.rs` | unit |
| **T14** | `screen_to_tile_truncates_toward_zero_like_legacy_c` — E15: G4 `(10,10) → (-13,-1)` (asserting the exact values, not floors) and `real_to_tile(-41,-19) == (0,0)`; doc comment carries the division arithmetic | `src/input/tests.rs` | unit |
| **T15** | `is_pt_in_map_gates_inclusive_screen_bounds` — E16: the six-value table of §8, corners inclusive | `src/input/tests.rs` | unit |
| **T16** | `is_in_map_area_is_half_open` — E17: the six-value table of §8 on a `100×100` map | `src/input/tests.rs` | unit |
| **T17** | `clicks_outside_the_screen_rect_are_discarded` — E18: view wired (center `(100,100)`, `map_size (400,400)`), the four off-screen stimuli → `None`, `UiState` byte-identical after each | `src/input/tests.rs` | unit |
| **T18** | `clicks_on_tiles_outside_the_map_are_refused_by_name` — E19: view `map_size (100,100)`, click `(900,600)` (in screen; tile `(112,101)` out of map) → `None`, unchanged; doc comment records the hand-computed off-map tile | `src/input/tests.rs` | unit |
| **T19** | `click_on_empty_land_encodes_the_frozen_move_bytes` — E20: no targets → `LeftDown (480,340)` → `MoveTo { 100, 100 }` → `validate` → `ClientCommand::Order(Order::Move { x: 100, y: 100 })` → `encode_order` == `[0, 100, 0, 100, 0]` (charter gate 1 bytes) | `src/input/tests.rs` | unit |
| **T20** | `click_on_an_enemy_produces_the_frozen_attack_order` — E21: one `Enemy` candidate containing the point → `AttackTarget { 7 }` → `validate` → `Order::Attack { 7 }` → `encode_order` == `[1, 7, 0, 0, 0]` | `src/input/tests.rs` | unit |
| **T21** | `click_on_a_friendly_npc_is_discarded_not_moved` — E22: identical stimulus with `class: Npc` → `None`, no `MoveTo` fall-through, `UiState` unchanged | `src/input/tests.rs` | unit |
| **T22** | `click_on_a_friendly_player_falls_through_to_move` — E23: identical stimulus with `class: OtherUser` → `MoveTo { 100, 100 }` | `src/input/tests.rs` | unit |
| **T23** | `target_hit_test_uses_strict_rect_containment_and_the_three_tile_filter` — E24+E25: point on `rect.right` misses (→ `MoveTo`), `right + 1` hits (→ `AttackTarget`); a candidate at Chebyshev 4 with a containing rect is ignored (→ `MoveTo`) | `src/input/tests.rs` | unit |
| **T24** | `nearest_center_wins_and_first_candidate_wins_ties` — E26: `[far(100), near(0)]` → `AttackTarget { near }` (the D13 divergence, pinned); `[c(20), d(20)]` equal → first | `src/input/tests.rs` | unit |
| **T25** | `release_and_hover_never_produce_intents_even_with_the_view_wired` — E27: view + `Enemy` target wired, `in_world` true: `LeftUp (480,340)` and `Move (480,340)` → `None`, `UiState` byte-identical | `src/input/tests.rs` | unit |
| **T26** | `clicks_before_the_in_world_guard_are_discarded` — E28: full view + target wired, `in_world = false`: valid click → `None`, byte-identical (proves the guard precedes the 8.1 arm) | `src/input/tests.rs` | unit |
| **T27** | `clicks_act_while_the_chat_line_is_open` — E29: `chat_open = true`, line `"hi"`, valid click → `MoveTo { 100, 100 }`; `chat_open` still true, line still `"hi"` (D15 split: pointer not focus-gated) | `src/input/tests.rs` | unit |

**Baseline: 371 → 387 expected** (+16 = T12–T27, all lib tests; `asset_fixtures` 11 and e2e 25
untouched; T1–T11 byte-identical — the tests.rs diff is additions only; the 5 ignored tests
preserved: 1 lib + 4 `live_integration`). The matrix cell is copied from the delivered
`test.log`, never from this prose.

## 10. Forbidden shortcuts

- Any queue/ring/capacity/coalescing structure in `src/input/**` — `UiState.targets` is the
  §7.2-declared world snapshot, the only collection added, and `translate` never mutates it
  (D10/P4-C1).
- Editing `src/graphics/**` (or duplicating `ScreenRect`/`MAP_TILE_*`/`draw_center_xy` locally)
  to make the math work — the inverse must import and round-trip against the frozen forward
  transform (charter gate 1); any READ-ONLY file of §6 edited to make a test pass is a finding
  first (P3-6; F-7.4-01 disclosure pattern only if the compiler forces it).
- Deriving an expectation from the code under test (F-07) — §8 rows and the §2.2 hand
  arithmetic only; goldens are never produced by running `screen_to_tile`.
- Changing `translate`'s 2-arg signature, `Intent`, `validate`, `event.rs`, or any T1–T11 test
  body to fit the new behavior (D10 compatibility is a DoD item, not an aspiration).
- Clamping a click into the map, wrapping a negative tile, or emitting an order for an
  off-map/edge point instead of the typed `None` of §7.4 (D14, E18/E19).
- Porting `GetMouseMode` hover state, `GAME_CURSOR_*` cursor updates, the right-mouse user
  list, any dialog, drag/auto-repeat, right- or double-click (§5).
- New crates (`Cargo.toml` untouched), new opcodes, `Order`/`encode_order` changes, chat
  behavior changes, `in_world`/`view` production wiring (8.3), live runs promoted to gates.
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*` (Verifier/Scribe only), `README.md`,
  `GoonZu_Source/**`, `WO-8.0.md` or any other doc.

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/8.1/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
  `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`, `zero-queue-grep.txt`,
  `f7401-precheck.txt`, `write-guard.txt`, `SHA256SUMS` — hashes over the committed (LF) bytes
  (6.3 L-2 lesson); the matrix `Tests Baseline` cell is copied from `test.log` (**387** expected:
  351 lib + 11 + 25).
- **Charter parity pins (gate 1):** the round-trip run (T12), the hand goldens (T13/T14) and
  the click→`Order::Move` byte pin (T19) each captured green in `test.log`, with §2.2's
  arithmetic cross-checked against the legacy citations in `summary.md`.
- **Zero-queue audit (gate 4, P4-C1):** `grep -inE 'VecDeque|coalesc|MAX_KEY|cltKey' -r src/input/`
  → no matches, captured in `zero-queue-grep.txt`. **Pattern note (F-8.0-01 lesson applied):**
  the audit deliberately uses the *machinery* subset only — no bare `ring` alternation (it
  substring-matches `String`) and no bare `queue` word (it matches doc prose) — and the expected
  inventory line records `ui.targets: Vec<CharTarget>` as the §7.2 snapshot field, not a queue.
- **F-7.4-01-style pre-check:** whole-tree grep for the new identifiers
  (`MapView|CharTarget|TargetClass|pick_target|screen_to_tile|screen_to_real|real_to_tile|
  is_pt_in_map|is_in_map_area`) → matches confined to `src/input/**`
  (`f7401-precheck.txt`); plus proof that the `intent.rs`/`keybinds.rs`/`mod.rs` diffs are the
  §6 rows and that `tests.rs` has **0 deletions** (T1–T11 byte-identical).
- Write-guard confirmation: `git diff --stat` limited to §6; `src/client|game|graphics|auth|
  net|protocol`, `tests/**`, `Cargo.*`, matrix, contributing, README, `WO-8.0.md` and the legacy
  tree untouched.
- Four mutation probes recorded (§12): mutated diff, failing output, pristine restore, porcelain
  CLEAN after each, baseline re-run green afterwards.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test | Restore |
|---:|---|---|---|
| 1 | `screen_to_real`: delete the `+ MAP_TILE_YSIZE_HALF` term (`:187`'s trailing constant) | **T14** (G4 now yields `(-14,-2)` — the `+20` is what makes the truncation land at `-13/-1`) | `git checkout --` |
| 2 | `real_to_tile`: change the **y-branch** divisor from `MAP_TILE_XSIZE_HALF` to `MAP_TILE_YSIZE_HALF` (the "obvious symmetry fix" — legacy divides both axes by 40) | **T13** (G1's y becomes `4020/20 = 201 ≠ 100`) | `git checkout --` |
| 3 | `translate`: delete the `is_pt_in_map` gate (clicks past the screen edge compute and proceed) | **T17** (`(1000,340)` now yields `Some(MoveTo { 107, 94 })` instead of `None`) | `git checkout --` |
| 4 | `translate`: the `TargetClass::Npc` arm falls through to `Some(MoveTo { .. })` instead of `None` | **T21** (the friendly-NPC click now moves — D12's discard violated) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit 101
and the failing assertion, restore, verify `git status --porcelain` clean — then re-run the full
gate set to the delivered baseline (`WO-6.3.md` §9 / `evidence/7.x/mutation-probes.md`
discipline). Probe mix: two math-parity breaks (1–2) and two refusal/discard breaks (3–4),
covering the "parity with legacy" and "refuses/discards" claim classes of gate item 6.

## 13. Definition of done

A Step 8.1 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at the delivered baseline from `test.log` (**387** expected:
   351 lib + 11 fixtures + 25 e2e), `fmt` 0 diffs, `doc` 0 warnings, porcelain clean.
2. T12–T27 all present and green; **no pre-existing test modified** — T1–T11 byte-identical
   (the `tests.rs` diff shows additions only: new `use` lines + appended tests; 0 deletions),
   `event.rs`/`validate.rs` untouched.
3. Every decision D9–D16 is pinned by a failing-if-corrected test or probe (T12–T27 + probes
   1–4), and the three charter gate-1 parity pins — round-trip (T12), hand goldens (T13/T14),
   click→frozen `Order::Move` bytes (T19) — are green.
4. All four §12 probes show their named test failing at exit 101, with pristine restores and a
   green post-probe baseline.
5. Write guard holds: only §6 files changed; `src/client/**`, `src/graphics/**` (imported only),
   matrix, contributing, README, `WO-8.0.md`, `src/game/**` and the legacy tree untouched
   (row update, if any, is the Verifier's).
6. F-07 holds: every expectation traces to an E-row in §8 (E13–E29), and every E-row traces to
   a legacy `file:line`, the §2.2 hand arithmetic, or a frozen constant — never to the code
   under test.
7. Zero-queue audit green (§11 machinery grep, no matches, F-8.0-01-corrected pattern) and the
   D7 structural claim still holds (no `.await`/buffer between `InputEvent` and
   `CommandSender::send` — the click arm is as synchronous as the key arms; T10 stays green).
8. No new crates, no new opcodes, no wire-byte changes (byte pins use the frozen
   `encode_order`); no window/Win32/network in any gate test; no live run as a gate.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

- **Branch:** `step/8.1-mouse-move` (cut from `main` @ `449d60f`, this WO's sign-off), delivery
  SHA **`f91897e`** — implementation only; this note lands as a follow-up docs commit on the
  same branch.
- **Baseline:** 371 → **387** (+16 = T12–T27, all lib; `asset_fixtures` 11 and e2e 25
  untouched; 1 ignored lib + 4 ignored `live_integration` preserved), counted from `cargo
  test` output, never from prose (351 lib + 11 + 25). T1–T11 remain in the 351 and are
  byte-identical (0 deletions).

**Gates at the delivery SHA, and re-run post-probe on the pristine tree (both runs green):**

| Gate | Command | Result |
|---|---|---|
| 1 | `RUSTFLAGS=-D warnings cargo check --all-targets` | exit 0 |
| 2 | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 3 | `cargo fmt --check` | exit 0 — 0 diffs |
| 4 | `cargo doc --no-deps` | exit 0 — 0 warnings |
| 5 | `cargo test` | exit 0 — **387 passed / 0 failed** (351 lib + 11 fixtures + 25 e2e) |

**T12–T27, each run individually** (`cargo test --lib <name>` → `1 passed; 0 failed`, 16/16):

`screen_to_tile_round_trips_draw_center_xy`, `screen_to_tile_matches_hand_computed_goldens`,
`screen_to_tile_truncates_toward_zero_like_legacy_c`,
`is_pt_in_map_gates_inclusive_screen_bounds`, `is_in_map_area_is_half_open`,
`clicks_outside_the_screen_rect_are_discarded`,
`clicks_on_tiles_outside_the_map_are_refused_by_name`,
`click_on_empty_land_encodes_the_frozen_move_bytes`,
`click_on_an_enemy_produces_the_frozen_attack_order`,
`click_on_a_friendly_npc_is_discarded_not_moved`,
`click_on_a_friendly_player_falls_through_to_move`,
`target_hit_test_uses_strict_rect_containment_and_the_three_tile_filter`,
`nearest_center_wins_and_first_candidate_wins_ties`,
`release_and_hover_never_produce_intents_even_with_the_view_wired`,
`clicks_before_the_in_world_guard_are_discarded`, `clicks_act_while_the_chat_line_is_open`.

**Charter gate-1 parity pins green:** T12 (round trip, 66 pairs over 2 frozen rects × 3
centers × 11 tiles), T13/T14 (hand goldens G1–G5), T19 (click → `encode_order ==
[0, 100, 0, 100, 0]`), plus T20's attack bytes `[1, 7, 0, 0, 0]` — all through the frozen
`encode_order`, never new encoding code.

**Write guard (DoD 5):** `git diff --stat 449d60f..f91897e` = exactly the six §6 files
(786 insertions, 5 deletions): NEW `src/input/screen.rs` (90) and `src/input/target.rs` (103),
`intent.rs` (15), `keybinds.rs` (52), `mod.rs` (12, insertions only), `tests.rs` **519
insertions / 0 deletions** (additions-only diff, DoD 2). The 5 deletions are the sanctioned §7
edits: one `intent.rs` `Default`-doc line (§7.3) and the four `:93-96` placeholder lines
(comment + `Move | LeftDown | LeftUp => None` arm, §7.4). Forbidden paths (`Cargo.*`,
`tests/**`, `src/client|game|graphics|auth|net|protocol`, matrix, contributing, README,
`WO-8.0.md`, `GoonZu_Source/**`): diff empty. `event.rs`/`validate.rs` untouched.

**Gate-forced mechanical rewrites vs §7 (disclosed; semantics identical, no E-row affected):**

- §7.4's `let Some(view) = ui.view else { return None; };` → `let view = ui.view?;` —
  `clippy::question_mark` denies the former under `-D warnings` (gate 2); `?` on `Option`
  inside a function returning `Option<Intent>` is the same D10 discard.
- §7.2's explicit `pick_target<'a>` → elided single-input form — `clippy::needless_lifetimes`
  denies the explicit lifetime under `-D warnings`; with one input reference, elision binds
  the output to `candidates`'s lifetime either way (stated in the fn's doc comment).

**Findings:**

- **F-8.1-01 (disclosed, never edited — §1 write guard):** §11's literal zero-queue grep
  `grep -inE 'VecDeque|coalesc|MAX_KEY|cltKey' src/input/` returns **1** match, not "no
  matches": `src/input/event.rs:6` — the pre-existing Step 8.0 `//!` line citing the legacy
  `cltKey` payload (what was *not* ported; last touched by `fc1a7c7`, READ-ONLY for this
  step) — the same citation already adjudicated under F-8.0-01. The audit's intent holds: the
  machinery subset `VecDeque|coalesc|MAX_KEY` has **zero** matches, and `UiState.targets` is
  the §7.2-declared snapshot field that `translate` never mutates. Adjudication is the
  Verifier's.
- **F-7.4-01-style pre-check (§11):** among Rust sources the new identifiers
  (`MapView|CharTarget|TargetClass|pick_target|screen_to_tile|screen_to_real|real_to_tile|
  is_pt_in_map|is_in_map_area`) occur only inside `src/input/**`. Other hits are docs
  (`WO-8.1.md` defining them; pre-existing `CONTRIBUTING_PHASE4.md`/`TASK_MATRIX.md`/
  `WO-8.0.md` already naming `screen_to_tile`) and incidental legacy-tree `MapView`
  occurrences under `GoonZu_Source/**` — no Rust file outside `src/input/**` references any
  new identifier.

**Implementation notes:** the inverse is composed term-for-term from `Map-Client.cpp:185-187`
+ `MapCommon.cpp:965-966` — the trailing `+ MAP_TILE_YSIZE_HALF` at `:187` kept verbatim
(it is what lands G4 at (-13,-1)); `draw_center_xy`, `ScreenRect` and `MAP_TILE_*` are
imported from `src/graphics/**`, called never duplicated (D16); `translate` keeps its 2-arg
signature; `UiState`'s two new fields are `#[derive(Default)]`-compatible (`None`/`[]`),
which is what leaves T1–T11 compiling and passing untouched (D10); every assertion cites an
§8 E-row, the §2.2 hand arithmetic or a frozen constant — never the code under test (F-07);
no `.await`/buffer between `InputEvent` and `CommandSender::send` (D7 structural claim, T10
green); `Cargo.toml`/`Cargo.lock` untouched (no new crates, no new opcodes); no window/Win32/
network in any gate test (P4-C4).

**§12 mutation probes: 4/4** — each: one semantic break → named test fails at **exit 101**
→ `git checkout --` → `git status --porcelain` clean → full gate re-run green at 387:

| # | Mutation | Named test | Failing assertion observed |
|---:|---|---|---|
| 1 | `screen_to_real`: the trailing `+ MAP_TILE_YSIZE_HALF` deleted | **T14** | `E15: G4 truncates toward zero like C99 …` — `left: (-14, -2)`, `right: (-13, -1)` |
| 2 | `real_to_tile`: y-branch divisor `MAP_TILE_XSIZE_HALF` → `MAP_TILE_YSIZE_HALF` | **T13** | `E14: G1 — the rect center lands on the view center` — `left: (100, 201)`, `right: (100, 100)` |
| 3 | `translate`: the `is_pt_in_map` gate deleted | **T17** | `E18: off-screen click (1000, 340) is discarded …` — `left: Some(MoveTo { x: 107, y: 94 })`, `right: None` |
| 4 | `translate`: the `TargetClass::Npc` arm falls through to `Some(MoveTo { … })` | **T21** | `E22: Npc class → typed None, never a MoveTo …` — `left: Some(MoveTo { x: 100, y: 100 })`, `right: None` |

Probe runs carry no `RUSTFLAGS` (WO §12's plain named-test run), so probe 3's deleted gate
also surfaced its `unused import: is_pt_in_map` as a non-fatal warning — the failure itself
is T17's assertion as tabled. Post-probe full gate re-run on the restored tree: all five
gates exit 0, `cargo test` = **387** (351 + 11 + 25), porcelain clean.

### Verdict - Verifier
