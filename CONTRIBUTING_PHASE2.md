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
