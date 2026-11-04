# Step 6 Structural Refactoring — Independent Verifier Audit

**Date:** 2026-11-04
**Verifier session (`psi-ginger`), QC lane**
**Branch under audit:** `refactor/step6-structure` @ `efb0d1a`
**Merge-base with `main`:** `0cd1788` (= current `main` head → fast-forward is possible)
**Verdict:** `PASSED` — fast-forward merge authorized, with two non-blocking observations (§9).

## 1. Scope of the delivery

Seven commits on top of `main` @ `0cd1788` (one WO + one per phase; linear, no merges):

| Phase | Commit | Change |
|-------|--------|--------|
| WO | `fd8e8d1` | `docs/agents/WO-step6-structure.md` (Option B, C3+C4 scope; Status line says `DRAFT` — see §9.1) |
| 0 | `ec0d5e9` | baseline evidence over `0cd1788` (5 gates, 336/336) + `bytes.rs` module-doc truth fix ("C1–C4 applied" → C1–C2 + Step 6 pointer for C3/C4) |
| 1 | `c7d9eb8` | characterization test `short_credentials_are_terminated_and_zero_filled_at_every_field` (ID `test2` / password `test`), **336 → 337**; +40 lines, `src/auth/login/tests.rs` only, 0 deletions |
| 2 | `6b2fde5` | T-1 / T-2 surveys **N/A**, evidence-only (boundary `git diff -- src tests` empty) |
| 3 | `b67caf1` | **C3 motion:** shared `copy_credential` + `CredentialTooLong` in `src/protocol/bytes.rs`; 5 call sites converted via explicit `.map_err`; error enums untouched |
| 4 | `5a6d523` | **C4 motion:** new private `src/game/gate.rs` (`GateViolation` + `gate_frame` core); `dispatcher`/`world_entry` become thin wrappers; call sites untouched |
| 5 | `efb0d1a` | evidence bundle: 4 adversary probes, 3× determinism, final 5 gates, blob-hash table, `summary.md` + unsigned audit-report draft |

Files touched vs `main` (102 total): **8 source files** — `src/protocol/bytes.rs`, `src/auth/login.rs`, `src/auth/login/tests.rs`, `src/game/login.rs`, `src/game/dispatcher.rs`, `src/game/world_entry.rs`, `src/game/gate.rs` (new), `src/game/mod.rs` — plus the WO and 93 evidence files. **`Cargo.toml`/`Cargo.lock`, `src/net/**`, all fixtures, the e2e `tests/` tree, `TASK_MATRIX.md`, and `docs/audit/` untouched** (§6).

## 2. S4 clean-clone audit (independent, from origin)

Fresh clone from `origin`, checked out at `efb0d1a`; porcelain empty before and after the whole battery. All five mandatory gates exit 0:

| Gate | Exit | Result |
|------|:----:|--------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | — |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warnings |
| `cargo fmt --check` | 0 | 0 diff lines |
| `cargo test` | 0 | **337/337** = 304 lib + 11 fixtures + 22 e2e |
| `cargo doc --no-deps` | 0 | 0 warnings |

**Determinism:** three consecutive full `cargo test` runs in the clone, all exit 0 at **337/337**, matching the record's Phase 5 trio (`determinism-1..3.exit`, all `EXIT=0`, result lines all 337).

**Named test:** `auth::login::tests::short_credentials_are_terminated_and_zero_filled_at_every_field ... ok` ✅

**Trajectory 336 → 337 confirmed.** The record's Phase 0 baseline (over `0cd1788`) and the WO baseline both state 336 (303 lib + 11 + 22); the tip runs 337 (304 lib + 11 + 22). The +1 is exactly Phase 1's single `#[test]` (numstat `40/0` in `src/auth/login/tests.rs`, the only test-count change in Step 6); Phases 2–5 hold the count locked, as the WO §3 requires ("the test count grows only in this phase").

**Ignored set unchanged: 5** = 1 lib operator-window demo (`operator_game_window_opens_on_screen`) + 4 `live_integration` (`live_event_loop_idles_cleanly_and_ticks`, `live_front_handshake_extracts_secret_key`, `live_game_handshake_decodes_the_link_cipher`, `live_heartbeat_keeps_the_connection_up_over_60s`). Gates never touch the network (F-01).

## 3. C3 verification — shared `copy_credential`

- **Body byte-identical to both pre-move donors**, modulo exactly the two disclosed token classes: the const name (donors `CREDENTIAL_LEN` in `src/auth/login.rs:167` and `PLAYER_NAME_LEN` in `src/game/login.rs:141`, **both `= 20`** on `0cd1788` → private `CREDENTIAL_FIELD_LEN = 20` in `bytes.rs`) and the error type (per-crate `CredentialTooLong` → shared struct). The StringCchCopy semantics (NUL cut, zero-fill, `len >= 20` refusal) are unchanged.
- **Citation carried with the body:** the donor doc's `Lib\commonutil.cpp:6-16` citation now lives in `src/protocol/bytes.rs` alongside the copier. Tree-wide `commonutil.cpp` references in `src/`: **9 → 10** (no loss).
- **5 explicit `.map_err` call sites**, exactly as claimed: `src/auth/login.rs:264,270,276` (`userid`, `userpw`, `macaddress`) and `src/game/login.rs:194,200` (`szID`, `szPassword`). Each converts the shared `CredentialTooLong` into the crate's existing public variant; the `#[error(...)]` display strings of both error enums are untouched.
- **Public API:** `copy_credential` and `CredentialTooLong` are `pub(crate)`; no new public exports; error enum variants unchanged.

## 4. C4 verification — shared `gate_frame` core

- **`src/game/gate.rs` core is the dispatcher donor body verbatim.** The one substantive difference is disclosed and sound: the donors' differing field labels (both assigned `expected_cmd`) are unified as `GateViolation::UnexpectedOpcode { expected, cmd }`; the opcode guard returns before any length logic, so no observable error path changes.
- **Wrapper signatures byte-identical pre/post:**
  - `dispatcher.rs:428` — `fn gate_frame(cmd: u16, expected_cmd: u16, expected_len: usize, payload: &[u8]) -> Result<(), DispatchError>`
  - `world_entry.rs:384` — same four parameters, `-> Result<(), WorldEntryError>`
  - both bodies are now one call: `super::gate::gate_frame(...).map_err(...)` into their own unchanged error enums.
- **Zero call-site hunks.** Phase 4's numstat is confined to `dispatcher.rs`, `world_entry.rs`, `gate.rs` (new), `mod.rs` (+1 `mod gate;`, private). No test or production call site was edited; the 5 `gate_frame(` occurrences in `dispatcher.rs` (definition + wrapper + call sites) are structurally unchanged.
- **Phase 0 claims verified:** `git diff 0cd1788..fd8e8d1 -- src tests Cargo.*` = **0 files** (no byte moved before the WO), and the `bytes.rs` module-doc truth fix (C1–C4 → C1–C2 + Step 6 pointer) is present as recorded.

## 5. Adversarial probes — all 4 independently re-executed

Each record-exact mutation (`P-0X.diff`) applied to one committed file in the Verifier's clone, only the named tests run, tree restored byte-clean before the next probe:

| Probe | Mutation (one file) | Named tests | Record | Verifier |
|-------|---------------------|-------------|:------:|:--------:|
| PR-1 | `bytes.rs` bound `len >= CREDENTIAL_FIELD_LEN` → `len >` | `credentials_that_do_not_fit_are_refused_not_truncated`, `a_credential_without_room_for_its_terminator_is_refused` | 101 | **101/101, both FAILED** ✅ |
| PR-2 | `bytes.rs` drop the NUL cut (`position(...)` → `raw.len()`) | `strings_stop_at_their_terminator_and_zero_fill_the_rest`, `login_request_payload_is_byte_exact_at_literal_offsets` | 101 | **101/101, both FAILED** ✅ |
| PR-3 | `gate.rs` invert the opcode check (`!=` → `==`) | `server_info_rejects_foreign_opcodes_and_wrong_lengths`, `village_struct_rejects_foreign_opcodes_and_wrong_lengths` | 101 | **101/101, both FAILED** ✅ |
| PR-4 | `gate.rs` swap `Truncated`/`TrailingBytes` arms | `a_char_update_length_boundary_is_refused_exactly`, `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | 101 | **101/0** — boundary test FAILED, chat test ok ✅ (exactly the disclosed split) |

- No compile errors in either the record's runs or the Verifier's; all eight named outcomes match the record.
- **Restores byte-clean after every probe:** full porcelain empty, `git diff --exit-code -- src` = 0, HEAD still `efb0d1a` at the end.
- **PR-4 disclosure evaluated and accepted.** The WO §5 matrix line expected both PR-4 tests to fail; only the boundary test does. Technical reason verified directly in the code: `parse_chat` **never routes through `gate_frame`** — it has its own inline declared-size gate at `dispatcher.rs:491-511` (`ChatDeclaredSize` range check, then inline `Truncated`/`TrailingBytes` against `declared`), so mutating the shared core cannot reach it. **No coverage loss:** the chat test continues to pin the inline gate, and the char-update test pins the shared core's length arms (it failed under the mutation). The disclosure in `phase5/README.md` is accurate and complete.

## 6. Write guard, F-07, citations

- **Write guard clean.** Full branch diff = the 8 source files of §1 + WO + 93 evidence files. No `src/net/**`, no fixtures (`*.txt` hits under the diff are evidence logs only), no `Cargo.*`, no `tests/` e2e tree, no `TASK_MATRIX.md`. `docs/audit/` has an **empty diff vs `main`** — the Implementer added only `docs/agents/WO-step6-structure.md`; this verdict is the first `docs/audit/` change on the branch.
- **F-07:** the Phase 1 test's expectations are hand-derived from cited legacy headers (`FrontMsg.h:344-347`, `Lib/commonutil.cpp:6-16`, `Client.cpp:7286`) and inline in the test — not re-derived from the parser under test. No fixture edits anywhere in the branch.
- **Citations net no-loss, +8.** Aggregate `*.cpp`/`*.h` references across the 8 touched files: **339 → 347**. The single per-file decrease is `src/game/login.rs` (−1): the moved donor doc line (`commonutil.cpp:6-16`), which relocated into `bytes.rs` — the same line counted in the tree-wide 9 → 10 above. No other file decreased.

## 7. Evidence-bundle integrity

- **Blob-hash table 8/8 OK** — all eight `git rev-parse` hashes in `phase5/blob-hashes.txt` match the committed bytes at **both** `5a6d523` (the Phase 4 boundary the table cites) and `efb0d1a` (the tip).
- **37 `.exit` files reconcile exactly:** 30 gate records (all `EXIT=0`) + 3 determinism runs (all `EXIT=0`) + 4 probes (all `EXIT=101`) — the only non-zero files are `P-01..P-04.exit`, matching the record.
- **Phase 1 test diff** re-read line-by-line: +40 lines, tests file only, byte-exact 60-byte framing expectation (`test2\0` at 0..6, `test\0` at 20..25, MAC at 40..52), terminators and zero-filled tails asserted — consistent with the WO's characterization mandate and the directive's named test.
- **Phase 2 N/A surveys** (`phase2/README.md`) read: T-1 cites the four `flow()` seams (`:597`/`:536`/`:629`/`:762`), T-2 the two `[0u8; 4096]` scratches; evidence-only, boundary diff empty — same precedent as Step 5 Phase 2.
- `audit-report-draft.md` remains unsigned (`Verdict: PENDING`); evidence logs are committed blobs, so git integrity covers them (same mechanism note as Steps 4/5).

## 8. Merge authorization

`main` @ `0cd1788` is an **ancestor** of `efb0d1a` (`git merge-base --is-ancestor` exit 0); the merge is a **clean fast-forward**. History is linear (7 commits, no merges). No rebase, no force-push (S7): the delivery tip `efb0d1a` is preserved as a parent of this verdict commit. This verdict is filed **on the branch**, so committing it does not advance `main` and break the fast-forward property (the Step 7.1 lesson). Per `CONTRIBUTING_PHASE2.md`, the short `docs(qc)` completion note quoting `evidence/step6-refactor/` goes on `main` immediately after the Custodian's advance, followed by the Verifier's merge-skew `fmt` + `test` re-run on `main`.

## 9. Observations (non-blocking)

1. **WO approval prose vs the in-repo record.** `summary.md` and `audit-report-draft.md` both state the WO is "(APPROVED by PsiGinger)", yet the committed WO at `fd8e8d1` carries `Status: DRAFT for Custodian alignment — definition-before-implementation` and no approval record exists anywhere on the branch (`git grep APPROVED` hits only those two prose lines). Nothing technical depends on it — the scope matches the audit directive — but the bundle asserts an approval the repository does not record. Recommend the completion note state the approval factually (same class as Step 5 §9.1's push-prose observation).
2. **Stale acceptance count in the WO.** WO §5 acceptance says "**336/336**" and §4 says "Phase 1 is N/A → the count may not change", but Phase 1 took the additive branch and grew the suite **336 → 337**. The growth is pre-authorized by WO §3 ("the additive test lands here — before any motion; the test count grows only in this phase"), the test is the one the audit directive names, it is additive-only (+40 lines, 0 production edits), and all gates hold at 337 — but the WO's own §4/§5 prose is now stale relative to execution. The `summary.md` and the audit directive both state 336 → 337 correctly; only the WO text lags.
