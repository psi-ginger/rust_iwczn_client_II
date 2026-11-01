# Step 4 Structural Refactoring — Independent Verifier Audit

**Date:** 2026-11-02
**Verifier session (`psi-ginger`), QC lane**
**Branch under audit:** `refactor/step4-structure` @ `458acf0`
**Merge-base with `main`:** `e6ee8a9` (= current `main` head → fast-forward is possible)
**Verdict:** `PASSED` — fast-forward merge authorized, with two documentation corrections noted (§8, non-blocking).

## 1. Scope of the delivery

Five commits, one per phase, on top of `main` @ `e6ee8a9`:

| Phase | Commit | Change |
|-------|--------|--------|
| 1 | `383ba3b` | characterization tests first: G-1…G-7 + 3 additive mutation arms, no production code |
| 2 | `3175a2d` | hoist the `[0u8; 4096]` read scratch above the loop in `GameLink::next_frame` (T-2g) |
| 3 | `5857d5e` | extract `flow()`'s game leg into `game_phase()` (T-1g) |
| 4 | `e5643f7` | consolidate duplicated game-suite helpers into `src/game/test_support.rs` (T-3g) |
| 5 | `458acf0` | evidence bundle (probes, 3× determinism, baseline/phase gate logs) |

Files touched vs `main`: `src/client/driver{.rs,/tests.rs}`, `src/game/{entry.rs,mod.rs,test_support.rs}` + 3 test suites, `tests/e2e_loopback*` (4 files), plus `evidence/step4-refactor/`. **`src/net/**` and the Step 3 `src/auth/**` refactor untouched; `Cargo.toml`/`Cargo.lock` untouched.**

## 2. S4 clean-clone audit (independent, from origin)

Clean clone at `458acf0`, tree pristine before and after every probe. All five mandatory gates exit 0:

| Gate | Exit | Result |
|------|:----:|--------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | — |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warnings |
| `cargo fmt --check` | 0 | 0 diff lines |
| `cargo test` | 0 | **334/334** = 301 lib + 11 fixtures + 22 e2e |
| `cargo doc --no-deps` | 0 | 0 warnings |

**Determinism:** four consecutive full `cargo test` runs on the restored tree (three in one batch, one in the exit-code re-capture), all exit 0 at 334.

Ignored set unchanged and correctly scoped: 1 lib operator-window demo (P3-3) + 4 `live_integration` (F-01, no live server here) = 5, identical to baseline.

Test-count trajectory confirmed from the record's own `test result:` lines and re-verified on the Verifier's runs: baseline **327** (300 lib + 11 + 16, at `e6ee8a9`) → final **334** = +7 (G-1…G-7; lib +1, e2e +6). Phase 1 is the only boundary that grows the suite; Phases 2-4 hold it at 334 (motion-only) — consistent with the record.

## 3. Pure-motion verification of the T-1g extraction

Per S5 the extracted `game_phase` body was compared **byte-for-byte** against the pre-refactor `flow()` game segment (`driver.rs` @ `5857d5e~1`):

- **129-line segment vs 129-line segment**; the only differing line is the disclosed `request_game_login(…, &auth, …)` → `auth` (clippy `needless_borrow` under `-D warnings`). After normalizing that single disclosed token the two are **identical** — zero further semantic drift.
- Checklist counts match exactly on both sides: 7 `check_shutdown`, 8 `map_err`, 5 `debug!`, 1 `info!`, 1 `warn!`, 1 `dispatcher.adopt_local`, 34 `Stage::`, 1 `GameLink::connect`, 1 `read_roster`, 1 `is_in_world`.
- Commit numstat is **35 insertions / 2 deletions**, exactly as the summary claims; the deletions are the two signature/doc lines and the `&auth` token.

The T-2g hoist (Phase 2) matches the Step 3 T-2 precedent (`auth/front.rs:303-319`) structurally — same comment, scratch hoisted to one stack slot above the loop. Its known-neutrality is disclosed (no probe can fail it; its warrant is the verbatim precedent plus the split-frame reassembly test), and the Verifier concurs: the decoder accumulates across reads either way.

## 4. Adversarial probes — all 14 runs independently re-executed

Each mutation applied to one committed file in the Verifier's own clone, only the named test run, tree restored byte-clean before the next probe, no compile errors anywhere:

| Probe | Mutation | Named test | Record | Verifier |
|-------|----------|------------|:------:|:--------:|
| P-1 | `NotHandshaken` gate removed in `send` | `send_before_the_handshake_is_refused_and_writes_nothing` | 101 | **FAILED** ✅ |
| P-2 | outbound encoder keyed `link_cipher()` | `handshake_keys_the_outbound_channel_and_ciphers_every_frame_after_it` | 101 | **FAILED** ✅ |
| P-3 | decoder keyed `session_cipher(0x7E,0xAD)` at construct | `the_link_cipher_is_installed_before_the_first_read` | 101 | **FAILED** ✅ |
| P-4 | roster name read `80` → `84` | `char_list_fields_read_their_literal_offsets` | 101 | **FAILED** ✅ |
| P-5 | `uiX` mask `0x3FF` → `0x7FF` | `every_bitfield_mask_stops_at_its_declared_width` | **0 (neutral)** | **PASS** ✅ (neutral confirmed) |
| P-5b | (mutation held) full lib | — | 0 | **301 pass** ✅ |
| P-5c | (mutation held) all targets | — | 0 | **334 pass** ✅ |
| P-6 | NPC stride forced to 592 | `login_many_walks_a_mixed_batch_by_each_records_own_stride` | 101 | **FAILED** ✅ |
| P-7 | region gate inverted | `the_region_gate_drops_a_mismatched_banner_without_storing_it` | 101 | **FAILED** ✅ |
| P-8a | `my_char_unique` gate removed (lib) | `an_opcode_5_record_for_another_character_is_not_adopted` | 101 | **FAILED** ✅ |
| P-8b | (mutation held) e2e | `a_flipped_byte_in_the_opcode_5_record_is_refused_at_the_char_client_stage` | 101 | **FAILED** ✅ |
| P-9 | `is_in_world` ignores `village_struct_pending` | `entry_completes_only_when_banner_record_and_village_agree` | 101 | **FAILED** ✅ |
| P-10 | `Stage::LinkKey` → `GameLogin` (3 sites) | e2e `a_foreign_first_game_frame_is_refused_at_the_link_key_stage` | 101 | **FAILED** ✅ |
| P-11 | `read_roster` bound `CHAR_SLOTS` → `- 1` | e2e `a_full_session_conversation_is_byte_exact_and_tears_down_cleanly` | 101 | **FAILED** ✅ |

Every named-test failure matches the record; the record's probe **table** is accurate line for line.

**P-5 neutrality — verified honest and root-caused.** The Verifier re-ran all three recorded checks with the mutation compiled in: the named test passes, the full lib passes (301), and all targets pass (**334**, exit 0) — the mutation is invisible to the suite, exactly as disclosed. Root cause confirmed in the fixture: the `if reserved_bits_set` block (`char_info/tests.rs:141-149`) ORs 1s into `type_kind`, `unique_level` and `y_map_speed` but **never into `life_x`**, although its own comment claims "the 2 bits above uiX" are parked; bits 30-31 of the `uiLife|uiX` word stay 0, so the widened mask absorbs zeros. Per the step's no-fixture-edits invariant the Implementer correctly did **not** patch the fixture to make the probe bite; the follow-up is to park bits 30-31 of `life_x` in `make_char_payload` (recommendation, not a blocker).

**`adopt_local` blind spot — disclosure verified honest.** The record states deleting `dispatcher.adopt_local(&char_client)` (`driver.rs:725`) passes the whole suite. The Verifier reproduced it: with the call deleted the full suite runs **334/334, exit 0**. The blind spot is real, correctly disclosed, and protected only by T-1g's verbatim checklist (1 `adopt_local` moved — verified). Carry it forward as a follow-up test gap.

## 5. F-07, additive-only Phase 1, and citations

- **F-07 (T-3g):** `HANDSHAKE_PAYLOAD` (`0x2A,0x00,0x00,0x00,0x7E,0xAD,0x01,0x00`), `HANDSHAKE_KEY1` (`0x7E`), `HANDSHAKE_KEY2` (`0xAD`) byte-identical to the pre-refactor donors; `frame()` body byte-identical to the `world_entry` donor; all three donor suites **import** from `test_support` (0 redefinitions). No expectation re-derived from a parser under test.
- **Phase 1 additive-only:** 0 deleted `assert` lines across the whole commit, +33 added; the 33 deleted lines are import reorganization plus send-sequence lines that **moved** into the new `run_game_double_gated` seam (verified in place). The `SERVICE_AREA_ENGLISH` default is preserved on both branches — the gate parameterizes the mutation arm only.
- **Citations — net +10, zero lost.** Aggregate `*.cpp`/`*.h` references across the 11 shared files: **207 → 217**. Per-file, no file lost a citation; the gains (`Client-System.cpp:1682`, `Client-SystemNPC.cpp:880`, `itzbase.h:63-68`, …) come from the new characterization tests, each carrying its authority inline.

## 6. Evidence-bundle integrity

Step 4 replaces Step 3's `SHA256SUMS` with a **committed-blob hash table** in `summary.md` (12 source files, `git rev-parse HEAD:path` over git's LF-normalized committed bytes — the L-2 lesson applied). The Verifier re-derived all **12/12 blob hashes: exact match**. Phase 5 is evidence-only (code identical `e5643f7` ↔ `458acf0`), so the final gate logs are valid for the branch tip. All 32 boundary `.exit` files read `EXIT=0` (baseline + 4 phase boundaries + final, incl. 3 determinism runs); every probe `.exit` matches its recorded table row.

Note: the evidence *logs* themselves carry no separate hash manifest; they are however committed blobs, so git integrity covers them. Mechanism adequate — recorded as a design difference from Step 3, not a defect.

## 7. Merge authorization

`main` @ `e6ee8a9` is an **ancestor** of `458acf0` (`--is-ancestor` exit 0); the merge is a **clean fast-forward**. No rebase, no force-push (S7); the delivery SHA is preserved. This verdict is filed **on the branch** so that committing it does not advance `main` and break the fast-forward property (the Step 7.1 lesson). Per `CONTRIBUTING_PHASE2.md:256-259` the short `docs(qc)` completion note quoting `evidence/step4-refactor/` goes on `main` immediately after the Custodian's advance, followed by the Verifier's merge-skew `fmt` + `test` re-run on `main`.

## 8. Observations (non-blocking — two documentation corrections requested)

1. **Probe-count prose is wrong.** `summary.md` states "13 of 14 probe runs failed their named test at exit 101". The evidence shows **11 of 14** runs at exit 101 (P-01…P-04, P-06…P-11 incl. P-08a/P-08b); the P-05 trio exited 0 as disclosed. The probe **table** is accurate — only this sentence miscounts. Recommend correcting the prose in a follow-up `docs(qc)` note; no technical claim depends on it.
2. **Stale push prose.** `summary.md` says "the branch has not been pushed yet", yet `refactor/step4-structure` is present on `origin`. Same stale-prose class as Step 3's summary; harmless.
3. *(Informational)* Evidence env records rustc **1.99.1** — consistent with the Step 7.2 bundle (Step 3's recorded 1.99.0).
