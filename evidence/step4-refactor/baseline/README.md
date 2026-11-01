# Step 4 Phase 0 — Baseline (evidence only, no commit)

Branch: `refactor/step4-structure` (created from `main` @ `e6ee8a9`).

## Head

`00-head.txt`:

    e6ee8a92b3df07c4781b1a2be0c8915e27216b0c

Tree was verified clean (`git status --porcelain` empty) before capture.

## Environment

`00-env.txt`:

- rustc 1.99.1 (58cf8f9c3 2026-10-13), host `x86_64-pc-windows-gnu`
- cargo 1.99.1 (5f94df478 2026-08-27)
- git 2.55.0.windows.5

`core.autocrlf` is `true` on this host: the working tree is CRLF. Every hash in
the Phase 5 evidence bundle is computed over **committed blobs (LF
normalized)**, per the Step 3 §7 lesson — never over working-tree bytes.

## Gates (all 5 exit 0)

| # | Gate | Log | Exit |
|---|------|-----|------|
| 1 | `cargo check --all-targets` | `01-cargo-check.log` | `01-cargo-check.exit` = 0 |
| 2 | `cargo clippy --all-targets -- -D warnings` | `02-cargo-clippy.log` | `02-cargo-clippy.exit` = 0 |
| 3 | `cargo fmt --check` | `03-cargo-fmt.log` | `03-cargo-fmt.exit` = 0 |
| 4 | `cargo test` | `04-cargo-test.log` | `04-cargo-test.exit` = 0 |
| 5 | `cargo doc --no-deps` | `05-cargo-doc.log` | `05-cargo-doc.exit` = 0 |

Exit codes were captured with delayed expansion
(`cmd /v:on /c "… > file 2>&1 & echo EXIT=!ERRORLEVEL! > file.exit"`); the
plain `%ERRORLEVEL%` form expands at parse time and always reports the
*previous* command's code — verified and rejected during capture.

## Test counts (copied verbatim from `test result:` lines in `04-cargo-test.log`)

| Target | Result line |
|--------|-------------|
| unittests `src\lib.rs` | `ok. 300 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out` |
| unittests `src\main.rs` | `ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `tests\asset_fixtures.rs` | `ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `tests\e2e_loopback.rs` | `ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `tests\live_integration.rs` | `ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out` |
| Doc-tests `goonzu_client` | `ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |

Totals — executed: **327** (300 + 0 + 11 + 16 + 0 + 0); ignored: **5** (1 lib
operator-window test + 4 live_integration). This is the number Phase 1 must
grow from and every later phase must match or exceed.
