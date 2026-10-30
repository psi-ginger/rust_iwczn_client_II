# evidence/7.2 - Adversarial mutation probes (WO-7.2 §8, Implementer run, restored after each)

Each probe mutates one kernel/loop site, runs the WO-named test subset, records the
loud deterministic failure (exit 101 + the named test in output), then restores the
source from a pre-probe byte copy. Probe (d) lands on the pure present-crop math test
(directive amendment: no headless recorder exists).

## Probe (a) - flip the pitch advance in `put_spr` (row+width -> row+1 cell)

Mutation in `src/graphics/blit.rs`:

```rust
let dest_at = dest_y as usize * fb.width;  //  ->  let dest_at = dest_y as usize;
```

exit 101. Named test FAILED:

- `graphics::blit::tests::pitch_advance_lands_each_row_on_its_own_scanline`
  (panicked at `src\graphics\blit.rs:265:9`)

Restored clean (file byte-compares to the pre-probe copy).

## Probe (b) - break the color key so `None` cells write palette index 254

Mutation in `src/graphics/blit.rs`:

```rust
let Some(index) = index else { continue; };  //  ->  color-key removed:
let index = match index { Some(i) => i, None => &254u8 };
```

exit 101. Named test FAILED:

- `graphics::blit::tests::transparent_cells_never_write`
  (panicked at `src\graphics\blit.rs:284:9`, sentinel cell no longer kept)

Restored clean.

## Probe (c) - swap the `unique == 90` layer selection

Mutation in `src/graphics/draw_map.rs`:

```rust
Some(&draw_info) if ((draw_info >> 24) as u8) == UNIQUE_LAYER0 => 0,
_ => 1,
//  ->  arms swapped: unique 90 maps to layer 1, default maps to layer 0
Some(&draw_info) if ((draw_info >> 24) as u8) == UNIQUE_LAYER0 => 1,
_ => 0,
```

exit 101. Named test FAILED:

- `graphics::draw_map::tests::unique_90_swaps_layer_1_for_layer_0`
  (panicked at `src\graphics\draw_map.rs:332:9`)

Restored clean.

## Probe (d) - move the present crop by one tile

Mutation in `src/graphics/present.rs`:

```rust
src_x: MAP_TILE_XSIZE + SCREEN_RX,  //  ->  src_x: MAP_TILE_XSIZE * 2 + SCREEN_RX,
```

exit 101. Named test FAILED (the pure crop-math test, `Draw.cpp:1145-1149`):

- `graphics::present::tests::crop_is_the_one_tile_border_for_every_mode`
  (panicked at `src\graphics\present.rs:202:13`)

Restored clean.

## Probe (e) - drain bound 10 -> 11

Mutation in `src/graphics/tick.rs`:

```rust
for _ in 0..10 {  //  ->  for _ in 0..11 {
```

exit 101. Named test FAILED:

- `graphics::tick::tests::at_most_ten_messages_drain_per_tick`
  (panicked at `src\graphics\tick.rs:212:9`, `left: 11  right: 10`)

Restored clean.

## Determinism / restoration proof

After all five reverts the full suite re-ran green (the restored files' mtimes were
refreshed so cargo rebuilt from the restored sources, not a stale probe binary):

```
test result: ok. 299 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Counts copied verbatim from the `test result:` lines (no handwritten numbers).
The 1 ignored lib test is `operator_game_window_opens_on_screen` (operator on-screen
verification only, excluded from every count, P3-3); the 4 ignored
`live_integration` tests are pre-existing (7.1 baseline). No `PROBE` mutation
markers remain in `src/` (searched after the run).
