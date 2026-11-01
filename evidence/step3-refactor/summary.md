# Step 3 Refactoring — Execution Summary (Phases 0–5)

Branch: `refactor/step3-structure` from `main` @ `f5ec238`, one commit per
phase, all work under `goonzu-rs` identity, **not pushed** (no push directed).

## Phase 0 — baseline (no commit; evidence only)

Captured at `f5ec238` into `baseline/`: head, env (rustc/cargo/toolchain) and
the five gate logs — check / clippy `-D warnings` / fmt / test / doc all
exit 0, **324/324** (299 lib + 11 + 14; 5 ignored).

## Phase 1 — characterization tests first (T-4) — commit `708dad8`

- `Mutation::ShortNotice` / `Mutation::LoginResult` added to
  `tests/e2e_loopback/frames.rs` (citations kept); front double arms for the
  notice (15 of 16 bytes) and login (`usResult` 2) steps.
- **G-1** `a_rejected_login_result_is_refused_at_the_front_login_stage` —
  `DriverError::Stage{stage: FrontLogin}` with source `LoginError::Rejected
  { result: 2 }`, frames 3/4, `closed_cleanly`, transcript front-leg only.
- **G-2** `a_short_notice_response_is_refused_at_the_world_list_stage` —
  `Stage::WorldList` with source `WorldListError::UnexpectedNoticeLength
  { len: 15 }`, `frames_sent == 2`, `ended_by_peer`.
- **G-3** `a_malformed_frame_after_the_handshake_refuses_and_sticks` —
  sleep-free (server syncs on the client's keyed frame), usSize 0 refuses as
  `Header(SizeBelowMinimum{0})` and the sticky refusal re-returns identically
  after the peer's EOF.

Gates: all 0 → **327/327** (300 lib + 11 + 16).

## Phase 2 — scratch hoist (T-2) — commit `7a13bdc`

`[0u8; 4096]` moved above the loop in `FrontLink::next_frame`
(`src/auth/front.rs`); one stack slot reused per iteration, QC P-12 doc stays
attached to the buffer, behavior unchanged. Gates: all 0, 327/327.

## Phase 3 — `front_phase` extraction (T-1) — commit `f9b60cc`

`flow()`'s front segment (connect → handshake → notice → world list → front
login, previously `driver.rs:529-577`) moved **verbatim** into
`front_phase(cfg, shutdown) -> Result<(FrontAuth, usize), DriverError>`;
`flow()` opens with `let (auth, worlds) = front_phase(cfg, shutdown).await?;`.
Checklist verified on the diff (pure motion): 4 `check_shutdown` gates, 9
`map_err(at(stage))`, 3 `read_front`, `drop(front)` before the final gate,
`let _secret`, every `debug!` and citation unchanged. Gates: all 0, 327/327.

## Phase 4 — test helper consolidation (T-3) — commit `56d197a`

`src/auth/test_support.rs` (`#[cfg(test)] mod` in `auth/mod.rs`) owns
`SECRETKEY_FRAME`/`SEED_KEY1`/`SEED_KEY2` (was triplicated across front /
world_list / login suites) and `frame()`/`read_ciphered_frame()` (was
duplicated across world_list / login) — byte-identical, citation docs moved
verbatim, no expectation re-derived (F-07 intact). Donors import from
`test_support`; net −21 lines. Gates: all 0, 327/327.

## Phase 5 — probes, determinism, evidence — this bundle

- **P-1…P-7 (incl. P-6a/P-6b):** all seven mutations failed their named test
  at **exit 101**, restored byte-clean each time — see `mutation-probes.md`
  for sites, verbatim failure output and the P-2 two-step note.
- **Determinism:** three consecutive full `cargo test` runs on the restored
  tree, all exit 0 at 327 — `final/determinism.log`.
- **Final gates:** check / clippy / fmt / test / doc all exit 0 on the final
  tree — `final/{check,clippy,fmt,test,doc}.log`, `final/head.txt`,
  `final/env.txt`.

## Invariants held throughout

- All 5 gates green at every phase boundary.
- Wire-exactness: no fixture edits (e2e `frames.rs` constants are assertions
  and were only *added to* with citations), F-07 expectations hand-derived,
  F-01 loopback-only addresses, no citation/doc loss (T-3 docs verbatim).
- Test-count trajectory: baseline 324 → final **327** (299→300 lib,
  14→16 e2e, 11 doc/integration unchanged; 5 ignored preserved).
