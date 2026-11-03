# Step 5 — Phase 3: T-1e In-World Phase Extraction

**Verdict: complete — byte-identical verbatim extraction, zero behavioral
alteration, all 5 gates green, test count locked at 336/336.**

The in-world tail of `game_phase` (`src/client/driver.rs:724-763`, 40 lines)
moved into a new `async fn in_world_phase(...) -> Result<DriverOutcome,
DriverError>` placed after `InWorldError`. `game_phase` now delegates with a
single call. This completes the three-phase composition `flow` documents:
`front_phase` → `game_phase` → `in_world_phase`.

## What changed (`git diff --stat`)

```
 src/client/driver.rs | 70 ++++++++++++++++++++++++++++++--------------
 1 file changed, 52 insertions(+), 18 deletions(-)
```

Five edits, all in `src/client/driver.rs`:

1. Import: `use crate::game::entry::{ClientCharUnique, GameError, GameLink};`
   (+`ClientCharUnique` — the new `unique` parameter type).
2. Import: `world_entry::{…, CharClient, …}` (+`CharClient` — the new
   `char_client` parameter type).
3. `game_phase` tail → `in_world_phase(cfg, shutdown, &unique, worlds,
   char_client, game).await` (the delegation call).
4. New `in_world_phase` inserted after `InWorldError`, body moved verbatim.
5. Doc updates: `flow` ("three phases"), `game_phase` ("then
   [`in_world_phase`]"), plus the new function's own doc.

**The two new `use` bindings are the only non-moved tokens in the diff.**

## Byte-identity proof (`00-block-identity.txt`)

| | lines | SHA256 |
|---|---|---|
| Before (`driver.rs:724-763`) | 40 | `307e06e578e17023517fffc49e33b0dd52f36c1f553267951116f14914bb7db0` |
| After (`in_world_phase` body `:770-809`) | 40 | `307e06e578e17023517fffc49e33b0dd52f36c1f553267951116f14914bb7db0` |

`byte_identical=True` — the block is the same bytes before and after, only
relocated.

## `git diff --color-moved` (`00-diff-color-moved.diff`)

- 300 ANSI codes, **90 color-band codes** — git itself painted the moved
  lines as a move band.
- Reading `00-diff-plain.diff`: the 40-line block appears only as
  **context** lines (never `-`/`+`) — git anchored both sides to the same
  bytes; only imports, the delegation call, the new signature, and doc lines
  are `-`/`+`.

## Code-token checklist (`00-checklist-counts.txt`)

Whole-file counts over non-comment lines, HEAD (`5ef7dc4`) vs working tree:

- **All 21 pre-existing structural tokens equal** — e.g. `Stage::` 60→60,
  `check_shutdown(` 12→12, `map_err(` 24→24, `debug!(` 8→8, `info!(` 2→2,
  `DispatcherState::new(` 1→1, `run_with_shutdown(` 1→1, `Flow::Continue`
  2→2, `DriverOutcome {` 2→2, `LoopError::Link` 1→1, `LoopError::Handler`
  1→1.
- `char_client` 4→6: **+2 = the new parameter + the delegation call** —
  expected, both are new binding sites, not content changes.
- New bindings: `in_world_phase` 0→2 (definition + `game_phase` call),
  `ClientCharUnique` 0→2 (import + parameter).

Design note: `char_client` is passed **by value** so the body reads
byte-identical (a `&char_client` reference would have rewritten the body's
existing `char_client` uses to a double reference — a content change).
`unique` is passed by ref and appears only in the untouched
`unique.char_unique` line.

## Gates (all 5, this boundary)

| Gate | Exit | Log |
|------|------|-----|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | **0** | `01-cargo-check.log` |
| `cargo clippy --all-targets -- -D warnings` | **0** | `02-cargo-clippy.log` |
| `cargo fmt --check` | **0** | `03-cargo-fmt.log` |
| `cargo test` | **0** | `04-cargo-test.log` |
| `cargo doc --no-deps` | **0** | `05-cargo-doc.log` |

Each gate has a paired `.exit` file recording `EXIT=0`.

**Test count held: 336/336** — 303 lib + 11 fixtures + 22 e2e; 5 ignored
preserved (1 lib + 4 `live_integration`). Zero tests added, moved, or
removed this phase (extraction only).

Both Phase 1 tests still pass after the move:

- `client::driver::tests::world_entry_adoption_is_wired_before_the_in_world_loop_starts ... ok`
  — the `include_str!` whole-line match and ordering assert see the adopted
  line at its new position inside `in_world_phase`.
- `game::dispatcher::tests::a_self_char_update_before_adoption_reports_the_same_outcome_and_drops_the_write ... ok`

## Zero-behavioral-alteration summary

- Same statements, same order, same stage gates, same tracing, same errors.
- No type changed, no condition changed, no early return changed.
- Only motion + 2 imports + 1 delegation call + docs.
