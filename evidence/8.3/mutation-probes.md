# Step 8.3 mutation probes - Verifier/Adversary re-run (S4 clone, committed tree)

Each probe: one semantic break applied to the pristine tree at the delivery tip
(`90a0f07`), the tabled named test run, `git checkout --` restore, then
`git status --porcelain -- ':!:evidence'` => empty (TRACKED_CLEAN). All four probes were
executed by the Verifier between gate run 1 and gate run 2; run 2 re-ran the full five
gates green at 414/414 afterwards (postprobe/). Exits are `cargo test`'s 101.

## P8.3-1 - drop the window->backbuffer translation (section 12 row 1)

Mutation (window.rs, the two coordinate terms):
` (raw & 0xFFFF) as i32 + MAP_TILE_XSIZE + SCREEN_RX,` -> `(raw & 0xFFFF) as i32,`
and the HIWORD term likewise (2-line diff).

- **Disclosure 1 VERIFIED:** first run under the gate flags
  (`RUSTFLAGS=-D warnings cargo test --lib wm_to_input_decodes_the_mouse_messages`)
  fails at **compile**, exit 101: `error: unused imports: SCREEN_RX and SCREEN_RY` +
  `MAP_TILE_XSIZE and MAP_TILE_YSIZE` (`-D unused-imports implied by -D warnings`) -
  exactly the Implementer's disclosed reason for running the probe without the flag.
- Tabled run (no RUSTFLAGS, per section 12 "run the named test"): **exit 101**,
  `graphics::window::tests::wm_to_input_decodes_the_mouse_messages` FAILED at
  `window/tests.rs:29`:
  `E30: WM_MOUSEMOVE decodes to Move in backbuffer space`
  `left: Some(Move { x: 480, y: 360 })  right: Some(Move { x: 560, y: 400 })`
  (T40's `left {480,360}` -> `{560,400}` exactly as tabled; the 2 unused-import warnings
  are visible in the run's output, harmless without `-D`).
- Restore: `git checkout -- src/graphics/window.rs`, TRACKED_CLEAN.

## P8.3-2 - take the key from the wrong slot (section 12 row 2)

Mutation (window.rs): `WM_KEYDOWN => Some(InputEvent::KeyDown { vk: l_param as u32 }),`
(was `w_param as u32`).

- Compiled clean under `RUSTFLAGS=-D warnings` (as disclosed for probes 2-4): **exit 101**
  at `window/tests.rs:68`:
  `E32: RETURN (0x0D) round-trips as KeyDown { vk: 13 }`
  `left: Some(KeyDown { vk: 0 })  right: Some(KeyDown { vk: 13 })`
  (tabled `{ vk: 0 }` result).
- Restore: TRACKED_CLEAN.

## P8.3-3 - swallow the channel failure (section 12 row 3)

Mutation (feed.rs, as disclosed: the tabled `send(command)` then `Ok(Some(command))` is a
use-after-move and would not compile, so the disclosed `command.clone()` form preserves
the exact semantic break):
`self.sender.send(command.clone())?;` -> `let _ = self.sender.send(command.clone());`

- Under `RUSTFLAGS=-D warnings`: **exit 101** at `src/input/tests.rs:1523`:
  `E38: the closed channel surfaces by name - no panic, no Ok`
  (matches the delivery note's tabled line number and assertion verbatim).
- Restore: TRACKED_CLEAN.

## P8.3-4 - enter_world forgets the view (section 12 row 4)

Mutation (feed.rs): `enter_world` sets only `self.ui.in_world = true`; the unused
`view` parameter is neutralised with `let _ = view;` so the probe compiles under
`-D warnings` (the semantic break is identical: `ui.view` is never set).

- Under `RUSTFLAGS=-D warnings`: **exit 101** at `src/input/tests.rs:1472`:
  `E37: enter_world wires the click context in the same call (D20)`
  `left: None  right: Some(MapView { screen: ScreenRect { left: 0, top: 0, right: 959,
  bottom: 679 }, center: (100, 100), map_size: (400, 400) })` (tabled `left: None`).
- Restore: TRACKED_CLEAN.

## Summary

| # | Mutation | Named test | Result |
|---|---|---|---|
| P8.3-1 | drop `+ MAP_TILE_* + SCREEN_*` | T40 | exit 101 - `E30 ... {480,360}` vs `{560,400}` (compile-fail under `-D warnings` first, disclosed and verified) |
| P8.3-2 | `KeyDown { vk: l_param as u32 }` | T41 | exit 101 - `E32 ... { vk: 0 }` vs `{ vk: 13 }` |
| P8.3-3 | `let _ = send(...); Ok(Some(...))` | T45 | exit 101 - `E38` at tests.rs:1523 |
| P8.3-4 | `enter_world` without the view | T44 | exit 101 - `E37 ... left: None` |

4/4 tabled failures reproduced independently by the Verifier; every restore left the tree
TRACKED_CLEAN; the full gate set then re-ran green at 414/414 (run 2). D18's mapping
(-> LeftDown) is pinned failing-if-corrected by T40's dedicated DBLCLK assertion
(`E31/D18: WM_LBUTTONDBLCLK is one more LeftDown, same coordinates`), audited in the
delivered test body.
