# GoonZu Client Rust — Phase 2 Governance (Step 6 onward)

> **Scope of this document.** `CONTRIBUTING.md` governs Steps 1.1–5.4 (the Phase II
> micro-step catalog — a closed, audited record of delivered work). This document governs
> **Step 6.1 onward**. It was established 2026-10-27 by the Acting QC Lead when the Step 6
> workstream (end-to-end orchestration) opened, so that Phase II stays a clean record and
> Step 6 gets its own definition-before-implementation discipline.

---

## Inherited verbatim from `CONTRIBUTING.md`

The following apply **unchanged** to every Phase 2 step. `CONTRIBUTING.md` is canonical; this
list is the pointer, not a fork:

1. **Core philosophy** — zero-evasion, precision micro-steps; zero tolerance for stubbing,
   fake mocks, or unverified loops.
2. **The five mandatory gates** — `cargo check --all-targets` (`RUSTFLAGS=-D warnings`),
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (stock rustfmt defaults,
   no `rustfmt.toml` in this repo), `cargo test`, `cargo doc --no-deps` (0 warnings). A gate
   that is not run, or cannot run, is **not** a pass.
3. **Live integration is evidence, never a gate** — `tests/live_integration.rs` (and any new
   live/`#[ignore]`d test) proves reachability-dependent facts; no step is ever blocked on a
   server being up.
4. **Execution invariants 1–3** — never jump steps; never hide failures (log the raw hex
   diff, fabricate nothing); never stub wire parsers (every parser validates full payload
   lengths, refuses instead of truncating).
5. **The dual-source rule** — every behaviour claim is cited in both `GoonZu_Source/` and the
   server repo (`psi-ginger/rust_Iwcgz_server`), with accurate `file:line` on both sides.
6. **Haru/CAPA audit standard** — no speculative assumptions, no unverified mocks, no
   premature state mutations; tests must be falsifiable; an expectation must never come from
   the unit under test (**F-07**).

---

## Phase 2 gate-run protocol (operational notes)

- `RUSTFLAGS=-D warnings` is set for `cargo check` **only**. `cargo test` and `cargo doc`
  run in the default profile: setting `RUSTFLAGS` for them forces a cold dependency rebuild
  which fails in this environment on a missing `dlltool.exe` — an environmental artifact,
  not a code defect.
- PowerShell: never append `2>&1` to gate commands (it corrupts `$LASTEXITCODE` and raises
  spurious `NativeCommandError`); capture output to a temp file and check `$LASTEXITCODE`.
  Commit messages go through a temp file + `git commit -F` (no heredocs on PowerShell).
- **rustfmt leaf-file rule:** when formatting or checking a single file, name **leaf** files
  only — never `mod.rs` (rustfmt would recurse the entire module tree from the root and can
  rewrite unrelated history). `cargo fmt --check` on the whole repo remains the gate itself.
- Test counts are recorded in `TASK_MATRIX.md` at sign-off; the baseline moves only with a
  green `cargo test` on the delivery commit.

---

## Mode policy: Local Mode is the gate environment; Ngrok is evidence

- **Local Mode** (`127.0.0.1` front + game services): the environment in which every gate
  runs and in which a step may be marked `PASSED`.
- **Ngrok tunnel**: per-session external addresses. A run against a tunnel is **evidence** —
  recorded in the matrix row, never required for `PASSED`. Tunnel addresses are per-session
  and must never be hardcoded (same rule `CONTRIBUTING.md` states for
  `tests/live_integration.rs`).

---

## Hermetic wire-double policy (F-07 at the end-to-end layer)

Steps 6.1–6.3 are driven hermetically by in-process TCP **wire doubles** (fake front/game
services bound to loopback). To keep F-07 (the expectation must never be the unit under
test) sound at this layer, the harness obeys:

- Every server→client frame the doubles send is hand-placed from **bytes** whose layout
  cites the legacy/server authority — the same dual-sourced fixtures the per-module test
  suites already use, lifted as independent byte sources, **not** re-derived from the client
  parsers.
- Every client→server frame the harness asserts is checked **byte-for-byte** against an
  independent expectation (size/opcode/payload constants with their own citations), not
  against whatever the client's own encoder produced.
- **Mutation tests** prove the harness would notice: at least one flipped byte per flow
  stage (front, game login, world entry, in-world) must make the client refuse or diverge,
  and the harness must observe that.

The wire doubles prove the protocol conversation — not reachability, and not cipher-key
agreement with a real deployment. The latter remains live-evidence territory (live soaks,
`tests/live_integration.rs`), exactly as in Phase II.

---

## Step 6 — End-to-End Orchestration & Driver Integration

Source of scope: `README.md:115-120`. All per-module machinery exists (front flow 3.1–3.3,
game flow 4.1–4.7, event loop/heartbeat/teardown 5.1–5.3, dispatcher 5.4), but
`src/main.rs` is still the Step 1.1 scaffold: **nothing has ever assembled the full flow in
one process**. Step 6 closes that gap in three sub-steps, in order.

### Step 6.1: End-to-End Client Driver

- **Path:** `src/client/driver.rs` (new library module) + `src/main.rs` (replaces the 1.1
  scaffold with thin env-config wiring).
- **Scope:** one orchestrator running the complete session, in order: front connect →
  opcode-30000 secretkey handshake → worldlist → front login (`auth_key`) → game connect →
  opcode-1 handshake → opcode-2 game login → charlist (4.4) → select char (4.5) →
  SERVERINFO/LOGIN (4.6) → village blueprint when applicable (4.7) → in-world: the 5.1
  event loop pumping the 5.4 dispatcher with the 5.2 heartbeat ticking → teardown (5.3) on
  Ctrl+C **and** on server disconnect. Configuration (endpoints, credentials, optional
  village ref) comes from the environment — no addresses hardcoded. The orchestration logic
  lives in the **library** (`driver::run(...)` returning structured results) so the 6.2
  harness can drive it; `main.rs` is only config + logging + runtime + exit-code mapping.
- **Acceptance:** all five gates green; `driver::run` is exercised hermetically by Step
  6.2's harness through the full flow to in-world and back to clean teardown; every state
  transition is observable in the returned/logged state (no silent success); zero orphaned
  tasks at exit; the map-data boundary from 4.7 is preserved (the village ref is a caller
  input — the driver never fabricates a map-data lookup).

### Step 6.2: Hermetic E2E Loopback Harness

- **Path:** `tests/e2e_loopback.rs` (new integration test).
- **Scope:** the wire doubles described above — an in-process fake front service and fake
  game service speaking the full conversation byte-exactly and driving `driver::run` end to
  end, including in-world dispatcher frames (6/11/118/200/215) and heartbeat observation,
  then a server-side close that must surface as a clean teardown.
- **Acceptance:** the full-flow test passes with no network beyond loopback; byte-exact
  assertions cover every server→client frame sent and every client→server frame received in
  the flow; mutation tests prove at least one detection per stage (front, game login, world
  entry, in-world); the harness never calls a parser/encoder under test to build its
  expectations (F-07); all five gates green.

### Step 6.3: Sustained-Session Soak & Robustness

- **Path:** `tests/e2e_loopback.rs` (extended) + `tests/live_integration.rs` (evidence only).
- **Scope:** (a) a **scaled-cadence hermetic soak** as a gate — the full flow held in-world
  while the doubles stream many dispatcher frames and accelerated heartbeats, asserting
  state consistency and zero task leaks over the run; (b) **robustness** — malformed/oversize
  frames mid-stream, an abrupt disconnect mid-frame, and Ctrl+C mid-session, each terminating
  cleanly with the documented error/partial state and zero orphaned tasks; (c) a
  **≥2-minute live soak** against a reachable server as evidence (`#[ignore]`d, endpoints
  from env).
- **Acceptance:** all five gates green including the hermetic soak and robustness tests; the
  live soak result is recorded in the matrix row as evidence — pass or fail, it never blocks
  `PASSED`.

---

## Multi-agent execution model (Phase 2)

> **Provenance.** Proposed 2026-10-27 by the planning lane while Step 6 was opened. It takes
> effect on merge to `main`; until the QC Lead records that, this section is a proposal and
> the existing rules stand unchanged.

### Why this section exists

Phase 2 runs on several independent agents, each in its own session, with **no shared
memory**. The Phase I case study (`rust_Iwcgz_client`, `docs/rca/`) records what fails in that
setting: not weak models, but **self-certification** — one actor wrote the code, produced the
numbers, and declared the row green. Rows 4.5–4.7 and 5.4 closed under a disclosed one-actor
arrangement (Acting QC Lead); that is recorded as a reconciliation, **not** as the pattern.

Everything below serves one property: *the agent that writes `PASSED` must never be the agent
that wrote the code, the tests, or the numbers.* Throughput is negotiable; independence is not.

### The five lanes

| Lane | Commit trailer | Owns | Must never |
| :--- | :--- | :--- | :--- |
| **Implementer** | `Lane: implementer` | Source + tests for the one `PENDING` row; delivery note | Edit status, write `PASSED`, sign its own row, generate its own gate evidence |
| **Verifier (QC)** | `Lane: verifier` | Gate re-runs from a clean clone, evidence bundle, status transitions, findings dispositions | Write feature code for a row it signs |
| **Adversary** | `Lane: adversary` | Findings `F-<step>-NN`, mutation runs, repro scripts | Fix the code, sign the row, downgrade its own finding |
| **Scribe** | `Lane: scribe` | Work orders, step definitions, matrix structure, handoff ledger | Write status or code |
| **Custodian** | `Lane: custodian` | Branches, merges to `main`, repo hygiene, `.gitignore`, gate scripts, evidence archiving | Feature code, status |

Minimum team per step: **Implementer + Verifier**. Add **Adversary** whenever a row claims
"byte-exact", "no leak", or "soak". Add **Scribe** for any new step definition. Add
**Custodian** whenever the merge to `main` is not a fast-forward.

### Separation-of-duty rules (hard)

- **S1 — author ≠ signer.** No row may reach `PASSED` in a commit whose author also authored
  code in that row's verified range. There is no exception; with one actor available the row
  stays `QC_REVIEW` until a second actor runs the gates.
- **S2 — finder ≠ fixer ≠ signer.** Three different lanes, or the row is not independent.
- **S3 — status is written only by the Verifier**, in a dedicated `docs(qc)` commit.
- **S4 — gates run from a clean clone** at the delivery SHA with a fresh `target/`, never from
  the implementer's working tree.
- **S5 — implementer prose is not evidence.** Only command output and file content count.
- **S6 — a finding closes only on the Verifier's re-run**, never on the fixer's assertion.
- **S7 — evidence is bound to an immutable SHA.** Never force-push or rewrite a branch whose
  SHAs are cited in `evidence/` or in the matrix.

### Definition of Ready (before any code is written)

A row may start only when all six hold:

1. Its scope and acceptance exist in this document.
2. Its work order exists at `docs/agents/WO-<step>.md`.
3. A branch `step/<step>-<slug>` is cut from current `main`.
4. The baseline (SHA + test count) is recorded in the work order.
5. No other lane holds write access to the files in its plan.
6. All five gates can run in the environment (if one cannot, the row cannot start).

### Concurrency: serialization points and file ownership

Steps are **serial** (6.1 → 6.2 → 6.3): 6.2's harness calls `driver::run`, so 6.1's API must
be frozen first. Inside a step, parallelism is allowed only across disjoint files.

| Step | May run in parallel | Must stay serial |
| :--- | :--- | :--- |
| 6.1 | Scribe files the work order; Custodian cuts the branch and a gate script | `driver.rs` and `main.rs` are one hand (`main` depends on the driver signature); API freeze before 6.2 starts |
| 6.2 | Front double ∥ game double in separate files; Adversary drafts the mutation plan from the work order | Shared fixture module (single writer); harness root file (single writer); any `mod.rs` edit |
| 6.3 | Soak test ∥ robustness tests in separate files; Adversary builds fault injection | `src/**` is **read-only** here — a defect found in this step is a finding, not an edit |

| Path | Writer | Rule |
| :--- | :--- | :--- |
| `src/lib.rs`, `src/**/mod.rs`, `Cargo.toml` | Custodian | serialization point: announce, then one writer per window |
| `src/client/driver.rs`, `src/main.rs` | Implementer | single writer, this step only |
| `tests/e2e_loopback.rs` + `tests/e2e_loopback/**` | Implementer (6.2/6.3) | split by double, never the same file twice |
| `tests/live_integration.rs` | Verifier | evidence-only additions |
| `TASK_MATRIX.md` | Verifier (status) / Scribe (structure) | never by an implementer |
| `CONTRIBUTING*.md` | Scribe | definitions land before code |
| `docs/agents/**` | Scribe (work orders) / each lane (own block) | append-only per lane |
| `evidence/**` | Verifier | append-only, SHA-bound |

### Handoff protocol (agents share no memory)

Chat is not a channel: the next agent sees only the repository. Therefore:

1. **Work order first** — `docs/agents/WO-<step>.md` exists before any code (Scribe).
2. **Blocks, not rewrites** — Implementer appends `## Delivery note`; Adversary appends
   `## Adversarial findings`; Verifier appends `## Verdict`. Nobody edits another lane's block;
   corrections are amendments written by the lane that owns the block.
3. **Handoff is complete** only when the branch is pushed, `git status --porcelain` is empty,
   and the block cites the SHA that the next lane will clone.
4. **A claim that is not in the work order file does not exist** — not for the Verifier, not
   for the next implementer, not for the user.

### Evidence bundle (the only admissible proof)

`evidence/<step>/` per row: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
`doc.log`, `env.txt`, `SHA256SUMS`.

- `head.txt` records `git rev-parse HEAD`, the clone path, and an empty `git status --porcelain`.
- `test.log` contains the literal `test result:` line the matrix number is copied from.
- `env.txt` records rustc/cargo/rustfmt versions, UTC timestamp, OS.
- **No number is typed by hand.** A number without a log line is not evidence. Screenshots and
  pasted summaries are not evidence. A gate that could not run is recorded as *not run*, which
  is not a pass.

### Branch, commit and merge

- Branch: `step/<step>-<slug>` off `main`.
- Commit trailer block:

  ```text
  Step: 6.1
  Lane: implementer
  Work-order: docs/agents/WO-6.1.md
  ```

- Merge to `main` is the **Custodian's** act, only after a `PASSED` verdict: a **fast-forward**
  of `main` to the branch tip (no merge commit, no squash — full lane history preserved), with
  the verdict's evidence path quoted in a short `docs(qc)` completion note committed on `main`
  immediately after the advance.
- After merge the Verifier re-runs `cargo fmt --check` and `cargo test` on `main`
  (merge-skew check) and records the new baseline.

### Cadence, escalation, stop conditions

- One step per delivery window; no lane opens the next step until the previous row is `PASSED`.
- **Stop and escalate** if: a gate cannot run for environmental reasons (the row does not pass;
  say why); Implementer and Verifier disagree twice on the same point; or a fabricated number,
  stub, or simulated green is suspected.
- **Fabrication is never local.** If one is confirmed: revert the row to `PENDING`, file the
  finding, and **re-audit the preceding row** — a fabricator's earlier evidence is no longer
  trustworthy (blast radius, per the Phase I case study).
- A repeated gate failure (twice) escalates to an RCA under `docs/rca/` before any fix.

### RACI — Step 6

`R` responsible · `A` accountable/sign-off · `C` consulted · `I` informed

| Row | Implementer | Verifier | Adversary | Scribe | Custodian |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 6.1 Driver | R | A | I | C | C |
| 6.2 Hermetic E2E harness | R | A | R | C | C |
| 6.3 Soak & robustness | R | A | R | C | C |

### Anti-pattern → the control that blocks it

| Phase I pattern (from the case study) | Control here |
| :--- | :--- |
| Mock green: unit tests over mock buffers declared as feature proof | `S4` clean-clone gates + 6.2's byte-exact harness is the only full-flow proof |
| Fabricated telemetry (hardcoded CSV rows) | Evidence bundle; every matrix number copied from a log line |
| Protocol inversion / silent desync | 6.2 mutation tests: one flipped byte per stage must be detected |
| Self-signed `PASSED` | `S1`, `S2`, `S3` |
| Retroactive step definitions as habit | Definition of Ready item 1–2; retro-definition explicitly closed as one-time |
| Untestable acceptance ("server logs record it") | Acceptance must name a command whose output is recorded |

### Appendix — lane to capability profile (environment-specific, advisory)

Binding happens per dispatch; a name grants no authority, the lane does. Repository language is
English only: work orders, commits, evidence, log lines and code comments are written in
English even when the dispatch references a persona defined elsewhere.

| Lane | Capability profile | Notes |
| :--- | :--- | :--- |
| Implementer | write + exec | Owns source and tests for exactly one row |
| Verifier | read source, exec for gates | Must run in an independent session from a clean clone |
| Adversary | read + exec | Writes repro scripts under `docs/agents/repro/`; never edits `src/` |
| Scribe | write, documentation only | Work orders, definitions, matrix structure; no code, no status |
| Custodian | write + exec, repository scope | Branches, merges, `.gitignore`, gate scripts |
| Design/ADR review | read-only | Architecture and risk opinions; no delivery authority |
| Soak reporting (6.3 only) | write, documentation only | Charts derive from `evidence/6.3/` logs; never from recollection |

---

## Execution rules (Phase 2)

1. **Define before implementing:** a sub-step's scope and acceptance must exist in this
   document before code is written. (Steps 4.5–4.7/5.4 were retroactively defined in
   `CONTRIBUTING.md` when this governance gap was found; that was a one-time reconciliation,
   not precedent.)
2. **One sub-step per delivery:** implement only the first `PENDING` row of the Phase C
   table in `TASK_MATRIX.md`; QC sign-off moves it.
3. **Status rows are QC-Lead-managed** — implementers never write `PASSED`.
4. Invariants 1–3, the five gates, the dual-source rule and Haru/CAPA discipline apply
   exactly as in Phase II; every gate runs before a sub-step is submitted for QC.
5. **The multi-agent model is mandatory** for every Step 6 row: `S1`–`S7`, the Definition of
   Ready, the work-order handoff and the evidence bundle above. A row that cannot satisfy
   `S1` (no second actor available) stays `QC_REVIEW` — it does not get signed by its author.
