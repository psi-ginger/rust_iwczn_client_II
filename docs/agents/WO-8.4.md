# Work Order - Step 8.4: Live Acceptance — Moving, Chatting, Interacting (+ Latency Evidence)

**Status:** `ACCEPTED` — definition-before-implementation satisfied by this WO's commit on
`main` (Scribe lane, under the Custodian's Step 8.4 directive, 2026-11-08) **before** any
`src/**` byte moves. The Implementer may pull and begin.
**Date:** 2026-11-08 · **Revision:** Rev 1 (initial, Scribe lane under directive).
**Branch:** `step/8.4-live-acceptance`, to be cut from `main` at this WO's commit.
**Baseline (re-derived this session):** `main` @ `ff4a1ed` (the `--no-ff` merge of Step 8.3),
porcelain clean, all five mandatory gates exit-0 — `cargo check --all-targets` and
`cargo clippy --all-targets -- -D warnings` under `RUSTFLAGS=-D warnings`,
`cargo fmt --check` 0 diffs, `cargo test` **414/414** (371 lib + 11 `asset_fixtures` +
25 `e2e_loopback` + 7 `input_loopback`; 1 ignored lib + 4 ignored `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `TASK_MATRIX.md:143` Phase IV tracker (row 8.4 `PENDING`, est. +4–8, lanes
Implementer → Verifier + Adversary (mutation probes), evidence `evidence/8.4/` — *live = evidence,
never a gate*) · `CONTRIBUTING_PHASE4.md` (P4-C1…C4 at `:63-126`, verification gates 1–6 at
`:130-152`, write-guard at `:165-169`, steps table `:185`, non-goals `:193-201`) ·
`CONTRIBUTING.md:28-46` (live integration runs are evidence, never required for `PASSED`) ·
`WO-8.3.md` §2.2-3 / §8 divergence (a) (the stage trigger + pump-thread `CommandSender`
wiring deferred **to this step**) · `docs/rca/2026-10-28-live-opcode6-npc-stride.md`
(LIVE-01 `RESOLVED` at `701963a` — live world entry is achievable).

## 1. Provenance, governance, lane hats

- **Registration & scope (Custodian directive, 2026-11-08).** Row 8.4 is the binding scope:
  *"Live Acceptance - moving, chatting, interacting + input→wire latency evidence
  (≤ 16 ms / < 50 ms per P4-C3)"*, est. +4–8 tests, lanes Implementer → Verifier +
  **Adversary (mandatory)**, evidence `evidence/8.4/` (*live = evidence, never a gate*). The
  matrix is untouched by this WO — status is the Verifier's to set, per `TASK_MATRIX.md`
  header rule 4.
- **Sequence.** 8.0–8.3 are all `PASSED`; `main` sits at `ff4a1ed` with the combined baseline
  **414/414** (re-derived above). This is the first WO authored after 8.3's merge — no
  parallel lane is active, so the byte-identity guarantees below run against the *merged*
  tree, not against a sibling branch. 8.4 "accepts the whole"
  (`CONTRIBUTING_PHASE4.md:187-189`): it composes what 8.0–8.3 delivered, runs the composed
  chain against a live server as **evidence**, and pins the P4-C3 budget in-gate.
- **Numbering disclosure.** WO-8.3 owns **D17–D23 / E30–E41 / T40–T51** (8.0: D1–D8 / E1–E12 /
  T1–T11; 8.1: D9–D16 / E13–E29 / T12–T27; 8.2: the parallel-lane D9–D12 / E13–E19 overlap,
  T28–T39 — all disclosed in WO-8.3 §1). This WO continues at **D24+ / E42+ / T52+**:
  `D24–D30` (7 decisions), `E42–E48` (7 expectation rows), `T52–T58` (7 tests). Where this
  document cites an earlier row it is qualified `WO-8.3 §9 E40` style — never a bare `E40`.
- **Lanes:** Implementer → Verifier + **Adversary** — every load-bearing claim here is
  "the composed chain is parity with what the step that owns each component proved",
  "the trigger admits exactly the stage signal", or "the budget holds per event", all
  dischargeable by verified mutation probes (§12), per `CONTRIBUTING_PHASE4.md` gate item 6.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs and
  `docs/**`, and every **existing** `evidence/**` bundle is READ-ONLY — a defect found there
  is a finding `F-8.4-NN`, never an edit (P3-6; F-7.4-01 disclosure pattern if the compiler
  ever forces one). Four guardrails are explicit:
  1. **`src/client/**` is READ-ONLY phase-wide** (`CONTRIBUTING_PHASE4.md:165-167`) — the
     stage trigger consumes the driver's *existing* tracing record; the trigger never edits
     the driver (§2.2-1 is why that record is the only seam).
  2. **`src/game/**` frozen at 7.3; `src/input/**` is untouched entirely** — this WO's file
     plan contains **zero** `src/input` files: no `tests.rs` append, no `feed.rs` line. A
     single added line anywhere under `src/input/` is a violation (§13.5 audits an empty
     diff over the whole directory).
  3. **`src/graphics/**` via `window.rs`.** `CONTRIBUTING_PHASE4.md:168-169` reads
     *"`src/graphics/**` is touched only by 8.1 (read-only imports) and 8.3 (`window.rs`
     `WM_*` arms only)"* — a sentence enumerating the **prior** steps' windows. 8.4's
     composition necessarily lands in `window.rs` (the `window_proc` arm is the only place a
     window message becomes an action — WO-8.3 §2.2-3 deferred exactly this to 8.4, and
     verification-gate item 4 (`:144-146`) makes **this WO's file plan** the authority the
     delivered diff is checked against). The charter line is **not edited** by this WO (the
     directive covers WO authoring only); this paragraph is the standing reconciliation —
     a stated governance note (Haru/CAPA style), never silent.
  4. **`src/main.rs` is this step's composition file** (deferred by WO-8.3 divergence (a));
     its exit-code contract (WO-6.1 §5, codes 0/1/2) stays semantically identical (§7.1).
- **Surface-stability guarantee.** `translate`/`validate`/`Intent`/`InputEvent`/`UiState`
  (8.0–8.2 delivered bytes), `wm_to_input` and the `WindowState.input` arm (8.3), `src/input/`
  in full, `src/client/**`, `src/game/**`, and the frozen `tests/e2e_loopback.rs` +
  `tests/e2e_loopback/**` harness are **byte-for-byte untouched** — DoD §13.5.
  `src/graphics/window/tests.rs` and `tests/input_loopback.rs` are **append-only** (T40–T51
  and T47–T50 stay byte-identical).
- **Source-of-record split.** This step adds **no new wire bytes**: the 240/117/118 layouts
  stay `WO-step7-in-world-actions.md` §8.3 (frozen — its literals are this WO's goldens),
  the sending half stays `src/client/commands.rs` (7.4, frozen), the dispatcher stays
  `src/game/dispatcher.rs` (7.3, frozen). This WO owns only the `window.rs` composition
  seam (§7.2), the `src/main.rs` composition (§7.1), the in-gate acceptance script (§7.3),
  the live vehicle (§7.4), and the tests (§9) — every expectation traced to a legacy
  `file:line`, a frozen constant, or the §2.4 hand arithmetic (F-07).
- **P4-C4 split (headless vs operator).** No gate test in this step constructs a Win32
  object, opens a window, or needs a display (`CONTRIBUTING_PHASE4.md:120-126`): T52–T55
  drive raw integers through the frozen loopback doubles, T56/T58 are pure unit tests,
  T57 is pure string matching, and the live vehicle is `#[ignore]`d. The `WM_ENTER_WORLD`
  arm, the window thread, `post_enter_world`/`close_demo` and the shutdown pairing are
  **operator-only** (§11 `live/`, §12 note).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 What 8.0–8.3 delivered (the surface this step accepts) — as of `ff4a1ed`

1. **8.0 (input core, merged):** `InputEvent`/`Intent`/`UiState`, `translate`/`validate`
   (zero-queue direct feed, P4-C1), keybind table, named refusals; E1–E12 / T1–T11; T10's
   100-event `< 16 ms` batch (`WO-8.0 §9`) is the in-gate latency proxy P4-C3 names.
2. **8.1 (mouse, merged `7844a37`):** `screen_to_tile` inverse + hand goldens **G1–G3**
   (`WO-8.1.md:101-103`: backbuffer `(480,340)→(100,100)`, `(520,360)→(101,100)`,
   `(440,360)→(100,101)` at center (100,100)), click→`Order::Move` parity.
3. **8.2 (keyboard, merged `49eead6`):** Enter (focus/flush, D9.1 repeat guard),
   Insert (`SitToggle`, guard `keybinds.rs:105`), Escape (cancel/close, **D9.6 deliberately
   no guard** — `keybinds.rs:77-99`); T28–T39, incl. T30's Escape idempotence pin.
4. **8.3 (driver wiring, merged at `ff4a1ed`):** the seven-arm `wm_to_input` decode
   (`window.rs`, +80/+40 backbuffer translation = E30), `InputFeed` (`feed.rs` —
   `translate → validate → CommandSender::send`, one synchronous call), `enter_world(view)`
   (D20), the `window_proc` input arm + `WindowState.input` (wired, inert: `input: None`),
   T40–T51, and the `tests/input_loopback.rs` harness (T47–T50: raw message → frozen golden
   bytes → `Stage::TornDown`). **WO-8.3 divergence (a) (`WO-8.3 §2.2-3`, `§8`, `§10`): the
  live stage trigger and the pump-thread `CommandSender` wiring were deferred to this step.**
5. **6.x/7.x infrastructure (frozen):** `driver::run_with_commands(cfg, shutdown, receiver)`
   (`driver.rs:475-481` — arms the production `spawn_ctrl_c_watcher` exactly like `run`),
   `commands::channel()` (`commands.rs:93`, unbounded mpsc), `Shutdown: Clone` +
   `trigger(Cause)` (`teardown.rs:145-167`), the loopback doubles (`Transcript`,
   `Tail::Commands { expect, reply }` — `game_double.rs:114,689-736`), and the
   `live_integration` `#[ignore]` pattern.
6. **LIVE-01 resolved at `701963a`** (opcode-6 records strided by `uiType` — 592-byte PC /
   68-byte NPC, live-verified world entry + wire heartbeats, `docs/rca/…-opcode6-npc-stride.md`):
   a live session reaches `Stage::InWorld`, which is the precondition for this step's scope.

### 2.2 Constraints discovered (each forces a design choice or a documented divergence)

1. **`src/client/**` is frozen, and the driver exposes no stage callback.** The full tracing
   inventory of `driver.rs` contains **no `stage=InWorld` event** (stage records exist only
   at `SecretKey`/`GameLogin`/`CharSelected` — `driver.rs:572,678,696` — and at the terminal
   `session ended` record `:534`), `DriverOutcome` arrives only when the session *ends*, and
   `event_loop.rs` has no tracing calls at all. The only read-only, in-world, pre-completion
   signal outside `src/client/**` is the driver's per-frame record
   `debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched")` (`driver.rs:850`)
   carrying `DispatchOutcome::Entered` (produced for the opcode-6 world-entry batch —
   `dispatcher.rs:994-997`, variant `:766`). → **D24**: a `tracing` Layer in `src/main.rs`
   keys on that record.
2. **The signal is `DEBUG`-level.** The composed binary must subscribe with
   `with_max_level(Level::DEBUG)` or the signal is disabled by construction — a disclosed
   stdout-volume change (the driver's stage/heartbeat/frame records join the log; no
   credential ever reaches any record at any level, `WO-6.1 §6.4`'s contract stands).
3. **P4-C4: the Win32 half is operator-only.** The Layer's `PostMessageW`, the
   `window_proc` arm, the window thread and `close_demo` cannot have a gate test
   (`CONTRIBUTING_PHASE4.md:120-126`). → the *decision logic* is factored into the pure
   `enter_world_signal(name, outcome)` predicate (T57, string-only); the posting half is
   verified by the §11 operator run.
4. **`main` is `#[tokio::main(flavor = "current_thread")]` and `run_demo` blocks in the
   message loop** — the driver and the window cannot share one thread. → **D25**: the window
   runs on one `std::thread`; the driver awaits inline on the runtime thread.
5. **The trigger's target (HWND) must exist before the first `Entered` record**, or the
   trigger is silently lost (a dead window would leave the feed disarmed forever — clicks
   discarded by the D4 guard, no error anywhere). → **D25**: a ready channel delivers
   `Ok(hwnd)` (or `Err(reason)`) from `run_demo_with` *before* the driver starts; the HWND
   stashes in `Arc<OnceLock<isize>>` read by the Layer.
6. **`Cause` is frozen at `{CtrlC, ServerDisconnect}`** (`src/client/teardown.rs` — READ-ONLY;
   no third variant may be added). A window close must still end the session. → **D26**: it
   claims `Cause::CtrlC`, the port's designated user-requested-shutdown cause (its own doc
   already reads "port-added — the legacy GUI client has no console signal") — stated
   divergence, logged `cause=CtrlC`.
7. **Neither the player's position nor the live map's dimensions reach `main`.**
   `DriverOutcome` (`driver.rs:431-458`) carries no `x/y`; no frame carries `MapXsize/
   MapYsize` to the client (no `.Map` download — `src/graphics/map.rs` is the offline
   decoder), and the window renders the demo's synthetic 64×64 map. → **D27**: the
   composition view tracks **what the window renders**; center-follows-player is a stated
   divergence (legacy `Client.cpp:4336`).
8. **The live game server is external** — no `services/` exists in this repo. → **D28**: the
   live session is an env-driven `#[ignore]`d test (`DriverConfig::from_env`, the published
   `GOONZU_*` contract), wire bytes via the transparent relay pattern
   (`evidence/live/relay/` → `evidence/8.4/relay/`); **never a gate**
   (`CONTRIBUTING.md:28-46`, `CONTRIBUTING_PHASE4.md:148-149`).
9. **e2e's capture machinery is frozen.** `Capture`/`capture_subscriber`/
   `debug_capture_subscriber` live in `tests/e2e_loopback.rs`'s root (D22 — re-declare only
   what is used, and what T53 needs is *just formatted DEBUG bytes*). → **D30**: a local
   ~20-line `Capture` (`Write` + `MakeWriter`) over `fmt().with_max_level(DEBUG)`, installed
   with thread-scoped `set_default` — the proven same-dependency-set pattern
   (`e2e_loopback.rs` `capture_subscriber` docs, callsite-interest argument).
10. **No UI flag, no new config, no new crate.** Phase non-goals
    (`CONTRIBUTING_PHASE4.md:193-201`): no feature adds without disclosure; the zero-new-
    crates precedent extends to zero-new-flags. → **D29**: the Windows binary always
    composes; portability is a `#[cfg]` boundary (today's headless path survives under
    `#[cfg(not(windows))]`), not a mode.
11. **Matrix arithmetic.** Row 8.4 says est. **+4–8** (`TASK_MATRIX.md:143`); this WO plans
    **+7** → **414 → 421** expected, plus one new `#[ignore]`d live test (ignored 5 → 6).
    The delivered number is copied from `test.log`, never from this prose.

### 2.3 Legacy semantics inherited (why these behaviors exist)

1. **Stage warm-up → the arming gate.** Legacy discarded input until the frame counter
   warmed up (`mainfunction.cpp:221,236,254,288,302,1367` — `GetFrame()` guards), which
   8.0 adopted as the D4 guard (`keybinds.rs:50-52`) and 8.3 replaced with the explicit
   `enter_world` (WO-8.3 D20/E37): before it, **every** event is discarded (`Ok(None)`,
   `ui()` byte-default). Legacy never had an "arming message" either — the port makes the
   stage explicit instead of counting frames; D24 is the live half of that design.
2. **The server owns the sit toggle (Q2).** `Order::SitDown` (frozen `action.rs:99-100`,
   240-kind-2) is sent per press; toggle semantics live server-side
   (`CONTRIBUTING_PHASE4.md:95-98`) — two presses are two identical frames by design.
3. **Chat is a minimal line buffer (Q3).** Focus → type → Enter flush; the line travels
   whole or not at all (`validate` refuses empty/over-cap, never truncates — D3);
   Escape-while-typing cancels UI-only (`keybinds.rs:77-99`, E3) — no intent leaves the
   translator. No InterfaceMgr dialog exists for Escape to open (`CloseDialog` is refused
   at `validate.rs:46-47`), so "escape dialog" in the row's title means exactly the
   line-cancel + typed-refusal behavior the 8.0/8.2 contract delivers.
4. **Legacy's view center follows the player** (`Client.cpp:4336`
   `pclmap->VirtualMapX = pclCM->CR[id]->GetX()`) while the *demo* window renders a fixed
   crop at a fixed `draw_map` center — divergence D27(i) documents why the port's center is
   the render center for now (Q4 camera scrolling is a phase non-goal).
5. **Window close ends the session** — legacy's `WM_CLOSE → WM_DESTROY → PostQuitMessage(0)`
   (`mainfunction.cpp:1598,1656`) leaves the main loop, which closes the sockets; D26 ports
   that pairing through the frozen `Shutdown` flag.
6. **Chat echo = `GAMEMSG_RESPONSE_CHAT = 118`** (`MsgType.h:87`; port `parse_chat`
   `dispatcher.rs:512`, dispatch `:1003`, UI-bound no-state-change — `Client-System.cpp:
   1468-1597`). Whether a live server echoes a client's *own* line back to it is
   server behavior; the live vehicle records what it observes (D28) and the loopback
   stand-in pins the client's dispatch side (E43).

### 2.4 Hand arithmetic (the F-07 source for §8/§9 — never re-derived from code)

1. **The three-click walk script** (center (100,100) = `wired_view()`, the frozen test view
   `WO-8.1.md` §2.4 / `src/input/tests.rs:1327-1343`): window point `+80/+40`
   (`wm_to_input`, WO-8.3 E30) → backbuffer → `screen_to_tile` (WO-8.1 G1–G3 rows):

   | # | window point (packed `lParam`) | +80/+40 → backbuffer | G-row | tile | Move golden (frozen §8.3-A layout) |
   |---:|---|---|---|---|---|
   | 1 | `(400,300)` | `(480,340)` | G1 | **(100,100)** | `0A 00 00 F0 00 00 64 00 64 00` |
   | 2 | `(440,320)` | `(520,360)` | G2 | **(101,100)** | `0A 00 00 F0 00 00 65 00 64 00` |
   | 3 | `(360,320)` | `(440,360)` | G3 | **(100,101)** | `0A 00 00 F0 00 00 64 00 65 00` |

   Layout parse of the frozen A golden (`WO-step7-in-world-actions.md` §8.3-A; the
   `(103,101)` instance is `MOVE_103_101_WIRE`, `input_loopback.rs:81`):
   `usSize (LE16) = 0A 00` · `ucChk = 00` · `cmd (LE16) = F0 00` (240) · `kind (u8) = 00` ·
   `x (LE16)` · `y (LE16)` → 10 bytes. G1–G3 instances above are hand-derived from the
   (103,101) instance by substituting the hand tiles — never read back from the encoder.
2. **The frozen C/D goldens** (already in `input_loopback.rs:77-81`):
   SitDown `06 00 00 F0 00 02` (6 bytes; §8.3-C) · Normal chat `"hi"`
   `08 00 00 75 00 01 68 69` (8 bytes; §8.3-D: `usSize 08 00`, `ucChk 00`, `cmd 75 00` = 117,
   mode `01`, `h i`).
3. **The composition view arithmetic** (E47/T58): `ScreenMode::Mode800x600.backbuffer(false)`
   → 960×680 backbuffer → `screen_rect()` = `(0, 0, 959, 679)` (`screen.rs:129-138`,
   `right = xsize-1`, `Client.cpp:3206-3208`); `center = (32, 32)` = the render's own
   `draw_map(…, 32, 32, …)` arguments (`window.rs:440-445`); `map_size = (64, 64)` =
   `build_scene`'s synthetic header `map_xsize: 64` (`window.rs:624-629`, `map.h:182`,
   `i16` → `i32::from`).
4. **The trigger predicate table** (E48/T57): signal name is the literal
   `"in-world frame dispatched"` (tracing metadata name = the message format string of
   `driver.rs:850`); the outcome field's `Debug` rendering of
   `DispatchOutcome::Entered { char_uniques, npcs }` begins with `Entered`, and no other
   variant's rendering does (`Chat(..)`, `Removed {…}`, `Applied {…}`, `Ordered {…}`,
   `Unhandled {…}`, `Unmatched` — `dispatcher.rs:733-790`).

## 3. Scope decision — this delivery

**In:**

1. `src/main.rs` — the composition (WO-8.3 divergence (a) closed): the DEBUG-level
   subscriber + `EnterWorldTrigger` Layer + pure `enter_world_signal` predicate (§7.1),
   `commands::channel()` + `InputFeed` + the window thread + ready/exit ordering, the
   `driver::run` → `driver::run_with_commands` swap, `close_demo()` + join before the
   unchanged exit match, the `#[cfg(windows)]`/`#[cfg(not(windows))]` boundary, and
   `#[cfg(test)] mod tests;` + **T57** at end of file.
2. `src/graphics/window.rs` — the seam (§7.2): `WM_ENTER_WORLD` const, `post_enter_world`
   (+ its `PostMessageW` declaration in the existing `extern "system"` block), the
   `window_proc` arm, `enter_world_view`, `run_demo_with` + the `run_demo` delegate +
   the ready plumb in `run_demo_inner`, `close_demo`, needed imports, module-doc sentences.
   Render/present/tick/pump code and the `draw_map`/sprite paths stay byte-identical.
3. `src/graphics/window/tests.rs` — **append** T56, T58 after T51 (zero deletions;
   T40–T42/T51 byte-identical).
4. `tests/input_loopback.rs` — **append** the local `Capture`/subscriber (D30) and
   T52–T55 (§7.3); T47–T50 and the mirrored harness items byte-identical.
5. `tests/live_acceptance.rs` — **NEW**, one `#[ignore]`d live session vehicle (§7.4),
   cross-platform like `live_integration` (no Win32).
6. `docs/agents/WO-8.4.md` — this document, committed **before** any code.
7. Seven tests: baseline **414 → 421** expected (+7 = T52–T58; §9), plus 1 new ignored
   test (5 → 6); the delivered numbers are copied from `test.log`.

**Out:** see §5. Explicitly unchanged: `src/client/**`, `src/game/**`, **`src/input/**` in
full**, `src/lib.rs`, `src/graphics/**` except `window.rs`/`window/tests.rs`,
`tests/e2e_loopback.rs` and `tests/e2e_loopback/**`, `tests/asset_fixtures*`,
`tests/live_integration*`, `Cargo.toml`/`Cargo.lock`, `TASK_MATRIX.md`, `CONTRIBUTING*.md`,
`README.md`, `GoonZu_Source/**`, every other WO/audit doc, and every existing
`evidence/**` bundle (`evidence/8.4/` is new and Verifier-owned).

## 4. Architectural decisions (binding for this WO)

- **D24 — the stage trigger is a `tracing` Layer over the frozen driver's `Entered` record.**
  `src/client/**` is READ-ONLY and exposes no stage callback (§2.2-1), so the composition
  observes: `debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched")`
  (`driver.rs:850`) with `outcome = DispatchOutcome::Entered` (the opcode-6 world-entry
  batch, `dispatcher.rs:994-997`). The chain, event → wire:

  1. `main` subscribes with `fmt().with_max_level(Level::DEBUG)` + `.with(EnterWorldTrigger)`
     (the signal is DEBUG — §2.2-2), thread-scoped `set_default` held for all of `main`
     (the current-thread runtime polls the driver on this very thread —
     `e2e_loopback.rs` `capture_subscriber` callsite argument);
  2. `EnterWorldTrigger::on_event` visits the event's `outcome` field (its `Debug`
     rendering) and calls the **pure** predicate
     `enter_world_signal(event.metadata().name(), outcome_debug) -> bool` =
     `name == "in-world frame dispatched" && outcome_debug.starts_with("Entered")`;
  3. on `true`: `window::post_enter_world(hwnd)` with the HWND from
     `Arc<OnceLock<isize>>` → `PostMessageW(hwnd, WM_ENTER_WORLD, 0, 0)`;
  4. the `window_proc` `WM_ENTER_WORLD` arm (pump thread) builds the view via
     `enter_world_view` (D27) and calls `feed.enter_world(view)` — **the arm is the feed's
     only writer, exactly like 8.3's input arm: D1 single-writer preserved** (all feed
     mutation happens on the pump thread; `DEMO_STATE` is the same `AtomicPtr` access
     pattern, `window.rs:320`).

  *Disclosed, never silent:* (i) a later opcode-6 broadcast re-fires `Entered` →
  `enter_world` re-runs with the same constants → state identical (WO-8.3 E37/T44: one call
  sets `in_world` + `view`; a repeat changes nothing) — harmless by construction;
  (ii) input during the sub-second window between window paint and world entry is
  **discarded** (the D4 guard) — legacy warm-up parity (§2.3-1); (iii) the `DEBUG` log-volume
  change (§2.2-2). Legacy parallel: no arming message existed there either (§2.3-1).

- **D25 — composition threading: driver on the runtime thread, window on one
  `std::thread`, ordered by a ready channel.** `main` stays
  `#[tokio::main(flavor = "current_thread")]`. Sequence:
  1. config first (`DriverConfig::from_env` → exit 2 **before** any window exists — the
     config-error contract never flashes a window);
  2. install the subscriber (HWND slot still empty — no trigger event can fire yet);
  3. `commands::channel()` → `InputFeed::new(sender)`; `shutdown.clone()` to the window side
     (`Shutdown: Clone`, Arc flag — `teardown.rs:141-147`);
  4. `std::thread::spawn` → `window::run_demo_with(Some(feed), ready_tx)`; when it returns
     for **any** reason the thread calls `shutdown.trigger(Cause::CtrlC)` (D26);
  5. `ready_rx.recv()` **before the driver starts** — `Ok(Ok(hwnd))` → stash into the
     `OnceLock` (the trigger's target provably exists before the first `Entered` record);
     `Ok(Err(reason))` → `error!` + join + **exit 1** (driver never starts);
     `Err(_)` (thread panicked) → `error!` + join + **exit 1**;
  6. `driver::run_with_commands(&config, &shutdown, receiver).await` — the production
     Ctrl+C watcher is armed exactly as `run` arms it (`driver.rs:475-481` calls
     `run_with_watcher(…, spawn_ctrl_c_watcher, Some(commands))`);
  7. session returned → `window::close_demo()` (no-op when the window is already gone) →
     `join` (the pump is woken by `WM_CLOSE`, never blocked on `main`) → the **unchanged**
     exit match runs (§7.1).

  The window thread never waits on `main` (the ready send happens before the pump loop; the
  shutdown flag is atomic) — no lock ordering exists to deadlock. P4-C4: steps 3–7's Win32
  halves are operator-only (§11).

- **D26 — the shutdown pairing runs both directions through the one frozen flag.**
  *window close → session shutdown:* the user's X → `WM_DESTROY → PostQuitMessage(0)`
  (`window.rs` arm, `mainfunction.cpp:1598,1656`) → `run_demo_with` returns → the thread
  calls `shutdown.trigger(Cause::CtrlC)` → the in-world loop observes the flag → teardown →
  `Stage::TornDown` → main's Ok arm → **exit 0**. *session end → window close:* any driver
  return (clean finish, server-side close, Ctrl+C via the armed watcher, D25 step 7) →
  `close_demo()` posts `WM_CLOSE` → `DefWindowProc` → `DestroyWindow` → the loop breaks →
  thread joined → exit match. **Stated divergence (Haru/CAPA):** `Cause` is frozen at
  `{CtrlC, ServerDisconnect}` (WRITE-guard: `src/client/**`), so the window close claims
  `CtrlC` — the port's designated user-requested-shutdown cause (its doc already reads
  "port-added — the legacy GUI client has no console signal"); the GUI close is that user
  interrupt's analogue, and the terminal record logs `cause=CtrlC` — documented, never
  silent. Exit codes 0/1/2 (WO-6.1 §5) are preserved (§7.1).

- **D27 — the composition view is built on the pump thread from render-derived inputs.**
  `fn enter_world_view(screen: ScreenRect, map_size: (i32, i32)) -> MapView` (pure,
  `window.rs`), called from the arm with `state.geometry.screen_rect()` — the *same* rect
  the render draws through (`screen.rs:129-138`; (0,0,959,679) for the 960×680 backbuffer) —
  and `(i32::from(state.map.header.map_xsize), i32::from(state.map.header.map_ysize))` — the
  map actually on screen (`build_scene`, `window.rs:624-629`); `center: (32, 32)` is the
  render's own `draw_map(…, 32, 32, …)` literal (`window.rs:440-445`). A click therefore
  decodes against exactly what the window shows (P4-C2's inverse: *view parity with render*).
  **Stated divergences:** (i) legacy's `VirtualMapX/Y` follows the player
  (`Client.cpp:4336`); the port's center is the fixed render center — the demo renders a
  fixed crop, Q4 camera scrolling is a phase non-goal, and the player's `x/y` is not
  observable from `main` (`DriverOutcome` has no position; `src/client/**` frozen) — a
  future position seam may follow the avatar, disclosed here, never silent;
  (ii) the live game map's true dimensions never reach the client (no `.Map` download),
  so the view tracks the **rendered (demo) map** by construction — the only map the window
  draws.

- **D28 — the live session is one `#[ignore]`d driver-level test, evidence-only.**
  `tests/live_acceptance.rs` (§7.4): one session test driven through
  `DriverConfig::from_env()` (published env contract `GOONZU_FRONT_ADDR`/`GOONZU_GAME_ADDR` +
  `GOONZU_USER_ID`/`GOONZU_USER_PW`/`GOONZU_MAC_ADDRESS`/`GOONZU_CHAR_SLOT`, `driver.rs:72-83`
  — note `live_integration`'s `GOONZU_LIVE_*` names serve its direct-socket tests; this
  vehicle goes through the driver config), a local DEBUG capture (D30 shape), and a feed the
  **test itself** composes (it stands in for the window: `enter_world` after observing the
  `Entered` record — headless, no Win32). Measurements: per-event input→enqueue timed with
  `Instant` and **asserted ≤ 16 ms** (P4-C3's in-process leg — `CONTRIBUTING_PHASE4.md:103-115`);
  input→echo observed by 1 ms polling of the capture and **recorded, never asserted**
  (the network leg is evidence, not a budget); wire bytes via the transparent relay
  (`evidence/live/relay/` → `evidence/8.4/relay/`). Credentials come from operator env and
  are **never written to any file** (§11). **Never a gate:** if no server is reachable, the
  evidence README records the non-run and `PASSED` is unaffected
  (`CONTRIBUTING.md:28-46`; gates 1–6, `CONTRIBUTING_PHASE4.md:130-152`, contain no live item).

- **D29 — no UI flag: the Windows binary always composes; portability is a `cfg` boundary.**
  From this step on, the composed client **is** the binary — no `GOONZU_UI`-style flag, no
  new env var, no feature add (phase non-goals `CONTRIBUTING_PHASE4.md:193-201`;
  zero-new-crates precedent extends to zero-new-config). Window-creation failure is a
  runtime failure: exit 1 with the reason logged (D25 step 5). The composition's Win32
  items sit under `#[cfg(windows)]`; `#[cfg(not(windows))]` keeps **today's** headless
  `driver::run` path so off-Windows `cargo check`/`cargo doc` stay green (WO-8.3 §2.2-9's
  standard) — a platform boundary, not a mode. Headless automation remains the test
  harnesses: no gate and no runbook invokes the binary.

- **D30 — gate-test echo capture is a local `Capture`, not a harness mirror.** e2e's
  `Capture`/`capture_subscriber`/`debug_capture_subscriber` stay in the frozen
  `tests/e2e_loopback.rs` root (D22). T53 and the live vehicle each gain their own
  ~20-line copy: `Capture(Arc<Mutex<Vec<u8>>>)` implementing `std::io::Write` +
  `tracing_subscriber::fmt::MakeWriter`, a subscriber built as
  `fmt().with_max_level(Level::DEBUG).with_ansi(false).with_writer(capture).finish()`,
  installed with thread-scoped `tracing::subscriber::set_default` (the current-thread test
  runtime runs the driver on the test thread — the callsite-interest argument documented at
  `e2e_loopback.rs` `debug_capture_subscriber`). Assertions read the captured bytes
  (`in-world frame dispatched`, `opcode=118`, `outcome=Chat`). The two binaries cannot share
  test code without the frozen `#[path]` pattern, so the small duplication is **disclosed**
  in `summary.md` (§11).

## 5. Non-goals

- No InterfaceMgr widget/dialog framework and no dialog rendering — "escape" means the
  8.0/8.2 line-cancel + typed-refusal contract (§2.3-3), nothing is drawn (Q3).
- No chat rendering — the echo is observed via dispatch records and wire bytes, not pixels
  (`CONTRIBUTING_PHASE4.md:196-197`).
- No camera scrolling / no center-follows-player — Q4 non-goal; D27(i) documents the gap.
- No `.Map` download, no live map-dimension protocol, no position frame — D27(ii).
- No new opcodes, no wire-layout change (240/117/118 stay WO-7.3 §8.3-frozen), no new
  crates, no `Cargo.*` edit (the FFI stays hand-declared `extern "system"` per 7.2 R1).
- No new env var / UI flag / mode (D29); no edit to the frozen `Cause` enum or any
  `src/client/**` item (D26's claim uses the enum as-is).
- No live-server run as a gate, ever; no latency assertion on the network leg
  (input→wire / input→echo are recorded evidence — P4-C3's budget is the in-process
  input→enqueue leg, `CONTRIBUTING_PHASE4.md:111-115`).
- No gate test for the `WM_ENTER_WORLD` arm, `post_enter_world`, `close_demo`, the window
  thread or the Layer's posting half — operator-only under P4-C4 (§13.3's pinning language
  says so explicitly).
- Arrow keys, right-click, double-click machinery, gamepad/touch/IME, login/gateway input
  (inherited phase non-goals, `CONTRIBUTING_PHASE4.md:197-201`).
- No change to T1–T51, no change to T47–T50, no touching `tests/e2e_loopback*`, no
  touching `src/input/**` (this WO appends **nothing** there).

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change | Tests |
|---:|---|---|---|
| 1 | `docs/agents/WO-8.4.md` | this document (committed **before** code) | — |
| 2 | `src/main.rs` | composition: subscriber + `EnterWorldTrigger` + `enter_world_signal`, channel/feed, window thread + ready/exit ordering, `run_with_commands` swap, `close_demo` + join before the unchanged exit match, `#[cfg]` boundary, module-doc, `#[cfg(test)] mod tests;` | **T57** |
| 3 | `src/graphics/window.rs` | `WM_ENTER_WORLD` const + `PostMessageW` decl, `post_enter_world`, the `window_proc` arm, `enter_world_view`, `run_demo_with` + `run_demo` delegate + ready plumb in `run_demo_inner`, `close_demo`, imports, module-doc sentences | — |
| 4 | `src/graphics/window/tests.rs` | **append** T56, T58 (zero deletions; T40–T42/T51 byte-identical) | T56, T58 |
| 5 | `tests/input_loopback.rs` | **append** local `Capture`/subscriber + T52–T55 (+ the `frames` imports T53 needs); T47–T50 and the mirrors byte-identical | T52–T55 |
| 6 | `tests/live_acceptance.rs` | **NEW**: one `#[ignore]`d live session vehicle (§7.4) | +1 ignored |
| 7 | `evidence/8.4/` | Verifier-owned (§11) — never the Implementer's | — |

**Baseline arithmetic:** 414 → **421 expected** (+7): lib **371 → 373** (T56/T58 in
`graphics::window::tests`), bin `src/main.rs` **0 → 1** (T57), `input_loopback` **7 → 11**
(T52–T55), `asset_fixtures` 11 and `e2e_loopback` 25 untouched; ignored **5 → 6** (+1
`live_acceptance`; 1 lib operator + 4 `live_integration` preserved). Matrix est. +4–8 →
+7 delivered ✓. The matrix cell is copied from the delivered `test.log`.

## 7. API & wiring spec (the shared contract)

### 7.1 `src/main.rs` — the composed entry

Module-doc gains a paragraph: composition (8.4) = window pump thread + driver session, the
trigger consumed from the driver's frozen `Entered` record (D24), exit codes unchanged.

Flow (Windows; the `#[cfg(not(windows))]` body keeps today's headless code — D29):

```text
1. tracing::info!(version = CLIENT_VERSION, "goonzu client starting")   (unchanged)
   + install subscriber: fmt().with_max_level(Level::DEBUG).with(EnterWorldTrigger{hwnd: Arc<OnceLock<isize>>}).finish()
     held via tracing::subscriber::set_default for the whole of main          (D24, §2.2-2)
2. DriverConfig::from_env()  -> Err: error! + exit(2)                    (unchanged, pre-window)
3. let shutdown = Shutdown::new();  let (tx, rx) = commands::channel();  let feed = InputFeed::new(tx);
   let (ready_tx, ready_rx) = std::sync::mpsc::channel();
   let window_shutdown = shutdown.clone();                               (D25)
4. std::thread::spawn(move || { let _ = window::run_demo_with(Some(feed), ready_tx);
                                let _ = window_shutdown.trigger(Cause::CtrlC); })   (D26)
5. ready_rx.recv():
     Ok(Ok(hwnd))  -> hwnd_slot.set(hwnd)
     Ok(Err(why))  -> error!(%why, "the demo window could not be opened"); join; exit(1)
     Err(_)        -> error!("the window thread panicked"); join; exit(1)          (D25)
6. driver::run_with_commands(&config, &shutdown, receiver).await          (was driver::run;
     the Ctrl+C watcher is armed identically — driver.rs:475-481)
7. window::close_demo();  let _ = window_handle.join();                  (D26 — before ANY exit)
8. match result {                       // byte-for-byte the delivered arms (WO-6.1 §5):
     Ok(outcome)  -> info!(stage=…, worlds=…, char_unique=…, char_slot=…, village=…,
                           "session complete");            // falls off main -> exit 0
     Err(DriverError::Config(e)) -> error!(%error, "driver configuration is invalid"); exit(2)
     Err(error)  -> error!(%error, "session failed");                       exit(1)
   }
```

The pure predicate (ungated — T57's subject, no Win32 in sight):

```rust
/// D24 — the one signal the composition consumes (never a Win32 type,
/// never an event built here): the frozen driver record's metadata name
/// (`driver.rs:850` message literal) and the `outcome` field's Debug
/// rendering (`DispatchOutcome::Entered`, `dispatcher.rs:766,994-997`).
fn enter_world_signal(event_name: &str, outcome_debug: &str) -> bool {
    event_name == "in-world frame dispatched" && outcome_debug.starts_with("Entered")
}
```

`EnterWorldTrigger` (cfg(windows)) is a `tracing_subscriber::Layer<S>` whose `on_event`
records the event's field named `outcome` (a `Visit::record_debug` rendering it to `String`),
applies `enter_world_signal(event.metadata().name(), …)`, and on `true` calls
`goonzu_client::graphics::window::post_enter_world(*hwnd)` from the `OnceLock` (unset slot =
skip; provably unreachable — D25 orders it before the driver starts).

### 7.2 `src/graphics/window.rs` — the composition seam

Authorized seam lines only (§1 guardrail 3; render/present/tick/pump/scene code untouched):

1. **`pub const WM_ENTER_WORLD: u32 = 0x8001;`** (`WM_APP + 1`, port-added; legacy has no
   arming message — §2.3-1) + a `PostMessageW` declaration in the existing
   `extern "system"` user32 block (`window.rs:171-240`, 7.2 R1 hand-declared FFI).
2. **`pub fn post_enter_world(hwnd: isize)`** — `PostMessageW(hwnd, WM_ENTER_WORLD, 0, 0)`.
3. **The `window_proc` arm** (next to the 8.3 input arm, `window.rs:511-528`), returning 0:

   ```rust
   WM_ENTER_WORLD => {
       let state = DEMO_STATE.load(Ordering::Relaxed);
       if !state.is_null() {
           let state = &mut *state;             // one thread: the pump (D1, as 8.3)
           if let Some(feed) = state.input.as_mut() {
               let view = enter_world_view(
                   state.geometry.screen_rect(),
                   (i32::from(state.map.header.map_xsize),
                    i32::from(state.map.header.map_ysize)),
               );
               feed.enter_world(view);          // D20 mechanism, live trigger (D24)
           }
       }
       0
   }
   ```

   Headless demo (`input: None`) and the operator test: the arm is a no-op — inert by
   construction, exactly like the 8.3 arm before composition.
4. **`fn enter_world_view(screen: ScreenRect, map_size: (i32, i32)) -> MapView`** — pure
   (D27): `MapView { screen, center: (32, 32), map_size }`, doc-citing
   `draw_map(…, 32, 32, …)` (`window.rs:440-445`), `build_scene`'s header
   (`window.rs:624-629`), and `screen_rect()` (`screen.rs:129-138`). Private; the sibling
   `mod tests` reaches it (T58).
5. **`pub fn run_demo_with(feed: Option<InputFeed>, ready: std::sync::mpsc::Sender<Result<isize, String>>) -> Result<(), String>`**
   — today's `run_demo` body plus: move `feed` into `WindowState.input` before the
   `DEMO_STATE` store, and deliver **exactly one** `Result<isize, String>` on `ready`:
   `Ok(hwnd)` once `CreateWindowExA` succeeded (`(*state_ptr).hwnd = hwnd`, `window.rs:899`)
   **before** `ShowWindow`/the tick loop, `Err(reason)` when creation failed (then return
   that same `Err`). `pub fn run_demo()` stays: a thin delegate passing `None` and a
   discarded channel — the 7.2 operator test (`window.rs:871`) behavior is unchanged.
6. **`pub fn close_demo()`** — post `WM_CLOSE` to the `DEMO_STATE` hwnd if non-null,
   otherwise no-op (the session-end path, D26); window-thread teardown (`PostQuitMessage`
   → loop break → reclaim) is the existing code, unchanged.

### 7.3 `tests/input_loopback.rs` — the in-gate acceptance script

Appended after T50 (`:479+`); T47–T50 bytes untouched. Each test mirrors T47's proven
shape: `bind()` ×2 → `spawn_front(…, Mutation::None, transcript)` → `spawn_game_with(…,
Tail::Commands { expect, reply }, DOUBLE_TIME_BUDGET)` → `channel()` → `InputFeed::new` →
`feed.enter_world(wired_view())` → feed the raw script **before** the run (asserting each
result) → `drop(feed)` → `run_driver_with_commands` → transcript/report assertions
(`Stage::TornDown`, front `closed_cleanly`, game `!saw_corruption`, `heartbeats 0`).

- **T52** — script: `pack(400,300)`, `pack(440,320)`, `pack(360,320)` via
  `wm_to_input(WM_LBUTTONDOWN, 0, …)` → three `Ok(Some(Order::Move))` with the §2.4 hand
  tiles; `expect: 3`; transcript prefix == `expected_conversation`, then the three C2S
  entries byte-exact **in script order** (§2.4 goldens, `wire_len 10` each).
- **T53** — script: `WM_KEYDOWN 0x0D` (focus, `Ok(None)`), `WM_CHAR 'h'`, `WM_CHAR 'i'`,
  `WM_KEYDOWN 0x0D` (flush → `Ok(Some(ClientCommand::Chat { mode: ChatMode::Normal,
  text: "hi" }))`); install the local D30 DEBUG capture (`set_default`) around the run;
  `expect: 1` with
  `reply: vec![frame(OP_CHAT, &chat_body(CHAR_UNIQUE as i16, <the session's own name>, "hi"))]`
  (the reply's `char_unique` is a roster id the in-world set upserted — `frames.rs:659`,
  valid-target gating `dispatcher.rs` 118). Assertions: one C2S entry
  `08 00 00 75 00 01 68 69`, one S2C entry `== frame(OP_CHAT, …)` byte-exact, and the
  capture contains `in-world frame dispatched` with `opcode=118` and `outcome=Chat`
  (the dispatch side of the echo, E43); clean `TornDown`.
- **T54** — script: `WM_KEYDOWN 0x2D` → `Ok(Some(Order::SitDown))`, `WM_KEYUP 0x2D`
  (clears the D9.1 guard, `keybinds.rs:141`), `WM_KEYDOWN 0x2D` → `Ok(Some(SitDown))`
  again (two physical presses, Q2 — the server toggles); `expect: 2`; two C2S entries
  `06 00 00 F0 00 02` each (`wire_len 6`); clean `TornDown`.
- **T55** — script: `WM_KEYDOWN 0x0D` (focus) → `Ok(None)`; `WM_KEYUP 0x0D`;
  `WM_CHAR 'h'`, `WM_CHAR 'i'` (line held) → `Ok(None)`; `WM_KEYDOWN VK_ESCAPE (0x1B)` →
  **`Ok(None)`** (cancel: `chat_open=false`, line cleared, nothing reaches `translate`'s
  output — `keybinds.rs:77-99` E3); `WM_KEYDOWN 0x0D` (re-focus) → `Ok(None)`;
  `WM_KEYUP 0x0D`; `WM_KEYDOWN 0x0D` (flush the **empty** line — `validate` refuses before
  any send) → `Ok(None)`. **Zero** commands queued; `expect: 0` (the T50 fast FIN);
  transcript **==** `expected_conversation` byte-for-byte (zero pushed entries — if the
  cancelled line had survived, the flush would have pushed the D golden and this equality
  fails); clean `TornDown`, `heartbeats 0`.
- **The local `Capture`** (D30): struct + `Write` + `MakeWriter` + the DEBUG `fmt`
  subscriber, documented as the e2e-pattern copy; assertions are byte `contains` checks
  (`in-world frame dispatched`, `opcode=118`, `outcome=Chat`).

### 7.4 `tests/live_acceptance.rs` — the live vehicle (evidence, never a gate)

Cross-platform (driver-level, like `live_integration`; no Win32, no `cfg(windows)`).
Module doc states the four contract lines: **evidence-only**
(`CONTRIBUTING.md:28-46`), `#[ignore]`, the env recipe, credentials never recorded.

```text
#[ignore = "live acceptance: needs a reachable game service (GOONZU_GAME_ADDR) and credentials (GOONZU_USER_ID, GOONZU_USER_PW, GOONZU_MAC_ADDRESS, GOONZU_CHAR_SLOT)"]
#[tokio::test]
async fn live_session_walks_chats_sits_and_cancels_within_budget() { … }
```

Steps (one session, one connection — the session *is* the evidence narrative):

1. `DriverConfig::from_env().await` — failure here is an operator-env error (the ignore
   message is the recipe); no credential value is ever printed or written.
2. Install the local D30 DEBUG capture (`set_default`, thread-scoped — the test runtime is
   current-thread).
3. `commands::channel()` → `InputFeed::new` → **do not arm yet**; `driver::run_with_commands`
   on a task with a session budget (mirror `live_integration`'s timeout style).
4. Poll the capture (1 ms sleep loop) until the `Entered` record appears — then
   `feed.enter_world(view)` with the frozen `wired_view()` constants (screen
   `(0,0,959,679)`, center (100,100), map (400,400)). *Disclosed:* the live map's real
   dimensions are unknown to the client (D27(ii)); clicks stay near the center, and any
   server-side refusal is recorded, never asserted away.
5. Script — **every `feed` timed** (`Instant::now()` → `feed.feed(ev)` → `elapsed`), asserted
   `≤ 16 ms` per event (P4-C3's in-process leg), min/avg/max kept for the printout:
   a. **walk:** the three T52 window points → three `Ok(Some(Order::Move))`;
   b. **chat:** focus, type a marker line (e.g. `8.4 accepted`), flush → `Ok(Some(Chat))`;
      poll for a `118` echo — *if observed*, record `input → echo` elapsed and assert the
      capture never shows the cancel marker (step d) in any `118` dispatch;
      *if the server does not echo its recipient's own line*, record that as the session's
      observed behavior (§2.3-6) — the assertion is conditional and documented;
   c. **sit:** two Insert presses (Down/Up/Down) → two `Ok(Some(SitDown))`; record any
      `241` order-echo observations from the capture;
   d. **escape:** focus, type `cancel-probe`, `Escape` (`Ok(None)` — UI-only cancel),
      re-focus, flush (empty → refused) → zero commands from this group.
6. End the session ourselves: `shutdown.clone().trigger(Cause::CtrlC)` → await the driver →
   assert `stage == Stage::TornDown` (the hard session-survival assertion: the server kept
   the session through walk/chat/sit/cancel).
7. Print the evidence block to stdout: the per-event latency table (input→enqueue),
   observed `118`/`241` dispatch counts and input→echo figures (1 ms poll granularity —
   disclosed margin), feed results per script group, final stage. The operator captures
   stdout into `evidence/8.4/live/live-session.log` (README recipe, §11); wire bytes come
   from the relay copy `evidence/8.4/relay/`.

### 7.5 Unchanged surfaces (explicit)

`src/client/**`, `src/game/**`, `src/input/**` (all of it), `src/lib.rs`,
`src/graphics/screen.rs`/`present.rs`/`draw_map.rs`/`map.rs`/`window` render+present+tick+
pump+scene code, `tests/e2e_loopback.rs` + `tests/e2e_loopback/**`, `tests/asset_fixtures*`,
`tests/live_integration*`, `Cargo.*`, matrix/contributing/README, `GoonZu_Source/**`, all
other WOs, all existing `evidence/**`. The `driver.rs` tracing events are consumed exactly
as delivered — no event is added, renamed, raised, or re-leveled.

## 8. Expectation table (the F-07 source for §9)

E1–E41 (WO-8.0/8.1/8.3 §8, plus WO-8.2's qualified rows) remain in force for their tests —
T52–T55 compose them end-to-end. The new rows:

| # | Behavior (as tested) | Provenance |
|---|---|---|
| **E42** | A three-click walk script through decode + feed + driver reaches the wire as **three** `Order::Move` frames byte-exact against the §2.4 goldens — `0A 00 00 F0 00 00 64 00 64 00`, `0A 00 00 F0 00 00 65 00 64 00`, `0A 00 00 F0 00 00 64 00 65 00` — in script order, as the session's only C2S frames, then a clean `Stage::TornDown` | frozen `WO-step7-in-world-actions.md` §8.3-**A** (layout, never regenerated); §2.4 hand table (G1–G3 of `WO-8.1.md:101-103` + the `+80/+40` decode, WO-8.3 **E30**); legacy arm `mainfunction.cpp:232` (left press → move action chain); harness T6 shape (`e2e_loopback.rs` `run_driver_with_commands`) |
| **E43** | Focus → type → Enter flush sends the frozen §8.3-D chat frame (`08 00 00 75 00 01 68 69`), and a 118 reply for the session's own character is **dispatched as `DispatchOutcome::Chat`** — the capture holds `in-world frame dispatched` with `opcode=118` and `outcome=Chat(… hi …)` — then a clean `TornDown` | frozen §8.3-**D**; `parse_chat` `dispatcher.rs:512` + `RESPONSE_CHAT` `:1003`; driver record `driver.rs:850`; `game_double` `Tail::Commands { reply }` `game_double.rs:114,697-713`; D30 capture |
| **E44** | Two physical Insert presses (Down/Up/Down) enqueue **two** `Order::SitDown` and the wire carries `06 00 00 F0 00 02` twice — identical frames by design: the **server** owns the toggle (Q2), the client owns one-frame-per-press (D9.1 guard cleared by KeyUp) | frozen §8.3-**C**; `CONTRIBUTING_PHASE4.md:95-98` (Q2 ruling); `keybinds.rs:101-119` (`VK_INSERT`, guard `:105`) + `:141` (KeyUp clears); `action.rs:99-100` |
| **E45** | Escape while typing cancels **UI-only** — `chat_open=false`, line cleared, `translate` returns `None` — and the subsequent flush of the now-empty line is refused by `validate` before any send: **zero** C2S frames, transcript byte-identical to `expected_conversation`, clean `TornDown` | `keybinds.rs:77-99` (`VK_ESCAPE`, D9.6 no-repeat guard, E3 cancel); `validate.rs:35-37` (empty → `None`) + `:46-47` (`ChatFocus`/`CloseDialog` → `None`); WO-8.0 §8 **E2/E4**; T30's idempotence pin (WO-8.2 §9); legacy `NExitDlg.cpp:63` `IsCreate()` idiom cited in the arm |
| **E46** | **Per event** (P4-C3's literal unit), the composed chain `wm_to_input → translate → validate → CommandSender::send` completes in `< 16 ms` for every event of T10's 100-event script — no `.await`, no hop, no buffer anywhere in the path | `CONTRIBUTING_PHASE4.md:103-115` (≤ 16 ms per event, hard sub-tick < 50 ms); WO-8.0 **E12**/T10 margin rationale; WO-8.3 **T51** (the batch sibling); structural claim P4-C1 |
| **E47** | `enter_world_view(screen, map_size)` yields the view that matches the render exactly: `screen == (0,0,959,679)` (the rect `render_and_present` draws through), `center == (32, 32)` (the `draw_map` call's own arguments), `map_size == (64, 64)` (the synthetic map's header) — clicks decode against what is on screen | §2.4-3 arithmetic; `screen.rs:129-138` (`Client.cpp:3206-3208`, `:5713-5716`); `window.rs:440-445` (`draw_map` center literal); `window.rs:624-629` + `map.h:182`; `MapView` `src/input/screen.rs:24-35`; D27 (both divergences stated) |
| **E48** | The trigger predicate admits **exactly** the stage signal: `("in-world frame dispatched", "Entered { … }")` → fire; every other outcome of that message (`Chat(…)`, `Removed {…}`, `Applied {…}`, `Ordered {…}`, `Unhandled {…}`, `Unmatched`, empty) → no; every other message (`heartbeat sent`, `session ended`, `in-world order echo reported`, a prefix-typo of the name) with an `Entered` outcome → no | D24 signal: `driver.rs:850` message literal + `dispatcher.rs:766,994-997` (`Entered` produced only by opcode 6); legacy warm-up guard `mainfunction.cpp:221,…` → WO-8.3 **D20/E37** (the live half, divergence (a) closed); §2.4-4 |

**Stated divergences of this step's composition (Haru/CAPA — documented, never silent):**
(a) **center does not follow the player** — legacy `Client.cpp:4336`; player position is
not observable from `main` and Q4 camera is a non-goal (D27(i));
(b) **`map_size` is the rendered demo map's**, because no frame carries the live map's
dimensions to the client (D27(ii));
(c) **window close logs `cause=CtrlC`** — the frozen `Cause` enum has no third variant and
`src/client/**` may not add one (D26);
(d) **the composed binary always opens the window and logs at DEBUG** — no flag, disclosed
stdout-volume change (D29, §2.2-2);
(e) **later opcode-6 broadcasts re-fire `Entered`** — `enter_world` is idempotent
(D24(i)); input before the trigger is discarded (legacy warm-up parity, D24(ii)).

## 9. Test invariants (T52–T58) and the baseline

Pattern authorities: WO-8.3 §9 (loopback harness, frozen goldens, append-only discipline)
and WO-6.2 §5 (doubles). F-07: expectations from §8 only — goldens are §2.4/§8.3 literals,
hand arithmetic is §2.4, never read back from the code under test. **T1–T51 are not
modified** (§10). P4-C4: T52–T55 build raw integers over the two frozen doubles, T56/T58
are pure, T57 is pure string matching — no test in this step constructs a window, DC,
message struct, or Win32 object beyond what the frozen harness already does.

| # | Invariant | File | Phase |
|---|---|---|---|
| **T52** | `three_click_walk_reaches_the_wire_as_three_frozen_moves` — E42: entered feed over `wired_view()`; raw `WM_LBUTTONDOWN` at packed points `(400,300)`, `(440,320)`, `(360,320)` → `LeftDown {480,340}`/`{520,360}`/`{440,360}` → `Ok(Some(Order::Move))` with tiles **(100,100)/(101,100)/(100,101)** (§2.4, G1–G3); all three enqueued (drained count 3), feed dropped → `Tail::Commands { expect: 3 }` → prefix == `expected_conversation` + exactly three C2S entries byte-exact in order (`wire_len 10` each); `Stage::TornDown`, front `closed_cleanly`, `!saw_corruption`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T53** | `typed_chat_flushes_and_its_echo_dispatches` — E43: `WM_KEYDOWN 0x0D` / `WM_CHAR 'h'` / `WM_CHAR 'i'` / `WM_KEYDOWN 0x0D` → `Ok(None)`, `Ok(None)`, `Ok(None)`, `Ok(Some(Chat { Normal, "hi" }))`; local D30 DEBUG capture installed; `expect: 1`, `reply: vec![frame(OP_CHAT, &chat_body(CHAR_UNIQUE as i16, …, "hi"))]` → one C2S entry `08 00 00 75 00 01 68 69` (`wire_len 8`), one S2C entry `== frame(OP_CHAT, …)`; capture contains `in-world frame dispatched` + `opcode=118` + `outcome=Chat`; `Stage::TornDown`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T54** | `two_insert_presses_enqueue_two_sitdowns` — E44: `KeyDown 0x2D` → `Ok(Some(SitDown))`, `KeyUp 0x2D`, `KeyDown 0x2D` → `Ok(Some(SitDown))` (guard `keybinds.rs:105` cleared by `:141`); drained count 2 → `expect: 2` → two C2S entries `06 00 00 F0 00 02` (`wire_len 6` each), in order; `Stage::TornDown`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T55** | `escape_cancels_the_line_and_nothing_reaches_the_wire` — E45: the seven-message T7.3(d) script (focus, `h`, `i`, **Escape**, re-focus, flush) → **every** step `Ok(None)`, zero commands queued → `Tail::Commands { expect: 0 }` (T50 fast path) → transcript **==** `expected_conversation` byte-for-byte (zero pushed entries — a surviving line would push the D golden), `Stage::TornDown`, front `closed_cleanly`, `!saw_corruption`, `heartbeats 0` | `tests/input_loopback.rs` | loopback |
| **T56** | `every_composed_event_enqueues_within_the_16ms_frame_budget` — E46: after `enter_world(view)`, T10's exact 100-event script re-encoded as **raw messages** (WO-8.3 T51's encoding: 16 × `[WM_KEYDOWN 0x0D, WM_CHAR 'h', WM_CHAR 'i', WM_KEYDOWN 0x0D, WM_KEYDOWN 0x70, WM_MOUSEMOVE (5,7)]` + `[WM_KEYDOWN 0x0D, WM_CHAR 'h', WM_CHAR 'i', WM_KEYDOWN 0x0D]`) through `wm_to_input → feed`, each event timed **individually**: all 100 `< 16 ms` **per event**, exactly **17** commands enqueued (T10/T51's counts), all drained; margin rationale (µs-scale enum + mpsc costs, debug-mode worst case) documented in the test and never relaxed | `src/graphics/window/tests.rs` | unit |
| **T57** | `the_enter_world_trigger_filter_admits_only_the_stage_signal` — E48 table, over the pure `enter_world_signal`: `("in-world frame dispatched", "Entered { char_uniques: [3], npcs: 0 }")` → **true**; same name × `Chat(ChatMessage { … "hi" … })`, `Removed { char_unique: 3, applied: true }`, `Applied { … }`, `Ordered { … }`, `Unhandled { cmd: 999 }`, `Unmatched`, `""` → **false** each; `("heartbeat sent", "Entered { … }")`, `("session ended", "Entered { … }")`, `("in-world order echo reported", "Entered { … }")`, `("in-world frame dispatch", "Entered { … }")` (prefix typo) → **false** each; `enter_world_signal` never panics (any/empty inputs) | `src/main.rs` (`#[cfg(test)] mod tests`) | unit (bin target) |
| **T58** | `enter_world_view_pins_the_render_center_and_map_size` — E47: `enter_world_view(ScreenMode::Mode800x600.backbuffer(false).screen_rect(), (64, 64))` == `MapView { screen: ScreenRect { left: 0, top: 0, right: 959, bottom: 679 }, center: (32, 32), map_size: (64, 64) }` — hand rect from `Client.cpp:3206-3208` (`screen.rs:129-138`), hand center from `draw_map`'s literal (`window.rs:440-445`), hand map from `build_scene`'s header (`window.rs:624-629`); plus the empty-input purity check (`enter_world_view` is a pure constructor — no state, no Win32) | `src/graphics/window/tests.rs` | unit |

**Baseline: 414 → 421 expected** (+7 = T52–T58): lib **371 → 373** (T56/T58 appended to
`graphics::window::tests`), bin `src/main.rs` **0 → 1** (T57), `input_loopback` **7 → 11**
(T52–T55; D22's `frames.rs` 3 inner tests re-run there unchanged), `asset_fixtures` 11 and
`e2e_loopback` 25 untouched; ignored **5 → 6** (the 1 lib operator + 4 `live_integration`
preserved, +1 `live_acceptance`). The matrix cell is copied from the delivered `test.log`.
Cross-checks the Verifier runs: `cargo test --lib graphics::window` = **6 passed + 1
ignored** (was 4 + 1); `cargo test --test input_loopback` = **11/11**;
`cargo test --bin goonzu_client` = **1/1**; `cargo test --lib input::tests` = **43/43**
(unchanged — `src/input/**` untouched).

## 10. Forbidden shortcuts

- **Touching `src/input/**` in any way** — this WO's file plan contains zero files there;
  a single added line (even a test) is a violation. T1–T51 stay byte-identical
  (`src/input/tests.rs` full diff empty; `window/tests.rs` append-only after T51,
  zero deletion lines).
- **Editing `tests/e2e_loopback.rs` or `tests/e2e_loopback/**`** (frozen harness, D22) —
  "adding a helper for convenience" (a shared `Capture`, a tail variant) is a write-guard
  violation; the local D30 copies are the sanctioned route.
- Touching `src/client/**`, `src/game/**`, `src/lib.rs`, `Cargo.*`, `tests/asset_fixtures*`,
  `tests/live_integration*`, or any READ-ONLY file of §6 — finding first, P3-6. Adding a
  driver tracing event "for a cleaner trigger" is exactly such a violation (D24 consumes
  `driver.rs:850` as delivered).
- Editing `src/graphics/**` outside the §7.2 seam lines — the render/present/tick/pump/
  scene/draw hunks stay **zero** (hunk-level audit, §13.5); `run_demo_inner`'s ready plumb
  and the `run_demo` delegate are the only touched regions of the demo path.
- Any change to `translate`/`validate`/`Intent`/`InputEvent`/`UiState`/`wm_to_input`/
  `InputFeed` — 8.0–8.3 delivered bytes are the contract; the composition *uses* them.
- Any queue/ring/capacity/coalescing structure anywhere (P4-C1 — now also in `main.rs`:
  the channel is the frozen `commands::channel()`, nothing buffers events);
  **regenerating or "fixing" a §8.3 golden** (F-07 — the §2.4 literals are the only
  expectation source for E42–E45).
- Building a Win32 object, window, DC, message struct, or display in any **gate** test
  (P4-C4) — T57 must never call `post_enter_world`/`PostMessageW` or name a `HWND`
  constant; the arm and the posting half have no gate test (operator-only, §11);
  the live vehicle stays `#[ignore]`d (stripping the `#[ignore]` to "make it count" is a
  violation — live runs never become gates).
- Asserting the network leg (input→wire, input→echo) against 16 ms/50 ms (P4-C3 scopes the
  budget to input→enqueue; the network figures are printed evidence, §7.4); relaxing
  T10/T51/T56 margins if they flake (disclose, never silently).
- Promoting any live result to a gate; recording credentials/passwords/MAC values in any
  evidence file; running the live vehicle without the `#[ignore]` message's env contract.
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*.md` (Verifier/Scribe only), `README.md`,
  `GoonZu_Source/**`, any other WO or audit doc, any **existing** `evidence/**` bundle.
- Intra-doc-linking `graphics::window` from non-`cfg(windows)` code paths, or leaving an
  intra-doc link that breaks `cargo doc --no-deps` **off-Windows** (WO-8.3 §2.2-9's
  standard — the reason D29 keeps the `#[cfg(not(windows))]` body).

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/8.4/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`,
  `test.log`, `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`,
  `zero-queue-grep.txt`, `write-guard.txt`, `tests-unchanged.txt`,
  `src-input-untouched.txt`, `SHA256SUMS` — hashes over the committed (LF) bytes
  (6.3 L-2 lesson); the matrix `Tests Baseline` cell is copied from `test.log`
  (**421** expected: 373 lib + 1 + 11 + 25 + 11).
- `postprobe/` — the second full gate run after all probes are restored (exit-0 logs;
  §12's discipline: pristine → probes → restore → **green baseline re-run** recorded here).
- **Zero-queue audit (gate item 4, the WO-8.2 §2.2-6 corrected pattern):**
  `grep -rinE 'VecDeque|coalesc|MAX_KEY|\bring\b' src/input/` → no matches in
  `zero-queue-grep.txt` (P4-C1), plus a note that `main.rs`'s composition introduces no
  buffer between event and `send` (the channel is frozen `commands::channel()`).
- **`src/input/**` untouched (§13.5):** `src-input-untouched.txt` — `git diff <base>
  <delivery> -- src/input/` **empty**, and `cargo test --lib input::tests` green at 43/43.
- **Append-only proofs:** `tests-unchanged.txt` — `git diff -U0 <base> <delivery>` over
  `src/graphics/window/tests.rs` and `tests/input_loopback.rs` shows additions only
  (zero deletion lines; T40–T51 and T47–T50 regions byte-identical), and the frozen
  `tests/e2e_loopback*` diff is empty.
- **Write guard:** `write-guard.txt` — `git diff --name-only` limited to the §6 files, plus
  a hunk-level audit of `window.rs`: every changed line is seam-line (§7.2's six items),
  render/present/tick/pump/scene hunks **zero**, the `_ => DefWindowProcA` fallthrough and
  the 8.3 input arm byte-identical; `src/client/**`, `src/game/**`, `src/input/**`,
  `src/main.rs`'s exit match arms (semantics + codes), matrix, contributing, README,
  legacy tree and existing evidence untouched.
- **D30 duplication disclosed:** `summary.md` names both local `Capture` copies
  (`input_loopback`, `live_acceptance`) and why e2e's frozen one is not reused — so the
  `+7` arithmetic and the two new small helpers stay auditable.
- Four mutation probes recorded (§12): mutated diff, failing output at exit 101, pristine
  restore, porcelain CLEAN after each, and the green post-probe baseline in `postprobe/`.
- **`evidence/8.4/live/`** — the D28 bundle, recorded **when a reachable server and the
  operator env are available** (the `CONTRIBUTING.md:28-46` recipe):
  `README.md` (env recipe, relay wiring, capture commands, credential policy),
  `relay/` (the copied transparent-capture project from `evidence/live/relay/`),
  `live-session.log` (the vehicle's stdout: latency table, feed results, echo
  observations, final stage), `latency.md` (input→enqueue per-event figures against
  P4-C3's ≤ 16 ms / < 50 ms lines, input→echo observed figures marked *evidence, not a
  budget*), and `windowed-run.md` (the operator walkthrough of the composed binary —
  window opens, click walks, chat line flushes + 118 in the debug log, Insert toggles,
  Escape cancels, X closes with `stage=TornDown` exit 0 — D25/D26's operator verification).
  **When no server is reachable:** `README.md` records `NOT RUN — no reachable game
  service` with the `CONTRIBUTING.md:28-46` citation; **PASSED is unaffected** — no
  verification-gate item (1–6) and no §13 item requires the live bundle.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test | Restore |
|---:|---|---|---|
| 1 | `wm_to_input`: drop the window→backbuffer translation — return the raw zero-extended coordinates (delete `+ MAP_TILE_XSIZE + SCREEN_RX` / `+ MAP_TILE_YSIZE + SCREEN_RY`) | **T52** (`LeftDown {480, 340}` becomes `{400, 300}` → the hand-golden Move assertion fails — E42's `+80/+40` proven load-bearing *through the acceptance script*, not just 8.3's unit) | `git checkout --` |
| 2 | `enter_world_view`: wrong composition center — `center: (100, 100)` instead of `(32, 32)` | **T58** (`center: (32, 32)` expected, `(100, 100)` got — E47's render-parity pin) | `git checkout --` |
| 3 | `enter_world_signal`: drop the message-name check — `outcome_debug.starts_with("Entered")` only | **T57** (`("heartbeat sent", "Entered { … }")` expected `false`, got `true` — E48's name half) | `git checkout --` |
| 4 | `enter_world_signal`: drop the outcome check — `event_name == "in-world frame dispatched"` only | **T57** (`("in-world frame dispatched", "Chat(ChatMessage { … })")` expected `false`, got `true` — E48's outcome half; a chat frame would re-arm the feed on every line) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit
101 and the failing assertion, restore, verify `git status --porcelain` clean — then re-run
the full gate set to the delivered baseline (**421**) and record it in `postprobe/`
(`WO-6.3.md` §9 / `evidence/7.x/mutation-probes.md` discipline). **Probe disclosures
pre-emptive:** (p1) deleting the `+80/+40` terms orphans `MAP_TILE_XSIZE/MAP_TILE_YSIZE/
SCREEN_RX/SCREEN_RY` inside `window.rs` (their only use), so under `-D warnings` the probe
fails compilation instead of showing the tabled assertion — run it as §12 words it ("run the
named test") with default flags to record exit 101 + the failing assertion, then re-run the
full `-D warnings` gate set after every restore (the same disclosure 8.3 recorded, verified
again); (p2–p4) compile clean under `-D warnings` either way. Probes 2–4 target **this
step's own code**; probe 1 re-proves the 8.3 decode through the new script.

**Not probe-able in gates (P4-C4), stated up front:** the `WM_ENTER_WORLD` arm, the window
thread + ready ordering (D25), `close_demo`/`trigger` pairing (D26), and the Layer's
`post_enter_world` call are Win32-side — their verification is the §11 `live/` operator
run (`windowed-run.md`), never a mutation probe and never a gate.

## 13. Definition of done

A Step 8.4 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at the delivered baseline from `test.log` (**421**
   expected: 373 lib + 1 bin + 11 `asset_fixtures` + 25 `e2e_loopback` + 11
   `input_loopback`), `fmt` 0 diffs, `doc` 0 warnings **on Windows and off-Windows**
   (D29's `cfg` boundary), porcelain clean.
2. **T1–T51 byte-identical** — `src/input/**` diff **empty** (`src-input-untouched.txt`,
   `cargo test --lib input::tests` 43/43) and `window/tests.rs`/`input_loopback.rs`
   append-only (`tests-unchanged.txt`: zero deletion lines); the 5 pre-existing ignored
   tests preserved, **6** ignored total (the new `live_acceptance` named).
3. Every decision D24–D30 pinned: D24 → T57 (+ probes 3–4) with the Win32 posting half
   under P4-C4's operator-only clause (`CONTRIBUTING_PHASE4.md:120-126`); D25/D26 → the
   §11 `windowed-run.md` operator record (window open → walk/chat/sit/Escape on screen →
   X → `stage=TornDown` exit 0; config error exits 2 without a window); D27 → T58 (+ probe
   2) with both divergences stated in §8; D28 → the live vehicle `#[ignore]`d + run under
   `evidence/8.4/live/` **or** the not-run disclosure (never blocking); D29 → no new env
   var/flag in the diff + off-Windows doc green; D30 → T53's capture assertion green.
4. All four §12 probes show their named test failing at exit 101 with the tabled assertion,
   pristine restores, porcelain clean after each, and the green post-probe baseline at
   **421** recorded in `postprobe/`.
5. Write guard holds: only §6 files changed; `src/client/**`, `src/game/**`,
   **`src/input/**`**, `src/lib.rs`, `tests/e2e_loopback*`, `Cargo.*`, matrix, contributing,
   README, legacy tree and existing evidence untouched; `window.rs`'s diff is seam-lines
   only (hunk audit — render/present/tick/pump/scene zero, the 8.3 input arm and the
   `_ => DefWindowProcA` fallthrough byte-identical); `src/main.rs`'s exit-code match keeps
   the delivered messages and codes 0/1/2 (WO-6.1 §5), reached after `close_demo` + join.
6. F-07 holds: every expectation traces to E42–E48 in §8, and every E-row traces to a legacy
   `file:line`, a frozen constant (§8.3 goldens), or the §2.4 hand arithmetic — never to the
   code under test.
7. Zero-queue audit green (the corrected §11 pattern over `src/input/`, no matches) and the
   structural claim holds: no `.await`/buffer/hop between the decoded event and
   `CommandSender::send` in the composed chain (D24's path is `decode → feed → send`, one
   synchronous call); T56 green with its margin documented; T10 and T51 still green
   unchanged.
8. No new crates, no new opcodes, no wire-byte changes, no `src/input`/`src/client`/
   `src/game` changes, no Win32 object in any gate test (P4-C4 — T57 is strings-only),
   no live run used as a gate (the vehicle stays `#[ignore]`d; live evidence present **or**
   its not-run disclosure, per `CONTRIBUTING.md:28-46`), the `+7` test arithmetic and the
   two D30 `Capture` copies disclosed in `summary.md`.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

_(appended at delivery: branch, code delivery SHA, gates, probe table, disclosures — same
format as WO-8.3 §14; a note commit cannot cite its own SHA)_

### Verdict - Verifier

_(appended at verification: S4 clone SHA, both gate runs, DoD 13.1–13.8 table, probe
re-runs, live-evidence disposition (run or not-run disclosure), findings `F-8.4-NN` and
their adjudication)_
