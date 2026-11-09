# Step 8.4 verification - evidence bundle (S4 clean clone)

- **Captured:** 2026-11-09 (+07:00) - exact stamps in `head.txt` and the per-file captures.
- **Clone:** S4 clean clone of `git@github.com:psi-ginger/rust_iwczn_client_II.git`, branch
  `step/8.4-live-acceptance`, checkout at note tip **`19e603d66bad5c9cf5a2f8582e318ba9ea98cb61`**
  (tree `3d28e2fa2b39580b8be073747556599dc7b14e80`), porcelain clean before and after every
  run (logs held outside the tree, in `evidence/8.4/`). **Delivery SHA
  `d47d57cfe79d69348a52c88c92a43b368ff15d9d`** (parent `dddd822`, the Scribe WO-8.4 commit
  = the branch's cut point and current `origin/main`); `d47d57c..19e603d` =
  `docs/agents/WO-8.4.md` alone (the section-14 delivery note), docs-only - `src/`, `tests/`
  and `Cargo.*` at the tip are byte-identical to the delivery SHA, so gating the tip fully
  characterizes the delivery (same docs-only note pattern as 8.0-8.3).
- **Baseline:** 414 -> **421** (+7 = T52-T58: 2 lib + 1 bin + 4 loopback; `asset_fixtures`
  11 and `e2e_loopback` 25 untouched). Matrix cell copy target: **421/421**, taken from
  `test.log` only. Ignored 5 -> **6** (+1 `live_acceptance`; 1 lib operator + 4
  `live_integration` preserved).
- **Environment:** `env.txt` (rustc / cargo / OS / PowerShell / `RUSTFLAGS=-D warnings` on
  gate 1).

| # | Gate | Command | Exit | Log |
|---:|---|---|---:|---|
| 1 | check | `RUSTFLAGS=-D warnings cargo check --workspace --all-targets` | 0 | `check.log` - 0 warnings |
| 2 | clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `clippy.log` - 0 warnings |
| 3 | fmt | `cargo fmt --all --check` | 0 | `fmt.log` - 0 bytes (0 diffs) |
| 4 | doc | `cargo doc --workspace --no-deps` | 0 | `doc.log` - 0 warnings |
| 5 | test | `cargo test --workspace` | 0 | `test.log` - **421 passed, 0 failed, 6 ignored** |

**Both gate runs green:** run 1 (pre-probe) above, run 2 (post-probe, restored pristine
tree) in `postprobe/` - identical exits `0/0/0/0/0`, identical **421/421**, 6 ignored,
porcelain CLEAN before and after (`postprobe/gate-exits.txt`).

**Test composition (from `test.log`, identical both runs):** 373 lib (1 ignored) +
1 `goonzu_client` bin + 11 `asset_fixtures` + 25 `e2e_loopback` + 11 `input_loopback` =
**421 passed**; `live_acceptance` 1 ignored + `live_integration` 4 ignored + lib operator
1 ignored = **6 ignored**; 0 doc-tests. T52-T58 all green (all 7 named in `test.log`).

**Cross-checks (all match section 9):** `cargo test --lib graphics::window` = **6 passed
+ 1 ignored** (T40-T42, T51 + new T56, T58); `cargo test --test input_loopback` =
**11/11** (T47-T50 + new T52-T55 + `frames::tests` x3, D22's disclosed duplication);
`cargo test --bin goonzu_client` = **1/1** (new T57); `cargo test --lib input::tests` =
**43/43** unchanged (`src/input/**` zero-touch).

## Directive item 4 - section 14 Item 7 (tracing subscriber) verification

The delivery note's Item 7 discloses a parallel-test failure mode (T53 capture emptied by
callsite-interest poisoning) and its fix (a process-global DEBUG subscriber writing to
`std::io::sink`, installed before the thread-scoped D30 capture). Verified **structurally
and behaviorally**:

1. **Fix present in both capture tests** - `set_global_default(... .with_writer(io::sink)
   .finish())` immediately before the scoped `set_default`, with the order-rationale
   comment, at `tests/input_loopback.rs:767` and `tests/live_acceptance.rs:160`.
2. **Both mechanism citations exact** - `tracing-core subscriber.rs:676` =
   `NoSubscriber::register_callsite -> Interest::never()` (tracing-core 0.1.36, byte-line
   verified); `driver.rs:850` = the frozen `debug!(opcode = ..., ?outcome, "in-world
   frame dispatched")` record in `src/client/driver.rs` (frozen, empty diff).
3. **Behavioral, with the guard:** **7/7 full parallel gate runs green** (gate runs 1+2 +
   5 campaign runs, `item7-parallel-runs.txt`) - 421/421 each, T53 `... ok` in every run
   (the note's "7/7 parallel runs green after" reproduced).
4. **Behavioral, without the guard (ad-hoc reverse check, disclosed in
   `mutation-probes.md`):** removing just T53's global-sink line made the parallel
   `input_loopback` binary **3/3 RED** (exit 101, T53 FAILED) while the **isolated** T53
   run still **PASSED** - reproducing both halves of the disclosure ("2/2 red before",
   "isolated runs passed") and the exact observed signature (`E43: the frozen driver
   dispatch record must appear (driver.rs:850); got:` with an empty capture). Restored,
   porcelain clean.

**Item 7 verdict: VERIFIED** - the disclosure is accurate, the fix is load-bearing, and
the disclosed parallel-run stability holds on this host.

## DoD (section 13) items 1-8, audited

1. Five gates exit-0 at 421 in both runs, fmt 0 diffs, doc 0 warnings, 6 ignored, porcelain
   clean before/after every run - **hold**. *Off-Windows execution caveat (disclosed):* this
   host is Windows-only, so the "doc 0 warnings off-Windows" half of DoD 1 cannot be
   executed here; instead the D29 boundary was **structurally audited** - `main.rs` carries
   the `#[cfg(windows)]`/`#[cfg(not(windows))]` pair (lines 25/30/32/40/83), the predicate
   is `#[cfg(any(windows, test))]` (line 198), the Layer + trigger/visitor are
   `#[cfg(windows)]` (210-233), the test mod is `#[cfg(test)]` (257), no intra-doc link
   crosses the boundary (delivery disclosure 3), and `cargo doc --no-deps` exits 0 with 0
   warnings on this host. The structural audit matches the Implementer's own disclosure.
2. `src/input/**` diff **empty** (`src-input-untouched.txt`: name-only/stat/name-status all
   empty; blob-identical `tests.rs`/`feed.rs`/`mod.rs`; 43/43 green) and
   `window/tests.rs`/`input_loopback.rs` **append-only** (`tests-unchanged.txt`: one hunk
   each - `@@ -218,0 +219,170 @@` and `@@ -557,0 +558,579 @@`, **zero deletion lines**, base
   file byte-identical **prefix** of the delivered file; frozen `e2e_loopback`/
   `asset_fixtures`/`live_integration` empty); 5 pre-existing ignored preserved, 6 total,
   the new `live_acceptance` named - **hold**.
3. D24-D30: D24 -> T57 + probes 3-4 (Win32 posting half operator-only, P4-C4 - the arm
   never called in gates); D25/D26 -> `live/windowed-run.md` **not-run disclosure**
   (`live/README.md`, CONTRIBUTING.md:28-46; non-blocking per section 11); D27 -> T58 +
   probe 2; D28 -> vehicle `#[ignore]`d (1 ignored seen in both runs) + not-run
   disclosure; D29 -> **0 new env vars/flags** in the diff (grep), off-Windows doc green
   (doc gate 0 warnings on this host; the `#[cfg(any(windows, test))]` predicate and
   `#[cfg(windows)]` Layer audited); D30 -> T53 capture assertion green (7/7) - **hold**
   (D25/D26 operator record pending a reachable service + display, explicitly non-blocking).
4. Probes **4/4**, independently re-run: each one semantic break -> named test FAILED at
   **exit 101** with the tabled assertion verbatim (T52 `E30/§2.4 ... (480, 340)` left
   `{400,300}`; T58 `E47` center `(100,100)` vs `(32,32)`; T57 `E48 heartbeat sent...`;
   T57 `E48 outcome "Chat(ChatMessage ...)"...`), pristine restore + porcelain CLEAN after
   each, then the full gate set re-ran green at 421 (run 2) - **hold**
   (`mutation-probes.md`).
5. Write guard: `dddd822..d47d57c` = exactly the five §6 code files (`window.rs`,
   `window/tests.rs`, `main.rs`, `input_loopback.rs`, NEW `live_acceptance.rs`); the whole
   forbidden set empty (`src/client/**`, `src/game/**`, **`src/input/**`**, `src/lib.rs`,
   frozen tests, `Cargo.*`, matrix, contributing, README, legacy tree); `window.rs` = 14
   hunks all seam-lines (module-doc, 2 extended `use` lines, `PostMessageW` decl,
   `WM_CLOSE`+`WM_ENTER_WORLD` consts, the `WM_ENTER_WORLD` arm, the three seam fns, the
   `run_demo` delegate + ready plumb), render/present/tick/pump/scene hunks **zero**, only
   4 deletion lines (2 `use` joins + 2 `run_demo` delegate lines), `_ => DefWindowProcA`
   **byte-identical**, the 8.3 input arm untouched (no changed line reaches it);
   `src/main.rs` exit match keeps messages + codes **0/1/2** (Ok fallthrough -> 0,
   `Err(Config)` -> 2 "driver configuration is invalid", `Err` -> 1 "session failed"),
   reached **after** `close_demo()` + `join` (main.rs:162-166) - **hold**
   (`write-guard.txt`).
6. F-07: E42-E48 cited across the delivered tests (21 + 9 + 7 citation lines; full row
   census E42-E48 complete), each E-row tracing to a legacy `file:line` (`driver.rs:850`),
   a frozen §8.3 golden, or §2.4's hand arithmetic - never read back from the code under
   test (spot checks: T52's `E42/§2.4 G-row`, T58's E47 hand-rect comment, T57's
   §2.4-4 hand table) - **hold**.
7. Zero-queue audit: the literal section-11 pattern over `src/input/` returns exactly **1**
   match - `src/input/tests.rs:1541`, the T46 doc prose asserting absence of machinery.
   This line is **byte-identical to the base** (this step does not touch `src/input/**` at
   all), so it is the already-adjudicated **F-8.3-01 carry-over, not a new finding**;
   machinery subset = 0, code-only scan = 0. Structural claim holds: the composed chain is
   `decode -> feed -> send` with the frozen `commands::channel()` (main.rs:125) and no
   queue type or buffer added; T56's 100-event `<16 ms` batch green with its margin
   documented; T10/T51 green unchanged - **hold** (`zero-queue-grep.txt`).
8. No new crates (`Cargo.*` empty), no new opcodes, no wire-byte changes, no
   `src/input`/`src/client`/`src/game` changes, no Win32 object in any gate test (P4-C4 -
   T57 strings-only, `post_enter_world` never called from tests; deep scan over all four
   delivered files = 0 `unsafe`/`DEMO_STATE`/`CreateWindow`/`GetDC`/`BeginPaint`/`PostMessage`
   occurrences, the single `DefWindowProc` hit being a comment in `window/tests.rs:114`),
   no live run used as a gate (vehicle `#[ignore]`d; `live/README.md` records
   the not-run disclosure, `CONTRIBUTING.md:28-46`), the **+7 arithmetic** (373 + 1 + 11 +
   25 + 11) and **both D30 `Capture` copies** (`tests/input_loopback.rs` T53 and
   `tests/live_acceptance.rs`, each with its own `Capture`/`debug_capture_subscriber`;
   `tests/e2e_loopback.rs`'s frozen one deliberately not reused, D22) disclosed here -
   **hold**.

## Disclosed process notes (Verifier lane)

1. **Probes run as plain `cargo test`** (section 12 wording), mirroring disclosure p1
   (probe 1 orphans the border consts; default flags); full `-D warnings` gate set re-ran
   after every restore.
2. **The Item-7 reverse check is an ad-hoc verifier mutation, not a section-12 probe** -
   it temporarily removed T53's global-sink guard three times (each restored, porcelain
   clean). No gate result was produced from a mutated tree. Its output strengthens rather
   than contradicts the delivery.
3. **No `F-8.4-NN` findings raised.** The zero-queue literal-pattern hit is F-8.3-01
   carried over unchanged from the frozen base (recorded in `zero-queue-grep.txt`, never
   edited - P3-6).
4. **`evidence/8.4/live/`** carries the section-11 **NOT RUN** disclosure (no reachable
   game service); per section 11's closing clause this does not affect PASSED.

## Matrix / bookkeeping

- `TASK_MATRIX.md` untouched - row 8.4 stays `PENDING`; this verdict and `evidence/8.4/`
  are produced for Custodian acceptance. No matrix flip, commit or push is authorized by
  the verification directive (8.2/8.3 precedent: the `docs(qc)` sign-off commit comes
  after acceptance).
- Matrix `Tests Baseline` copy target: **421/421** (from `test.log` only).
- Delivery-note cross-check: the Implementer's section-14 note matches every measured
  number here (421, 6 ignored, 6+1, 11/11, 1/1, 43/43, probes 4/4 with verbatim
  assertions, 14-hunk write guard, append-only proofs) - no discrepancy found.
