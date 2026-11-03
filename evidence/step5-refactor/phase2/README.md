# Step 5 — Phase 2: T-2e Motion Survey (per-frame allocation hoist)

**Verdict: N/A — no qualifying hoist candidate exists in the Step 5 module
group.** Zero production code changes this phase; the deliverable is this
survey with its verified justification.

Surveyed at `0f52ca2` (Phase 1 head), before any Phase 3 edit.

## Scope

The four Step 5 files (`CONTRIBUTING.md:161-183`), i.e. exactly the group
Step 5 owns:

- `src/client/event_loop.rs`
- `src/client/heartbeat.rs`
- `src/client/teardown.rs`
- `src/game/dispatcher.rs`
- plus `src/game/char_info.rs:686` (`parse_make_char_info_many`), the
  parser `dispatch` calls for opcode 6, and `src/client/driver.rs:728-745`
  (the in-world loop call site)

## Method

1. Enumerate **every loop construct** in the group (`loop {` / `for … in` /
   `while …`), code lines only — doc-prose matches excluded.
2. For each loop, enumerate **every heap allocation reachable in its body**
   (`Vec`/`String`/`format!`/`to_vec`/`*_map::new`/`collect`) and classify:
   **loop-invariant scratch** (same content each iteration → hoistable) vs
   **content-varying result** (fresh per iteration → inherent) vs
   **persistent state** (the model itself, not scratch).
3. Reject any candidate that is not loop-invariant scratch.

A candidate *qualifies* only if it is allocated per iteration inside a loop
and its value would be identical across iterations — i.e. it could move
above the loop with zero behavioral delta.

## Findings — every loop in the group

| # | Loop | Site | Heap in body? | Qualifies? |
|---|------|------|---------------|------------|
| 1 | `run_with_shutdown` main `loop` | `event_loop.rs:369` | **None.** The only `Vec` (`states: Vec<TickerState>`) is built **once before the loop** (`:358-367`), already hoisted. Per iteration: `states.iter().map(deadline).min()` over `Copy` `Instant`s (`:378`), stack futures in `tokio::select!` (`:384-397`), `Wake` enum by-move. | **No** |
| 2 | due-ticker `for` | `event_loop.rs:427` | **None.** `state.fired += 1`, `state.deadline += state.interval` — `Instant`/`u64` arithmetic, all `Copy`. | **No** |
| 3 | catch-up `while` | `event_loop.rs:430` | **None.** `Instant <= now` / `+=` only. | **No** |
| 4 | `TaskTracker::drain` `loop` | `teardown.rs:282` | **None.** `AtomicUsize::load`, `Instant::now()`, `tokio::time::sleep` (stack future). `terminate` (`:352`) has no loop at all. | **No** |
| 5 | `apply_login_many` `for` | `dispatcher.rs:903` | **None.** The output `Vec` is allocated **once, before** the loop, pre-sized (`Vec::with_capacity(records.len())`, `:901` — already the ideal form). `nearby.get_mut`/`insert` touch the **persistent** `BTreeMap` (the modeled `CR[]` table — state, not scratch); `WorldChar` (`:677`) has no heap fields. `char_uniques.push` is the output accumulator. | **No** |
| 6 | opcode-6 walk `while` | `char_info.rs:706` | **None per iteration beyond the accumulator push.** `let mut records = Vec::new()` (`:704`) is created **outside** the loop; each `push` carries a content-varying record. | **No** |
| 7 | heartbeat send | `heartbeat.rs:233` | **None.** `send_heartbeat` is `link.send(REQUEST_HEARTBEAT, &[])` — "No payload is built" (`:221`); the zero-length `&[]` allocates nothing. Framing/ciphering lives in `GameLink::send` (I/O-inherent, session-cipher state). Fires from loop arm `:433`, not its own loop. | **No** |

Allocation grep across the three named targets (`event_loop`, `teardown`,
`dispatcher`) returned exactly **three** hits, all already outside loop
scope: `dispatcher.rs:823` `BTreeMap::new()` (in `DispatcherState::new`,
once per connection), `dispatcher.rs:901` pre-sized `with_capacity`
(before its loop), `event_loop.rs:367` `.collect()` (before its loop).

## Near-misses considered and rejected

1. **`ChatMessage`'s `String` fields** (`dispatcher.rs:298,301`) — content
   differs every frame → content-varying *result*, not loop-invariant
   scratch. Hoisting would fabricate stale names/text.
2. **`DispatchOutcome::Entered { char_uniques: Vec<i32> }`** (`:648`) — same
   class: a fresh per-frame result, consumed immediately at the call site
   (`driver.rs:740` debug-record, then dropped). Reusing a buffer would
   require `&mut self` aliasing against the returned value for no gain.
3. **The invariant 5-byte heartbeat frame** — not built in this crate's
   heartbeat path at all (finding 7).
4. **`parse_make_char_info_many`'s `Vec::new()`** (`char_info.rs:704`) — a
   capacity *estimate* (`payload.len() / stride`) would only pre-size, not
   hoist: the `Vec` already lives outside its loop, and `push` is
   amortized O(1). A pre-size is an optimization, not a motion, and T-2e
   asks for loop-scope scratch.
5. **`driver.rs:740` `debug!(… ?outcome …)`** — per-frame by definition;
   `tracing` records nothing when the level is disabled, and the content
   varies per frame when enabled.

## Verdict

**T-2e = N/A, documented.** Every loop in the Step 5 module group is
allocation-free in its body, or already carries its accumulator outside the
loop. There is no loop-invariant scratch left to hoist, so the phase makes
**zero production edits** — consistent with Phase 1's test-only boundary and
leaving Phase 3's verbatim extraction diff pristine.

## Gates (all 5, this boundary)

| Gate | Exit |
|------|------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 (`01-cargo-check.exit`) |
| `cargo clippy --all-targets -- -D warnings` | 0 |
| `cargo fmt --check` | 0 |
| `cargo test` | 0 |
| `cargo doc --no-deps` | 0 |

**Test count held: 336/336** — 303 lib + 11 fixtures + 22 e2e; 5 ignored
preserved (1 lib + 4 `live_integration`). No tests added, moved, or
removed this phase.
