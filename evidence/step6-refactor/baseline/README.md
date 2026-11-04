# Step 6 — Baseline (Phase 0)

Captured on branch `refactor/step6-structure` @ `fd8e8d1` (the approved
`WO-step6-structure.md` on top of `0cd1788`; `git diff 0cd1788..fd8e8d1 --
src tests Cargo.toml Cargo.lock` is empty, so the audited code is exactly
`main`'s post-Step-5 tree), before any Step 6 edits, laid out as the
Step 5 baseline (`../step5-refactor/baseline/`).

| File | Gate |
|------|------|
| `00-head.txt` | `git show -s --format=fuller HEAD` |
| `00-env.txt` | rustc / cargo / git versions, RFLAGS used for gate 1 |
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (empty log = no diff) |
| `04-cargo-test.exit/.log` | `cargo test` |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` |

Result: **all 5 gates exit 0, 336/336** (303 lib + 11 fixtures + 22 e2e;
5 ignored = 1 lib operator-window demo + 4 `live_integration`, unchanged).

Phase 0 also lands one comment-only correction in `src/protocol/bytes.rs`
(module doc: the "C1–C4 applied" claim → C1–C2 only, with the Step 6
pointer for C3/C4), per WO §1; it follows this capture and is re-verified
at the Phase 1 boundary.
