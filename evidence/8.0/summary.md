# Step 8.0 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-06 (UTC) - exact stamp in `head.txt`.
- **Clone:** S4 clean clone, detached at delivery SHA `fc1a7c7c3f6ed684fdc78c3eae236347bd8dd600`
  (tree `30f9bfc22085897e7287c277f5fb38ea73582b88`, parent `d1a5267` = `origin/main`).
  `origin/step/8.0-input-core` verified via `git ls-remote` = branch tip `89b9a9d`, which adds
  only the docs-only §14 delivery note (`docs/agents/WO-8.0.md` 73+/3−); `src/`, `tests/`,
  `Cargo.*` are byte-identical between `fc1a7c7` and `89b9a9d`, so gating at `fc1a7c7` fully
  characterizes the tip. Porcelain CLEAN before/after every gate and probe.
- **Baseline:** 360 -> **371** (+11 = T1–T11, all lib; `asset_fixtures` 11 and e2e 25 untouched;
  1 ignored lib + 4 ignored `live_integration` preserved).
- **Environment:** `env.txt` (rustc 1.99.0, cargo 1.99.0, git 2.55.0, `core.autocrlf=false`).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `cargo check --all-targets` (`RUSTFLAGS=-D warnings`) | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **371 passed, 0 failed** |

**Test composition (from `test.log`, not prose):** 335 lib (1 ignored) + 11 `asset_fixtures` +
25 `e2e_loopback` = **371 passed**; `live_integration` 4 ignored; 0 failed. Ignored total **5**
(1 lib operator demo + 4 live) - the preserved baseline. T1–T11 all present and green
(`cargo test --lib -- --list` enumerates the eleven `input::tests::*` names). Second full run
after the four mutation probes reproduced 371/371 on the restored pristine tree
(`postprobe/gate-exits.txt` = `check=0 clippy=0 fmt=0 doc=0 test=0`), with pre- and post-gate
porcelain CLEAN (`postprobe/*-gate-porcelain.txt`, both 0 bytes).

**Write guard (§6):** `write-guard.txt` - `git diff --stat d1a5267..fc1a7c7` = exactly the six
new `src/input/**` files plus the two-line `src/lib.rs` edit (7 files, 690 insertions,
**0 deletions**); forbidden-path grep over the delivery diff = none. The only pre-existing file
touched is `src/lib.rs` (one `//!` Phase IV bullet + `pub mod input;` in alphabetical position,
verbatim in `f7401-precheck.txt` §D). Untouched: `src/client|game|graphics|auth|net|protocol`,
`tests/**`, `Cargo.*`, `TASK_MATRIX.md` (row 8.0 still `PENDING`), `CONTRIBUTING*`, `README`,
other `docs/**`, previously committed `evidence/**`, legacy tree `GoonZu_Source/**`.

**F-7.4-01 pre-check:** `f7401-precheck.txt` - whole-tree grep over `InputEvent`/`Intent` at
baseline `d1a5267` is **empty** (no pre-existing exhaustive match anywhere); at delivery the
same grep matches only inside `src/input/` (`keybinds.rs`, `validate.rs`, `tests.rs`). No
forced hunk on any pre-existing test (unlike 7.4 there are none to disclose).

**F-8.0-01 (disclosed by the implementer, adjudicated here):** `zero-queue-grep.txt` - the
literal §11 pattern `grep -inE 'VecDeque|ring|coalesc|MAX_KEY|cltKey' src/input/` returns
**5 matches**, all of them §7 contract text delivered verbatim: `event.rs:6` (the §7.1-mandated
doc comment citing the legacy `cltKey` payload - documentation of what was *not* ported),
`intent.rs:28/:48` (the §7.2-mandated `line: String` fields - the bare `ring` alternation
substring-matches `String`), `intent.rs:40` (§7.2 `Default` sketch restated), `tests.rs:259`
(T9's empty-line stimulus). The audit's intent holds: the machinery subset
`VecDeque|coalesc|MAX_KEY` has **zero** matches, and the word-bounded variant leaves only the
§7.1 `cltKey` citation - no queue/ring/cap/coalescing type exists in `src/input/**`. The
finding is a **spec-internal inconsistency** (§11's grep cannot be satisfied while §7's
contract is delivered verbatim), disclosed rather than edited (§1 write guard); the grep as
written is unsatisfiable-by-contract, so it cannot fail the delivery on its own terms.
**Adjudication: F-8.0-01 upheld as a documentation finding against the WO, not the code** -
no delivery defect; recommend the Custodian word-bound the pattern (`\bring\b`, drop `cltKey`
or exempt the §7 citation) in a future WO revision.

**F-07:** every assertion message in `src/input/tests.rs` cites its §8 E-row (29 assertions,
E1–E12 all covered); every E-row's provenance verified against the frozen/legacy source:
`MAX_CHAT_BYTES = 255` (`action.rs:53`, exact), `ChatMode::Normal = 1`
(`action.rs:145`, WO cites `:148` - line drift in the WO, constant correct), `Order::Move
{ x: u16, y: u16 }` (`action.rs:80-85`, WO cites `:87-94` - same drift, type bound correct),
refuse-never-clamp style (`action.rs:198-214`, exact), and legacy `mainfunction.cpp:208`
(`WM_MOUSEMOVE`), `:1202` (`WM_KEYUP`), `:1218` (`VK_RETURN`), `:1229` (`VK_INSERT`),
`:1304` (`VK_ESCAPE`), `GetFrame() > 40` at `:221,236,254,288,302,1367` - all verified
verbatim. No expectation is read back from the code under test; the two `action.rs` line
drifts are WO-prose nits, not traceability failures (the constants are real and frozen).

**D7 structural claim (DoD 13.7):** no `.await` in the enqueue path - the only `.await`s under
`src/input/**` are the two test-side `receiver.recv().await` drains (T7/T10); the production
chain `translate -> validate -> sender.send` is synchronous with no intermediate storage of
any kind (grep in this bundle: `zero-queue-grep.txt` machinery subset = 0). T10's margin
rationale (the whole 100-event batch must fit in one 16 ms frame = ~1/100 of the per-event
budget each; debug enum dispatch + mpsc push is µs-scale, so >=10x headroom under the
assertion) is in its doc comment; T10 green in both runs.

**Adversary lane:** `mutation-probes.md` - all four §12 probes fail their named test at exit
101, `git checkout --` restores with empty porcelain after each, full gate set green at 371
afterwards (`postprobe/`). Raw artifacts: `probes/P*-mutation.diff`, `probes/P*-test.log`,
`probes/P*.exit`, `probes/P*-restore-porcelain.txt`.

**No new crates / opcodes / wire bytes:** `Cargo.toml` and `Cargo.lock` byte-identical to
baseline (0 diff lines); no file under `src/protocol/**` touched; no opcode table or frame
code changed. Headless throughout: no window/Win32/network object in any gate test (P4-C4) -
`src/input/**` contains only doc-comment mentions of HWND/Win32, no code references.

`SHA256SUMS` covers every other file in this directory tree, hashed over the committed (LF)
bytes per the Step 6.3 L-2 lesson.
