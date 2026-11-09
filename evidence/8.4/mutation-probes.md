# Step 8.4 - mutation probes (WO-8.4 section 12) - Verifier independent re-run

- **S4 clone** `step/8.4-live-acceptance` @ `19e603d` (delivery `d47d57c`); every probe
  applied to the committed tree, one semantic break at a time, named test run with plain
  `cargo test` (section 12 wording: run the named test), then `git checkout --` restore +
  `git status --porcelain` verification after each.
- Disclosure p1 **verified**: probe 1 orphans the four border consts inside `window.rs`
  (`MAP_TILE_XSIZE/MAP_TILE_YSIZE/SCREEN_RX/SCREEN_RY` — their only use), so it runs at
  default flags; probes 2-4 compile clean either way. The full `-D warnings` gate set
  re-ran green after every restore (run 2 in `postprobe/`, 421/421).
- **4/4 reproduced independently** - not taken on trust from the Implementer's note;
  every tabled assertion reproduced verbatim.
- Plus an **ad-hoc Item-7 reverse check** (directive item 4; disclosed; NOT a section-12
  probe) recorded at the end.

captured: 2026-11-09 (+07:00), S4 clone @ `19e603d`, tree `3d28e2fa...`

## PROBE 1: wm_to_input drops the window->backbuffer translation -> T52

mutated diff (src/graphics/window.rs):

    @@ -308,8 +308,8 @@ pub fn wm_to_input(msg: u32, w_param: usize, l_param: isize) -> Option<InputEven
         let raw = l_param as u32;
         let (x, y) = (
    -        (raw & 0xFFFF) as i32 + MAP_TILE_XSIZE + SCREEN_RX,
    -        ((raw >> 16) & 0xFFFF) as i32 + MAP_TILE_YSIZE + SCREEN_RY,
    +        (raw & 0xFFFF) as i32,
    +        ((raw >> 16) & 0xFFFF) as i32,
         );

named test: `input_loopback::three_click_walk_reaches_the_wire_as_three_frozen_moves` (T52)
exit: **101** (101 required)

    thread 'three_click_walk_reaches_the_wire_as_three_frozen_moves' panicked at tests\input_loopback.rs:670:9:
    assertion `left == right` failed: E30/§2.4: the packed window point decodes through the +80/+40 border to (480, 340)
      left: Some(LeftDown { x: 400, y: 300 })
     right: Some(LeftDown { x: 480, y: 340 })
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out

- matches the tabled expectation exactly (`LeftDown {480, 340}` becomes `{400, 300}`).
- restore: `git checkout -- src/graphics/window.rs` — porcelain CLEAN after restore.

## PROBE 2: enter_world_view wrong composition center -> T58

mutated diff (src/graphics/window.rs):

    @@ -764,7 +764,7 @@ fn enter_world_view(screen: ScreenRect, map_size: (i32, i32)) -> MapView {
         MapView {
             screen,
    -        center: (32, 32),
    +        center: (100, 100),
             map_size,
         }

named test: `graphics::window::tests::enter_world_view_pins_the_render_center_and_map_size` (T58)
exit: **101** (101 required); compiled clean under `-D warnings` (disclosure p2 verified)

    thread 'graphics::window::tests::enter_world_view_pins_the_render_center_and_map_size' panicked at src\graphics\window\tests.rs:340:5:
    assertion `left == right` failed: E47: the view is the render's rect + draw_map's center + the demo map's header - clicks decode against exactly what is on screen
      left: MapView { screen: ScreenRect { left: 0, top: 0, right: 959, bottom: 679 }, center: (100, 100), map_size: (64, 64) }
     right: MapView { screen: ScreenRect { left: 0, top: 0, right: 959, bottom: 679 }, center: (32, 32), map_size: (64, 64) }
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 373 filtered out

- tabled expectation reproduced (`center: (32, 32)` expected, `(100, 100)` got).
- restore: `git checkout -- src/graphics/window.rs` — porcelain CLEAN after restore.

## PROBE 3: enter_world_signal drops the message-name check -> T57

mutated diff (src/main.rs):

    -fn enter_world_signal(event_name: &str, outcome_debug: &str) -> bool {
    -    event_name == "in-world frame dispatched" && outcome_debug.starts_with("Entered")
    +fn enter_world_signal(_event_name: &str, outcome_debug: &str) -> bool {
    +    outcome_debug.starts_with("Entered")
     }

named test: `tests::the_enter_world_trigger_filter_admits_only_the_stage_signal` (T57, bin target)
exit: **101** (101 required)

    thread 'tests::the_enter_world_trigger_filter_admits_only_the_stage_signal' panicked at src\main.rs:310:13:
    E48: message "heartbeat sent" must not fire even with an Entered outcome
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

- tabled assertion reproduced verbatim (E48's name half).
- restore: `git checkout -- src/main.rs` — porcelain CLEAN after restore.

## PROBE 4: enter_world_signal drops the outcome check -> T57

mutated diff (src/main.rs):

    -fn enter_world_signal(event_name: &str, outcome_debug: &str) -> bool {
    -    event_name == "in-world frame dispatched" && outcome_debug.starts_with("Entered")
    +fn enter_world_signal(event_name: &str, _outcome_debug: &str) -> bool {
    +    event_name == "in-world frame dispatched"
     }

named test: `tests::the_enter_world_trigger_filter_admits_only_the_stage_signal` (T57, bin target)
exit: **101** (101 required)

    thread 'tests::the_enter_world_trigger_filter_admits_only_the_stage_signal' panicked at src\main.rs:296:13:
    E48: outcome "Chat(ChatMessage { declared_size: 20, char_unique: 3, mode: 1, is_game_master: false, system_txt: 0, level: 0, player_name: \"someone\", text: \"hi\" })" of the stage message must not fire
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

- tabled assertion reproduced verbatim (E48's outcome half).
- restore: `git checkout -- src/main.rs` — porcelain CLEAN after restore.

## Post-probe baseline

`postprobe/gate-exits.txt`: all five gates exit 0 at **421/421**, 6 ignored,
porcelain CLEAN before and after (run 2).

## Ad-hoc Item-7 reverse check (directive item 4; disclosed; NOT a section-12 probe)

Purpose: verify the section-14 delivery-note **Item 7** tracing-subscriber disclosure
("7/7 parallel runs green after, 2/2 red before") by removing the process-global DEBUG
sink guard from T53 and re-running the parallel `input_loopback` binary on this host.

- guard block removed: `let _global = tracing::subscriber::set_global_default(
  tracing_subscriber::fmt().with_max_level(Level::DEBUG).with_writer(std::io::sink).finish());`
- **without the guard, parallel runs: 3/3 RED** — `cargo test --test input_loopback`
  exit 101 each time, T53 `... FAILED`, 10 passed / 1 failed.
- **without the guard, isolated run: PASS** (`cargo test --test input_loopback
  typed_chat_flushes` → 1 passed) — reproducing the disclosure's "isolated runs passed"
  split exactly.
- failure signature reproduced: `E43: the frozen driver dispatch record must appear
  (driver.rs:850); got:` with an empty capture — the disclosed `capture: ""`.
- **with the guard (delivered tree): 7/7 full parallel gate runs green** (gate runs 1+2
  plus 5 campaign runs, `item7-parallel-runs.txt`), T53 `... ok` in every one.
- both citations spot-verified at their sources: `tracing-core subscriber.rs:676` =
  `NoSubscriber::register_callsite -> Interest::never()` (tracing-core 0.1.36, exact);
  `driver.rs:850` = the frozen `debug!(... "in-world frame dispatched")` record in
  `src/client/driver.rs` (blob-identical, frozen).
- restore: `git checkout -- tests/input_loopback.rs` — porcelain CLEAN after every
  mutation; no gate result was ever produced from a mutated tree (run 2 followed all
  restores).
