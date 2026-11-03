# DRAFT — Step 5 Structural Refactoring — Independent Verifier Audit

**Status: DRAFT prepared by the Implementer for Verifier review.
`Verdict: PENDING Verifier sign-off.`** Every number below is this
bundle's own record; the Verifier re-derives each one before signing.
On sign-off, file verbatim (with verdict and session header filled in)
at `docs/audit/2026-11-04-step-5-refactor-verification.md` — a path the
Implementer has deliberately not created.

**Date:** 2026-11-04
**Branch under audit:** `refactor/step5-structure` (tip = this bundle's
Phase 5 commit)
**Merge-base with `main`:** `68d3ad9` (= current `main` head →
fast-forward possible; `git merge-base --is-ancestor 68d3ad9 HEAD`
exit 0, re-derive at sign-off)
**Verdict:** `PENDING`

## 1. Scope of the delivery

Six phase commits on top of `main` @ `68d3ad9` (one per phase; the
Phase 4 pair is disclosed in §4):

| Phase | Commit | Change |
|-------|--------|--------|
| 0 | `943679d` | baseline evidence only (5 gates over `68d3ad9`, 334/334) |
| 1 | `0f52ca2` | characterization: `adopt_local` wiring test + behavioral blind-spot test (+2 lib tests, **0 production code**) |
| 2 | `5ef7dc4` | T-2e motion survey: **N/A** — no loop-scope scratch to hoist (zero production edits) |
| 3 | `7251cbc` | T-1e: extract `game_phase`'s in-world segment into `in_world_phase` (byte-identical 40-line body) |
| 4 | `d06bf63` (+ `df316a3`) | T-3e: `src/client/test_support.rs` — `local_listener` ×3 deduped; `HANDSHAKE_PAYLOAD`/`SECRETKEY_FRAME` imported from the Step 4/Step 3 donors; `df316a3` = evidence README line-count fix (30 → 27), no source change |
| 5 | *(this bundle)* | evidence: 4 probes, 3× determinism, 5 gates, summary + drafts |

Files touched vs `main` (10): `src/auth/mod.rs`,
`src/client/{driver.rs, driver/tests.rs, event_loop/tests.rs,
heartbeat/tests.rs, mod.rs, teardown/tests.rs, test_support.rs (new)}`,
`src/game/{dispatcher/tests.rs, mod.rs}`, plus `evidence/step5-refactor/`.
**`Cargo.toml`/`Cargo.lock` untouched; `src/game/dispatcher.rs`,
`src/net/**`, the Step 3 `src/auth/**` production files and all fixtures
untouched.**

## 2. Gates and determinism (re-derive)

All 5 gates at every boundary, exit 0 — **32 gate `.exit` files read
`EXIT=0`** (`baseline/`, `phase1/`…`phase4/`, `phase5/` incl.
`04-cargo-test-run{1,2,3}`). Check gate runs `RUSTFLAGS=-D warnings`;
clippy `--all-targets -- -D warnings`. The only non-zero `.exit` files
are intentional: `phase1/06-gap-mutation-check.exit` and
`phase5/probes/P-0{1..4}.exit`, all `EXIT=101`.

Phase 5 determinism: three consecutive `cargo test` runs, each
**336/336** (303 lib + 11 fixtures + 22 e2e; 5 ignored = 1 lib + 4
`live_integration`), exit 0.

## 3. Test counts (re-derive from `test result:` lines)

| Boundary | executed | ignored |
|---|---|---|
| 0 baseline | **334** | 5 |
| 1 | **336** | 5 |
| 2 / 3 / 4 | **336** | 5 |
| 5 run 1 / 2 / 3 | **336 ×3** | 5 |

Phase 1 is the only growth (+2). Phases 2–5 are motion/verification
only; the count is locked at 336.

## 4. Probes, disclosures, deviations (re-execute)

Probe matrix (`phase5/probes/P-0X.{log,exit,diff,txt}`), **4/4 at
EXIT=101**, each mutation compiled first (logs begin `Compiling` /
`Finished`), each followed by `git checkout -- <file>` +
`git status --porcelain -- src tests` verified empty:

| # | Mutation | Named test | Exit |
|---|---|---|---|
| P-01 | delete `dispatcher.adopt_local(&char_client);` (`driver.rs:771`) | `world_entry_adoption_is_wired_before_the_in_world_loop_starts` | 101 |
| P-02 | `Stage::InWorld` → `Stage::CharClient` in `in_world_phase`'s Handler arm (`driver.rs:797`) | e2e `a_flipped_byte_in_a_chat_frame_is_refused_at_the_in_world_stage` | 101 |
| P-03 | delete `RESPONSE_CHAT` arm of `dispatch()` (`dispatcher.rs:872`) | `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | 101 |
| P-04 | fall-through `other` arm returns `Err(UnexpectedOpcode)` (`dispatcher.rs:881`) | `a_foreign_opcode_is_unhandled_not_an_error` | 101 |

P-01 is the Step 4 §7 `adopt_local` blind spot, now closed by Phase 1's
wiring test and re-probed **after** Phases 3–4 moved the surrounding
code.

Disclosures to verify honest:

- **Phase 2 T-2e = N/A** — a survey result, not a motion; no probe
  exists because there was nothing to hoist (`phase2/README.md`).
- **Phase 4 two commits** (`d06bf63` + `df316a3`) — deviation from
  one-commit-per-phase, disclosed in `summary.md`; the second touches
  evidence prose only.
- **Phase 4 non-consolidations** — `frame()`/`encode_link()` copies
  kept (distinct citation docs), driver's `frame()` isolation,
  `plaintext_frame()`, auth/net `local_listener`s: documented in
  `phase4/README.md`, out of the approved T-3e scope.
- **Wiring test positional constraint** — first-`DispatcherState::new(`
  / first-`run_with_shutdown(` matching; driver source must never
  contain `run_with_shutdown(` before the adoption line.

## 5. F-07, additive-only Phase 1, citations (re-derive)

- **Zero redefinitions (grep):** `async fn local_listener` in the
  client group → 1 (`src/client/test_support.rs`); `const
  HANDSHAKE_PAYLOAD` tree-wide → 1 (`src/game/test_support.rs`);
  `const SECRETKEY_FRAME` tree-wide → 1 (`src/auth/test_support.rs`).
- **Removed literals byte-identical to donors:** heartbeat
  `[0x2A,0x00,0x00,0x00,0x7E,0xAD,0x01,0x00]` == game donor; driver
  `[0x0A,0x00,0x00,0x30,0x75,0x00,0x00,0xA7,0x00,0x5C]` == auth donor
  (both checks recorded `True` in `phase4/README.md`).
- **Citations:** aggregate `*.cpp`/`*.h` references across the 10
  changed files **48 → 49** (+1; no file decreased: `driver/tests.rs`
  2 → 3, all others equal). Phase 3 moved comment lines verbatim
  (21/21 token checklist equal); Phase 4 moved docs and preserved both
  the driver's `FrontMsg.h:114` and the donor's `FrontMsg.h:132-141`.
- **Additive-only Phase 1:** the commit touches exactly
  `src/client/driver/tests.rs` and `src/game/dispatcher/tests.rs`;
  0 production edits.

## 6. Evidence-bundle integrity (re-derive)

Committed-blob hash table in `summary.md` (10 source files,
`git rev-parse HEAD:path` over git's LF-normalized committed bytes —
the L-2 lesson). Phase 5 is evidence-only, so the `src/`/`tests/` blobs
are identical at the bundle commit; re-derive each hash against the
branch tip. Every probe `.exit` must match its table row (101), every
gate `.exit` `EXIT=0`. The evidence logs are themselves committed blobs,
so git integrity covers them (same mechanism note as Step 4 §6).

## 7. Merge authorization (re-derive)

`main` @ `68d3ad9` is an **ancestor** of the branch tip (exit 0) →
clean fast-forward. No rebase, no force-push; the branch has **no
upstream** (`git status -sb` → `## refactor/step5-structure`). File this
verdict **on the branch** so committing it does not advance `main` and
break the fast-forward property (Step 7.1 lesson). The short `docs(qc)`
completion note (drafted at `../docs-qc-completion-note-draft.md`,
including the Step 4 §8 "11 of 14" correction) goes on `main` immediately
after the Custodian's advance, followed by the Verifier's merge-skew
`fmt` + `test` re-run on `main`.

## 8. Observations

1. **Step 4 §8 follow-ups — addressed in this bundle:** the probe-count
   prose correction ("13 of 14" → **"11 of 14"**) is recorded in
   `summary.md` and drafted into the completion note, per the Verifier's
   recommendation that the fix travel as a `docs(qc)` note rather than an
   edit to committed Step 4 evidence; the stale "not pushed" prose is
   noted (this bundle states its own push status factually); the rustc
   informational is covered by the env disclosure (Step 5 records
   rustc 1.99.0 / `stable-x86_64-pc-windows-msvc`, where Step 4 recorded
   1.99.1 / `x86_64-pc-windows-gnu`).
2. *(Placeholder — Verifier's own observations go here at sign-off.)*
