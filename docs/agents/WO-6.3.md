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
### Adversarial findings - Adversary
### Verdict - Verifier

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
