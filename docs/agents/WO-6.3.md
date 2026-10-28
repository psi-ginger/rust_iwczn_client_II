# Work Order - Step 6.3: Sustained-Session Soak & Robustness

## 1. Provenance, lane hats, baseline

Filed 2026-10-28 by the Verifier session (`psi-ginger`) at the Custodian's
direct instruction ("set up the next task for the other agent"). No Scribe
session was active; precedent `f74bf68` (WO-6.2) was likewise filed by a
non-Scribe actor with this disclosure. Scope and acceptance below are
verbatim-sourced from `CONTRIBUTING_PHASE2.md:126-138` (Scribe-owned; this
work order does not amend it).

- **Baseline:** `main` @ `9ba6226`, **220/220** (211 lib + 9 e2e;
  `evidence/live-fix/test.log`, re-run green post-merge — merge-skew clean).
- **Branch:** `step/6.3-soak`, cut from `main` @ `9ba6226`.
- **Lanes:** Implementer -> Verifier + **Adversary (mandatory)** — the row
  claims "no leak" and "soak" (`CONTRIBUTING_PHASE2.md:169-171`).
- **`src/**` is READ-ONLY in this step** (`CONTRIBUTING_PHASE2.md:207`):
  a defect found here is a finding `F-6.3-NN`, never an edit.

## 2. Scope (verbatim authority: `CONTRIBUTING_PHASE2.md:126-138`)

- **(a) scaled-cadence hermetic soak, as a gate** — the full flow held
  in-world while the doubles stream many dispatcher frames and accelerated
  heartbeats, asserting state consistency and zero task leaks over the run.
- **(b) robustness, as a gate** — malformed/oversize frames mid-stream, an
  abrupt disconnect mid-frame, and Ctrl+C mid-session, each terminating
  cleanly with the documented error/partial state and zero orphaned tasks.
- **(c) >=2-minute live soak as evidence** (`#[ignore]`d or Verifier-run;
  endpoints from env) — recorded in the matrix row, pass or fail; it never
  blocks `PASSED`.

## 3. Resolutions recorded up front (read before designing)

- **"Accelerated heartbeats" cannot mean the client's outbound ticker.**
  The heartbeat interval is fixed at `HEARTBEAT_INTERVAL` = 10 s (row 5.2,
  a documented port decision) and `src/**` is read-only here, so the client
  cadence cannot be shortened without an out-of-scope src edit. Resolution:
  the **dispatcher-frame stream** is the scaled-cadence dimension (the
  doubles' outbound rate is freely tunable); the soak observes the client's
  real 10 s heartbeats byte-exactly over a bounded window of >=3 intervals
  (~35-45 s wall clock). Making the interval configurable is a legitimate
  `F-6.3-NN` finding for a future row — not this one's edit.
- **Live-soak ownership.** `tests/live_integration.rs` is Verifier-owned
  (`CONTRIBUTING_PHASE2.md:214`); the implementer never touches it. The
  Verifier produces the >=2-minute live soak at verification time (driver
  binary hold; procedure already proven in
  `evidence/live-fix/run_driver_acceptance.ps1`) and records the result.
- **Mid-run task introspection does not exist** in the driver's public
  surface, and inventing it would be a src edit. The falsifiable proxies
  are stated in section 5.2; do not fabricate a stronger observation.

## 4. Non-goals

- No `src/**` edits, no frozen-API changes (`WO-6.1.md` section 4).
- No rewriting the 6.2 conversation or fixtures — additive hooks and
  hand-placed literals only (F-07).
- No live server, tunnel, or network beyond loopback in any gate test.
- No performance benchmarking: this is a robustness soak, not a profiler.

## 5. File plan (ownership: `CONTRIBUTING_PHASE2.md:203-218`)

| File | Role | Writer |
|---|---|---|
| `tests/e2e_loopback.rs` | test root: soak + robustness entry points, session orchestration | Implementer (single writer) |
| `tests/e2e_loopback/soak.rs` (new) | scaled-cadence soak: frame storm, heartbeat observation, consistency + leak assertions | Implementer |
| `tests/e2e_loopback/robustness.rs` (new) | fault scenarios R1-R4 (section 6.4) | Implementer |
| `tests/e2e_loopback/frames.rs` | shared fixture — additive hand-placed literals/builders for malformed frames only | Implementer (single writer) |
| `tests/e2e_loopback/front_double.rs`, `game_double.rs` | additive streaming/fault hooks (storm mode, mid-frame abort); extend, never rewrite the 6.2 conversation | Implementer |
| `tests/live_integration.rs` | Verifier-owned; the >=2-min live soak evidence | Verifier only |
| `src/**` | READ-ONLY | nobody |

## 6. Design

### 6.1 Soak shape (gate)

- Full flow through `driver::run` (frozen WO-6.1 API) to `Stage::InWorld`,
  reusing the 6.2 doubles' conversation unchanged.
- In-world hold: the game double streams **>=1000 dispatcher frames** across
  all five in-world opcodes (6/11/118/200/215) at maximum sustainable rate,
  **including opcode-6 NPC-led batches** — the live shape is NPC-first
  (34 x 68-B `MAKECHARTYPE_NPC` records then PC records; LIVE-01,
  `docs/rca/2026-10-28-live-opcode6-npc-stride.md`, delivery `701963a`).
  This is the regression the soak must never let return: a PC-only storm
  would reproduce the 6.2 hermetic blind spot the live server exposed.
- Heartbeats: **>=3 client heartbeats observed byte-exactly** during the
  storm (`HEARTBEAT_WIRE = [0x05,0x00,0x00,0x64,0x00]`,
  `tests/e2e_loopback/frames.rs`, dual-source `heartbeat/tests.rs`
  `EXPECTED_WIRE`).
- End of hold: double-side FIN -> clean teardown; assert outcome fields
  (`worlds = 1`, `char_unique` stable, `stage = TornDown`).

### 6.2 Consistency and leak assertions (the row's load-bearing claims)

- After the storm the session is still coherent: the doubles' scripted end
  state is asserted through the driver's **observable outcome/log surface**
  — never by calling the client's parser under test to build the
  expectation (F-07).
- Zero orphaned tasks: reuse the 5.3 drain discipline (`TaskTracker`) —
  teardown completes within its documented latency bound (<= one read
  budget past the trigger instant); the watcher and ticker are awaited by
  `run_with_watcher`, not leaked.
- Bounded behavior: storm size must not translate into unbounded pending
  work — the observable proxy is that heartbeat cadence and teardown latency
  at storm end are independent of storm size (state the measured numbers in
  the delivery note).

### 6.3 Heartbeat observation window

Real 10 s cadence, >=3 intervals, while the storm runs concurrently —
roughly 35-45 s wall clock for the soak test. This is the deliberate cost
of the read-only-ticker resolution (section 3); do not shrink it below 3
observed beats, and do not drive `event_loop` directly to fake a shorter
interval (that would bypass `driver::run` and void the "full flow" claim).

### 6.4 Robustness scenarios (each a separate, falsifiable test)

- **R1 malformed frame mid-stream** — usSize < 5, or usSize > the bytes
  actually transmitted: stage-named refusal on the documented error path
  (exit-1 mapping, WO-6.1 section 5), bounded time, no hang.
- **R2 integrity-corrupt frame** — a byte flip after the length header:
  the decoder's link-cipher integrity refusal surfaces as the documented
  stage-named error (pattern: `Codec(Integrity{..})`, seen live in D-1).
- **R3 abrupt disconnect mid-frame** — partial frame then FIN: pins the
  **observed documented** behavior (5.1's EOF partial-frame discard on
  `run`/`next_frame`, or the stage-named error — whichever `src` actually
  implements). If the actual behavior is undocumented or surprising, that
  is `F-6.3-NN`, not a reason to write the test around it.
- **R4 Ctrl+C mid-session** — the injected-future seam, per the 5.3 suite's
  own deliberate choice (`watch_signal`, a console event would signal
  cargo's console group) and the probe-proven fact that the OS-level event
  is undeliverable in this environment (`evidence/live-fix/ctrl_c_investigation.md`,
  `probe_fire.log`). Expect teardown to `Stage::TornDown`, zero orphans.
  The seam is the test — this is not a shortcut, and the live Ctrl+C path
  was separately proven by the LIVE-01 acceptance.

Every scenario asserts all three: bounded termination, stage-named outcome,
zero orphaned tasks.

## 7. Forbidden shortcuts

- Editing `src/**` to make any scenario pass (finding first).
- Shortening/accelerating the heartbeat by editing the ticker, or by
  driving `event_loop` instead of `driver::run`.
- Deriving soak expectations from the client's parsers/encoders (F-07).
- A soak that only counts frames with no state/leak assertions (that is a
  timing test, not a soak).
- Any live server or tunnel dependency inside a gate test.

## 8. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/6.3/`: `head.txt`, `check.log`, `clippy.log`, `fmt.log`,
  `test.log`, `doc.log`, `env.txt`, `SHA256SUMS` — the standing bundle
  standard; the matrix test number is copied from `test.log`.
- **Live soak (>=120 s, Verifier-run):** driver-binary hold against the
  wire-verified tunnels (front `:14195` / game `:27911`, per-session
  addresses from env, never hardcoded), session log showing
  `stage=InWorld` sustained >=2 min with heartbeats on the wire and a
  clean teardown. Recorded in the matrix row as evidence — pass or fail
  never blocks `PASSED`.

## 9. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

Branch `step/6.3-soak`, delivery SHA **`3285d07`** (parent `fc5ffb1`,
baseline `main` @ `9ba6226`). Five files, `src/**` untouched
(`git diff -- src` empty).

**Gates (all exit 0, `CONTRIBUTING_PHASE2.md:238-239` commands):**
`RUSTFLAGS=-D warnings cargo check --all-targets` · `cargo clippy
--all-targets -- -D warnings` · `cargo fmt --check` (0 diffs) ·
`cargo test` · `cargo doc --no-deps` (0 warnings). Test count from the
run's `test.log` line: `test result: ok. 211 passed; 0 failed; 0
ignored` (lib) + `test result: ok. 14 passed; 0 failed; 0 ignored;
finished in 30.15s` (e2e) — **225/225**, e2e 9 → 14 = soak + R1–R4; the
4 `live_integration` tests remain ignored and untouched.

**(a) Soak** (`tests/e2e_loopback/soak.rs`, 1 test): two concurrent
full `driver::run` sessions (one current-thread task, `tokio::join!`)
held in-world while their doubles stream storms of different sizes at a
fixed 100 ms pace; the 10 s heartbeat ticker is untouched, so the frame
stream is the scaled dimension. Every 6-frame cycle leads with the
NPC-led opcode-6 batch (34 × 68-B NPC records, ids 8500..8533, then the
two PCs — LIVE-01's shape), plus 215/200/11/118 and one foreign opcode.
Measured storm-size independence (this run, the `soak measurements:`
line): small target=1000 **sent=1208**, large target=4000
**sent=4228**; both sessions' cadence intervals **(9.9919107 s,
10.0044285 s)** vs **(9.9918991 s, 10.0043954 s)** — inside the fixed
10 s window; **max cross-session same-index heartbeat drift 31.1 µs**;
**dispatches=5446** (client-processed, DEBUG dispatch-log count at
`driver.rs:684` — not writer counts); **npc_led_batches=907**
(≥ frames/6 − 2 asserted); teardown FIN→return **1.2573 ms** (small) /
**1.8432 ms** (large), bound 500 ms. Zero-orphan evidence is
behavioral: the double observes the client's own close (the close phase
of `terminate` ran, `teardown.rs:366`), both terminal records present,
the Err-path drain warning absent, state consistent (TornDown,
`char_unique`/village intact) — falsifiable assertions, not counts
alone.

**(b) Robustness** (`tests/e2e_loopback/robustness.rs`, 4 tests, one
per scenario, each bounded and each asserting a stage-named outcome +
clean aftermath): **R1** `usSize` 3 and 64993 both refused at
`InWorld` carrying the header guard's exact variant
(`SizeBelowHeader{3}` / `SizeTooLarge{64993}`) as the source; **R2**
tag-flipped frame → `CodecError::Integrity` at `InWorld`; **R3**
3-byte partial + FIN → **no error**, clean `Cause::ServerDisconnect`
teardown, the documented behavior pinned (`game/entry.rs:472-476`);
**R4** Ctrl+C mid-session through the injected `watch_signal` seam
(fired after the first byte-exact heartbeat) → `Cause::CtrlC` claimed
by that watcher, `TornDown`. R1–R3 finished in ms (bound 15 s), R4 in
10.12 s (bound 30 s).

**(c)** The ≥2-min live soak is Verifier-owned per §8
(`tests/live_integration.rs` untouched, 4 tests still ignored).

Handoff: Adversary lane for §9 adversarial findings; Verifier lane for
`evidence/6.3/` from a clean clone at `3285d07` and the S4/S1 verdict.

### Adversarial findings - Adversary
### Verdict - Verifier

**2026-10-28: Step 6.3 `PASSED` (Verifier, independent second actor;
dedicated `docs(qc)` commit).** The Adversary section above was not
filled by a separate lane; the Verifier performed the mandatory
adversarial review inline (same practice as rows 6.1/6.2 and the
LIVE-01 fix) and records it here — no blocking findings.

**S1:** verified range `fc5ffb1..a5151ca` (delivery code `3285d07` +
doc-only delivery note `a5151ca`; trees differ only by
`docs/agents/WO-6.3.md` +55) — both commits author `goonzu-rs`; the
signer authored none of the range.

**S4 (clean clone from origin at `a5151ca`, fresh `target/`, porcelain
empty): all five gates exit 0** — check/clippy 0 warnings, `fmt.log`
0 bytes, `doc.log` 0 warnings, **225/225** = 211 lib + 14 e2e (9 → 14 =
soak + R1–R4) with the 4 live tests still `#[ignore]`d; every number
copied from `evidence/6.3/test.log`. `git diff fc5ffb1..a5151ca --
src/` empty in the clone: **`src/**` untouched claim TRUE**.

**Adversarial review — no blocking findings.** `soak.rs`: dispatch
counts are client-processed (DEBUG dispatch records, not writer
counts); the NPC-led batch floor is derived from actual sent frames;
zero-orphan evidence is behavioral (double observes client EOF ⇒ the
`terminate` close phase ran; no Err-path drain warning); cadence window
9.5–11.0 s, drift ≤1 s, teardown ≤500 ms; two concurrent sessions 1000
vs 4000 frames. `game_double.rs`: storm paces at a fixed 100 ms tick
with split read/write halves keeping both ciphers lockstep;
`assert_heartbeat` byte-exact on every observed frame; R2 flips the
last wire byte (tag-side, `cipher.rs:279-295`); R3 writes 3 wire bytes
then FIN; R4 notifies the seam only after the first heartbeat;
`drain_to_eof` asserts heartbeats byte-exactly through teardown; the
6.2 default (`Tail::HeartbeatThenFin`) is unchanged. `robustness.rs`:
R1 asserts the header guard's exact variants (`SizeBelowHeader{3}` /
`SizeTooLarge{64993}`) at `InWorld` on both sizes; R2 asserts
`CodecError::Integrity`; R3 asserts **no error** + `TornDown` +
`cause=Some(ServerDisconnect)` (the documented clean-teardown discard,
`game/entry.rs:472-476`); R4 joins its own injected watcher and asserts
`Some(Cause::CtrlC)`; all four through `assert_clean_aftermath`
(bounded, front leg complete, client close observed, no drain warning).
`frames.rs`: both malformed frames are hand-placed raw byte literals
(“never ciphered, never built by any encoder”); `OP_UNHANDLED_IN_WORLD`
= 9999; the NPC-led body reuses the hand-placed record helpers with the
LIVE-01 id span 8500..=8533. F-07 holds — no expectation anywhere is
derived from the client's parsers or encoders.

**Delivery-note claims vs the Verifier's independent run:**
deterministic counts reproduce **exactly** — `dispatches=5446`,
`npc_led_batches=907`, `sent=1208/4228`, `heartbeats=3`
(`evidence/6.3/soak_nocapture.log`). The timing figures are real-clock
and run-variable by nature; this run: cadence (9.9951 s, 10.0098 s)
both sessions, drift 22.3 µs, teardown 2.53/2.98 ms — all inside the
asserted bounds (10-s window, ≤1 s drift, ≤500 ms teardown). The
note's own-run decimals (9.9919/10.0044 s, 31.1 µs, 1.26/1.84 ms) are
consistent with that variance. The `soak measurements:` line is a
`println!` and is harness-captured under plain `cargo test`; evidence
requires `--nocapture` (recorded in the bundle) — noted, not a defect.

**(c) live soak — PASS (evidence, per §8 never a gate):** the S4-clone
binary against the wire-verified tunnels via the capture relay (FIN
watchdog +130 s): **in-world 129.94 s ≥ 2 min**, **12 wire heartbeats**
on the c2s capture (53-B login + 9-B select + 12 × 5-B = 122 B),
`stage=TornDown worlds=1 char_unique=16 cause=Some(ServerDisconnect)`,
**exit 0** (`evidence/6.3/live_soak.txt`, `soak63_live.log`, `cap/`).

**Carried non-blocking finding L-1** (from the LIVE-01 fix
verification): `frames.rs` `make_npc_record` cites rec[00] “y 338”; the
capture's `w12=0x00027852` gives uiY = **82** — comment-only, no
assertion depends on it; still owed by the implementer lane, may ride
with any future delivery.

**Evidence:** `evidence/6.3/` (14 files, SHA256SUMS-verified) —
`head/env/gates/live_soak` + the five gate logs + `soak_nocapture.log`
+ `soak63_live.log` + `run_live_soak.ps1` (sanitized, credentials from
operator env) + raw `cap/c2s.bin`/`cap/s2c.bin`. Test-count history:
220 @ `9ba6226` → **225** @ `a5151ca`.

**Verifier self-defect, caught and fixed forward in this commit.** The
earlier matrix-cleanup commit `fc5ffb1` (WO filing + note archive)
corrupted **every non-ASCII character in both files it wrote** — it was
built from PowerShell's OEM-codepage decoding of `git show` output, so
`→`/`§`/`—`/`×` became mojibake. The Verifier's original “0/49 verbatim,
31/31 rows byte-identical” check was **false**: it compared identically
corrupted strings on both sides. Caught here by a byte-level hexdump
during the 6.3 review; fixed forward by re-splicing `TASK_MATRIX.md`
and `docs/audit/2026-10-28-matrix-note-archive.md` from a byte-safe
`cmd /c` redirection of `git show 9ba6226:TASK_MATRIX.md`, with
byte-level re-verification (0/49 note-line mismatches, 0/26 preserved
matrix-line mismatches, proper-character census reconciled: 22 arrows /
7 § / 2 × / 48 —). `fc5ffb1` is cited in this delivery's range, so S7
bars rewriting it — the fix is forward-only, recorded in the archive
postscript and this block. No delivered test code was affected: all six
delivery files scan clean (0 mojibake triples); the `µs` in the soak
output is Rust's `Duration` `{:?}` formatting, not a source byte.

## 10. Definition of Done

1. Soak + robustness tests exist; all five gates green; the test count is
   recorded in the matrix from a `test.log` line (no handwritten numbers).
2. Soak: >=1000 mixed dispatcher frames (incl. NPC-led opcode-6 batches)
   during >=3 observed byte-exact client heartbeats; state-consistency and
   zero-orphan assertions present and falsifiable; measured
   storm-size-independence numbers in the delivery note.
3. R1-R4 each terminate within a bounded time with a stage-named outcome
   and zero orphaned tasks.
4. `src/**` untouched (`git diff` empty over `src/`).
5. Verifier produces `evidence/6.3/` from a clean clone and records the
   >=2-min live soak as evidence; Verifier signs the row (S1: the signer
   authored none of the verified range).
