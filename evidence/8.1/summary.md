# Step 8.1 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-07 (+07:00) - exact stamps in `head.txt` and the section-11 files.
- **Clone:** S4 clean clone of `git@github.com:psi-ginger/rust_iwczn_client_II.git`, checkout at
  delivery tip `cada5ed4a8ba2f2dbd58e047e121b3c11188a36e` (tree `fa771f118c473fe064f172dba6551f384f554430`),
  porcelain clean before and after every run. Delivery SHA `f91897e8ffb76fc6355adeb319e20db7599bd30b`;
  `f91897e..cada5ed` = `docs/agents/WO-8.1.md` alone (the section-14 delivery note), so `src/`,
  `tests/` and `Cargo.*` at the tip are byte-identical to the delivery SHA and gating at the tip
  fully characterizes the delivery (same docs-only pattern as 8.0's `fc1a7c7` -> `89b9a9d`).
  Branch cut point: `449d60f` (this branch contains the Scribe-lane WO-8.1 commit).
- **Baseline:** 371 -> **387** (+16 = T12-T27, all lib; `asset_fixtures` 11 and `e2e_loopback` 25
  untouched). Matrix cell copy target: **387/387**, taken from `test.log` only.
- **Environment:** `env.txt` (rustc / cargo / OS / PowerShell / `RUSTFLAGS=-D warnings` on gate 1).

| # | Gate | Command | Exit | Log |
|---|---|---|---:|---|
| 1 | check | `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test` | 0 | `test.log` - **387 passed, 0 failed** |

**Test composition (from `test.log`, identical in both runs):** 351 lib (1 ignored) +
11 `asset_fixtures` + 25 `e2e_loopback` = **387 passed**; `live_integration` 4 ignored;
0 doc-tests; 0 failed. Ignored total **5** = preserved baseline. Run 1 exits in
`gate-exits.txt`; run 2 (post-probe) exits in `postprobe/gate-exits.txt`.

**Run 2 (post-probe baseline):** after all four section 12 probes were applied and restored,
the full five-gate set re-ran green at the same numbers (`postprobe/`): check=0, clippy=0,
fmt=0 (0 bytes), doc=0 (0 warnings), test=0 at 387; porcelain clean before and after.

**T12-T27 (DoD 2/3):** all sixteen present and green - name-verified line by line against
`test.log` (16/16 `... ok`), each asserted against its section-8 E-row (E13-E29). **T1-T11
byte-identical:** `tests.rs` numstat vs the cut point = **519 insertions / 0 deletions**
(additions only: new `use` lines + appended tests); `event.rs`/`validate.rs` untouched;
the three charter gate-1 parity pins are green in `test.log`:
- **T12** round-trip `screen_to_tile(draw_center_xy(...)) == tile` (11-tile grid x 3 centers
  x 2 screen rects = 66 pairs);
- **T13/T14** hand goldens G1-G5 asserted against the section-2.2 arithmetic, cross-checked
  against the legacy citations: G1 `realx,realy = 0,4020` -> `(0+4020)/40 = 100 r20` = **(100,100)**
  (`Map-Client.cpp:185-187` + `MapCommon.cpp:965-966`); G4 `-545/40 -> -13`, `-1.875 -> -1` =
  **(-13,-1)** (floor would give (-14,-2)); G5 `real_to_tile(-41,-19)` -> **(0,0)** - C99
  trunc-toward-zero, Rust `/` matches; no golden was ever produced by running `screen_to_tile`;
- **T19/T20** click -> `Order::Move` bytes `[0, 100, 0, 100, 0]` and `Order::Attack` bytes
  `[1, 7, 0, 0, 0]` through the frozen `encode_order` (`action.rs:113-125`), never new encoding
  code.

**Write guard (section 6, DoD 5):** `write-guard.txt` - `git diff --stat 449d60f..f91897e` is
exactly the six section-6 files (786 insertions / 5 deletions): NEW `screen.rs` (90) +
`target.rs` (103), `intent.rs` (15), `keybinds.rs` (52), `mod.rs` (12, insertions only),
`tests.rs` **519 / 0** (additions-only, DoD 2). The 5 deletions are the sanctioned section-7
edits (one `intent.rs` Default-doc line + the four `:93-96` placeholder lines). The
`f91897e..cada5ed` docs delta is `WO-8.1.md` alone. Every READ-ONLY class is untouched in the
range diff: `src/client|game|graphics|auth|net|protocol`, `tests/**`, `Cargo.toml`/`Cargo.lock`,
`TASK_MATRIX.md`, `CONTRIBUTING*`, `README.md`, `docs/agents/WO-8.0.md`, `GoonZu_Source/**`,
existing `evidence/**`. Delivered file set == the section 6 file plan. No pre-existing test
was modified.

**Zero-queue audit (P4-C1, phase gate 4):** `zero-queue-grep.txt` - machinery subset
`VecDeque|coalesc|MAX_KEY`: **0 matches** (grep exit 1, audit GREEN) under the F-8.0-01-corrected
pattern (no bare `ring` alternation, no bare `queue` word). Inventory note: `ui.targets:
Vec<CharTarget>` is the section-7.2 snapshot field, not a queue, and `translate` never mutates it.
- **F-8.1-01 (disclosed, never edited):** the section-11 literal pattern including `cltKey`
  returns **1 match** - `src/input/event.rs:6`, the pre-existing Step 8.0 `//!`/doc citation of
  the legacy `cltKey` payload (READ-ONLY for this step, last touched by `fc1a7c7`), the same
  citation already adjudicated under F-8.0-01. The audit's intent (no queue/ring/cap/coalescing
  machinery) holds on every machinery probe. Adjudication is the Verifier's: non-blocking
  pre-existing doc citation, no code impact.

**F-7.4-01-style pre-check (DoD 2):** `f7401-precheck.txt` - among Rust sources the new
identifiers (`MapView|CharTarget|TargetClass|pick_target|screen_to_real|real_to_tile|
screen_to_tile|is_pt_in_map|is_in_map_area`) occur **only inside `src/input/**`** (six files,
all section-6 owned); other hits are docs (`WO-8.1.md` defining them; pre-existing
`CONTRIBUTING_PHASE4.md`/`TASK_MATRIX.md`/`WO-8.0.md` already naming `screen_to_tile`) and
incidental legacy-tree `MapView` under `GoonZu_Source/**`. No disclosed hunks beyond section 6.

**Adversary lane (section 12):** `mutation-probes.md` - all four probes executed on the
pristine tree, each: one semantic break -> named test FAILED at **exit 101** with its E-row
assertion -> `git checkout --` -> empty porcelain:

| # | Break | Named test | Failing assertion (left / right) |
|---|---|---|---|
| P8.1-1 | `screen_to_real` trailing `+ MAP_TILE_YSIZE_HALF` deleted | T14 | `E15: G4 truncates toward zero like C99` - `(-14, -2)` / `(-13, -1)` |
| P8.1-2 | `real_to_tile` y-branch divisor XSIZE -> YSIZE | T13 | `E14: G1 - the rect center lands on the view center` - `(100, 201)` / `(100, 100)` |
| P8.1-3 | `translate` IsPtInMap gate deleted | T17 | `E18: off-screen click (1000, 340) is discarded` - `Some(MoveTo { x: 107, y: 94 })` / `None` |
| P8.1-4 | `TargetClass::Npc` arm falls through to `Some(MoveTo { .. })` | T21 | `E22: Npc class → typed None, never a MoveTo` - `Some(MoveTo { x: 100, y: 100 })` / `None` |

Then run 2 green at 387 (`postprobe/`). Probe 3's deleted gate also surfaced its
`unused import: is_pt_in_map` as a disclosed non-fatal warning (plain probe run, no RUSTFLAGS);
the failure itself is T17's E18 assertion as tabled.

**F-07:** 87 assertion lines in `src/input/tests.rs`, 66 E-row citation occurrences covering
**E13-E29 (all 17 rows)** - every expectation traces to a section-8 row, and every E-row traces
to a legacy `file:line` (`Map-Client.cpp:185-187/:196-206/:199-200`, `MapCommon.cpp:965-966`,
`MouseOperation.cpp:534/:3259-3261/:3267/:3283/:4138/:5409`, `UnderMouseObject.cpp:156-159/
:168-175/:212-215/:335`, `map.cpp:224-228`, `Char-Client.cpp:1015-1016/:1018`,
`mainfunction.cpp:208/:232-249/:236/:251-258`, frozen `action.rs:65/:67/:113-125`), the
section-2.2 hand arithmetic, or a frozen constant - never the code under test.

**Headlessness (P4-C4 / DoD 8):** grep over `src/input/**` - `unsafe|TcpStream|std::net|
tokio::net|UdpSocket|connect(|winapi|extern "C"`: **0 matches**; `HWND`/`FFI` only in the two
`event.rs:3/:6` doc lines that state their absence. `Cargo.toml`/`Cargo.lock` untouched (no new
crates), no `protocol/` change (no new opcodes), no wire-byte change (byte pins use the frozen
`encode_order`), no live run used as a gate. D7 structural claim holds: production `.await` = 0
(only `mod.rs:16` doc prose); the only async code is the two pre-existing `#[tokio::test]`
harnesses (`tests.rs:201` `move_streams_never_enqueue_a_command`, `:317`
`the_enqueue_path_stays_within_the_16ms_frame_budget` = T10) receiving from the frozen 7.4
channel - T10 green.

**Bundle layout:** the section-11 standing set (13 files including `SHA256SUMS`) plus
`gate-exits.txt` and `postprobe/` (run 2), mirroring the 8.0 bundle layout (20 files total).
`SHA256SUMS` covers the other **19** files, hashed over the delivered (LF) bytes per the Step
6.3 L-2 lesson: every bundle file was written UTF-8 without BOM, normalized to LF endings and
verified CR=0 before hashing.

**Status:** row 8.1 remains `PENDING` - this bundle and the WO-8.1 section-14 verdict are
produced for Custodian acceptance; the matrix flip, the verdict commit and any push are the
governance step that follows acceptance (none authorized by the verification directive).
