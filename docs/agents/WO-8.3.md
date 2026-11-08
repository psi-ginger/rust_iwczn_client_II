# Work Order - Step 8.3: Driver Wiring — `wm_to_input` Decode, `InputFeed`, Loopback Proof

**Status:** `ACCEPTED` — definition-before-implementation satisfied by this WO's commit on
`main` (Scribe lane, under the Custodian's Step 8.3 directive, 2026-11-08) **before** any
`src/**` byte moves. The Implementer may pull and begin.
**Date:** 2026-11-08 · **Revision:** Rev 1 (initial, Scribe lane under directive).
**Branch:** `step/8.3-driver-wiring`, to be cut from `main` at this WO's commit.
**Baseline (re-derived this session):** `main` @ `3440453`, porcelain clean, all five mandatory
gates exit-0 — `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` under
`RUSTFLAGS=-D warnings`, `cargo fmt --check` 0 diffs, `cargo test` **399/399** (363 lib +
11 `asset_fixtures` + 25 `e2e_loopback`; 1 ignored lib + 4 ignored `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `TASK_MATRIX.md` Phase IV tracker (row 8.3 `PENDING`, est. +12–16, lanes
Implementer → Verifier + Adversary) · `CONTRIBUTING_PHASE4.md` (P4-C1…C4, verification gates
1–6, `src/client/**` READ-ONLY, `src/graphics/**` "8.3 (`window.rs` WM_* arms only)" ·
`docs/agents/WO-8.0.md` (§7 chain contract, §8 E1–E12 — E12 is T51's budget row) ·
`docs/agents/WO-8.1.md` (§7 mouse surface; D9 defers the +80/+40 decode to this step) ·
`docs/agents/WO-8.2.md` (§7 keyboard surface; D9–D12, numbering collision disclosed in §1) ·
`docs/agents/WO-7.4.md` (`CommandSender`/`run_with_commands`) ·
`docs/agents/WO-step7-in-world-actions.md` §8.3 (frozen goldens A/C/D — the F-07 source) ·
`docs/agents/WO-6.3.md` §9 (mutation-probe precedent).

---

## 1. Provenance, governance, lane hats

- **Registration & scope (Custodian directive, 2026-11-08).** Row 8.3 is the binding scope:
  *"Driver Wiring — `wm_to_input` decode arms in `window.rs` → translator → `CommandSender`
  (pump thread); `src/client/**` untouched"*, est. +12–16 tests, lanes
  Implementer → Verifier + **Adversary (mandatory)**. The matrix is untouched by this WO —
  status is the Verifier's to set, per `TASK_MATRIX.md` header rule 4.
- **Sequence.** 8.1 (mouse, merged `7844a37`) and 8.2 (keyboard, merged `49eead6`) are both
  `PASSED`; `main` sits at `3440453` with the combined baseline **399/399** (re-derived above).
  This is the first WO authored after both merges — no parallel lane is active, so the
  byte-identity guarantees below run against the *merged* tree, not against a sibling branch.
- **Numbering disclosure (the 8.1/8.2 collision, restated).** WO-8.1 owns **D9–D16 /
  E13–E29 / T12–T27**; WO-8.2 — authored in a parallel lane before 8.1 merged — re-used
  **D9–D12 / E13–E19** while continuing the T-series at T28–T39 (no T collision). This WO
  therefore starts at **D17+ / E30+ / T40+**: `D17–D23` (7 decisions), `E30–E41` (12
  expectation rows), `T40–T51` (12 tests). Where this document cites an 8.2 row it is
  qualified `WO-8.2 §8 E16` style — never a bare `E16`.
- **Lanes:** Implementer → Verifier + **Adversary** — every load-bearing claim here is
  "parity with legacy", "decodes/refuses", or "the frozen bytes reach the wire", all
  dischargeable by verified mutation probes (§12), per `CONTRIBUTING_PHASE4.md` gate item 6.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs and
  `docs/**` is READ-ONLY — a defect found there is a finding `F-8.3-NN`, never an edit (P3-6;
  F-7.4-01 disclosure pattern if the compiler ever forces one). Two guardrails are explicit:
  1. **`src/client/**` is READ-ONLY phase-wide** (CONTRIBUTING:165-167) — the receiving half
     is complete since 7.4; this step only *feeds* it.
  2. **`src/graphics/**` via `window.rs` "WM_* arms only"** (CONTRIBUTING:169, matrix row
     8.3): read here as the input additions only — the `WM_*` constants, `wm_to_input`, the
     `WindowState.input` field and its `None` initialization, the `window_proc` input arm, the
     imports they need, one module-doc sentence, and the `mod tests` line. The render,
     present, tick, pump and demo-geometry code stays byte-identical (§13.5 audits it at hunk
     level).
- **Surface-stability guarantee (replaces 8.1/8.2's parallel-lane guarantee).**
  `translate`'s table (`keybinds.rs`), `event.rs`, `validate.rs`, `intent.rs` (all six
  `Intent` variants, all six `InputEvent` variants, all eight `UiState` fields), and the
  mouse/keyboard arms delivered by 8.1/8.2 are **byte-for-byte untouched** by this delivery —
  DoD items §13.2/§13.5. `src/input/tests.rs` is **append-only after line 1362** (T1–T39 stay
  byte-identical).
- **Source-of-record split.** This step adds **no new wire bytes**: the 240/117 layouts stay
  `WO-step7-in-world-actions.md` §8.3 (frozen — its literals are this WO's goldens, never
  regenerated from the encoder), the sending half stays `src/client/commands.rs` (7.4, frozen),
  the chat cap stays `action.rs:53`, and `Intent`/`InputEvent`/`UiState`/`validate`/`translate`
  stay exactly as 8.0–8.2 delivered them. This WO owns only the `window.rs` decode (§7.1–7.2),
  the new `InputFeed` (§7.3), and the tests (§9), each expectation traced to a legacy
  `file:line`, a frozen constant, or the §2.4 hand arithmetic (F-07).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 What 8.0–8.2 delivered (the surface this step wires) — as of `3440453`

| Piece | Delivered behavior | Where |
|---|---|---|
| `translate` | `!in_world → None` checked first, then the full table (keys, clicks, hover) | `src/input/keybinds.rs:46-209` |
| click chain | `LeftDown`: `view?` → `is_pt_in_map` → `screen_to_tile` → `is_in_map_area` → `pick_target` → `MoveTo`/`AttackTarget`/`None` | `keybinds.rs:166-203` |
| inert rows | `Move` / `LeftUp` → `None` (hover/release never enqueue) | `keybinds.rs:207` |
| `InputEvent` | six variants, `Copy`, pure data — "decoded from a window message (8.3) or built by a test" | `src/input/event.rs:8-48` |
| `Intent` / `UiState` | six variants; eight fields incl. `view: Option<MapView>` (default `None` — "8.3 wires it") | `src/input/intent.rs:12-37`, `:48-74` |
| `validate` | intent → `Option<ClientCommand>`; typed refusals, never clamps | `src/input/validate.rs:16-48` |
| `CommandSender::send` | synchronous; `Err(CommandError::Closed)` when the receiver is gone | `src/client/commands.rs:61-71`, `:47-52` |
| `channel()` / `run_with_commands` | the frozen 7.4 channel and its receiving half | `commands.rs:93-96`; `src/client/driver.rs:475-481` |
| loop mirror | `Wake::CommandsClosed → commands = None` (the session survives its input layer) | `src/client/event_loop.rs:547` |
| test `feed` helper | the production chain shape: `translate → validate → sender.send` | `src/input/tests.rs:407-419` |
| `wired()` test view | screen `(0,0,959,679)`, center `(100,100)`, map `(400,400)` | `src/input/tests.rs:1327-1343` |
| T10 latency | 100 events through the chain in `< 16 ms`, margin documented | `src/input/tests.rs:306-381` |
| window module | `WindowState` `:293`, `DEMO_STATE` `:320`, `window_proc` `:423` (`_` → `DefWindowProcA` `:445`), `TranslateMessage` in the pump `:343`, `CS_DBLCLKS` `:694`, `build_scene` `:507-580`, `run_demo` `:595`, file ends `:784` (ignored operator test) | `src/graphics/window.rs` |
| module gating | `#[cfg(windows)] pub mod window;` — everything in `window.rs` compiles on Windows only | `src/graphics/mod.rs:56-57` |
| callers | **zero** non-test callers of `translate`/`validate`/`InputEvent` outside `src/input/**`, and zero of `wm_to_input`/`InputFeed`/`enter_world` anywhere (tree grep 2026-11-08) — 8.3 is the first | — |

### 2.2 Constraints discovered (each forces a design choice or a documented divergence)

1. **P4-C4 fixes the decode signature** (`CONTRIBUTING_PHASE4.md:120-126`): `wm_to_input`
   takes raw `wparam`/`lparam` **integers** so it is unit-testable without a message; "the
   Win32 `WindowProc` arms and any on-screen behavior are operator-only (`#[ignore]`d)
   precedent from Step 7.2". Consequence: T40–T42/T51 are integer-only, and the arm itself
   carries no gate test (D21's discharge path, §13.3).
2. **The `window.rs` guardrail is input-only** (CONTRIBUTING:169). One doc consequence: the
   module-doc `WindowProc` bullet (`window.rs:25-28`, "everything else falls through to
   `DefWindowProc`") becomes stale once seven message types are armed — it gains one sentence
   (disclosed in §6; doc-only, no behavior byte).
3. **`src/client/**` frozen ⇒ the stage-entry trigger cannot be delivered here.** Only
   `src/client/**` knows when the session reaches `Stage::InWorld` (`driver.rs:121,746`) and
   it is READ-ONLY for all of Phase IV. So this step delivers the *mechanism* (`enter_world`,
   D20) but not the live *trigger*: the tests stand in for composition, and wiring the pump
   thread's `CommandSender` + stage trigger into `src/main.rs` is 8.4's scope (divergence (a),
   §8).
4. **Single-writer pump (D1 preserved).** `window_proc` runs on the demo's one thread (the
   `run_demo` loop, `window.rs:595+`), and `DEMO_STATE` is an `AtomicPtr` (`window.rs:320`).
   The arm takes `&mut WindowState` through that pointer exactly as `render_and_present` does
   (`window.rs:359-361`) — no new synchronization, no second writer, no `static mut`.
5. **`CS_DBLCLKS` is in the class style** (`window.rs:694` ← `main.cpp:852-862`), so on a
   double-click Windows withholds the second `WM_LBUTTONDOWN` and sends
   `WM_LBUTTONDBLCLK` instead — this is what forces D18.
6. **Legacy button coordinates differ in provenance from ours.** Legacy's button arms push
   `IpD.Mouse_X/Y` — the *last* `WM_MOUSEMOVE` position (`Client.cpp:7689-7694`) after
   `SetMousePos`'s toolbar shift (`MouseOperation.cpp:3369-3384`); our decode reads the button
   message's own `lparam` (Windows packs the cursor position into every mouse message), and
   the port has no toolbar (Q3: no InterfaceMgr widgets) → divergence (c), §8.
7. **The harness is frozen, so the new binary re-declares it.** `tests/e2e_loopback.rs` and
   `tests/e2e_loopback/**` are READ-ONLY; `tests/input_loopback.rs` therefore re-declares the
   three harness modules at *its own* crate root with the same `#[path]` form e2e_loopback.rs
   itself uses (`:26-35`) — `frames.rs`, `front_double.rs`, `game_double.rs` (their
   `use crate::frames::{…}` then resolves; D22). `robustness.rs`/`soak.rs` are deliberately
   **not** re-declared: they mirror e2e_loopback's root helpers (not ours) and contribute no
   test this step needs — the §9 count math counts only `frames.rs`'s **3 inner tests**
   re-running in the new binary.
8. **Latency (P4-C3): this step adds structural cost, so it adds a test.** 8.2 correctly added
   none (two field comparisons, WO-8.2 §2.2-7); 8.3 adds a decode match + integer arithmetic
   per message *in front of* the chain, so T51 measures the **composed**
   `wm_to_input → InputFeed::feed` path with T10's documented margin rationale
   (`tests.rs:306-316`), never a relaxed one (D23).
9. **Doc-gate portability.** `graphics::window` is `#[cfg(windows)]`; `src/input/**` docs must
   never intra-doc-link it (a broken link on a non-Windows doc build fails `cargo doc`
   0-warnings) — `feed.rs` mentions the window side in plain text only.
10. **`-D warnings` gates the new binary's imports.** The mirrored helpers/constants in
    `tests/input_loopback.rs` must be exactly the ones used (no `run_driver`, no `Capture`,
    no `spawn_game` wrapper) — an unused mirror is a gate failure, not a style nit.

### 2.3 Legacy semantics inherited (why these behaviors exist)

| Legacy behavior | Citation |
|---|---|
| `WM_MOUSEMOVE` → `MouseMoveAction(LOWORD(lParam), HIWORD(lParam))`, `return 0` | `mainfunction.cpp:208-217` |
| `WM_LBUTTONDBLCLK` → pushes a left-press key at `IpD.Mouse_X/Y`, `return 0` | `mainfunction.cpp:218-230` |
| `WM_LBUTTONDOWN` → pushes a left-press key, `return 0` | `mainfunction.cpp:232-249` |
| `WM_LBUTTONUP` → pushes a no-flag key, `return 0` | `mainfunction.cpp:251-284` |
| `WM_RBUTTONDOWN/UP` → push right-press keys, `return 0` (**not ported** — right-click is a charter non-goal) | `mainfunction.cpp:285-311` |
| `WM_KEYUP` bound keys `return 0`; default `return 0` | `mainfunction.cpp:1202-1258` |
| `WM_CHAR` handles map-edit digits only, then `break` → `DefWindowProc` | `mainfunction.cpp:1261-1280` |
| `WM_KEYDOWN` bound keys `return 0`/`FALSE`; **unbound keys `break` at `:1356` → `DefWindowProc`** | `mainfunction.cpp:1282-1356`, `:1695` |
| the window switch's tail: `return DefWindowProc(hWnd, message, wParam, lParam)` | `mainfunction.cpp:1695` (= `window.rs:445` fallthrough) |
| `GetFrame() > 40` warm-up gate → this port's `in_world` boolean (8.0 D4) | `mainfunction.cpp:221,236,254,288,302,1367` |
| window→backbuffer translation `+ MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY` | `MouseOperation.cpp:232-233`, `Client.cpp:7709-7710`, `map.h:33,35` |
| `siRX = siRY = 0` (no camera; the present crop origin is `(80,40)`) | `Client.cpp:1005` ← `present.rs:11-13,41-45,66-67` |
| the pump already runs `TranslateMessage` before `DispatchMessageA` (so `WM_CHAR` exists) | `window.rs:341-344` ← `main.cpp:1559-1573` |
| class style carries `CS_DBLCLKS` (double-clicks become `WM_LBUTTONDBLCLK`) | `window.rs:694` ← `main.cpp:852-862` |

### 2.4 Hand arithmetic (the F-07 source for §9's wire goldens — never re-derived from code)

**G1 — a left click at window client `(480, 360)` (T49):**
1. decode: `+80/+40` → backbuffer `(560, 400)` (`map.h:33,35`, `RX/RY = 0`);
2. `screen_to_real((560,400), center (100,100), rect (0,0,959,679))`:
   `mid = (480, 340)`; `realx = 560 − 480 − (−100·40 + 100·40) = 80`;
   `realy = 400 − 340 − (−100·20 − 100·20) + 20 = 4080`;
3. `real_to_tile`: `((80/2 + 4080)/40, (−80/2 + 4080)/40) = (103, 101)`;
4. `validate` → `Order::Move { x: 103, y: 101 }` → §8.3-**A** layout with `x = 0x0067`,
   `y = 0x0065` → wire **`0A 00 00 F0 00 00 67 00 65 00`** (10 bytes: `usSize 10`, `ucChk 0`,
   `cmd 240`, `kind 00`, LE `x`, LE `y`).

**G2 — `lParam = -1` (64-bit all-ones) into a button arm (T42):** `lParam as u32 =
0xFFFF_FFFF` → `LOWORD = HIWORD = 0xFFFF = 65535` (zero-extension, legacy-faithful) →
`+80/+40` → **`(65615, 65575)`** — an off-window coordinate that `is_pt_in_map` later refuses
(fail-safe, E34).

**Frozen goldens** (`WO-step7-in-world-actions.md` §8.3, literals only):
- **C — SitDown:** `06 00 | 00 | F0 00 | 02` = `06 00 00 F0 00 02` (6 bytes).
- **D — Chat `ChatMode::Normal (01)`, `"hi"`:** `08 00 | 00 | 75 00 | 01 68 69` =
  `08 00 00 75 00 01 68 69` (8 bytes).
- **A — Move layout** (x/y per G1) = `0A 00 00 F0 00 00 67 00 65 00` (10 bytes).

## 3. Scope decision — this delivery

**In:**

1. `src/graphics/window.rs` — the seven `WM_*` constants, `wm_to_input` (§7.1), the
   `WindowState.input` field + its `input: None` initialization, the `window_proc` input arm
   (§7.2), the imports they need, one module-doc sentence (§2.2-2), and
   `#[cfg(test)] mod tests;` at end of file. Render/present/tick/pump/demo code untouched.
2. `src/graphics/window/tests.rs` — **NEW**: T40, T41, T42, T51 (§9), integer-only (P4-C4).
3. `src/input/feed.rs` — **NEW**: `InputFeed` (D19/D20, §7.3).
4. `src/input/mod.rs` — one `pub mod feed;` line + one `pub use feed::InputFeed;` line.
5. `src/input/tests.rs` — **append** T43–T46 after line 1362 (zero deletions; T1–T39
   byte-identical).
6. `tests/input_loopback.rs` — **NEW**, `#![cfg(windows)]`: the D22 harness re-declaration,
   its mirrored helpers, and T47–T50 (§9).
7. `docs/agents/WO-8.3.md` — this document, committed **before** any code.
8. Twelve tests: baseline **399 → 414** expected (+15 = T40–T51's 12 + `frames.rs`'s 3 inner
   tests re-running in the new binary; §9); the delivered number is copied from `test.log`.

**Out:** see §5. Explicitly unchanged: `src/client/**`, `src/game/**`, `src/main.rs`,
`src/lib.rs`, `src/input/{event,intent,keybinds,screen,target,validate}.rs`,
`src/graphics/**` except `window.rs`/`window/tests.rs`, `tests/e2e_loopback.rs` and
`tests/e2e_loopback/**`, `Cargo.*`.

## 4. Architectural decisions (binding for this WO)

- **D17 — `wm_to_input(msg: u32, w_param: usize, l_param: isize) -> Option<InputEvent>`.**
  A pure, integer-only decode in `window.rs` (P4-C4): no HWND, no message struct, no Win32
  call — a test hands it raw numbers. Exhaustive table:

  | `msg` | Result |
  |---|---|
  | `WM_MOUSEMOVE` (`0x0200`) | `Move { x, y }` |
  | `WM_LBUTTONDOWN` (`0x0201`) | `LeftDown { x, y }` |
  | `WM_LBUTTONUP` (`0x0202`) | `LeftUp { x, y }` |
  | `WM_LBUTTONDBLCLK` (`0x0203`) | `LeftDown { x, y }` (D18) |
  | `WM_KEYDOWN` (`0x0100`) | `KeyDown { vk: w_param as u32 }` |
  | `WM_KEYUP` (`0x0101`) | `KeyUp { vk: w_param as u32 }` |
  | `WM_CHAR` (`0x0102`) | `CharInput { ch: char::from_u32(w_param as u32)? }` — an invalid scalar (surrogate half) → `None` |
  | anything else | `None` (the decode is total; never panics) |

  Mouse coordinates: legacy's `LOWORD/HIWORD` extraction of `lParam`
  (`mainfunction.cpp:213`) zero-extended to `i32` (WO-8.1 §8 (a): legacy's `SHORT` stays in
  range for on-screen coordinates), **plus** the window→backbuffer border
  `+ MAP_TILE_XSIZE + SCREEN_RX` / `+ MAP_TILE_YSIZE + SCREEN_RY` = `+80/+40`
  (`MouseOperation.cpp:232-233`, `Client.cpp:7709-7710`, `map.h:33,35`, `RX/RY = 0`
  `Client.cpp:1005`, crop origin `present.rs:66-67`) — the deferred half of WO-8.1 D9, the
  constants reused from `super::screen`/`super::present` (never hand-literals). The resulting
  coordinates are `ScreenRect`/backbuffer space — the space `translate`'s mouse rows and
  `is_pt_in_map` already operate in (`screen.rs:10-16`).
- **D18 — `WM_LBUTTONDBLCLK` → `LeftDown` (a stated divergence, never silent).** The class
  style carries `CS_DBLCLKS` (`window.rs:694` ← `main.cpp:852-862`), so Windows *withholds*
  the second `WM_LBUTTONDOWN` of a double-click and delivers `WM_LBUTTONDBLCLK` instead;
  legacy's own arm pushes a left-press key (`mainfunction.cpp:218-230`), i.e. one press
  intent per physical press. Mapping DBLCLK → `LeftDown` preserves exactly that contract: two
  physical presses still yield two `LeftDown` intents, no more, no fewer. **No double-click
  machinery** is ported — legacy's `IpD.DoubleLeftPressSwich` / `OperateAutoDoubleLeftOrder`
  (`mainfunction.cpp:229`, `MouseOperation.cpp:3386+`) stay deferred with 8.0 §5 / 8.1 D11's
  right-click deferral. T40 pins the mapping (failing-if-corrected).
- **D19 — `InputFeed`, the production shape of the delivered test helper.** New
  `src/input/feed.rs`:

  ```rust
  pub struct InputFeed { ui: UiState, sender: CommandSender }   // #[derive(Debug)]
  impl InputFeed {
      pub fn new(sender: CommandSender) -> Self;               // ui = UiState::default()
      pub fn feed(&mut self, event: InputEvent)
          -> Result<Option<ClientCommand>, CommandError>;      // translate → validate → send
      pub fn ui(&self) -> &UiState;                            // read-only accessor
  }
  ```

  `feed` is the whole P4-C1 chain in one synchronous call — the byte-for-byte production
  shape of `tests.rs:407-419`'s `feed` helper, with the channel failure *surfaced by name*
  instead of `.expect`ed. `Ok(Some(command))` ⇔ exactly one `sender.send` happened;
  `Ok(None)` ⇔ `translate` or `validate` refused and **nothing** was sent;
  `Err(CommandError::Closed)` ⇔ the session is over (never a panic, never a retry).
  `#[derive(Debug)]` only (no `Clone`: the feed owns the session's sender, it is not a value
  to duplicate). `Send` holds by inspection (`UiState` + `CommandSender` are `Send`) and is
  asserted by T46 — it must ride the pump thread.
- **D20 — `enter_world(&mut self, view: MapView)`: the stage mechanism, one call.** Sets
  `ui.in_world = true` **and** `ui.view = Some(view)` together — the production mechanism
  behind 8.0's D4 guard and 8.1's D10 click context (both fields' docs already say "8.3 sets
  it"). **The live trigger cannot be delivered in Phase IV**: only `src/client/**` knows the
  stage and it is frozen read-only (§2.2-3), so the trigger arrives with 8.4's composition
  (`src/main.rs` wiring the window's pump thread to `driver::run_with_commands`). This step
  delivers the mechanism and exercises it end-to-end with tests standing in for composition —
  disclosed as divergence (a), §8.
- **D21 — the window side: an inert-by-default arm with a sender-side close mirror.**
  `WindowState` gains `input: Option<InputFeed>`, initialized **`None`** in `build_scene`
  (`window.rs:565-579`) — the 7.2 demo has no session, so the arm is wired but inert until
  8.4's composition (the operator test and the demo's on-screen behavior are unchanged).
  The `window_proc` arm, placed before the `_` fallthrough:
  1. decode with `wm_to_input`; `None` (bad `WM_CHAR` scalar, …) changes nothing;
  2. load `DEMO_STATE`, skip when null (the `WM_PAINT` pattern, `window.rs:433-436`),
     take `&mut *state` (single-writer pump, §2.2-4);
  3. `feed.feed(event)` — on `Err(CommandError::Closed)` **drop the feed**
     (`state.input = None`): the sender-side mirror of the loop's
     `Wake::CommandsClosed → commands = None` (`event_loop.rs:547`) — the pump keeps running,
     it just stops trying to send;
  4. **all seven armed message types return `0`** (decode succeeded or not, feed present or
     not); every other message keeps the existing `DefWindowProcA` fallthrough
     (`window.rs:445` ← `mainfunction.cpp:1695`) byte-for-byte.
  Stated divergence (b): legacy let *unbound* `WM_KEYDOWN` (`break` at `mainfunction.cpp:1356`)
  and non-map-edit `WM_CHAR` (`:1280`) fall through to `DefWindowProc`; ours returns 0 for the
  seven armed types because **binding knowledge lives in `translate`, not in decode** — a
  decoded message has already been fully classified. `DefWindowProc` is inert for these
  messages, so there is no user-visible effect. The arm itself has no gate test: P4-C4 makes
  Win32 arms operator-only (§2.2-1); its decode half is T40–T42 and its feed half T43–T46.
- **D22 — `tests/input_loopback.rs` re-declares the frozen harness at its own root.**
  `tests/e2e_loopback.rs` + `tests/e2e_loopback/**` are READ-ONLY, so the new integration
  binary declares, verbatim in form:

  ```rust
  #![cfg(windows)]                       // wm_to_input lives in a cfg(windows) module
  #[path = "e2e_loopback/frames.rs"]      mod frames;
  #[path = "e2e_loopback/front_double.rs"] mod front_double;
  #[path = "e2e_loopback/game_double.rs"]  mod game_double;
  ```

  (the same `#[path]` shape e2e_loopback.rs itself uses at `:26-35`), so the harness
  modules' `use crate::frames::{…}` resolves against the new root. The file then mirrors
  exactly the root items it uses — `bind`, `config`, `run_driver_with_commands`,
  `spawn_front`, `spawn_game_with`, `joined`, and the `TEST_USER`/`TEST_PASSWORD`/`TEST_MAC`/
  `DRIVER_TIME_BUDGET`/`DOUBLE_TIME_BUDGET` constants — importing only what it uses
  (`-D warnings`, §2.2-10). `robustness.rs`/`soak.rs` are not re-declared (§2.2-7).
  The pre-world test (T50) uses `game_double::Tail::Commands { expect: 0 }`, whose
  `commands_tail` loop body is skipped entirely (`game_double.rs:689-736`) → immediate
  reply-less FIN, no 10 s heartbeat wait, `Stage::TornDown` — the fast, deterministic shape.
- **D23 — T51 measures the composed decode+feed chain, with T10's margin rationale.**
  100 raw messages through `wm_to_input → InputFeed::feed` must land inside one 16 ms frame
  budget (P4-C3), asserted as a batch exactly like T10: the margin rationale is restated
  verbatim from `tests.rs:306-316` (100 events ≫ one frame's load, µs-scale enum dispatch +
  mpsc push ⇒ ~1 ms expected, ≥10× headroom) **plus** the decode's cost (one `match` and
  integer arithmetic — also µs-scale). Justified as new because 8.3 adds structural cost in
  front of the chain (§2.2-8). Any flake is disclosed and adjudicated (P4-C3) — never
  silently relaxed.

## 5. Non-goals

- **No `src/client/**` change of any kind** (READ-ONLY phase-wide): no stage trigger, no
  event-loop edit, no `CommandSender` wiring into the driver — a defect there is a finding.
- **No `src/main.rs` / composition work** (8.4): no window+driver wiring, no live `in_world`
  trigger, no `run_demo` signature change.
- No changes to `translate`/`validate`/`Intent`/`InputEvent`/`UiState` (D12-style surface
  stability): no new variant, no new field, no table row — 8.1/8.2's delivered bytes are the
  contract (§13.5).
- **No right-click** (`WM_RBUTTONDOWN/UP`, legacy `mainfunction.cpp:285-311`), no mouse
  wheel, no `WM_SYSKEYDOWN`/`WM_SYSCHAR`, no `WM_COMMAND`/accelerator handling, no modifier
  modeling — charter non-goals (8.0 §5, 8.2 §5).
- **No double-click machinery** (D18): no `IpD.DoubleLeftPress*` state, no
  `OperateAutoDoubleLeftOrder` (`MouseOperation.cpp:3386+`) — deferred with 8.1 D11.
- No dialogs, no chat widget, no toolbar term (Q3); no forward-delete/caret work (8.2 D11).
- No queue/ring/cap/coalescing of any kind (P4-C1); no new crates (`Cargo.*` untouched), no
  new opcodes (Q2), no wire-byte changes; goldens are §8.3 literals only (F-07).
- No modification of T1–T39 (append-only after `tests.rs:1362`), no edits to
  `tests/e2e_loopback*` (frozen — reused read-only), no matrix/status edits (Verifier), no
  `README.md`, `CONTRIBUTING*`, `GoonZu_Source/**`, other-WO or existing-`evidence/**` edits.
- No live-server run as a gate (8.4's evidence only); no on-screen input behavior in this
  step (the demo's `input` stays `None`; the operator test stays `#[ignore]`).

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change |
|---:|---|---|
| 1 | `docs/agents/WO-8.3.md` | NEW — this document, committed **before** any code |
| 2 | `src/graphics/window.rs` | EDIT — input additions only: seven `WM_*` `pub const`s + docs; `wm_to_input` (§7.1); `WindowState.input` field + `input: None` in `build_scene`; the `window_proc` input arm before the `_` fallthrough (§7.2); the imports they need (`crate::input::{InputEvent, InputFeed}`; the `MAP_TILE_*`/`SCREEN_R*` constants appended to the existing `super::screen`/`super::present` use lines); one module-doc sentence on the input arm (§2.2-2); `#[cfg(test)] mod tests;` at end of file. **Render/present/tick/pump/demo hunks: zero.** |
| 3 | `src/graphics/window/tests.rs` | NEW — T40, T41, T42, T51 (§9); `use super::*` + integer inputs only (P4-C4: no window, no DC, no message struct) |
| 4 | `src/input/feed.rs` | NEW — `InputFeed` + module docs (§7.3); plain-text window references only (§2.2-9) |
| 5 | `src/input/mod.rs` | EDIT — exactly two lines: `pub mod feed;` (alphabetically after `event`) and `pub use feed::InputFeed;` (after the `InputEvent` re-export); pipeline docs untouched |
| 6 | `src/input/tests.rs` | EDIT — **append only** after line 1362: the 8.3 block header, its imports, T43–T46 (§9); T1–T39 byte-identical |
| 7 | `tests/input_loopback.rs` | NEW — `#![cfg(windows)]`, D22 module declarations + mirrored helpers/constants, three frozen-golden consts, T47–T50 (§9) |
| 8 | `evidence/8.3/` | NEW at verification, written by the Verifier only (§11) |

**READ-ONLY:** `src/client/**`, `src/game/**`, `src/main.rs`, `src/lib.rs`,
`src/input/{event,intent,keybinds,screen,target,validate}.rs`,
`src/graphics/{mod,archive,blit,draw_map,framebuffer,map,palette,present,screen,spr,tick}.rs`
and every `src/graphics/*/tests.rs`, `tests/e2e_loopback.rs`, `tests/e2e_loopback/**`,
`tests/asset_fixtures*`, `tests/live_integration*`, `Cargo.toml`/`Cargo.lock`,
`TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, every other file in
`docs/**`, every existing `evidence/**` directory. A defect found there is a finding, never an
edit (P3-6).

## 7. API & wiring spec (the shared contract)

### 7.1 `src/graphics/window.rs` — the message constants and `wm_to_input`

```rust
/// `WM_KEYDOWN` (`0x0100`) — a virtual-key press (`mainfunction.cpp:1282`).
pub const WM_KEYDOWN: u32 = 0x0100;
/// `WM_KEYUP` (`0x0101`) — a virtual-key release (`mainfunction.cpp:1202`).
pub const WM_KEYUP: u32 = 0x0101;
/// `WM_CHAR` (`0x0102`) — a character generated by the pump's
/// `TranslateMessage` (`mainfunction.cpp:1261`; pump `window.rs:343`).
pub const WM_CHAR: u32 = 0x0102;
/// `WM_MOUSEMOVE` (`0x0200`) — pointer motion (`mainfunction.cpp:208`).
pub const WM_MOUSEMOVE: u32 = 0x0200;
/// `WM_LBUTTONDOWN` (`0x0201`) — left press (`mainfunction.cpp:232`).
pub const WM_LBUTTONDOWN: u32 = 0x0201;
/// `WM_LBUTTONUP` (`0x0202`) — left release (`mainfunction.cpp:251`).
pub const WM_LBUTTONUP: u32 = 0x0202;
/// `WM_LBUTTONDBLCLK` (`0x0203`) — the class style carries `CS_DBLCLKS`
/// (`window.rs:694` ← `main.cpp:852-862`), so Windows sends this **instead
/// of** the second `WM_LBUTTONDOWN`; D18 maps it to one more `LeftDown`
/// (legacy pushes a left-press key too, `mainfunction.cpp:218-230`).
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
```

`wm_to_input` — signature exactly as D17 (raw integers, P4-C4):

```rust
/// Step 8.3 (WO-8.3 D17): one window message → one raw input fact, or
/// `None` for messages outside the table (and for a `WM_CHAR` whose
/// `wparam` is not a scalar value — `char::from_u32` refuses surrogates).
///
/// Pure and integer-only (P4-C4): no HWND, no message struct, no Win32
/// call — a test hands it raw `wparam`/`lparam` values. Mouse coordinates
/// are legacy's `LOWORD`/`HIWORD` extraction (`mainfunction.cpp:213`),
/// zero-extended to `i32`, plus the window→backbuffer border
/// `+ MAP_TILE_XSIZE/YSIZE + GetScreenRX/RY` (`MouseOperation.cpp:232-233`,
/// `Client.cpp:7709-7710`, `map.h:33,35`; `SCREEN_RX/RY = 0`,
/// `Client.cpp:1005`) — the origin the present crop uses
/// (`present.rs:66-67`), Q4 (WO-8.1 D9's deferred half).
#[must_use]
pub fn wm_to_input(msg: u32, w_param: usize, l_param: isize) -> Option<InputEvent> {
    // One 32-bit view of `lParam`: LOWORD = bits 0..16, HIWORD = bits 16..32
    // (legacy-faithful zero-extension — WO-8.1 §8 (a): no sign extension).
    let raw = l_param as u32;
    let (x, y) = (
        (raw & 0xFFFF) as i32 + MAP_TILE_XSIZE + SCREEN_RX,
        ((raw >> 16) & 0xFFFF) as i32 + MAP_TILE_YSIZE + SCREEN_RY,
    );
    match msg {
        WM_MOUSEMOVE => Some(InputEvent::Move { x, y }),
        WM_LBUTTONDOWN => Some(InputEvent::LeftDown { x, y }),
        WM_LBUTTONUP => Some(InputEvent::LeftUp { x, y }),
        WM_LBUTTONDBLCLK => Some(InputEvent::LeftDown { x, y }), // D18
        WM_KEYDOWN => Some(InputEvent::KeyDown { vk: w_param as u32 }),
        WM_KEYUP => Some(InputEvent::KeyUp { vk: w_param as u32 }),
        WM_CHAR => Some(InputEvent::CharInput {
            ch: char::from_u32(w_param as u32)?,
        }),
        _ => None,
    }
}
```

Constants come from `super::screen::{MAP_TILE_XSIZE, MAP_TILE_YSIZE}` (`screen.rs:30-34`) and
`super::present::{SCREEN_RX, SCREEN_RY}` (`present.rs:41-45`) — the frozen values, never
literals. `InputEvent` arrives via `use crate::input::{InputEvent, InputFeed};` (the existing
8.0 re-export, `input/mod.rs:34`).

### 7.2 `src/graphics/window.rs` — the state field and the arm

```rust
/// Step 8.3 (D19/D21): the input half of the pump thread. `None` in the
/// 7.2 demo — no session exists, so the `window_proc` input arm is wired
/// but inert (8.4's composition hands it a `CommandSender` and the stage
/// trigger, D20 divergence (a)). On `Err(CommandError::Closed)` the arm
/// drops it — the sender-side mirror of `Wake::CommandsClosed`
/// (`event_loop.rs:547`).
input: Option<InputFeed>,
```

`build_scene`'s `Ok(WindowState { … })` gains exactly one field: `input: None,`.

The arm, immediately before the `_ => DefWindowProcA(...)` fallthrough (which stays
byte-identical):

```rust
// Step 8.3 (D17/D21): decode → feed → the direct send (P4-C1). All seven
// armed types return 0 whether or not a feed exists; every other message
// keeps the DefWindowProc fallthrough (mainfunction.cpp:1695).
WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK
| WM_KEYDOWN | WM_KEYUP | WM_CHAR => {
    if let Some(event) = wm_to_input(msg, w_param, l_param) {
        let state = DEMO_STATE.load(Ordering::Relaxed);
        if !state.is_null() {
            let state = &mut *state; // one thread: the pump (D1 single-writer)
            let mut closed = false;
            if let Some(feed) = state.input.as_mut() {
                closed = feed.feed(event).is_err();
            }
            if closed {
                state.input = None; // mirror of event_loop.rs:547
            }
        }
    }
    0
}
```

(The two-step borrow is deliberate: `state.input = None` may not run while `feed` is
borrowed. The `is_err()` form needs no `CommandError` import — if the Implementer prefers a
`match` naming the variant, the import goes in §6's list.)

### 7.3 `src/input/feed.rs` — `InputFeed`

```rust
//! Step 8.3 (WO-8.3 D19/D20) — [`InputFeed`]: the production shape of the
//! chain every 8.0 test drove through its local helper — `translate` →
//! `validate` → `CommandSender::send`, one synchronous call per event
//! (P4-C1: no intermediate storage; P4-C3: no `.await` between the event
//! and `send`). The window side (`graphics::window::wm_to_input`, Windows
//! only) decodes raw messages into `InputEvent`s and hands them here; this
//! module stays cross-platform and never references it (doc-gate
//! portability, §2.2-9).

use super::event::InputEvent;
use super::intent::UiState;
use super::keybinds::translate;
use super::screen::MapView;
use super::validate::validate;
use crate::client::commands::{ClientCommand, CommandError, CommandSender};

/// One session's input half: translator state plus the channel it feeds
/// (D19). `Send` (T46) — it rides the window's pump thread; `Debug` only,
/// deliberately not `Clone` (it owns the session's sender).
#[derive(Debug)]
pub struct InputFeed {
    /// The translator-local state `translate`/`enter_world` mutate.
    ui: UiState,
    /// The frozen 7.4 sending half (`commands.rs:54-71`).
    sender: CommandSender,
}

impl InputFeed {
    /// A fresh feed over `sender`: pre-world idle state
    /// (`UiState::default()` — every event discarded until `enter_world`).
    #[must_use]
    pub fn new(sender: CommandSender) -> Self {
        Self { ui: UiState::default(), sender }
    }

    /// One event through the whole chain, synchronously (D19):
    /// `Ok(Some(command))` ⇔ exactly one command was sent;
    /// `Ok(None)` ⇔ `translate` or `validate` refused and nothing was sent;
    /// `Err(CommandError::Closed)` ⇔ the session is over — surfaced by
    /// name, never a panic, never a retry.
    ///
    /// # Errors
    ///
    /// [`CommandError::Closed`] when every receiver is gone (`commands.rs:47-52`).
    pub fn feed(&mut self, event: InputEvent) -> Result<Option<ClientCommand>, CommandError> {
        let Some(intent) = translate(event, &mut self.ui) else {
            return Ok(None);
        };
        let Some(command) = validate(&intent) else {
            return Ok(None);
        };
        self.sender.send(command.clone())?;
        Ok(Some(command))
    }

    /// The in-world stage reached, in one call (D20): `in_world = true`
    /// (8.0's D4 guard replaces `GetFrame() > 40`,
    /// `mainfunction.cpp:221,236,…`) **and** `view = Some(view)` (8.1's D10
    /// click context). The live trigger is 8.4's composition (divergence
    /// (a), §8); tests stand in for it here.
    pub fn enter_world(&mut self, view: MapView) {
        self.ui.in_world = true;
        self.ui.view = Some(view);
    }

    /// Read-only access to the translator state (T45's pin): `feed` and
    /// `enter_world` are the only mutators.
    #[must_use]
    pub fn ui(&self) -> &UiState {
        &self.ui
    }
}
```

### 7.4 `src/input/mod.rs` — two lines

```rust
pub mod feed;          // after `pub mod event;`
pub use feed::InputFeed;   // after `pub use event::InputEvent;`
```

Nothing else in `mod.rs` changes (the pipeline diagram and step-8.1 paragraph stay verbatim).

### 7.5 Unchanged surfaces (explicit)

`Intent` (all six variants, `intent.rs:12-37`), `InputEvent` (all six, `event.rs:8-48`),
`UiState` (all eight fields, `intent.rs:48-74`), `translate` (`keybinds.rs:46-209` — every
row of WO-8.0 §7.3 / WO-8.1 §7.4 / WO-8.2 §7.2 byte-identical, including the mouse arms),
`validate` (`validate.rs:16-48`), the `VK_*` constants, `screen.rs`/`target.rs`,
`src/client/**` (the whole receiving half), the pump (`TranslateMessage`/`DispatchMessageA`),
the render/present/tick/demo paths, and the 7.2 demo's behavior (`input: None` → the arm is
inert; the ignored operator test unchanged). Any delivered byte differing from these is a
write-guard violation (§13.5).

## 8. Expectation table (the F-07 source for §9)

E1–E12 (WO-8.0 §8) remain in force — T1–T11 keep asserting them; **E12** (100 events
`< 16 ms`, `WO-8.0.md:322`) is T51's budget row. E13–E29 (WO-8.1 §8) and the 8.2 rows
(cited qualified) stay in force for their tests. The new rows:

| # | Behavior (as tested) | Provenance |
|---|---|---|
| **E30** | `wm_to_input` decodes the three ordinary mouse messages into backbuffer space: `WM_MOUSEMOVE` → `Move`, `WM_LBUTTONDOWN` → `LeftDown`, `WM_LBUTTONUP` → `LeftUp`, each at `LOWORD/HIWORD(lParam) + 80/+40` (window `(480,360)` → `(560,400)`) | `mainfunction.cpp:208-217` (MOVE → `MouseMoveAction(LOWORD, HIWORD)`, `return 0`), `:232-249` (DOWN, `return 0`), `:251-284` (UP, `return 0`); offset `MouseOperation.cpp:232-233` + `Client.cpp:7709-7710` + `map.h:33,35`; `RX/RY = 0` `Client.cpp:1005` ← `present.rs:41-45,66-67` |
| **E31** | `WM_LBUTTONDBLCLK` decodes to `LeftDown` — one intent per physical press under `CS_DBLCLKS` (the second DOWN never arrives); no double-click state is created | `mainfunction.cpp:218-230` (DBLCLK pushes a left-press key, `return 0`); `CS_DBLCLKS` `window.rs:694` ← `main.cpp:852-862`; D18 |
| **E32** | `WM_KEYDOWN` → `KeyDown { vk = wparam }`, `WM_KEYUP` → `KeyUp { vk = wparam }` (RETURN `0x0D`, INSERT `0x2D`, F1 `0x70` round-trip exactly) | `mainfunction.cpp:1282` (WM_KEYDOWN), `:1202` (WM_KEYUP); the pump already runs `TranslateMessage` (`window.rs:343` ← `main.cpp:1559-1573`) |
| **E33** | `WM_CHAR` → `CharInput { ch }` for `wparam = 'h'` (`0x68`); `wparam = 0xD800` (surrogate half) → `None` — typed refusal, never a panic | `mainfunction.cpp:1261-1280` (WM_CHAR arm); P4-C4 / WO-8.0 "never panics" invariant (`keybinds.rs:40`) |
| **E34** | Coordinates are 16-bit **zero-extension** of `lParam`: `lParam = -1` → `(65615, 65575)` after `+80/+40` (§2.4 G2) — an off-window value that `is_pt_in_map` later refuses (fail-safe) | `mainfunction.cpp:213` (`LOWORD`/`HIWORD`); WO-8.1 §8 (a) (i32 vs legacy `SHORT`, no in-range value differs); `map.h:33,35` |
| **E35** | The decode is total: `WM_RBUTTONDOWN (0x0204)`, `WM_PAINT (0x000F)`, `WM_DESTROY (0x0002)`, `WM_QUIT (0x0012)` → `None`; the arm returns 0 only for the seven armed types, everything else keeps `DefWindowProcA` | `mainfunction.cpp:1695` (`return DefWindowProc`); `window.rs:445` `_` fallthrough; D21 |
| **E36** | `InputFeed::feed` is the whole chain, synchronous: a valid intent returns `Ok(Some(command))` with **exactly one** command drained from the channel; `CloseDialog` and an empty-line `SendChat` return `Ok(None)` with **zero** sent | P4-C1 (`CONTRIBUTING_PHASE4.md:63-90`); the delivered production-shape helper `tests.rs:407-419`; `validate.rs:47` (UI-only → `None`), `:35-37` (empty → `None`); `commands.rs:68-70` |
| **E37** | Before `enter_world`, every event is discarded (`Ok(None)`, `ui()` byte-identical to `UiState::default()`); after `enter_world(view)` the state is `in_world = true` **and** `view = Some(view)` in one call, and the same click now enqueues `Order::Move { 103, 101 }` | warm-up gate `mainfunction.cpp:221,236,254,288,302,1367` → D4 guard `keybinds.rs:50-52`; WO-8.1 D10 (`view` default `None`, `intent.rs:57-60`); hand arithmetic §2.4 G1 |
| **E38** | A dead receiver surfaces `Err(CommandError::Closed)` **by name** from `feed` (never a panic, never a retry), with `ui()` unchanged; `ui()` is a read-only accessor (`&UiState`) — `feed`/`enter_world` are the only mutators | frozen `commands.rs:47-52` (`CommandError::Closed`); the loop-side mirror `event_loop.rs:547` (dropping the input layer must not kill a session) |
| **E39** | `InputFeed: Send` (compile-time — it rides the pump thread, D19) and `Debug`-formattable; the no-queue shape of `feed.rs` is §11's grep (P4-C1) | `CONTRIBUTING_PHASE4.md:63-90` (P4-C1), `:111-112` (synchronous feed); `commands.rs:54-59` (the sender is cloned *into* the input layer) |
| **E40** | Raw messages through decode + feed + driver reach the wire **byte-exact** against the frozen goldens: `WM_KEYDOWN(0x2D)` → `06 00 00 F0 00 02` (C); `WM_KEYDOWN(0x0D)` + `WM_CHAR 'h'` + `WM_CHAR 'i'` + `WM_KEYDOWN(0x0D)` → `08 00 00 75 00 01 68 69` (D, mode `01`); `WM_LBUTTONDOWN` at window `(480,360)` → `0A 00 00 F0 00 00 67 00 65 00` (A layout, tile `(103,101)`) — each as the session's only C2S frame, in order, then a clean `Stage::TornDown` | `WO-step7-in-world-actions.md` §8.3-**A/C/D** (frozen literals, never regenerated); §2.4 hand arithmetic; legacy arms `mainfunction.cpp:1218/:1229/:232`; harness shape T6 (`e2e_loopback.rs:1170-1250`) |
| **E41** | Raw messages **before** the stage entry each return `Ok(None)` (zero enqueued) yet the session still completes byte-exact and `Stage::TornDown` with zero heartbeats — the armed arm stays inert until `enter_world` | D4 guard `keybinds.rs:50-52` ← `mainfunction.cpp:221,236,…`; `Tail::Commands { expect: 0 }` fast path `game_double.rs:689-736`; `event_loop.rs:547` (an empty channel never disables a live session) |

**Stated divergences (Haru/CAPA — documented, never silent):**
(a) **the stage-entry trigger is not delivered in Phase IV** — only `src/client/**` knows the
stage and it is READ-ONLY phase-wide; `enter_world` (D20) is the mechanism, its live trigger
and the pump-thread `CommandSender` wiring arrive with 8.4's `src/main.rs` composition;
tests stand in for composition here, so "the driver told the input layer the world is up" is
exercised end-to-end but not production-connected;
(b) **return-0 policy** — legacy let unbound `WM_KEYDOWN` (`break`, `mainfunction.cpp:1356`)
and non-map-edit `WM_CHAR` (`:1280`) fall to `DefWindowProc` (`:1695`); ours returns 0 for
all seven armed types because binding knowledge lives in `translate`, not decode
(`keybinds.rs` decides what a key *means*); `DefWindowProc` is inert for these messages, so
no user-visible effect — unarmed messages keep the fallthrough byte-for-byte;
(c) **button-message coordinates** — legacy's button arms push `IpD.Mouse_X/Y`: the last
`WM_MOUSEMOVE` position (`Client.cpp:7689-7694`) after `SetMousePos`'s toolbar shift
(`MouseOperation.cpp:3369-3384`); ours reads the button message's own `lparam` (Windows
packs the cursor position into every mouse message) with no toolbar term — the port has no
toolbar (Q3: no InterfaceMgr widgets), and the `+80/+40` border is the entire translation the
present crop assumes (`present.rs:11-13`); equivalent for the port's map surface, stated not
silent;
(d) **no double-click machinery** — legacy's DBLCLK arm also flips `IpD.DoubleLeftPressSwich`
and `OperateAutoDoubleLeftOrder` reacts to it (`mainfunction.cpp:229`,
`MouseOperation.cpp:3386+`); D18 ports only the one-press-intent half, deferring the rest
with 8.0 §5 / 8.1 D11;
(e) **right-button messages are unarmed** — legacy pushed right-press keys
(`mainfunction.cpp:285-311`); ours returns `None` → `DefWindowProc`, because right-click
remains a charter non-goal (8.0 §5) and `DefWindowProc` is inert for them.

## 9. Test invariants (T40–T51) and the baseline

Pattern authorities: as WO-8.0 §9 (colocated unit tests, pure-function tables, typed-refusal
assertions) and WO-6.2 §5 (loopback harness). F-07: expectations from §8 (or E12) only —
goldens are §8.3 literals, hand arithmetic is §2.4, never read back from the code under test.
T1–T39 are not modified (§10). `wm_to_input` tests build raw integers (P4-C4); no test in
this step constructs a window, DC, message or network object beyond the two frozen doubles.

| # | Invariant | File | Phase |
|---|---|---|---|
| **T40** | `wm_to_input_decodes_the_mouse_messages` — E30/E31: `WM_MOUSEMOVE`/`WM_LBUTTONDOWN`/`WM_LBUTTONUP` at packed `lParam` `(480,360)` → `Move`/`LeftDown`/`LeftUp` `{560, 400}` (the `+80/+40`); at `(10,20)` → `{90, 60}`; **`WM_LBUTTONDBLCLK` → `LeftDown`** (D18, same coordinates) | `src/graphics/window/tests.rs` | unit |
| **T41** | `wm_to_input_decodes_the_keyboard_messages` — E32/E33: `WM_KEYDOWN` `wparam 0x0D`/`0x2D` → `KeyDown { vk: 13 }`/`{ vk: 45 }`; `WM_KEYUP` `0x70` → `KeyUp { vk: 112 }`; `WM_CHAR` `0x68` → `CharInput { 'h' }`; `WM_CHAR` `0xD800` → `None` (E33 refusal) | `src/graphics/window/tests.rs` | unit |
| **T42** | `wm_to_input_zero_extends_and_stays_total` — E34/E35: `WM_LBUTTONDOWN` with `lParam = -1` → `LeftDown { 65615, 65575 }` (§2.4 G2); `WM_RBUTTONDOWN 0x0204`, `WM_PAINT 0x000F`, `WM_DESTROY 0x0002`, `WM_QUIT 0x0012` → `None` each; a keyboard message with a garbage `lParam` still decodes from `wParam` only | `src/graphics/window/tests.rs` | unit |
| **T43** | `input_feed_enqueues_validated_commands_synchronously` — E36: entered feed, `KeyDown VK_INSERT` → `Ok(Some(ClientCommand::Order(Order::SitDown)))`, drains to exactly **1**; `KeyDown VK_ESCAPE` (nothing open → `CloseDialog`) → `Ok(None)`, still 1; `KeyDown VK_RETURN` (opens, `ChatFocus`) → `Ok(None)`, `KeyUp VK_RETURN` (clears the D9 guard), `KeyDown VK_RETURN` (flushes the **empty** line — `validate` refuses before any send) → `Ok(None)`, **still exactly 1** | `src/input/tests.rs` | unit |
| **T44** | `enter_world_arms_the_feed_in_one_call` — E37: a fresh feed discards a wired-space click (`LeftDown {560,400}` → `Ok(None)`, `ui()` equals `UiState::default()`); after `enter_world(view)` → `ui().in_world` true **and** `ui().view == Some(view)`; the same click now → `Ok(Some(Order::Move { x: 103, y: 101 }))` (§2.4 G1) | `src/input/tests.rs` | unit |
| **T45** | `feed_surfaces_the_closed_channel_by_name` — E38: drop the receiver, then `feed(…)` → `matches!(Err(CommandError::Closed))` (no panic, no `Ok`); `ui()` unchanged (still default) across the failure; `ui()` returns `&UiState` (read-only by type) | `src/input/tests.rs` | unit |
| **T46** | `input_feed_is_send_and_carries_no_queue` — E39: a compile-time `Send` assertion for `InputFeed` (it rides the pump thread), plus `Debug` formatting over a fresh feed without panic; the zero-queue claim itself is §11's grep over `feed.rs` | `src/input/tests.rs` | unit |
| **T47** | `raw_insert_key_down_reaches_the_wire_as_the_frozen_sitdown` — E40: `channel()` → `InputFeed` → `enter_world(wired_view)`; `wm_to_input(WM_KEYDOWN, 0x2D, 0)` = `Some(KeyDown { vk: 0x2D })`, fed → `Ok(Some(Order::SitDown))` (feed dropped, mirroring T6's `drop(sender)`) → `run_driver_with_commands` with `Tail::Commands { expect: 1 }` → transcript = `expected_conversation` + exactly one C2S entry whose `plain` is `06 00 00 F0 00 02`, `wire_len 6`; `Stage::TornDown`; front `closed_cleanly`; game `!saw_corruption`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T48** | `typed_chat_reaches_the_wire_as_the_frozen_normal_chat` — E40: raw `WM_KEYDOWN(0x0D)` (opens, no send), `WM_CHAR 'h'`, `WM_CHAR 'i'`, `WM_KEYDOWN(0x0D)` (flushes) → last feed = `Ok(Some(ClientCommand::Chat { mode: ChatMode::Normal, text: "hi" }))`, the three prior = `Ok(None)`; feed dropped; `expect: 1` → one C2S entry `08 00 00 75 00 01 68 69`, `wire_len 8`; clean `TornDown`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T49** | `a_left_click_reaches_the_wire_as_the_frozen_move` — E40 + §2.4 G1: raw `WM_LBUTTONDOWN` with `lParam = (360 << 16) | 480` → `Ok(Some(Order::Move { x: 103, y: 101 }))`; feed dropped; `expect: 1` → one C2S entry `0A 00 00 F0 00 00 67 00 65 00`, `wire_len 10`; clean `TornDown`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T50** | `pre_world_messages_enqueue_nothing_and_the_session_still_tears_down` — E41: six raw messages (`WM_MOUSEMOVE`, `WM_LBUTTONDOWN`, `WM_LBUTTONDBLCLK`, `WM_KEYDOWN 0x2D`, `WM_KEYUP 0x2D`, `WM_CHAR 'i'`) fed **before** `enter_world` → each `Ok(None)`, `ui()` still default; feed dropped; `Tail::Commands { expect: 0 }` (the fast FIN, D22) → `Stage::TornDown`, transcript **==** `expected_conversation` (no pushed entries), front `closed_cleanly`, game `!saw_corruption`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T51** | `the_composed_decode_and_feed_chain_stays_within_the_16ms_frame_budget` — E12 (inherited) + E36/D23: after `enter_world(view)` (so the D4 guard passes), 100 **raw messages** (T10's exact script re-encoded: 16 × `[WM_KEYDOWN 0x0D, WM_CHAR 'h', WM_CHAR 'i', WM_KEYDOWN 0x0D, WM_KEYDOWN 0x70, WM_MOUSEMOVE (5,7)]` + `[WM_KEYDOWN 0x0D, WM_CHAR 'h', WM_CHAR 'i', WM_KEYDOWN 0x0D]`) through `wm_to_input → InputFeed::feed` → exactly **17** enqueued, batch `< 16 ms`, all 17 drain — margin rationale = T10's (`tests.rs:306-316`) + the decode's µs-scale cost, documented in the test, never relaxed | `src/graphics/window/tests.rs` | unit |

**Baseline: 399 → 414 expected** (+15 = the 12 authored tests + `frames.rs`'s **3 inner
tests** re-running in the new `input_loopback` binary — D22's disclosed duplication):
lib **363 → 371** (T40–T42, T51 in `graphics::window::tests`; T43–T46 appended to
`input::tests`), `asset_fixtures` 11, `e2e_loopback` 25 (untouched), `input_loopback` **7**
(T47–T50 + 3); the 5 ignored tests preserved (1 lib operator + 4 `live_integration`). The
matrix cell is copied from the delivered `test.log`, never from this prose. Cross-checks the
Verifier runs: `cargo test --lib input::tests` = **43/43** (39 inherited + T43–T46);
`cargo test --lib graphics::window` = 4 passed + 1 ignored preserved.

## 10. Forbidden shortcuts

- **Modifying T1–T39 in any way** — the signed-off 8.0/8.1/8.2 contract; this delivery
  *appends* to `tests.rs` only (a diff hunk touching lines 1–1362, or any deletion line, is
  a violation).
- **Editing `tests/e2e_loopback.rs` or `tests/e2e_loopback/**`** (frozen harness) — the new
  binary re-declares them read-only via `#[path]` (D22); adding a helper to them "for
  convenience" is a write-guard violation.
- Touching `src/client/**`, `src/main.rs`, `src/game/**`, `src/lib.rs`, `Cargo.*`, or any
  READ-ONLY file of §6 (render/present/tick/pump/demo code inside `window.rs` included —
  the diff is audited at hunk level, §13.5) — finding first, P3-6.
- Any change to `translate`/`validate`/`Intent`/`InputEvent`/`UiState` — no new variant, no
  new field, no re-ordered guard, no touched arm (the 8.1/8.2 delivered bytes are the
  contract; F-7.4-01's exhaustiveness claim must survive untouched).
- Any queue/ring/capacity/coalescing structure in `src/input/**` or `feed.rs` (P4-C1); any
  new crate, opcode, or wire byte; **regenerating or "fixing" a §8.3 golden** (F-07 — the
  literals in §2.4/§8 are the only expectation source).
- Building a Win32 object, window, DC, message struct, or display in any gate test
  (P4-C4) — the decode takes raw integers; the `WindowProc` arm has no gate test (operator
  only, §2.2-1); the ignored operator test stays `#[ignore]`.
- Intra-doc-linking `graphics::window` from `src/input/**` (cfg(windows) — breaks
  `cargo doc` 0-warnings off-Windows, §2.2-9).
- Delivering the stage trigger via any `src/client/**` edit (frozen — divergence (a) says
  8.4, finding first).
- Deriving an expectation from `wm_to_input`/`feed` under test (F-07) — §8 rows and §2.4
  only; clamping, coercing, or panicking where §7 names a typed result; silently relaxing
  T51's/T10's margin if it flakes (disclose, P4-C3).
- A `.await`, channel hop, or buffer between the decoded event and `CommandSender::send`
  (D19/D23, P4-C1/C3); a latency test without its documented margin; live-server runs
  promoted to gates.
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*` (Verifier/Scribe only), `README.md`,
  `GoonZu_Source/**`, any other WO or audit doc, any existing `evidence/**`.

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/8.3/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
  `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`, `zero-queue-grep.txt`,
  `f7401-postcheck.txt`, `write-guard.txt`, `t1-t39-unchanged.txt`, `SHA256SUMS` — hashes
  over the committed (LF) bytes (6.3 L-2 lesson); the matrix `Tests Baseline` cell is copied
  from `test.log` (**414** expected: 371 lib + 11 + 25 + 7).
- **Zero-queue audit (corrected pattern — F-8.0-01 disposition, WO-8.2 §2.2-6):**
  `grep -rinE 'VecDeque|coalesc|MAX_KEY|\bring\b' src/input/` → no matches captured in
  `zero-queue-grep.txt` (P4-C1, gate item 4) — `feed.rs` included.
- **T1–T39 unchanged (§13.2):** `t1-t39-unchanged.txt` — `git diff <base> <delivery> --
  src/input/tests.rs` shows **only additions after line 1362** (one hunk, zero deletion
  lines), plus `cargo test --lib input::tests` green at 43/43.
- **F-7.4-01 post-check:** `f7401-postcheck.txt` — `Intent`/`InputEvent`/`UiState`/
  `translate`/`validate` byte-identical to the 8.1/8.2 delivery (`git diff` empty for those
  definitions and `keybinds.rs`); no new exhaustive match outside `src/input/**`.
- **Write guard:** `write-guard.txt` — `git diff --name-only` limited to the §6 files, plus a
  hunk-level audit of `window.rs`: every changed line is input-side (constants,
  `wm_to_input`, `WindowState.input` + its init, the arm, imports, one module-doc sentence,
  `mod tests`), with the render/present/tick/pump/demo hunks **zero**; `tests/e2e_loopback*`,
  `src/client/**`, `src/main.rs`, matrix, contributing, README and the legacy tree untouched.
- **D22 duplication disclosed:** the `input_loopback` binary's 7 = T47–T50 + `frames.rs`'s 3
  inner tests, named in `summary.md` (so the +15 arithmetic is auditable).
- Four mutation probes recorded (§12): mutated diff, failing output, pristine restore,
  porcelain CLEAN after each, baseline re-run green afterwards.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test | Restore |
|---:|---|---|---|
| 1 | `wm_to_input`: drop the window→backbuffer translation — return the raw zero-extended coordinates (delete `+ MAP_TILE_XSIZE + SCREEN_RX` / `+ MAP_TILE_YSIZE + SCREEN_RY`) | **T40** (`Move {560, 400}` becomes `{480, 360}` — E30's `+80/+40`) | `git checkout --` |
| 2 | `wm_to_input`: take the key from the wrong slot — `KeyDown { vk: l_param as u32 }` instead of `w_param as u32` | **T41** (`KeyDown { vk: 13 }` becomes `{ vk: 0 }` — E32) | `git checkout --` |
| 3 | `InputFeed::feed`: swallow the channel failure — `let _ = self.sender.send(command); Ok(Some(command))` | **T45** (expects `Err(CommandError::Closed)`, gets `Ok(Some(...))` — E38) | `git checkout --` |
| 4 | `InputFeed::enter_world`: forget the view — set only `self.ui.in_world = true` | **T44** (`ui().view` stays `None`, the click stays discarded — E37/D20) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit 101
and the failing assertion, restore, verify `git status --porcelain` clean — then re-run the
full gate set to the delivered baseline (`WO-6.3.md` §9 / `evidence/7.x/mutation-probes.md`
discipline). D18's mapping is pinned failing-if-corrected by T40's dedicated DBLCLK
assertion (a "helpful" `→ None` there must fail the suite and revise this WO).

## 13. Definition of done

A Step 8.3 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at the delivered baseline from `test.log` (**414**
   expected: 371 lib + 11 fixtures + 25 e2e + 7 input_loopback), `fmt` 0 diffs, `doc` 0
   warnings, porcelain clean.
2. **T1–T39 byte-identical** (only appended T43–T46 in `tests.rs`; `t1-t39-unchanged.txt`
   clean — one hunk after line 1362, zero deletions) and `cargo test --lib input::tests`
   green at 43/43; the 5 ignored tests preserved.
3. Every decision D17–D23 pinned: D17 → T40/T41/T42 (+ probes 1–2); D18 → T40's DBLCLK
   assertion; D19 → T43/T45/T46 (+ probe 3); D20 → T44/T50 (+ probe 4); D21 → its decode
   half by T40–T42 and feed half by T43–T46, the Win32 arm itself under P4-C4's operator-only
   clause (`CONTRIBUTING_PHASE4.md:120-126`); D22 → T47–T50 running on the re-declared
   harness; D23 → T51 green with its margin documented.
4. All four §12 probes show their named test failing at exit 101 with the tabled assertion,
   pristine restores, porcelain clean after each, and a green post-probe baseline.
5. Write guard holds: only §6 files changed; **`src/client/**`, `src/main.rs`,
   `src/game/**`, `src/lib.rs`, `event.rs`/`intent.rs`/`keybinds.rs`/`validate.rs`/
   `screen.rs`/`target.rs`, `tests/e2e_loopback*`, the `window.rs` render/present/tick/pump/
   demo code, matrix, contributing, README and the legacy tree untouched** — the `window.rs`
   diff is input-side only (hunk audit), and the `_ => DefWindowProcA` fallthrough plus the
   pump stay byte-identical.
6. F-07 holds: every expectation traces to E30–E41 (or inherited E12) in §8, and every E-row
   traces to a legacy `file:line`, a frozen constant (§8.3 goldens), or the §2.4 hand
   arithmetic — never to the code under test.
7. Zero-queue audit green (the corrected §11 pattern over `src/input/` incl. `feed.rs`, no
   matches) and the D1/D19/D23 structural claim holds: no `.await`/buffer between the raw
   message and `CommandSender::send`; T51 green with its documented margin (and T10 still
   green unchanged).
8. No new crates, no new opcodes, no wire-byte changes, no `Intent`/`InputEvent`/`UiState`/
   `translate`/`validate` changes; no window/Win32 object in any gate test (P4-C4 — decode
   takes raw integers); no live run as a gate; the `input_loopback` count (7 = 4 + 3 frames
   re-run) disclosed in the evidence.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

**Branch:** `step/8.3-driver-wiring` · **Code delivery SHA:** `8e9ba02` (parent `f4e93bc`,
the WO commit) · **Delivered:** 2026-11-08 · **Baseline from `test.log`: 414/414** (371 lib +
11 `asset_fixtures` + 25 `e2e_loopback` + 7 `input_loopback`; 1 ignored lib + 4 ignored
`live_integration` preserved; 399 → 414 exactly as §9). This §14 note lands in the follow-up
commit on the same branch (a note commit cannot cite its own SHA).

**Five gates** (`RUSTFLAGS=-D warnings`, all exit 0, run twice: pre-probe and post-probe
baseline — identical both times):

| Gate | Result |
|---|---|
| `cargo check --all-targets` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --check` | exit 0, 0 diffs |
| `cargo doc --no-deps` | exit 0, 0 warnings |
| `cargo test` | **414/414** (371 + 11 + 25 + 7), 0 failed |
| `cargo test --lib input::tests` | **43/43** (39 inherited + T43–T46) — matches §9 |
| `cargo test --lib graphics::window` | **4 passed + 1 ignored preserved** — matches §9 |

**T40–T51 individual runs:** T40/T41/T42/T51 green in the new `src/graphics/window/tests.rs`
(4/4; the operator test stays `#[ignore]`; D18 pinned failing-if-corrected by T40's dedicated
`WM_LBUTTONDBLCLK → LeftDown` assertion); T43–T46 green in the appended `input::tests` block
(counted in the 43/43 above); T47–T50 green in `tests/input_loopback.rs` (7/7 = T47–T50 +
`frames.rs`'s 3 inner tests re-running — D22's disclosed duplication, §9's arithmetic).
T51's 100-raw-message `< 16 ms` batch green with its margin documented and never relaxed
(T10 green unchanged).

**T1–T39 byte-identical (DoD 13.2):** `git diff -U0 f4e93bc..8e9ba02 -- src/input/tests.rs`
= exactly one hunk — `@@ -1362,0 +1363,192 @@` — zero deletions anywhere in the file; lines
1–1362 untouched.

**Write guard (DoD 13.5):** `git diff --name-only f4e93bc..8e9ba02` = the six §6 code files
only: `src/graphics/window.rs`, NEW `src/graphics/window/tests.rs`, NEW `src/input/feed.rs`,
`src/input/mod.rs` (exactly two lines), `src/input/tests.rs` (append-only), NEW
`tests/input_loopback.rs`. Forbidden set — `src/client/**`, `src/main.rs`, `src/lib.rs`,
`src/game/**`, `keybinds/event/intent/validate/screen/target`, `tests/e2e_loopback*`,
`tests/asset_fixtures*`, `tests/live_integration*`, `Cargo.*`, matrix, README, contributing,
legacy tree —: **NONE** (empty diff). `window.rs` = 7 hunks, all input-side (module-doc
sentence, imports, seven `WM_*` consts + `wm_to_input`, `WindowState.input` +
`input: None`, the arm, `mod tests`): render/present/tick/pump/demo hunks **zero**, the only
2 deletion lines repo-wide are the two `use` lines extended with the border constants, and
the `_ => DefWindowProcA` fallthrough plus the pump are byte-identical.

**F-7.4-01:** `Intent`/`InputEvent`/`UiState`/`translate`/`validate` byte-identical to the
8.1/8.2 delivery (empty `git diff` over those definitions and `keybinds.rs`).

**Probes 4/4** (each against the committed tree `8e9ba02`: one semantic break, the named test
run, pristine `git checkout --` restore + `git status --porcelain` clean after each, full
gate set re-run green at 414 afterwards; runs use plain `cargo test --lib <name>` — see
disclosure 1):

| # | Mutation | Named test | Result |
|---:|---|---|---|
| 1 | drop the window→backbuffer translation (raw zero-extended coords) | T40 | **exit 101** — `E30: WM_MOUSEMOVE decodes to Move in backbuffer space` (`left: Some(Move { x: 480, y: 360 })`, `right: Some(Move { x: 560, y: 400 })`), restored, clean |
| 2 | `KeyDown { vk: l_param as u32 }` | T41 | **exit 101** — `E32: RETURN (0x0D) round-trips as KeyDown { vk: 13 }` (`left: Some(KeyDown { vk: 0 })`), restored, clean |
| 3 | `let _ = self.sender.send(command.clone()); Ok(Some(command))` | T45 | **exit 101** at `tests.rs:1523` — `E38: the closed channel surfaces by name — no panic, no Ok` (got `Ok(Some(…))`), restored, clean |
| 4 | `enter_world` sets only `in_world = true` (view forgotten) | T44 | **exit 101** — `E37: enter_world wires the click context in the same call (D20)` (`left: None`, `right: Some(MapView { … })`), restored, clean |

**Disclosed edits / findings:**

1. **Probe 1 runs without `-D warnings`.** Deleting the `+80/+40` term orphans
   `MAP_TILE_XSIZE/MAP_TILE_YSIZE/SCREEN_RX/SCREEN_RY` inside `window.rs` (their only use),
   so under `-D warnings` the probe would fail compilation instead of showing the tabled
   assertion; it is run as §12 words it ("run the named test") with default flags to record
   exit 101 + the failing assertion. The full gate set re-ran `-D warnings` green after every
   restore (probes 2–4 compile clean under `-D warnings` either way).
2. **Probe 3 keeps `command.clone()`.** The tabled `let _ = self.sender.send(command);` does
   not compile (use-after-move of `command`); the clone preserves the exact semantic break —
   the channel failure is swallowed and `Ok(Some)` is returned.
3. **T45's "(still default)" precision fix.** The §9 row pins `ui()` "(still default)"
   across the `Err(Closed)` failure, but reaching `send` at all requires `enter_world`: the
   D4 guard (`keybinds.rs:50-52`) discards every event while the state is default, so the
   failure-time state can never be the default one. Implemented as: a fresh-feed
   `ui() == UiState::default()` pin before arming, plus "unchanged across the failure"
   asserted against a post-`enter_world` snapshot (documented in the test's doc comment).
   E38's substance — `Err(CommandError::Closed)` surfaced by name, no panic, `ui()` read-only
   by type — is pinned exactly as tabled.
4. **Module-scoped `#[allow(dead_code)]` on the two re-declared frozen harness modules**
   (`frames`, `game_double`) in `tests/input_loopback.rs`. Items used only by the
   deliberately non-re-declared `robustness`/`soak`/e2e-root siblings (`MALFORMED_*_WIRE`,
   `MOVE_COMMAND_WIRE`, `mutated_worldlist_response`, `Tail::HeartbeatThenFin/Storm/
   RawFault/…`) would otherwise fail `-D warnings` in the new binary (D22 §2.2-7 forbids
   re-declaring those siblings). The frozen files are untouched, and `input_loopback`'s own
   code — the mirrors included — stays fully linted: an unused mirror still fails the gate
   (§2.2-10).

No `F-8.3-NN` findings, no out-of-plan edits: every wire expectation is a §8.3 frozen literal
(F-07), no new crate/opcode/wire byte, and the stage trigger stays 8.4's scope (divergence
(a)).

### Verdict - Verifier

**Status: PASSED** (one non-blocking finding, **F-8.3-01**, adjudicated per the F-8.0-01
precedent) — formally verified 2026-11-08 (+07:00) on a fresh S4 clean clone of
`https://github.com/psi-ginger/rust_iwczn_client_II.git` (`git clone --branch
step/8.3-driver-wiring`), checked out at the delivery tip
**`90a0f0766a091e5b33f74a44e62d0e44d405d7d8`** (tree `ec1672af277b10a8bf87268001bae97168ffe7d0`).
Delivery SHA `8e9ba02cc533786b07fb81ec23046aa82dce8d00` (parent `f4e93bc`, the Scribe WO-8.3
commit = the branch cut point); `8e9ba02..90a0f07` = `docs/agents/WO-8.3.md` alone (docs-only
note commit), so `src/`, `tests/` and `Cargo.*` at the tip are byte-identical to the delivery
and gating the tip fully characterizes it (same pattern as 8.0 `fc1a7c7` → `89b9a9d`,
8.1 `f91897e` → `cada5ed`, 8.2 `26c07a7` → `937c9ab`). Porcelain clean at clone, after both
gate runs and after every probe restore (`git status --porcelain -- ':!:evidence'` empty — only
the Verifier's own untracked `evidence/8.3/` ever appeared).

**Both gate runs green** (`RUSTFLAGS=-D warnings` on every gate; run 1 pre-probe 11:32–11:34,
run 2 post-probe baseline 11:40–11:41, +07:00):

| Gate | Run 1 | Run 2 |
|---|---|---|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | exit 0, 0 warnings | exit 0, 0 warnings |
| `RUSTFLAGS=-D warnings cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | exit 0, 0 warnings |
| `cargo fmt --check` | exit 0, 0 diffs (0 bytes) | exit 0, 0 diffs (0 bytes) |
| `cargo doc --no-deps` | exit 0, 0 warnings | exit 0, 0 warnings |
| `cargo test` | **414/414**, 0 failed | **414/414**, 0 failed |

Composition from `test.log` (identical both runs): 371 lib + 11 `asset_fixtures` +
25 `e2e_loopback` + 7 `input_loopback` = **414 passed**; 0 failed; 5 ignored preserved
(1 lib operator + 4 `live_integration`); 0 doc-tests; no warning line in either log.
Cross-checks (run 1, appended to `test.log`): `cargo test --lib input::tests` = **43/43**;
`cargo test --lib graphics::window` = **4 passed + 1 ignored preserved** — both match §9.
T40–T51 name-verified **12/12 `... ok`** in both runs; `frames.rs`'s 3 inner tests re-run in
both binaries (D22's disclosed duplication confirmed); T51's `< 16 ms` batch green with its
margin documented, T10 unchanged green.

**DoD (§13) audit — 13.1 through 13.8:**

| # | DoD item | Verdict |
|---|---|---|
| 13.1 | Five gates exit-0 at 414 from `test.log`, fmt 0 diffs, doc 0 warnings, porcelain clean | **PASS** — both runs, table above |
| 13.2 | T1–T39 byte-identical, `input::tests` 43/43, 5 ignored preserved | **PASS** — `t1-t39-unchanged.txt`: exactly one hunk `@@ -1362,0 +1363,192 @@`, zero deletion lines, and the 1362-line base file is the **exact prefix** of the 1554-line delivered file (element-wise byte-identical, `-cne` = no mismatch); 43/43 and 5 ignored green both runs |
| 13.3 | D17–D23 pinned by named tests | **PASS** — D17 → T40/T41/T42 (+ probes 1–2); D18 → T40's dedicated `WM_LBUTTONDBLCLK → LeftDown` assertion (body audited); D19 → T43/T45/T46 (+ probe 3); D20 → T44/T50 (+ probe 4); D21 → decode half T40–T42 + feed half T43–T46, the Win32 arm under P4-C4's operator-only clause (no gate test builds a window/DC/message); D22 → T47–T50 running on the `#[path]`-re-declared frozen harness (`input_loopback.rs:27-34` audited); D23 → T51 green with its margin documented |
| 13.4 | Probes 4/4: named test fails exit 101 with the tabled assertion, pristine restores, green post-probe baseline | **PASS** — re-run independently by the Verifier (see `mutation-probes.md`): P1 `E30 ... left {480,360} right {560,400}` (compile-fail under `-D warnings` first — disclosure 1 verified by the Verifier's own capture), P2 `E32 ... {vk: 0}` vs `{vk: 13}`, P3 `E38` at `tests.rs:1523`, P4 `E37 ... left: None`; every restore TRACKED_CLEAN; run 2 green at 414/414 afterwards |
| 13.5 | Write guard: only §6 files, `src/client/**` et al. untouched, `window.rs` input-side hunk audit, fallthrough + pump byte-identical | **PASS** — `write-guard.txt`: exactly the six §6 files (1138+/2−); forbidden set (`src/client/**`, `src/main.rs`, `src/lib.rs`, `src/game/**`, `event/intent/keybinds/validate/screen/target`, `tests/e2e_loopback*`, `Cargo.*`, matrix, contributing, README, legacy tree) = **NONE** — **`src/client/**` strictly untouched**. `window.rs` hunk audit: 8 hunks at `-U0` / 7 at default context (the note's count — same content, the two import edits merge at U3), all input-side; render/present/tick/pump/demo hunks **zero**; the only 2 deletion lines repo-wide are the sanctioned `use` joins; `_ => DefWindowProcA(...)` and the pump's `TranslateMessage`/`DispatchMessageA` byte-identical; `mod.rs` exactly 2 lines |
| 13.6 | F-07 holds: E30–E41 (+ E12) each trace to legacy `file:line`, frozen constant or §2.4 arithmetic — never the code under test | **PASS** — 64 E-row citation occurrences across the three delivered test files (21 + 31 + 12); the three §8.3 goldens byte-verified at `input_loopback.rs:78/81/85` (`06 00 00 F0 00 02`, `08 00 00 75 00 01 68 69`, `0A 00 00 F0 00 00 67 00 65 00`) |
| 13.7 | Zero-queue audit green (corrected §11 pattern, no matches) + structural claim | **PASS with F-8.3-01 adjudicated** — machinery components (`VecDeque`, `coalesc*`, `MAX_KEY`) = **0 matches**; the literal pattern misses by exactly one prose line (the finding below). Structural claim green: no `.await`/buffer between the raw message and `CommandSender::send` (`feed.rs` = translate → validate → send, one synchronous call, audited); T51 green with its documented margin |
| 13.8 | No new crates/opcodes/wire bytes, no `Intent`/`InputEvent`/`UiState`/`translate`/`validate` change, no Win32 object in gates, no live run, 7-count disclosed | **PASS** — `Cargo.toml`/`Cargo.lock` untouched; F-7.4-01 post-check green (`event.rs`/`intent.rs`/`keybinds.rs`/`validate.rs` **blob-identical** base → delivery, `screen.rs`/`target.rs` empty diff, every `InputEvent::` use outside `src/input` is a construction not a destructure, `Intent::` zero uses there, no new exhaustive match); decode takes raw integers in every gate test; no live run used as a gate; `input_loopback` 7 = T47–T50 + 3 `frames` re-runs disclosed in §9 and the evidence |

**Finding (recorded, never edited — P3-6 write guard):**

- **F-8.3-01 — non-blocking (a §11 pattern-vs-prose defect, not a delivery defect):** the
  corrected §11 zero-queue pattern returns exactly one match — `src/input/tests.rs:1541`, the
  appended T46 doc comment *"(`P4-C1` — no `VecDeque`/ring/cap anywhere in `src/input/**`)"*:
  prose asserting the *absence* of queue machinery trips the audit with its own wording
  (machinery subset hits `VecDeque`; word-bounded `ring` sits between `/` and `/`). Machinery
  matches = 0 anywhere; the collection inventory confirms no queue/ring/cap/coalescing
  structure exists in `src/input/**` or `feed.rs` (only `Vec<CharTarget>`, 8.1's target list).
  Adjudicated exactly as F-8.0-01 (WO-8.0 §14: *"adjudicated, NON-BLOCKING — a §11
  prose/pattern defect, not a delivery defect"*), so DoD 13.7 reads **PASS with F-8.3-01
  adjudicated**. The line is part of the delivery and was never edited.

The Implementer's four disclosures were each verified independently: (1) probe 1's no-`-D`
run — the Verifier reproduced the compile-fail (`error: unused imports: SCREEN_RX/SCREEN_RY …`
exit 101) before recording the tabled exit-101 assertion; (2) probe 3's `command.clone()` —
the tabled form is a use-after-move and would not compile; (3) T45's "(still default)"
precision fix — fresh-`default` pin before arming + post-arm snapshot, E38's substance intact;
(4) module-scoped `#[allow(dead_code)]` at `input_loopback.rs:28/:33` — frozen files untouched,
the new binary's own code stays fully linted. The delivery note's "no `F-8.3-NN` findings"
claim stands except for F-8.3-01 above (a §11 audit-pattern interaction the note did not
anticipate, not an out-of-plan edit — its "no out-of-plan edits" claim is fully verified).

**Evidence:** `evidence/8.3/` — 21 files (§11 standing set + `gate-exits.txt` +
`postprobe/` run-2 set), `SHA256SUMS` covering the other 20 verified **20/20** against the
delivered LF bytes (every file CR=0, no BOM, pure ASCII — Step 6.3 L-2). Entry point
`summary.md`; `mutation-probes.md` the probe record; `write-guard.txt`, `zero-queue-grep.txt`,
`f7401-postcheck.txt`, `t1-t39-unchanged.txt` the audits; `head.txt`/`env.txt` the clone and
toolchain; `gate-exits.txt` + `postprobe/` the two runs.

**Integration considerations:** baseline moves **399 → 414** (matrix cell copy target
**414/414**, taken from `test.log`); row 8.3 flips `PENDING → PASSED @ 8e9ba02` with F-8.3-01
non-blocking — the Verifier's/Governance's to flip; §9's cross-check pins (43/43 and 4+1) are
re-confirmed for 8.4's verifier; no code change is required by this verdict (F-8.3-01 is a
spec-internal §11 wording note for the next WO, not a delivery edit).

**Status:** verification products produced for Custodian acceptance. This verdict and the
`evidence/8.3/` bundle are left uncommitted; the matrix flip, the commit of both, and any push
are the governance step that follows acceptance — none authorized by this verification
directive (8.2 precedent).
