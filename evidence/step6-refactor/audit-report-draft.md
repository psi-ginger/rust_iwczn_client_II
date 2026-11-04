# DRAFT — Step 6 Structural Refactoring (C3 + C4) — Independent Verifier Audit

**Status:** DRAFT prepared by the Implementer for Verifier review.
**Date:** 2026-11-04
**Branch under audit:** `refactor/step6-structure` (tip = this bundle's commit;
see `summary.md` → Commits/Phases)
**Merge-base with `main`:** `0cd1788` (current `main` head, Step 5 merged)
**Work order:** `docs/agents/WO-step6-structure.md` (APPROVED by PsiGinger) @ `fd8e8d1`
**Verdict:** `PENDING`

## 1. Scope of the delivery

Option B lean findings sweep, C3 + C4 only, per WO §1:

- **C3:** one `copy_credential` in `src/protocol/bytes.rs`
  (`pub(crate)` + `CredentialTooLong` + private `CREDENTIAL_FIELD_LEN`);
  former local donors in `src/auth/login.rs` and `src/game/login.rs` deleted;
  5 call sites converted via explicit `.map_err` into the **unchanged** public
  error variants.
- **C4:** new private `src/game/gate.rs` (`GateViolation` + `gate_frame` core);
  `dispatcher::gate_frame` / `world_entry::gate_frame` are thin `.map_err`
  wrappers with byte-identical signatures; **zero call-site edits**.
- Phase 1 added exactly one characterization test
  (`short_credentials_are_terminated_and_zero_filled_at_every_field`, ID
  `test2` / password `test`).
- Phase 2 recorded T-1/T-2 as N/A surveys (evidence-only).

`Cargo.toml`/`Cargo.lock`, fixtures, `src/net/**`, and `TASK_MATRIX.md`
untouched (re-derive: `git diff --stat 0cd1788..<tip>`).

## 2. Gates and determinism (re-derive)

- 30 gate `.exit` files over 6 boundaries (baseline + phases 1–5), all
  `EXIT=0`, including `RUSTFLAGS=-D warnings cargo check --all-targets`,
  `clippy --all-targets -- -D warnings`, `fmt --check`, `cargo test`,
  `cargo doc --no-deps` (final doc log: 0 warning lines).
- 3× full `cargo test` on the final tree: **337/337** each run
  (304 lib + 11 fixtures + 22 e2e; 5 ignored = 1 lib + 4 `live_integration`).

## 3. Test counts (re-derive from `test result:` lines)

| Boundary | total |
|---|---|
| baseline (`ec0d5e9`) | **336** (303 lib + 11 + 22) |
| Phase 1 through Phase 5 (+ determinism ×3) | **337** (304 lib + 11 + 22) |

Trajectory 336 → 337, +1 exactly (Phase 1 only), ignored set unchanged at 5.

## 4. Probes, disclosures, deviations (re-execute)

- **PR-1** (copier bound `>=`→`>`): EXIT=101, both WO-named tests FAILED.
- **PR-2** (copier drops NUL cut): EXIT=101, both WO-named tests FAILED.
- **PR-3** (shared gate opcode check inverted): EXIT=101, both WO-named tests
  FAILED.
- **PR-4** (shared gate `Truncated`/`TrailingBytes` swapped): EXIT=101,
  `a_char_update_length_boundary_is_refused_exactly` FAILED;
  **disclosure:** the WO's second named target
  (`a_chat_body_that_disagrees_with_its_si_data_size_is_refused`) passes
  because chat's length contract is `parse_chat`'s inline declared-size gate
  (`dispatcher.rs:499/:506`), out of C4's two-function scope — a probe-matrix
  target error in the WO, not a coverage loss (write-up in `phase5/README.md`).
- Every restore byte-clean: `git status --porcelain -- src tests` empty and
  `git diff --exit-code` EXIT=0 per probe; final working tree clean at commit.
- Deviations from the WO: none in code scope; one disclosed probe-target
  discrepancy (PR-4, above).

## 5. Pure-motion check (re-derive)

- Shared copier body equals both donors modulo the disclosed tokens
  (`CREDENTIAL_FIELD_LEN` const name, `CredentialTooLong` error type);
  `Lib\commonutil.cpp:6-16` + the uninitialized-stack note travel with it
  (citations net no-loss; aggregate legacy `*.cpp`/`*.h` refs did not
  decrease in any touched file).
- Both `gate_frame` wrappers: signature lines byte-identical to their donors;
  the diff contains no hunk at any call site.
- Blob-hash table (`phase5/blob-hashes.txt`, generated @ `5a6d523`) matches
  committed bytes: `git rev-parse 5a6d523:<path>`.

## 6. Sign-off (Verifier)

- [ ] Clean clone @ delivery SHA: 5 gates exit 0, 337/337, 5 ignored unchanged
- [ ] Pure-motion check clean (copier/gate bodies, wrapper signatures, zero call-site hunks)
- [ ] Probes 4/4 exit 101; restores byte-clean; PR-4 disclosure accepted
- [ ] Citations net no-loss; blob hashes match; prose self-audit clean
- [ ] Verdict: `PENDING` → `PASSED` / `FAILED`

**Signature:** ______________________  **Date:** ____________
