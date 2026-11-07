# Step 8.0 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-07 (+07:00) - exact stamps in `head.txt` and the section-11 files.
- **Clone:** S4 clean clone of `git@github.com:psi-ginger/rust_iwczn_client_II.git`, checkout at
  delivery SHA `fc1a7c7c3f6ed684fdc78c3eae236347bd8dd600` (tree `30f9bfc22085897e7287c277f5fb38ea73582b88`),
  porcelain clean before and after every run. Branch tip `89b9a9d` is docs-only:
  `git diff --name-only fc1a7c7 89b9a9d` = `docs/agents/WO-8.0.md` alone, so `src/`, `tests/`
  and `Cargo.*` at the tip are byte-identical to the gated SHA and gating at `fc1a7c7` fully
  characterizes the tip.
- **Baseline:** 360 -> **371** (+11 = T1-T11, all lib; `asset_fixtures` 11 and `e2e_loopback` 25
  untouched). Matrix cell copy target: **371/371**, taken from `test.log` only.
- **Environment:** `env.txt` (rustc / cargo / OS / PowerShell / `RUSTFLAGS=-D warnings` on gate 1).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **371 passed, 0 failed** |

**Test composition (from `test.log`, identical in both runs):** 335 lib (1 ignored) +
11 `asset_fixtures` + 25 `e2e_loopback` = **371 passed**; `live_integration` 4 ignored;
0 doc-tests; 0 failed. Ignored total **5** = preserved baseline. Run 1 exits in
`gate-exits.txt`; run 2 (post-probe) exits in `postprobe/gate-exits.txt`.

**Run 2 (post-probe baseline):** after all four section 12 probes were applied and restored,
the full five-gate set re-ran green at the same numbers (`postprobe/`): check=0, clippy=0,
fmt=0 (0 bytes), doc=0 (0 warnings), test=0 at 371; porcelain clean before and after.

**T1-T11 (DoD 2):** all eleven present and green in one filtered run
`cargo test --lib input::tests::` -> `11 passed; 0 failed; 325 filtered out`, every test
name listed in `mutation-probes.md`; the delivery note additionally records eleven
individual `cargo test --lib <name>` runs.

**Write guard (section 6, DoD 5):** `write-guard.txt` - the delivered diff vs merge-base
`d1a5267` is exactly the seven section-6 files: six NEW `src/input/**` plus `src/lib.rs`
(2 insertions, 0 deletions - one `//!` Phase IV bullet + one `pub mod input;`). Every
READ-ONLY class is untouched in the range diff: `src/client|game|graphics|auth|net|protocol`,
`src/main.rs`, `tests/**`, `Cargo.toml`/`Cargo.lock`, `TASK_MATRIX.md`, `CONTRIBUTING*`,
`README.md`, `GoonZu_Source/**`, `docs/**` (the WO itself is byte-identical at the delivery
SHA) and existing `evidence/**`. Delivered file set == the section 6 file plan. No
pre-existing test was modified.

**Zero-queue audit (P4-C1, phase gate 4):** `zero-queue-grep.txt` -
- machinery subset `VecDeque|coalesc|MAX_KEY`: **0 matches**; whole-word `\bring\b`: **0**;
- collection tokens in the five production files: **0** (the only collection anywhere is
  T10's harness `Vec::with_capacity(100)` in `tests.rs:323`, test data outside the measured
  section);
- async code in production: **0** (the only production `async`/`.await` hits are module-doc
  prose at `mod.rs:12/:16`; the only async code is the two `#[tokio::test]` harnesses T7/T10
  that *receive* from the frozen 7.4 channel) - the D7 no-await structural claim holds;
- **F-8.0-01 (disclosed, never edited):** the section-11 literal grep returns **5 matches** -
  `event.rs:6` (the section 7.1 *mandated* `cltKey` doc citation, mandated by the contract
  itself at WO line 190) and four `String`/`String::new()` lines where the bare alternation
  `ring` substring-matches the primitive type name. As written the WO command also needs
  `-r` (GNU grep refuses a bare directory operand). The literal pattern is unsatisfiable
  against the delivered section-7 contract; the audit's intent (no queue/ring/cap/coalescing
  machinery) holds on every machinery probe. Adjudicated by the Verifier in the WO-8.0
  section-14 verdict: non-blocking WO prose/pattern defect, Scribe-lane correction deferred.

**F-7.4-01 pre-check (DoD 2):** `f7401-precheck.txt` - variant-qualified search
(`InputEvent::` / `Intent::`) outside `src/input/**` = **0 matches**; the only bare type-name
mentions outside the delivered subsystem are prose (`src/game/action.rs:6`, a markdown table
header in the frozen 7.3 doc comment) and the delivery's own `src/lib.rs:30` doc line, with
`src/lib.rs` numstat = 2 insertions / 0 deletions. EMPTY: no pre-existing exhaustive match,
no compiler-forced edit, no disclosed hunks beyond section 6.

**Adversary lane (section 12):** `mutation-probes.md` - all four probes executed on the
pristine tree, each: one semantic break -> named test FAILED at **exit 101** with its E-row
assertion -> `git checkout --` -> empty porcelain:

| # | Break | Named test | Failing assertion (left / right) |
|---|---|---|---|
| P8.0-1 | over-cap line clamped then enqueued | T9 | `E10: a 256-byte line is refused, never truncated` - `Some(Chat{255 x})` / `None` |
| P8.0-2 | `!in_world` guard deleted | T6 | `E7: KeyDown { vk: 13 } before in_world must be discarded` - `Some(ChatFocus)` / `None` |
| P8.0-3 | unknown key -> `SitToggle` | T5 | `E6: KeyDown { vk: 112 } must be discarded without panic` - `Some(SitToggle)` / `None` |
| P8.0-4 | `SendChat` forced to `Secret` | T11 | `E11/D5: the declared default travels uncoerced` - `Chat{mode: Secret}` / `Chat{mode: Normal}` |

Then run 2 green at 371 (`postprobe/`). *Disclosed post-run correction:* the per-probe
"probe checks" metadata line's original compile-error flag was a regex false positive on
cargo's own `error: test failed` harness marker; the four lines were reworded to the
verified statement (no rustc `error[E...]` / `could not compile` signature exists anywhere
in the raw outputs, which are unmodified).

**F-07:** 43 assertions in `src/input/tests.rs`, 37 E-row citations covering **E1-E12** -
every expectation is the WO's frozen text (VK values from `mainfunction.cpp:1218/1229/1304`,
`MAX_CHAT_BYTES` from `action.rs`, goldens restated in section 8), never derived from the
`translate`/`validate` code under test.

**Headlessness (P4-C4 / DoD 8):** grep over `src/input/` - Win32/FFI tokens only in two
doc-comment lines that state their absence (`event.rs:3/:6` "no HWND, no FFI"); network
tokens (`TcpStream|std::net|tokio::net|UdpSocket|connect(`): **0**; `unsafe`: **0**;
`Cargo.toml` untouched (no new crates), no protocol/ file changed (no new opcodes); no live
run used as a gate.

**Bundle layout:** the section-11 standing set (13 files including `SHA256SUMS`) plus
`gate-exits.txt` and `postprobe/` (run 2), mirroring the 7.4 bundle layout. `SHA256SUMS`
covers the other **19** files, hashed over the committed (LF) bytes per the Step 6.3 L-2
lesson: every bundle file was written UTF-8 without BOM, normalized to LF endings and
verified CR=0 before hashing.

**Status:** row 8.0 remains `PENDING` - this bundle and the WO-8.0 section-14 verdict are
produced for Custodian acceptance; the matrix flip, the verdict commit and any push are the
governance step that follows acceptance.
