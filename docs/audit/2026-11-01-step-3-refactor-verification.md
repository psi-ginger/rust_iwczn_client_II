# Step 3 Structural Refactoring — Independent Verifier Audit

**Date:** 2026-11-01
**Verifier session (`psi-ginger`), QC lane**
**Branch under audit:** `refactor/step3-structure` @ `6bfbf7e`
**Merge-base with `main`:** `f5ec238` (= current `main` head → fast-forward is possible)
**Verdict:** `PASSED` — fast-forward merge authorized.

## 1. Scope of the delivery

Five commits, one per phase, on top of `main` @ `f5ec238`:

| Phase | Commit | Change |
|-------|--------|--------|
| 1 | `708dad8` | characterization tests first (T-4): G-1 / G-2 / G-3 close the coverage gaps |
| 2 | `7a13bdc` | hoist the `[0u8; 4096]` read scratch above the loop in `FrontLink::next_frame` (T-2) |
| 3 | `f9b60cc` | extract `flow()`'s front segment into `front_phase()` (T-1) |
| 4 | `56d197a` | consolidate duplicated test helpers into `src/auth/test_support.rs` (T-3) |
| 5 | `6bfbf7e` | evidence bundle (probes P-1…P-7, determinism, baseline/final gate logs) |

Files touched vs `main`: `src/auth/{front.rs,front/tests.rs,login/tests.rs,world_list/tests.rs,mod.rs,test_support.rs}`, `src/client/driver.rs`, `tests/e2e_loopback{.rs,/frames.rs,/front_double.rs}`, plus `evidence/step3-refactor/`. **`src/net/**` untouched; `Cargo.toml`/`Cargo.lock` untouched.**

## 2. S4 clean-clone audit (independent, from origin)

Clean clone at `6bfbf7e`, tree pristine before and after the run. All five mandatory gates exit-0:

| gate | exit | result |
|------|:----:|--------|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | — |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warnings |
| `cargo fmt --check` | 0 | 0 diff lines |
| `cargo test` | 0 | **327/327** = 300 lib + 11 + 16 |
| `cargo doc --no-deps` | 0 | 0 warnings |

5 tests ignored (1 `operator_game_window_opens_on_screen` P3-3 demo + 4 `live_integration`), identical to baseline — no gate test was added to or removed from the ignored set.

Test-count trajectory confirmed: baseline **324** (299 lib + 11 + 14) → final **327** (300 lib + 11 + 16) = +1 lib (G-3) + 2 e2e (G-1, G-2).

**Determinism:** three consecutive full `cargo test` runs on the restored tree, all exit 0 at 327.

## 3. Pure-motion verification of the `front_phase()` extraction (T-1)

Per S5 the extracted body was compared **byte-for-byte** against the pre-refactor `flow()` front segment (`driver.rs` at `f9b60cc~1`), not read off the diff:

- **49-line body byte-identical** → zero semantic drift.
- Checklist counts match exactly on both sides: 4 `check_shutdown` gates, 9 `map_err(at(stage))`, 3 `read_front`, 1 `drop(front)`, 1 `let _secret`, 3 `debug!`.

Only the signature and the split point differ (new return type `(FrontAuth, usize)`; `flow()` opens with `let (auth, worlds) = front_phase(cfg, shutdown).await?;`).

## 4. Adversarial probes — all seven independently re-executed

Each mutation applied to one committed file, only the named test run, exit **101** with that test in `FAILED`, no compile error, tree restored byte-clean before the next probe:

| Probe | Mutation | Named test FAILED | Exit |
|-------|----------|-------------------|:----:|
| P-1 | removed the `NotKeyed` gate in `FrontLink::send` | `send_before_the_handshake_is_refused_and_writes_nothing` | 101 |
| P-2 | `self.tick()` moved from `decode` entry to tail | `every_vector_matches_the_legacy` | 101 |
| P-3 | removed `self.failure = Some(err)` in `declared_at` | `the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks` | 101 |
| P-4 | `cipher: None` → `Some(link_cipher())` in `Decoder::new` | `a_plaintext_decoder_never_touches_its_bytes_or_keys` | 101 |
| P-5 | extra `cipher.decode` after the real decode (keys tick twice) | `cipher_keys_advance_exactly_once_per_pulled_frame` | 101 |
| P-6a | `parse_notice` refusal labelled `Stage::FrontLogin` | e2e `a_short_notice_response_is_refused_at_the_world_list_stage` (G-2) | 101 |
| P-6b | `login.capture()` refusal labelled `Stage::WorldList` | e2e `a_rejected_login_result_is_refused_at_the_front_login_stage` (G-1) | 101 |
| P-7 | `decoder.feed(&buf[..n])` → `feed(&buf[..])` | `handshake_reassembles_a_frame_split_across_reads` | 101 |

P-6a / P-6b failure messages reproduce the record verbatim (`expected a Stage{WorldList} refusal, got Stage { stage: FrontLogin, ... }` and the mirror).

**P-2 deviation note — verified honest.** The Implementer disclosed that the literal reading (tick moved to just after the size-guard block) ran **EXIT=0** and was escalated to a placement that makes tick timing observable. The Verifier reproduced the literal reading independently: it exits **0** against `every_vector_matches_the_legacy`, exactly as disclosed. The escalation was necessary and is transparently recorded, not silent.

## 5. Wire-level invariants and dual-source citations

**Disclosure — directive/reality mismatch:** the directive asks to verify "wire-level invariants (I-1 through I-11)", but **no `I-1`…`I-11` numbering exists anywhere in the tracked tree** (verified by `git grep`). The repo instead defines **three execution invariants** (`CONTRIBUTING.md` §Execution Invariants): (1) never jump steps, (2) never hide failures, (3) never stub wire parsers. Rather than fabricate eleven invariants, the Verifier verified the wire-level guarantees that actually exist, by probe and by full suite:

- **Wire layer untouched:** `git diff main...HEAD -- src/net` is empty — every `net/codec`, `net/cipher`, `net/session` invariant test is unmodified and still green (327/327).
- The seven probes (P-1…P-7) pin the load-bearing wire invariants: NotKeyed gate, tick-once-per-frame, sticky refusal, plaintext-decoder isolation, key advancement, stage attribution, partial-read reassembly.
- Execution invariant 3 (no stub / full payload validation) holds: no parser changed; refusals still pinned.

**Dual-source citations — net zero loss.** Aggregate `*.cpp` / `*.h` citation count across the six shared files: **88 → 88**.

- `FrontMsg.h:132-141` was **triplicated** across the front / world_list / login donors; it now lives **once** in `test_support.rs` (deduplication — the citation doc moved verbatim, front kept its own copy). This is the intended T-3 consolidation, not a loss.
- `front/tests.rs` **gained** `ITZNetIOCP.cpp:145-150` and `ITZRecvBuffer.cpp:163` — new citations introduced by the Phase-1 characterization tests.

## 6. F-07 integrity of the test-support consolidation (T-3)

- `SECRETKEY_FRAME` byte-identical to the pre-refactor donor: `0x0A,0x00,0x00,0x30,0x75,0x00,0x00,0xA7,0x00,0x5C`.
- `frame()` helper body byte-identical to the `world_list` donor.
- All three donors **import** from `test_support` (0 redefinitions) — no expectation re-derived from a parser under test.
- Net line delta for Phase 4: **−21** (70 added / 91 deleted), matching the summary exactly.

## 7. Evidence bundle integrity (SHA256SUMS, L-2)

Re-verified over the **committed bytes** (`git cat-file`), not the working tree: **17/17 entries match**. This matters because `core.autocrlf=true` makes the working tree CRLF while the committed blobs are LF — hashing the checked-out files would have failed. The bundle lists exactly the 17 files present (excluding `SHA256SUMS` itself); no file is missing from or absent to the manifest.

## 8. Observations (non-blocking)

- `summary.md` states the branch was "not pushed (no push directed)", yet the branch is present on `origin`. Stale prose in the summary; it does not affect any technical claim (all gates, probes and hashes were reproduced independently from `origin`).
- Evidence `env.txt` records `rustc 1.99.0`; the Step 7.2 bundle recorded `1.99.1`. Different toolchain patch across sessions — non-blocking, the gates are green on the Verifier's own run regardless.

## 9. Merge authorization

`main` @ `f5ec238` is an **ancestor** of `6bfbf7e`, so the merge is a **clean fast-forward** (`git merge-base --is-ancestor` exit 0). Fast-forward of `main` to the branch tip is authorized; the delivery SHA `6bfbf7e` is preserved (no rebase, no force-push — S7). This verdict is filed **on the branch** so that committing it does not advance `main` and break the fast-forward property (the Step 7.1 lesson). Per `CONTRIBUTING_PHASE2.md:256-259`, the short `docs(qc)` completion note quoting this evidence path (`evidence/step3-refactor/`) is committed on `main` immediately after the Custodian's advance.
