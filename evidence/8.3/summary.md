# Step 8.3 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-08 (+07:00) - exact stamps in `head.txt`, `env.txt` and the
  section-11 files.
- **Clone:** S4 clean clone of `https://github.com/psi-ginger/rust_iwczn_client_II.git`
  (`--branch step/8.3-driver-wiring`), checkout at delivery tip
  `90a0f0766a091e5b33f74a44e62d0e44d405d7d8` (tree `ec1672af277b10a8bf87268001bae97168ffe7d0`),
  porcelain clean at clone and before/after every run (only this bundle's own untracked
  `evidence/8.3/` ever appeared; every probe restore checked with
  `git status --porcelain -- ':!:evidence'` => empty). Delivery SHA `8e9ba02cc533786b07fb81ec23046aa82dce8d00`
  (parent `f4e93bc`, the Scribe WO-8.3 commit = the branch cut point); `8e9ba02..90a0f07`
  = `docs/agents/WO-8.3.md` alone (the section-14 delivery note), so `src/`, `tests/` and
  `Cargo.*` at the tip are byte-identical to the delivery SHA and gating at the tip fully
  characterizes the delivery (same docs-only pattern as 8.0/8.1/8.2).
- **Baseline:** 399 -> **414** (+15 = T40-T51's 12 + `frames.rs`'s 3 inner tests
  re-running in the new `input_loopback` binary, D22's disclosed duplication). Matrix cell
  copy target: **414/414**, taken from `test.log` only.
- **Environment:** `env.txt` (rustc 1.99.1 / cargo 1.99.1 / git 2.55.0 / Windows /
  PowerShell 5.1 / `RUSTFLAGS=-D warnings` on every gate of both runs).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | `check.log` - 0 warnings |
| 2 | clippy | `RUSTFLAGS=-D warnings cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **414 passed, 0 failed** |

**Test composition (from `test.log`, identical in both runs):** 371 lib (1 ignored) +
11 `asset_fixtures` + 25 `e2e_loopback` + 7 `input_loopback` = **414 passed**;
`live_integration` 4 ignored; 0 doc-tests; 0 failed. Ignored total **5** = preserved
baseline. Cross-checks (run 1, appended to `test.log`): `cargo test --lib input::tests`
= **43/43** (39 inherited + T43-T46); `cargo test --lib graphics::window` = **4 passed +
1 ignored preserved** - both match section 9. Run 1 exits in `gate-exits.txt`; run 2
(post-probe) exits in `postprobe/gate-exits.txt`. **No warning line in either test log.**

**T40-T51 (DoD 13.3):** all twelve name-verified `... ok` in both runs (`t1-t39-unchanged.txt`
lists them): T40/T41/T42/T51 in `graphics::window::tests`, T43-T46 in the appended
`input::tests` block, T47-T50 in `input_loopback` (7/7 = T47-T50 + `frames.rs`'s 3 inner
tests re-running - the D22 duplication counted twice in the log, disclosed in section 9's
arithmetic). T51's 100-raw-message `< 16 ms` batch green with its margin documented and
never relaxed (T10 green unchanged). **T1-T39 byte-identical:** `t1-t39-unchanged.txt` -
one hunk `@@ -1362,0 +1363,192 @@`, zero deletions, and an element-wise check that the
1362-line base file is the **exact prefix** of the 1554-line delivered file.

**Decision pinning (DoD 13.3):** D17 -> T40/T41/T42 + probes 1-2; D18 -> T40's dedicated
`WM_LBUTTONDBLCLK -> LeftDown` assertion (`E31/D18`, failing-if-corrected, body audited);
D19 -> T43/T45/T46 + probe 3; D20 -> T44/T50 + probe 4; D21 -> decode half by T40-T42,
feed half by T43-T46, the Win32 arm itself under P4-C4's operator-only clause (no gate test
builds a window/DC/message - the decode takes raw integers); D22 -> T47-T50 running on the
`#[path]`-re-declared frozen harness (the 3 re-declared modules and their `#[path]` lines
audited at `input_loopback.rs:27-34`); D23 -> T51 green with its margin documented.

**Write guard (section 6, DoD 13.5):** `write-guard.txt` - `git diff --name-only
f4e93bc..8e9ba02` is exactly the six section-6 files (1138 insertions / 2 deletions);
the forbidden set (`src/client/**`, `src/main.rs`, `src/lib.rs`, `src/game/**`,
`event/intent/keybinds/validate/screen/target`, `tests/e2e_loopback*`, `tests/asset_*`,
`tests/live_*`, `Cargo.*`, matrix, contributing, README, legacy tree) is **NONE** -
**`src/client/**` strictly untouched**. `window.rs` = 8 hunks, all input-side (module-doc
sentence, imports, seven `WM_*` consts + `wm_to_input`, `WindowState.input` + its `input:
None` init, the arm, `mod tests`); render/present/tick/pump/demo hunks **zero**; the only
2 deletion lines repo-wide are the two sanctioned `use` line joins; the
`_ => DefWindowProcA(...)` fallthrough and the pump's `TranslateMessage`/`DispatchMessageA`
calls are byte-identical. `mod.rs` = exactly 2 lines.

**Zero-queue audit (P4-C1, gate item 4):** `zero-queue-grep.txt` - machinery components
**0 matches**; **1 prose match** at `src/input/tests.rs:1541` (the appended T46 doc
comment "no VecDeque/ring/cap anywhere ..."), i.e. the F-8.0-01 pattern-vs-prose class
recurring in a new location -> **finding F-8.3-01, non-blocking** (adjudicated per the
F-8.0-01 precedent: a spec-internal pattern/prose defect, not a delivery defect; never
edited - P3-6). Inventory confirms no queue/ring/cap/coalescing machinery exists in
`src/input/**` or `feed.rs` (`Vec<CharTarget>` = 8.1's target list only), so the P4-C1
substance holds and DoD 13.7's structural claim is green.

**F-7.4-01 post-check (DoD 13.8):** `f7401-postcheck.txt` - `event.rs`, `intent.rs`,
`keybinds.rs`, `validate.rs` **blob-identical** (`git rev-parse` hashes equal base to
delivery); `screen.rs`/`target.rs` empty diff; every `InputEvent::` use in delivered files
outside `src/input` is a construction/assertion (none destructures), `Intent::` has zero
uses there, no `match <event|intent>` exists in `src/graphics` or `tests/` - no new
exhaustive match anywhere. **No new variant, no field, no validate change.**

**Adversary lane (section 12):** `mutation-probes.md` - all four probes re-run by the
Verifier against the committed tree, each: one semantic break -> named test FAILED at
**exit 101** with its E-row assertion -> `git checkout --` -> TRACKED_CLEAN:

| # | Break | Named test | Failing assertion |
|---|---|---|---|
| P8.3-1 | drop `+ MAP_TILE_* + SCREEN_*` | T40 | `E30: WM_MOUSEMOVE decodes to Move in backbuffer space` - `left {480,360}` / `right {560,400}` (compile-fail under `-D warnings` first - disclosure 1 verified) |
| P8.3-2 | `KeyDown { vk: l_param as u32 }` | T41 | `E32: RETURN (0x0D) round-trips as KeyDown { vk: 13 }` - `left { vk: 0 }` |
| P8.3-3 | `let _ = send(command.clone()); Ok(Some(...))` | T45 | `E38: the closed channel surfaces by name - no panic, no Ok` (tests.rs:1523) |
| P8.3-4 | `enter_world` without the view | T44 | `E37: enter_world wires the click context in the same call (D20)` - `left: None` |

**F-07:** 64 E-row citation occurrences across the three delivered test files
(`src/input/tests.rs` 21, `src/graphics/window/tests.rs` 31, `tests/input_loopback.rs` 12)
covering E30-E41 (+ inherited E12 for T51); every E-row traces to a legacy `file:line`
(`mainfunction.cpp:208/:218/:232/:251/:1202/:1261/:1282`, `MouseOperation.cpp:232-233`,
`Client.cpp:7709-7710`, `main.cpp:852-862`), a frozen constant (the three §8.3 goldens,
verified byte-for-byte in `input_loopback.rs:78/81/85`:
`06 00 00 F0 00 02`, `08 00 00 75 00 01 68 69`, `0A 00 00 F0 00 00 67 00 65 00`), or the
section 2.4 hand arithmetic - never the code under test.

**Headlessness / D7 (DoD 13.8):** no window, DC, message struct or display is built by any
gate test (decode = raw integers); `Cargo.toml`/`Cargo.lock` untouched (no new crates); no
protocol change (no new opcodes); no wire-byte change; no live run used as a gate. The
delivery adds no `.await` and no buffer between the raw message and
`CommandSender::send` (`feed.rs` is translate -> validate -> send, one synchronous call);
T51 green with its documented margin (DoD 13.7).

**Disclosures audited (delivery note):** (1) probe 1's no-`-D` run - **verified** by the
Verifier's own compile-fail capture; (2) probe 3's `command.clone()` - verified (the
tabled form is a use-after-move); (3) T45's "(still default)" precision fix - verified in
the test body (fresh-`default` pin + post-arm snapshot, E38 substance pinned); (4)
module-scoped `#[allow(dead_code)]` on the `frames`/`game_double` re-declarations only
(`input_loopback.rs:28/:33`) - verified; the frozen files are untouched and
`input_loopback`'s own code stays fully linted.

**Bundle layout:** the section-11 standing set (`head.txt`, `check.log`, `clippy.log`,
`fmt.log`, `test.log`, `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`,
`zero-queue-grep.txt`, `f7401-postcheck.txt`, `write-guard.txt`, `t1-t39-unchanged.txt`,
`SHA256SUMS` = 14) plus `gate-exits.txt` and `postprobe/` (run 2, 6 files) = **21 files**,
mirroring the 8.0/8.1/8.2 bundle layout. `SHA256SUMS` covers the other **20** files, hashed
over the delivered (LF) bytes per the Step 6.3 L-2 lesson: every bundle file verified
CR=0, UTF-8 without BOM, pure ASCII before hashing.

**Status:** verification products produced for Custodian acceptance. Verdict recorded in
`docs/agents/WO-8.3.md` section 14 (Verdict - Verifier): **PASSED with F-8.3-01
non-blocking** (adjudicated per F-8.0-01). Row 8.3 remains `PENDING` in `TASK_MATRIX.md` -
the matrix flip, this bundle's and the verdict's commit, and any push are the governance
step that follows acceptance (none authorized by the verification directive).
