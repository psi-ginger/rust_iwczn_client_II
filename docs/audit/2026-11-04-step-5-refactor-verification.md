# Step 5 Structural Refactoring — Independent Verifier Audit

**Date:** 2026-11-04
**Verifier session (`psi-ginger`), QC lane**
**Branch under audit:** `refactor/step5-structure` @ `ec3822e`
**Merge-base with `main`:** `68d3ad9` (= current `main` head → fast-forward is possible)
**Verdict:** `PASSED` — fast-forward merge authorized, with three non-blocking observations (§9).

## 1. Scope of the delivery

Eight commits on top of `main` @ `68d3ad9` (one per phase; two disclosed prose-only fixes):

| Phase | Commit | Change |
|-------|--------|--------|
| 0 | `943679d` | baseline evidence only: 5 gates over `68d3ad9`, tree clean, 334/334 |
| 1 | `0f52ca2` | characterization: `adopt_local` wiring test + behavioral blind-spot test (+2 lib tests, **0 production edits**) |
| 2 | `5ef7dc4` | T-2e motion survey: **N/A** — no loop-scope scratch to hoist (evidence-only) |
| 3 | `7251cbc` | T-1e: extract `game_phase`'s in-world segment into `in_world_phase` (52 ins / 18 del; 40-line body moved verbatim) |
| 4 | `d06bf63` (+ `df316a3`) | T-3e: `src/client/test_support.rs` — `local_listener` ×3 deduped to 1; cross-group wire literals imported from the Step 3/4 donors; `df316a3` = phase4 README line-count prose fix only |
| 5 | `65b4ee8` (+ `ec3822e`) | evidence bundle: 4 probes, 3× determinism, summary + drafts; `ec3822e` = summary subject-line prose fix only |

Files touched vs `main` (10 source files): `src/auth/mod.rs`, `src/client/{driver.rs, driver/tests.rs, event_loop/tests.rs, heartbeat/tests.rs, mod.rs, teardown/tests.rs, test_support.rs (new)}`, `src/game/{dispatcher/tests.rs, mod.rs}`, plus `evidence/step5-refactor/` (99 files). **`Cargo.toml`/`Cargo.lock` untouched; `src/game/dispatcher.rs` production code, `src/net/**`, all fixtures and all Step 3/4 production files untouched.**

## 2. S4 clean-clone audit (independent, from origin)

Clean clone at `ec3822e`, tree pristine before and after (porcelain empty both times). All five mandatory gates exit 0:

| Gate | Exit | Result |
|------|:----:|--------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | — |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warnings |
| `cargo fmt --check` | 0 | 0 diff lines |
| `cargo test` | 0 | **336/336** = 303 lib + 11 fixtures + 22 e2e |
| `cargo doc --no-deps` | 0 | 0 warnings |

**Determinism:** three consecutive full `cargo test` runs on the restored tree, all exit 0 at **336/336**, matching the record's Phase 5 trio.

Ignored set unchanged and correctly scoped: 5 = 1 lib operator-window demo (P3-3) + 4 `live_integration`. The gates never touch the network (F-01); the live suite is run separately in §3 as evidence only.

Test-count trajectory: baseline **334** (at `68d3ad9`) → **336** = +2, both from Phase 1; Phases 2–5 hold the count locked. Phase 1 is the only boundary that grows the suite — consistent with the record.

## 3. Live integration over the ngrok tunnels — 4/4 PASS

**Endpoint-mapping disclosure:** the audit directive's tunnel labels are swapped relative to the deployed services. Empirically, `0.tcp.ap.ngrok.io:18646` pushes a plaintext 10-byte cmd-30000 front `SECRETKEY` frame (repo default FRONT = 5700) and `0.tcp.ap.ngrok.io:14195` pushes a 13-byte ciphered game handshake (repo default GAME = 4010), so the run used `GOONZU_LIVE_FRONT=0.tcp.ap.ngrok.io:18646` and `GOONZU_LIVE_GAME=0.tcp.ap.ngrok.io:14195`. There are no local listeners on 4010/5700 and no ngrok agent on this host; both tunnels carried live remote services (each pushed its unsolicited handshake within 60 ms of connect).

`cargo test --test live_integration -- --ignored --nocapture` → **exit 0, 4/4**:

| Test | Outcome |
|------|---------|
| `live_front_handshake_extracts_secret_key` | ok — `session_seed=(0x1c, 0x39)` |
| `live_game_handshake_decodes_the_link_cipher` | ok — `seed=(0xe7, 0x67)`, `char_unique=6`, server version 1 → the link cipher `0x12/0x34` matches the **deployed** service (the non-circular assumption this suite exists for) |
| `live_event_loop_idles_cleanly_and_ticks` | ok — 3 ticks over the WAN, 0 frames, `char_unique=7` |
| `live_heartbeat_keeps_the_connection_up_over_60s` | ok — 7/7 heartbeats across 70.01 s, connection never dropped |

## 4. Pure-motion verification of the T-1e extraction

Per S5 the extracted `in_world_phase` body was compared byte-for-byte against the pre-move segment (`driver.rs` @ `7251cbc~1`, the 40 lines the record identifies):

- **40 lines vs 40 lines, byte-identical.** The Verifier's own SHA256 of both sides is equal; the record's digest `307e06e5…bb7db0` is reproduced exactly when framed as LF-joined lines plus trailing newline (a plain join without the trailing newline gives `99d42d4e…` on **both** sides — a framing difference only; both sides equal either way).
- Checklist counts match exactly on both sides: 1 `DispatcherState::new`, 1 `adopt_local`, 1 `run_with_shutdown`, 1 `heartbeat_ticker`, 1 `is_heartbeat`, 1 `send_heartbeat`, 1 `dispatcher.dispatch`, 2 `debug!`, 1 `info!`, 1 `LoopError::Link`, 1 `LoopError::Handler`, 2 `Flow::Continue`, 4 `Stage::InWorld`.
- Commit numstat is **52 insertions / 18 deletions** in `src/client/driver.rs`; the diff shows only the new function signature, two `use` bindings (`ClientCharUnique`, `CharClient`), doc lines and the delegation call — no statement-level change.

Phase 2 (T-2e) is an N/A survey, not a motion: `phase2/README.md` surveys the four Step 5 files at `0f52ca2` and justifies zero production edits; the commit is evidence-only (numstat verified).

## 5. Adversarial probes — all 4 runs independently re-executed

Each mutation applied to one committed file in the Verifier's clone at `ec3822e`, only the named test run, tree restored byte-clean (`git status --porcelain -- src` empty) before the next probe:

| Probe | Mutation | Named test | Record | Verifier |
|-------|----------|------------|:------:|:--------:|
| P-01 | delete `dispatcher.adopt_local(&char_client);` (`driver.rs:771`) | lib `world_entry_adoption_is_wired_before_the_in_world_loop_starts` | 101 | **101, FAILED** ✅ |
| P-02 | `Stage::InWorld` → `Stage::CharClient` in `in_world_phase`'s Handler arm (`driver.rs:797`) | e2e `a_flipped_byte_in_a_chat_frame_is_refused_at_the_in_world_stage` | 101 | **101, FAILED** ✅ |
| P-03 | delete the `RESPONSE_CHAT` arm of `dispatch()` (`dispatcher.rs:872`) | lib `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | 101 | **101, FAILED** ✅ |
| P-04 | fall-through `other` arm → `Err(DispatchError::UnexpectedOpcode { expected: RESPONSE_CHAT, cmd: other })` (`dispatcher.rs:881`) | lib `a_foreign_opcode_is_unhandled_not_an_error` | 101 | **101, FAILED** ✅ |

- No compile errors in the record's runs, and none in the Verifier's final runs. (Tooling note: the Verifier's *first* P-04 attempt omitted the `expected` field and failed to compile with `E0063`; re-running with the record's exact mutation from `P-04-mutation.diff` reproduced exit 101 — the record's mutation is accurate; the miss was the Verifier's.)
- **P-01 closes the Step 4 §7 `adopt_local` blind spot.** Phase 1's wiring test fails exactly when the call is deleted, and `phase1/06-gap-mutation-check.log` shows that same named test FAILED with `assertion left == right failed: world entry adopts the local avatar exactly once` — the log matches P-01's outcome.
- P-02 specifically re-probes the **moved** body (the stage label of the extracted `in_world_phase`), i.e. the mutation bites after Phases 3–4 relocated the code.

## 6. F-07, additive-only Phase 1, and citations

- **F-07 donors byte-identical.** `HANDSHAKE_PAYLOAD` (`0x2A,0x00,0x00,0x00,0x7E,0xAD,0x01,0x00`), `HANDSHAKE_KEY1/2` (`0x7E`/`0xAD`) in `src/game/test_support.rs` and `SECRETKEY_FRAME` (`0x0A,0x00,0x00,0x30,0x75,0x00,0x00,0xA7,0x00,0x5C`) in `src/auth/test_support.rs` are byte-identical between `main` and the branch (definition lines compared verbatim). The client suites now **import** both literals from those donors — zero redefinitions tree-wide; `local_listener` has exactly **1** definition in the client group (`src/client/test_support.rs:23`; on `main` it was 3).
- **F-01:** the new `test_support.rs` binds `127.0.0.1:0` only, so no suite importing it can name a non-loopback address.
- **Phase 1 additive-only:** `0f52ca2` touches exactly `src/client/driver/tests.rs` (+61) and `src/game/dispatcher/tests.rs` (+49), **0 deletions, 0 production files**; +2 lib tests = `world_entry_adoption_is_wired_before_the_in_world_loop_starts` and `a_self_char_update_before_adoption_reports_the_same_outcome_and_drops_the_write`.
- **Citations — +1, none lost.** Aggregate `*.cpp`/`*.h` references across the 10 changed files: **48 → 49**; the single gain is `FrontMsg.h:132` (the donor citation carried with the import in `driver/tests.rs`); no file decreased. The legacy `*.cpp`/`*.h` sources themselves are untouched (zero numstat; 4111 files on both sides).

## 7. Evidence-bundle integrity

- **Blob-hash table 10/10:** all ten `git rev-parse ec3822e:path` hashes match `summary.md` exactly (committed bytes, L-2).
- **37 `.exit` files:** 32 read `EXIT=0`; the only non-zero are the five intentional ones — `phase1/06-gap-mutation-check.exit` (101) and probes `P-01..P-04.exit` (101) — matching the record exactly.
- Phase 5 (`65b4ee8`) is evidence-only (no `src/`/`tests/` in numstat), so the final gate logs are valid for the branch tip; `df316a3` and `ec3822e` each touch one evidence prose line only (numstat verified).
- **`docs/audit/` untouched by the Implementer** (empty diff vs `main`); `audit-report-draft.md` remains unsigned (`Verdict: PENDING`), as does the completion-note draft. Both drafts live inside the bundle, so committing this verdict is the first `docs/audit/` change on the branch.
- Evidence logs are committed blobs, so git integrity covers them (same mechanism note as Step 4 §6).

## 8. Merge authorization

`main` @ `68d3ad9` is an **ancestor** of `ec3822e` (`--is-ancestor` exit 0); the merge is a **clean fast-forward**. No rebase, no force-push (S7): the delivery tip `ec3822e` is preserved as a parent of this verdict commit. This verdict is filed **on the branch** so committing it does not advance `main` and break the fast-forward property (the Step 7.1 lesson). Per `CONTRIBUTING_PHASE2.md:256-259` the short `docs(qc)` completion note quoting `evidence/step5-refactor/` (drafted at `evidence/step5-refactor/docs-qc-completion-note-draft.md`, including the Step 4 §8 "11 of 14" correction) goes on `main` immediately after the Custodian's advance, followed by the Verifier's merge-skew `fmt` + `test` re-run on `main`.

## 9. Observations (non-blocking)

1. **Stale push prose — repeated class.** `summary.md` states the bundle "states its own push status factually: `refactor/step5-structure` currently has no upstream" and "the branch has not been pushed", yet the branch is present on `origin` at `ec3822e` (the Verifier fetched it from there). The bundle correctly flags Step 4's identical issue two paragraphs earlier, then repeats it. Same stale-prose class as Steps 3/4; no technical claim depends on it. Recommend the completion note state push status factually.
2. **Directive tunnel labels swapped (environment note).** The audit directive maps Auth/Front → 14195 and Game/World → 18646; the deployed services are the reverse (§3). The run used the empirical mapping, proven by each service's own handshake. Nothing in the branch depends on either label.
3. *(Informational)* The implementer's completion-note draft predates the live run and does not mention it; the Custodian may wish to add one sentence — the live 4/4 record is §3 above.
