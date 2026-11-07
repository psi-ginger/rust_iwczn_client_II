# Step 8.2 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-07 (+07:00) - exact stamps in `head.txt` and the section-11 files.
- **Clone:** S4 clean clone of `git@github.com:psi-ginger/rust_iwczn_client_II.git`, checkout at
  delivery tip `937c9ab1e6c35456fa0f2748207fa4257960d000` (tree `37a55ca886c29b71a667b3e1fea4c8e95d8f9343`),
  porcelain clean before and after every run. Delivery SHA `26c07a7b13a38b46be25a945f77c72a022716b28`
  (parent `987a3b7`, the Scribe WO-8.2 commit = the branch's cut point and current `origin/main`);
  `26c07a7..937c9ab` = `docs/agents/WO-8.2.md` alone (the section-14 delivery note), so `src/`,
  `tests/` and `Cargo.*` at the tip are byte-identical to the delivery SHA and gating at the tip
  fully characterizes the delivery (same docs-only pattern as 8.0's `fc1a7c7` -> `89b9a9d` and
  8.1's `f91897e` -> `cada5ed`).
- **Baseline:** 371 -> **383** (+12 = T12-T23, all lib; `asset_fixtures` 11 and `e2e_loopback` 25
  untouched). Matrix cell copy target: **383/383**, taken from `test.log` only.
- **Environment:** `env.txt` (rustc / cargo / OS / PowerShell / `RUSTFLAGS=-D warnings` on gate 1).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **383 passed, 0 failed** |

**Test composition (from `test.log`, identical in both runs):** 347 lib (1 ignored) +
11 `asset_fixtures` + 25 `e2e_loopback` = **383 passed**; `live_integration` 4 ignored;
0 doc-tests; 0 failed. Ignored total **5** = preserved baseline. `cargo test --lib input::tests`
= **23/23** (T1-T11 inherited + T12-T23). Run 1 exits in `gate-exits.txt`; run 2 (post-probe)
exits in `postprobe/gate-exits.txt`.

**Run 2 (post-probe baseline):** after all four section 12 probes were applied and restored
(plus the disclosed probe-1 re-run - see `mutation-probes.md` process notes), the full five-gate
set re-ran green at the same numbers (`postprobe/`): check=0, clippy=0, fmt=0 (0 bytes), doc=0
(0 warnings), test=0 at 383; porcelain clean before and after.

**T12-T23 (DoD 13.2/13.3):** all twelve present and green - name-verified line by line against
`test.log` in **both** runs (12/12 `... ok`): `held_insert_fires_once_until_released`,
`held_enter_does_not_oscillate_the_chat_line`, `escape_repeats_are_deliberately_unguarded`,
`backspace_edits_the_chat_line`, `backspace_autorepeat_is_intentional`,
`control_characters_never_enter_the_chat_line`, `escape_closes_the_active_dialog`,
`chat_cancel_takes_priority_over_the_dialog`, `dialog_state_does_not_swallow_game_keys`,
`arrow_keys_remain_unbound`, `legacy_no_op_keys_are_discarded`,
`the_edited_line_flushes_through_the_full_chain` (11 plain + 1 tokio).
**T1-T11 byte-identical:** `t1-t11-unchanged.txt` - `git diff -U0 987a3b7..26c07a7 --
src/input/tests.rs` shows exactly one hunk, `@@ -412,0 +413,431 @@` (pure insertion after
line 412), zero deletion lines anywhere in the file; lines 1-412 untouched; the shared helpers
unchanged. T10 (`the_enqueue_path_stays_within_the_16ms_frame_budget`) green with the D9 guard
in place - no margin relaxed (DoD 13.3, D9.4 other-key clause exercised by its script).
**Decision pinning (DoD 13.3):** D9 -> T12/T13/T14/T16/T10 + probe 1; D10 -> T18/T19/T20 +
probe 3; D11 -> T15/T16/T17/T23 + probe 2; D12 (surface stability) -> `f7401-postcheck.txt`
(`event.rs`/`Intent` byte-identical, exhaustive-match stability) + T21/T22 + probe 4.

**Write guard (section 6, DoD 13.5):** `write-guard.txt` - `git diff --stat 987a3b7..26c07a7`
is exactly the four §6 files (519 insertions / 9 deletions): `intent.rs`, `keybinds.rs`,
`mod.rs`, `tests.rs` **431 / 0** (additions-only). The 9 deletions are the sanctioned §7 edits
(the `intent.rs` Default-doc line reworded + the `keybinds.rs`/`mod.rs` line joins at the new
bindings). `26c07a7..937c9ab` = `WO-8.2.md` alone. **Mouse arms byte-identical** (the 8.1
parallel-lane guarantee): 13 hunks in `keybinds.rs`, **0** overlap the arm region at old lines
93-96 (machine-checked), and base line 93 == delivery line 163 byte-for-byte. `Intent` enum
byte-identical (all three `intent.rs` hunks have old-side start >= line 40; `pub enum Intent {`
is at line 9 - the enum region precedes every hunk). `event.rs`, `validate.rs`, `src/lib.rs`,
`tests/**`, `Cargo.*`, matrix, contributing, README, `src/client|game|graphics|auth|net|protocol`,
legacy tree: all empty in the range diff. No pre-existing test was modified.

**Zero-queue audit (P4-C1, Phase IV gate 4):** `zero-queue-grep.txt` - **0 matches** on both
components of the corrected pattern (machinery subset `VecDeque|coalesc|MAX_KEY`, and the
word-bounded `ring`), run as `git grep` against the delivery commit plus a worktree run
(F-8.0-01 disposition, §2.2-6: the bare-`ring`/`cltKey`-in-prose defect does not recur).
- **Host quirk (disclosed):** this host's PowerShell strips backslashes from native grep
  arguments (diagnosed: `'#\[test\]'` returns 0 via PS direct vs 20 via `cmd /c`), and routing
  the full pattern through `cmd /c` is blocked by the `|` batch metacharacter - the audit
  therefore ran as its two semantically equivalent components (`-w ring` == `\bring\b`), both
  green, while §7's contract text stays verbatim. Inventory note: `UiState`'s collections are
  the §7.1-declared fields (`line: String`, `repeat_vk: Option<u32>`, `dialog_open: bool`) -
  no queue/ring/capacity/coalescing machinery exists in `src/input/**`.

**F-7.4-01 post-check (DoD 13.8):** `f7401-postcheck.txt` - (1) `event.rs` byte-identical to
the 8.0 delivery (empty diff); (2) `intent.rs` hunks touch only the `UiState` Default-doc and
struct body - `pub enum Intent` untouched, both new fields `#[derive(Default)]`-compatible so
no struct literal or exhaustive match can break; (3) the delivery touched only four files, all
inside `src/input/**`, so no file outside could have gained a new exhaustive match; (4)
pre-existing `Intent::`/`InputEvent::` files outside `src/input` are byte-identical to the
signed-off state. **No new `Intent`/`InputEvent` variant, no `validate` change.**

**Adversary lane (section 12):** `mutation-probes.md` - all four probes executed on the
pristine tree, each: one semantic break -> named test FAILED at **exit 101** with its E-row
assertion -> `git checkout --` -> empty porcelain:

| # | Break | Named test | Failing assertion |
|---|---|---|---|
| P8.2-1 | D9.1 block-checks deleted in `VK_RETURN`/`VK_INSERT` arms (6-line diff) | T12 | `E13/D9.1: auto-repeat 1 must be discarded, not re-fired` - `left: Some(SitToggle)` / `right: None` |
| P8.2-2 | D11 control filter removed (`line.push(ch)` unconditional) | T17 | `E15: the line stays empty after '\r'` |
| P8.2-3 | `ui.dialog_open = false` dropped from the Escape close branch | T18 | `E16/D10: the dialog is closed` |
| P8.2-4 | arrow bound: `0x26 => Some(SitToggle)` before the `_` catch-all | T21 | `E18: arrow KeyDown 0x26 must be discarded` - `left: Some(SitToggle)` / `right: None` |

Two process notes are disclosed in `mutation-probes.md`: the first probe-4 attempt was invalid
(a script array-literal bug dropped the `_ => {` line -> compile error, not a test failure) and
was discarded and re-run cleanly; the first probe-1 run produced the intended break with a
whitespace-only artifact and was re-run for the exact 6-deletion diff (gates re-run after the
final probe).

**F-07:** 104 assertion lines in `src/input/tests.rs`, **120** E-row citation occurrences
covering **E1-E19** (all §8 rows: E1-E12 inherited from 8.0, E13-E19 new here) - every
expectation traces to an §8 row, and every E-row traces to a legacy `file:line`
(`mainfunction.cpp:1202/:1218/:1229`, `Edit.cpp:1024`, `NChatBarDlg.cpp:715`,
`ItemInventory.cpp:1479`, `PartyDlg.cpp:411-413`, `NExitDlg.cpp:63`, `InterfaceMgr` chat-bar
citations, frozen `action.rs:53/:65/:94/:202-204`) or a frozen constant - never the code
under test.

**Headlessness (P4-C4 / DoD 13.8):** grep over `src/input/**` - `unsafe|TcpStream|std::net|
tokio::net|UdpSocket|connect(|winapi|extern "C"`: **0 matches**. `Cargo.toml`/`Cargo.lock`
untouched (no new crates), no `protocol/` change (no new opcodes), no wire-byte change, no
live run used as a gate. D7 structural claim holds: production `.await` = 0 (only `mod.rs:16`
doc prose); async code exists only in tests - the two pre-existing 8.0 `#[tokio::test]`
harnesses (`tests.rs:194`, `:310` = T10) plus this step's T23 chain test (`tests.rs:791`) -
none in the enqueue path; T10 green with its margin documented (DoD 13.7).

**Bundle layout:** the section-11 standing set (`head.txt`, `check.log`, `clippy.log`,
`fmt.log`, `test.log`, `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`,
`zero-queue-grep.txt`, `f7401-postcheck.txt`, `write-guard.txt`, `t1-t11-unchanged.txt`,
`SHA256SUMS` = 14) plus `gate-exits.txt` and `postprobe/` (run 2, 6 files) = **21 files**,
mirroring the 8.0/8.1 bundle layout. `SHA256SUMS` covers the other **20** files, hashed over
the delivered (LF) bytes per the Step 6.3 L-2 lesson: every bundle file was written UTF-8
without BOM, normalized to LF endings and verified CR=0 before hashing.

**Status:** row 8.2 remains `PENDING` - this bundle and the WO-8.2 section-14 verdict are
produced for Custodian acceptance; the matrix flip, the verdict commit and any push are the
governance step that follows acceptance (none authorized by the verification directive).
