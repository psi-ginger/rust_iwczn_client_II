# Step 7.3 baseline verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-05 (UTC) - exact stamp in `head.txt`.
- **Clone:** S4 clean clone at `origin/main` = `2436a547ca8bd994f899edb3022176b786513784` (tree `0e3e8fa79890fad9e78489456ce5f5ea6fd6e0f0`, byte-identical to the verified delivery `eb54f55` - the Step 7 completion note is an empty commit over it, so these gates apply to the accepted baseline).
- **Baseline:** 337 -> **352**, accepted by the custodian completion note on `main` (`2436a54`).
- **Environment:** `env.txt` (rustc / cargo / git versions, OS, `core.autocrlf=true`).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `cargo check --all-targets` (`RUSTFLAGS=-D warnings`) | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | test | `cargo test` | 0 | `test.log` - **352 passed, 0 failed, 5 ignored** |
| 5 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |

**Test composition (from `test.log`, not prose):** 319 lib (1 ignored) + 11 `asset_fixtures` + 22 `e2e_loopback` = **352 passed**; `live_integration` 4 ignored; 0 failed. Ignored total **5** (1 operator demo + 4 live) - the preserved baseline. Determinism: a second full run in the Step 7 verifier session reproduced 352/352.

**Adversary lane:** `mutation-probes.md` - all five Step 7 probes (`WO-step7-in-world-actions.md` section 12) fail their named test at exit 101 with clean `git checkout --` restores; `git status --porcelain` CLEAN after the final probe, tree byte-identical to the gated `2436a54`.

`SHA256SUMS` covers every other file in this directory, hashed over the committed (LF) bytes per the Step 6.3 L-2 lesson.
