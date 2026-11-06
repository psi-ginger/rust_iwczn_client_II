# Step 7.4 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-06 (UTC) - exact stamp in `head.txt`.
- **Clone:** S4 clean clone, detached at delivery SHA `b72ec33e8f13d5b0493292d1a10a1a2edb4db227` (tree `42dac1e25117b9fe150e7f5f23628a8239e4dddb`), cut from `origin/main` = `c7717c1`. Branch tip `46f3b8d` adds only the §14 delivery note (docs-only, 57+/2−, placeholder lines replaced); `src/`, `tests/`, `Cargo.*` are byte-identical between `b72ec33` and `46f3b8d`, so gating at `b72ec33` fully characterizes the tip.
- **Baseline:** 352 -> **360** (+8: T1-T3 `event_loop/tests.rs`, T4-T5 `commands/tests.rs`, T6-T8 `e2e_loopback.rs`).
- **Environment:** `env.txt` (rustc / cargo / git versions, OS, `core.autocrlf=true`).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `cargo check --all-targets` (`RUSTFLAGS=-D warnings`) | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **360 passed, 0 failed** |

**Test composition (from `test.log`, not prose):** 324 lib (1 ignored) + 11 `asset_fixtures` + 25 `e2e_loopback` = **360 passed**; `live_integration` 4 ignored; 0 failed. Ignored total **5** (1 lib operator demo + 4 live) - the preserved baseline. T1-T8 all present and green. Second full run after the five mutation probes reproduced 360/360 on the restored pristine tree (`postprobe/gate-exits.txt` = `check=0 clippy=0 fmt=0 doc=0 test=0`).

**Write guard (§6):** `write-guard.txt` - `git diff --name-status main..b72ec33` lists exactly the twelve §6 files plus the approved-disclosed `tests/live_integration.rs` (F-7.4-01). Untouched: `src/game|auth|net|main|lib`, `TASK_MATRIX.md` (row 7.4 still `PENDING`), `CONTRIBUTING*`, `README`, other `docs/**`, previously committed `evidence/**`, legacy tree, `soak.rs`/`front_double.rs`/`robustness.rs`.

**Disclosed edits (§2.2 + F-7.4-01):** `disclosed-edits.diff` - `driver/tests.rs` exactly two hunks (`, None` argument + rustfmt reflow at ~434; locator rename to `event_loop::run_with_commands(` at ~603); `event_loop/tests.rs` 175 insertions / **0 deletions** (the one `LoopEvent::Command` panic arm at :255 + T1-T3 appended); `live_integration.rs` 7 insertions / 0 deletions (the F-7.4-01 panic arm only - exhaustive match over the new `LoopEvent::Command` variant would otherwise fail E0004; test logic unaltered). Locator deviation justified: bare `run_with_commands(` would match the driver's own `pub fn` at `driver.rs:475`, before the adoption site `:805`, failing the `hits[0] < looping` assertion (locator scans `include_str!("../driver.rs")`).

**F-07 goldens:** G1 `0A 00 00 F0 00 00 64 00 C8 00` = frozen WO-step7 §8.3-A verbatim; G2 `08 00 00 75 00 03 68 69` = §8.3-D layout (mode `03` Town documented, text `"hi"`); G3 `0E 00 00 F1 00 92 10 00 00 00 64 00 C8 00` = §8.3-H layout (`CHAR_UNIQUE=4242`=0x1092 → `92 10 00 00`, hand-verified 4096+146); G4 = harness fixture. All hand-placed in `frames.rs`, none regenerated from the encoder under test; `WO-step7-in-world-actions.md` untouched.

**Adversary lane:** `mutation-probes.md` - all five §12 probes fail their named test at exit 101 (P1 additionally re-run for its second named test T6), `git checkout --` restores with empty porcelain after each, full gate set green at 360 afterwards (`postprobe/`).

**Live evidence (never a gate, §13.8):** Implementer-reported ngrok run `cargo test --test live_integration -- --ignored` = 4 passed / 0 failed / 70.01 s soak (`live_front_handshake_extracts_secret_key`, `live_game_handshake_decodes_the_link_cipher`, `live_event_loop_idles_cleanly_and_ticks`, `live_heartbeat_keeps_the_connection_up_over_60s`). Recorded as evidence only; matrix row 7.4 stays `PENDING` until the verdict is accepted and the row update + governance commit land.

`SHA256SUMS` covers every other file in this directory tree, hashed over the committed (LF) bytes per the Step 6.3 L-2 lesson.
