# Step 5 — Structural Refactor: execution record

Branch `refactor/step5-structure`, created from clean `main` @
`68d3ad9c3a48a01f31c5aae6bbe3e658fedabd74` (Phase 0 baseline). Source
work head: `df316a3`; this bundle is the Phase 5 commit on top of it.

Environment (`baseline/00-env.txt`): rustc 1.99.0 (b940084d7), cargo
1.99.0, toolchain `stable-x86_64-pc-windows-msvc`, git 2.55.0.windows.5,
`RUSTFLAGS=-D warnings` for gate 1 only. Gate exits are captured in pwsh
immediately after each native pipeline (`$LASTEXITCODE` → `EXIT=n`);
every probe `.exit` is cross-checked against the `test result: FAILED` /
`error: test failed` lines in its own log.

## Phases

| Phase | Commit | What moved | Evidence |
|---|---|---|---|
| 0 baseline | *(none — evidence only)* | 5 gates over `68d3ad9`, tree verified clean, 334/334 | `baseline/` |
| 1 characterization (T-4e) | `0f52ca2` | source-level `adopt_local` wiring test + behavioral blind-spot test — **+2 lib tests, 0 production edits** | `phase1/` |
| 2 T-2e | `5ef7dc4` | motion survey: **N/A** — every Step 5 loop is allocation-free or its accumulator already hoisted; zero production edits | `phase2/` |
| 3 T-1e | `7251cbc` | `in_world_phase` extracted verbatim from `game_phase` (52 insertions, 18 deletions; 40-line body SHA256 `307e06e5…bb7db0` **equal on both sides**) | `phase3/` |
| 4 T-3e | `d06bf63` (+ `df316a3` evidence-prose fix) | `src/client/test_support.rs` consolidates `local_listener` ×3; `HANDSHAKE_PAYLOAD`/`SECRETKEY_FRAME` import from the Step 4/Step 3 donors | `phase4/` |
| 5 verification | *(this bundle)* | 4 adversary probes (all 101), 3× determinism runs, all 5 gates | `phase5/` |

## Gates — every boundary, all exit 0

| Boundary | check | clippy `-D warnings` | fmt --check | test | doc |
|---|---|---|---|---|---|
| 0 baseline | 0 | 0 | 0 | 0 | 0 |
| 1 (`0f52ca2`) | 0 | 0 | 0 | 0 | 0 |
| 2 (`5ef7dc4`) | 0 | 0 | 0 | 0 | 0 |
| 3 (`7251cbc`) | 0 | 0 | 0 | 0 | 0 |
| 4 (`d06bf63`) | 0 | 0 | 0 | 0 | 0 |
| 5 final | 0 | 0 | 0 | 0 (×3 runs) | 0 |

**32 gate `.exit` files, all `EXIT=0`** (5 boundaries × 5 + Phase 5's
7: `04-cargo-test-run{1,2,3}` counted separately). The only `EXIT=101`
files are the Phase 1 mutation check (`phase1/06-gap-mutation-check.exit`)
and the four Phase 5 probes — by design.

## Test counts (copied verbatim from `test result:` lines)

| Boundary | lib | fixtures | e2e | executed | ignored |
|---|---|---|---|---|---|
| 0 baseline | 301 | 11 | 22 | **334** | 5 |
| 1 | 303 | 11 | 22 | **336** | 5 |
| 2 | 303 | 11 | 22 | **336** | 5 |
| 3 | 303 | 11 | 22 | **336** | 5 |
| 4 | 303 | 11 | 22 | **336** | 5 |
| 5 run 1 / 2 / 3 | 303 | 11 | 22 | **336 ×3** | 5 |

Ignored: 1 lib operator-window demo + 4 `live_integration` (F-01: no live
server in this environment), unchanged at every boundary. Phase 1 is the
only boundary that grows the suite (+2: the wiring test and its
behavioral counterpart). **Phases 2–5 hold the count at 336** — Phase 5
adds zero tests (probes + determinism only).

## Adversary mutation probes (Phase 5)

One probe applied to the working tree at a time, `cargo test <named
test>` run and captured, then `git checkout -- <file>` restored and
`git status --porcelain -- src tests` verified empty — after **every**
probe (`phase5/probes/P-0X-restore.txt`, which also records the
`HEAD:<file>` blob the file returned to). No fixture or expectation file
is ever edited.

| # | Mutation | Named test | Exit | Evidence |
|---|---|---|---|---|
| P-01 | delete `dispatcher.adopt_local(&char_client);` (`driver.rs:771`, inside `in_world_phase`) | `client::driver::tests::world_entry_adoption_is_wired_before_the_in_world_loop_starts` | 101 | `phase5/probes/P-01.*` |
| P-02 | `DriverError::refused(Stage::InWorld, …)` → `Stage::CharClient` in `in_world_phase`'s `LoopError::Handler` arm (`driver.rs:797`) | e2e `a_flipped_byte_in_a_chat_frame_is_refused_at_the_in_world_stage` | 101 | `phase5/probes/P-02.*` |
| P-03 | delete the `RESPONSE_CHAT` arm from `dispatch()` (`dispatcher.rs:872`) | `game::dispatcher::tests::a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | 101 | `phase5/probes/P-03.*` |
| P-04 | fall-through `other => Ok(DispatchOutcome::Unhandled { cmd: other })` → `other => Err(DispatchError::UnexpectedOpcode { … })` (`dispatcher.rs:881`) | `game::dispatcher::tests::a_foreign_opcode_is_unhandled_not_an_error` | 101 | `phase5/probes/P-04.*` |

**4 of 4 probe runs failed their named test at exit 101.** Every probe
compiled successfully before running (each log begins `Compiling` /
`Finished`), so no exit 101 is a build failure in disguise. Coverage by
moved invariant: **adopt_local wiring** (P-01 — the exact deletion the
Step 4 Verifier ran green at 334/334, now caught), **stage label of the
T-1e-extracted body** (P-02), **dispatcher routing + fall-through
boundaries** (P-03, P-04).

## Disclosures (honest findings)

### Phase 2 (T-2e) is an N/A survey, not a motion

No production code changed in Phase 2: the survey found no loop-scope
scratch to hoist (every Step 5 loop is allocation-free or its
accumulator was already hoisted outside). Warrant is the recorded survey
(`phase2/README.md`), not a probe — recorded so the absence of a Phase 2
mutation is not mistaken for an oversight.

### Phase 3: zero non-verbatim tokens

The `in_world_phase` body is byte-identical (40 lines, SHA256 equal on
both sides; 21/21 code-token checklist items equal; `00-block-identity.txt`).
Unlike Step 4's T-1g (which needed `&auth` → `auth`), no token changed:
the only edits outside the body are the new signature, two `use`
bindings and the `game_phase` delegation.

### Phase 4: deliberately NOT consolidated (surveyed, in `phase4/README.md`)

- `frame()` / `encode_link()` in `heartbeat` and `event_loop` tests —
  identical bodies but **different suite-local citation docs**; merging
  would lose an F-07 annotation. Out of the approved T-3e scope.
- `driver/tests.rs`'s `frame()` — deliberately isolated from the Step
  6.2 harness by its own documented decision (Step 4).
- `teardown`'s `plaintext_frame()`, and `auth/*` / `net/session`'s own
  `local_listener`s — different helpers / out of Step 5 scope.

### Two commits for Phase 4 (deviation from one-commit-per-phase)

`d06bf63` is the Phase 4 code+evidence commit; `df316a3` fixes a prose
slip in the phase4 README (line count "30" → "27"). Evidence-only, no
source change. Disclosed rather than squashed, per forward-only history.

### Wiring test's positional constraint

`world_entry_adoption_is_wired_before_the_in_world_loop_starts` uses the
**first** `DispatcherState::new(` and the **first** `run_with_shutdown(`
in `driver.rs`. Driver source must therefore never contain the literal
`run_with_shutdown(` *before* the adoption line — `in_world_phase`'s own
doc comment complies (it names the loop without that call form).

### Env record differs from Step 4's bundle (informational)

Step 5's baseline records rustc **1.99.0** / `stable-x86_64-pc-windows-msvc`;
Step 4's summary recorded 1.99.1 / `x86_64-pc-windows-gnu` (Step 3
recorded 1.99.0). Same class as Step 4 §8.3. Gates above are green under
the toolchain this bundle records.

## Step 4 verdict §8 follow-ups (addressed here)

1. **Probe-count prose — corrected.** Step 4's `summary.md` says "13 of
   14 probe runs failed their named test at exit 101"; the correct count
   is **11 of 14** (P-01…P-04, P-06…P-11 including P-08a/P-08b; the
   P-05 trio exited 0 as disclosed). The Step 4 *table* was always
   accurate — only that sentence miscounts. Per the Verifier's
   recommendation the fix goes in a follow-up `docs(qc)` note (drafted
   below), not by editing committed Step 4 evidence; no technical claim
   depends on it.
2. **Stale push prose — noted.** Step 4's summary says "the branch has
   not been pushed yet" while `refactor/step4-structure` is present on
   `origin` (Step 4 §8.2). This bundle states its own push status
   factually: `refactor/step5-structure` currently has **no upstream**
   (`git status -sb` → `## refactor/step5-structure`).
3. **rustc informational (Step 4 §8.3)** — see env disclosure above.

## Drafts ready for Verifier sign-off (`docs(qc)`)

- `docs-qc-completion-note-draft.md` — the short `docs(qc)` completion
  note quoting `evidence/step5-refactor/`, ready to go on `main`
  immediately after the Custodian's advance (`CONTRIBUTING_PHASE2.md:256-259`),
  including the §8 "11 of 14" correction.
- `audit-report-draft.md` — the Step 5 verification report in the Step 4
  verdict's structure, **unsigned** (`Verdict: PENDING`), ready for the
  Verifier to re-derive, sign and file at
  `docs/audit/2026-11-04-step-5-refactor-verification.md`.

Both are drafts inside the bundle: nothing in `docs/audit/` was created
or modified by the Implementer.

## Invariants held

- **F-07**: every expectation hand-placed with its legacy/server
  authority cited; no expected byte derived from a parser under test.
  Phase 4's redefinitions drop to zero (grep-verified: 1 `local_listener`
  in the client group, 1 `HANDSHAKE_PAYLOAD`, 1 `SECRETKEY_FRAME`
  tree-wide) and both removed literals are byte-identical to their donors
  (`True` / `True`).
- **F-01**: every address is loopback (`127.0.0.1:0`); no live server
  named.
- **No fixture edits**: the 4 probes mutate production source only and
  restore it; no test expectation or fixture byte was changed to make a
  probe bite (or to hide one).
- **Test count locked**: 336/336 from Phase 1 through Phase 5's three
  determinism runs.
- **One commit per phase, forward-only** (no rebase, no force-push; the
  branch has not been pushed), with the Phase 4 evidence-prose fix
  disclosed above.
- **Counts copied from `test result:` lines**, never hand-computed.
- Never pushed to `main`; all work on `refactor/step5-structure`.

## Committed-blob hashes (LF normalized — `git rev-parse HEAD:path`)

Base: `68d3ad9c3a48a01f31c5aae6bbe3e658fedabd74` (main, Phase 0)
Head of the source work: `df316a3` (Phase 4) — Phase 5 is evidence-only,
so the `src/`/`tests/` blobs below are identical at this bundle's commit.

| Path | Blob |
|---|---|
| `src/auth/mod.rs` | `20adbe350c10c7bc419038ef461d622537ed9205` |
| `src/client/driver.rs` | `ca6b571a1129991b2a50e5c228db1f7b7c4dfb64` |
| `src/client/driver/tests.rs` | `a58ced9ce41cf6a27f3ab6a4fb282b3ef0c8b40c` |
| `src/client/event_loop/tests.rs` | `23bef4a9aa3e027ebd705373db8c5ba8263180fc` |
| `src/client/heartbeat/tests.rs` | `dc74efae6cef767e09e00dc3bcf3e7bcc05e5558` |
| `src/client/mod.rs` | `87855b55040ff680ac225cc012677c355480e906` |
| `src/client/teardown/tests.rs` | `0ea99dfc394ac0f54383b6b49d08c55c0656fb1b` |
| `src/client/test_support.rs` | `1c60fdead6d52f33a9dedd51c2499b8813f2099a` |
| `src/game/dispatcher/tests.rs` | `37abcb7fb2bd5dff2cf9407e68ac3f5bb4c94ca4` |
| `src/game/mod.rs` | `2e2eacc80d52b383989642471b59118633398d90` |

10 files changed vs `main` (210 insertions, 55 deletions): test code and
module declarations only, plus the T-1e extraction in
`src/client/driver.rs`. `Cargo.toml`/`Cargo.lock` untouched.

Hashes are over committed blobs (git's LF-normalized content), never the
CRLF working-tree bytes — the Step 3 §7 lesson.

## Commits

```
df316a3 docs(step5): fix line count in phase4 evidence README (27, not 30)
d06bf63 refactor(step5): T-3e consolidate client test_support - local_listener x3 deduped, cross-group wire literals import from Step 3/4 donors (336/336 held)
7251cbc refactor(step5): T-1e extract in_world_phase from game_phase - byte-identical 40-line move, gates green, 336/336 held
5ef7dc4 evidence(step5): Phase 2 T-2e motion survey - N/A, no loop-scratch to hoist (336/336 held)
0f52ca2 test(step5): Phase 1 characterization - adopt_local wiring test closes the Step 4 blind spot
943679d evidence(step5): Phase 0 baseline @68d3ad9 - all 5 gates green, 334/334
```

(+ this Phase 5 bundle commit)

## Layout

```
evidence/step5-refactor/
  baseline/   Phase 0: head, env, 5 gates, README
  phase1/     Phase 1 gates + mutation check    phase4/  Phase 4 gates
  phase2/     Phase 2 gates (T-2e survey)       phase5/  final gates + 3 determinism runs
  phase3/     Phase 3 gates + verbatim proofs           + probes/P-0X.{log,exit,diff,txt}
  docs-qc-completion-note-draft.md   draft docs(qc) note for main
  audit-report-draft.md              draft Step 5 audit report (unsigned)
  summary.md                         this file
```
