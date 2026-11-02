# Step 5 — Baseline (Phase 0)

Captured on branch `refactor/step5-structure` @ `68d3ad9` (== `origin/main`),
before any Step 5 edits, exactly as the Step 4 baseline (`../step4-refactor/baseline/`)
is laid out.

| File | Gate |
|------|------|
| `00-head.txt` | `git show -s --format=fuller HEAD` |
| `00-env.txt` | rustc / cargo / active toolchain / RUSTFLAGS used for gate 1 |
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (empty log = no diff) |
| `04-cargo-test.exit/.log` | `cargo test` |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` |

Result: **all 5 gates exit 0, 334/334** (301 lib + 11 fixtures + 22 e2e;
5 ignored = 1 lib operator-window demo + 4 `live_integration`, unchanged).
