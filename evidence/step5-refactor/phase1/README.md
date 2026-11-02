# Step 5 — Phase 1: Characterization / Seams (T-4e)

Additive-only: **0 production code edits** (the commit touches exactly
`src/client/driver/tests.rs` and `src/game/dispatcher/tests.rs`).

## The gap being closed

Step 4 §7 (`docs/audit/2026-11-02-step-4-refactor-verification.md`,
"adopt_local blind spot"): deleting `dispatcher.adopt_local(&char_client);`
(`driver.rs:725`) left the whole suite green at 334/334 — the call's only
effect is `local.life/x/y` writes in `apply_char_update`'s self-path, the
`DispatcherState` dies with `game_phase`, and every dispatch outcome is
adoption-independent. The audit's stated protection was the T-1g verbatim
checklist alone.

## The two tests that close it

| Test | File | What it pins |
|------|------|--------------|
| `world_entry_adoption_is_wired_before_the_in_world_loop_starts` | `src/client/driver/tests.rs` | The wiring: the exact statement `dispatcher.adopt_local(&char_client);` exists **once** in `driver.rs`, strictly between `DispatcherState::new(` and `run_with_shutdown(`. Whole-line match, so a comment quoting it cannot satisfy it. This automates the T-1g checklist. |
| `a_self_char_update_before_adoption_reports_the_same_outcome_and_drops_the_write` | `src/game/dispatcher/tests.rs` | The behavior behind the blind spot: an unadopted self opcode-200 still reports `Applied { target: Local }` and still lands the full `local_delta` overlay while `life/x/y` are silently dropped — i.e. the outcome can never reveal a missing adoption, which is why the wiring assertion above is required. |

Why source-level for the wiring half: no behavioral seam exists without
production edits — `local_char` has no reader outside the self-path write,
`DispatcherState` is a local in `game_phase`, the in-world loop sends only
heartbeats, and `DriverOutcome` carries no dispatcher state. A
`#[cfg(test)]` probe inside `game_phase` was rejected to keep the phase
strictly test-only (and Phase 3's verbatim extraction diff pristine).

## Gates (all 5, this boundary)

| Gate | Exit |
|------|------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 (`01-cargo-check.exit`) |
| `cargo clippy --all-targets -- -D warnings` | 0 |
| `cargo fmt --check` | 0 |
| `cargo test` | 0 |
| `cargo doc --no-deps` | 0 |

**Test delta: 334 → 336 (+2)** — lib 301 → 303; fixtures 11 and e2e 22
unchanged; **5 ignored preserved** (1 lib + 4 `live_integration`).

## Mutation validation (design proof; re-probed formally in Phase 5)

`06-gap-mutation-check.log/.exit`: with `driver.rs:725` deleted, the named
wiring test fails at its `hits.len() == 1` assertion — **EXIT=101**, 303
filtered out. Restored with `git checkout -- src/client/driver.rs`;
`git status --porcelain -- src tests` shows only the two intended test
files, and both new tests re-run green.

## Tracked, not done here

- `life_x` bits 30-31 fixture park (audit P-05 follow-up): still open,
  deliberately untouched — flagged so it is not silently dropped.
- Step 4 `summary.md` prose fix ("13 of 14" → "11 of 14", stale push
  sentence): scheduled as a `docs(qc)` note inside the Phase 5 bundle.
