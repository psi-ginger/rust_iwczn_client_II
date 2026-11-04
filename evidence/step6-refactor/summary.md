# Step 6 — Structural Refactor (Lean Findings Sweep: C3 + C4): execution record

**Branch:** `refactor/step6-structure`, cut from `main` @ `0cd1788`
(merge-base with `main` = `0cd1788`).
**WO:** `docs/agents/WO-step6-structure.md` (APPROVED by PsiGinger) @ `fd8e8d1`.
**Scope:** Option B — C3 (`copy_credential` consolidation) + C4
(`gate_frame` core sharing) only; S1, G1, Option A, P-5, 7.3 deferred/excluded
with reasons in WO §1.

## Phases

| Phase | Commit | Content |
|---|---|---|
| 0 | `ec0d5e9` | Baseline evidence over the audited code (`git diff 0cd1788..fd8e8d1 -- src tests Cargo.*` empty): 5 gates, **336/336**; `bytes.rs` module-doc truth correction ("C1–C4 applied" → C1–C2 + Step 6 pointer for C3/C4) |
| 1 | `c7d9eb8` | Characterization: `short_credentials_are_terminated_and_zero_filled_at_every_field` — ID **`test2`**, password **`test`**, byte-exact 60-byte framing + terminators + zero-filled tails; **336 → 337**; 5 gates green |
| 2 | `6b2fde5` | T-1 **N/A** (loop structure complete: `flow()` `:597` dispatching `front_phase` `:536` / `game_phase` `:629` / `in_world_phase` `:762`) and T-2 **N/A** (both `[0u8; 4096]` scratches already hoisted: `auth/front.rs:306`, `game/entry.rs:500`) — evidence-only, `git diff -- src tests` empty |
| 3 | `b67caf1` | **C3**: shared `copy_credential` + `CredentialTooLong` + private `CREDENTIAL_FIELD_LEN` in `src/protocol/bytes.rs`; local donors deleted in `auth/login.rs` and `game/login.rs`; 5 call sites converted with explicit `.map_err`; error enums untouched; 5 gates green at **337** |
| 4 | `5a6d523` | **C4**: new private `src/game/gate.rs` (`GateViolation` + `gate_frame` core); `mod gate;` in `game/mod.rs`; `dispatcher`/`world_entry` keep thin `.map_err` wrappers with byte-identical signatures; **zero call-site hunks**; 5 gates green at **337** |
| 5 | (this bundle) | 4 adversary probes (4/4 EXIT=101, byte-clean restores), 3× determinism at 337, final 5 gates, blob-hash table, this summary + unsigned audit draft |

## Gates — every boundary, all exit 0

30 gate `.exit` files across 6 boundaries (baseline, phases 1–5), **all
EXIT=0**: `RUSTFLAGS=-D warnings cargo check --all-targets`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (0-byte logs),
`cargo test`, `cargo doc --no-deps` (0 warning lines at final).

## Test counts (copied verbatim from `test result:` lines)

| Boundary | lib | fixtures | e2e | total | ignored |
|---|---|---|---|---|---|
| baseline (`ec0d5e9`) | 303 | 11 | 22 | **336** | 5 (1 lib + 4 live_integration) |
| Phase 1 → 5 (×5 boundaries + 3 determinism runs) | 304 | 11 | 22 | **337** | 5 (unchanged) |

Trajectory: **336 → 337** (Phase 1's +1 only), held flat through Phases 2–5.

## Adversary mutation probes (Phase 5)

| # | Mutation | Named targets | Exit |
|---|---|---|---|
| PR-1 | copier bound `>= 20` → `> 20` | 2/2 FAILED | **101** |
| PR-2 | copier drops the NUL cut | 2/2 FAILED | **101** |
| PR-3 | shared gate opcode check inverted | 2/2 FAILED | **101** |
| PR-4 | shared gate `Truncated`/`TrailingBytes` swapped | 1/2 FAILED (see disclosure) | **101** |

All four restores byte-clean (`git status --porcelain -- src tests` empty,
`git diff --exit-code` EXIT=0 per probe).

## Disclosures (honest findings)

### PR-4's WO-named chat target passes (probe-matrix target error)
`a_chat_body_that_disagrees_with_its_si_data_size_is_refused` gates through
`parse_chat`'s own inline declared-size contract (`dispatcher.rs:499/:506`),
which C4's scope (exactly the two `gate_frame` functions) never touched — so it
correctly still passes under PR-4. Both shared arms are proven by the failing
target `a_char_update_length_boundary_is_refused_exactly` (asserts `Truncated`
at len−1 and `TrailingBytes` at len+1). Full write-up:
`phase5/README.md`.

### `opcode` field source unified in the shared gate
Dispatcher's donor filled `opcode` with `expected_cmd`; world_entry's with
`cmd`. The shared core uses `expected_cmd` — equal to `cmd` on every path past
the opcode guard (disclosed in WO §2 and `phase4/README.md`).

### Phase 0 doc-truth correction
`src/protocol/bytes.rs` claimed "C1–C4 applied under Step 7.1" while only
D1+C1+C2 had landed; corrected at Phase 0, widened when C3 landed at Phase 3
("C3 and C4 close the set under Step 6" — now true).

### Citations (net no-loss)
`Lib\commonutil.cpp:6-16` and auth's uninitialized-stack note moved with the
copier into `bytes.rs`; both caller module docs keep their own refs; Phase 4
removed no legacy `*.cpp`/`*.h` citation (neither donor wrapper carried one).

## Invariants held

- Write guard respected: only `protocol/bytes.rs`, `auth/login.rs`,
  `game/{login,dispatcher,world_entry,mod,gate}.rs`, `auth/login/tests.rs`,
  `evidence/step6-refactor/`, `docs/agents/WO-step6-structure.md` (pre-approval).
  No `src/net/**`, fixtures, `Cargo.*`, or `TASK_MATRIX.md` changes.
- Public API: error enums unchanged; `gate` module private;
  `CredentialTooLong`/`copy_credential` are `pub(crate)` — no new exports.
- F-07: no fixture edited; no expectation re-derived from a parser under test.
- One commit per phase (5/5).

## Committed-blob hashes (LF-normalized — `git rev-parse HEAD:path` @ `5a6d523`)

See `phase5/blob-hashes.txt` (8 write-guard src files).

## Layout

```
evidence/step6-refactor/
  baseline/          Phase 0 (README + head/env + 5 gates)  commit ec0d5e9
  phase1/            characterization test evidence         commit c7d9eb8
  phase2/            T-1/T-2 N/A surveys                    commit 6b2fde5
  phase3/            C3 motion evidence                     commit b67caf1
  phase4/            C4 motion evidence                     commit 5a6d523
  phase5/            probes, determinism, final gates, blob hashes
  summary.md         this record
  audit-report-draft.md   unsigned Verifier draft (verdict PENDING)
```
