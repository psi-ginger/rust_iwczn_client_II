# Work Order — Step 6.1: End-to-End Client Driver & Orchestration

| Field | Value |
| :--- | :--- |
| **Step** | 6.1 (first `PENDING` row of the Phase C table) |
| **Authoritative definition** | `CONTRIBUTING_PHASE2.md` § "Step 6.1" — this file is operational detail, **not** a fork of it |
| **Governance** | `CONTRIBUTING_PHASE2.md` § "Multi-agent execution model" (`S1`–`S7`) |
| **Lanes** | Implementer · Verifier · Custodian (Scribe only for amendments) |
| **Baseline** | `main` @ `b6b48a7`, **194/194**, `fmt` 0 diffs, `clippy` clean (re-measured 2026-10-27) |
| **Status** | `READY` — adoption by the QC Lead required before implementation |
| **Blocks** | 6.2 (the harness calls `driver::run`) |

> **Read this first, Implementer.** You may write only the files in § "File plan". You may not
> edit `TASK_MATRIX.md`, may not write `PASSED`, and may not add any protocol parsing that a
> delivered step does not already provide. If a frame you need has no delivered parser, stop
> and file a finding — do not invent one.

---

## 1. Scope

One library function assembles the whole session in one process, in order:

1. `FrontLink::connect(cfg.front)` → `handshake()` → `SecretKey` (3.1).
2. `request_notice` + `request_worldlist` → `parse_world_list` (3.2). Non-empty list required.
3. `request_login` → `parse_login_response` → `FrontAuth` (3.3), then close the front link.
4. `GameLink::connect(cfg.game)` → `handshake()` → `ClientCharUnique` (4.1–4.2).
5. `request_game_login(&mut link, &auth, cfg.password)` (4.3).
6. Character roster via the 4.4 parsers; assert `cfg.char_slot` exists in the roster.
7. `request_select_char(&mut link, cfg.char_slot)` (4.5).
8. SERVERINFO (4.6) with its service-area gate; the opcode-5 record populates `CharClient`;
   `WorldEntryState` records the in-world condition.
9. `request_village_struct` + `parse_village_struct` **only if** `cfg.village_ref` is `Some`
   (4.7). The driver never fabricates a map-data lookup — the ref is caller input.
10. In-world: `run_with_shutdown` with one `Ticker::new("heartbeat", 10s)`, a handler that
    feeds `DispatcherState::dispatch(cmd, payload)` for every `LoopEvent::Frame` and calls
    `send_heartbeat` on the heartbeat tick (5.1 + 5.2 + 5.4).
11. Teardown on Ctrl+C **and** on server disconnect via `terminate(link, shutdown, tracker,
    drain_bound)` (5.3); then return the outcome.

Stage observability is the acceptance, not a log line: the returned value must let a caller
prove which stages ran. **No silent success.**

## 2. Non-goals (do not do these)

- No new opcodes, no new wire parsing, no changes to `src/net/**`, `src/auth/**`, `src/game/**`.
- No multi-client / swarm behaviour. One process, one session.
- No live or ngrok dependency in any gate. Full-flow proof is 6.2's harness, not this step.
- No retry/reconnect policy, no UI, no persistence.
- No edits to `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `docs/agents/WO-*.md` (beyond your own
  delivery-note block).

## 3. File plan

| Path | Owner | Contents |
| :--- | :--- | :--- |
| `src/client/driver.rs` | Implementer | `DriverConfig`, `DriverOutcome`, `DriverError`, `Stage`, `run` |
| `src/client/driver/tests.rs` | Implementer | Unit tests: config validation, error→stage mapping, teardown-on-every-path |
| `src/client/mod.rs` | Custodian | one-line `pub mod driver;` — announce first (serialization point) |
| `src/main.rs` | Implementer | env → `DriverConfig`, runtime, exit-code mapping only |
| `Cargo.toml` | Custodian | only if a dependency is genuinely required; justify it in the delivery note |

## 4. Frozen API (6.2 consumes this)

```rust
pub struct DriverConfig {
    pub front: SocketAddr,
    pub game: SocketAddr,
    pub user_id: String,
    pub password: String,
    pub mac_address: String,
    pub char_slot: i32,
    pub village_ref: Option<i32>,
    pub drain_bound: Duration,
}

pub enum Stage { FrontConnected, SecretKey, WorldList, FrontLogin, GameConnected,
                 LinkKey, GameLogin, CharList, CharSelected, ServerInfo, CharClient,
                 InWorld, TornDown }

pub struct DriverOutcome {
    pub stage: Stage,          // TornDown on success
    pub worlds: usize,
    pub char_unique: i32,
    pub char_slot: i32,
    pub village: Option<i32>,
}

pub async fn run(cfg: &DriverConfig, shutdown: &Shutdown)
    -> Result<DriverOutcome, DriverError>;
```

`DriverError` must carry the stage reached, e.g. `DriverError::Stage { stage: Stage, source: … }`,
so a caller can tell "died at game login" from "died in world". Changing this signature after
submission requires a Scribe amendment to this file and notice to the 6.2 lane.

## 5. Environment contract (`main.rs`)

| Variable | Default | Rule |
| :--- | :--- | :--- |
| `GOONZU_FRONT_ADDR` | `127.0.0.1:5700` | `host:port`, resolved via `lookup_host` |
| `GOONZU_GAME_ADDR` | `127.0.0.1:4010` | `host:port`, resolved via `lookup_host` |
| `GOONZU_USER_ID` | — | **required**, no default; absent → exit 2 with a clear message |
| `GOONZU_USER_PW` | — | **required**, never logged, never in `Debug`/`Display` output |
| `GOONZU_MAC_ADDRESS` | — | **required** (the 3.3 payload carries it) |
| `GOONZU_CHAR_SLOT` | — | **required**, `i32`; invalid → exit 2 |
| `GOONZU_VILLAGE_REF` | unset | optional; unset ⇒ stage 9 is skipped and `outcome.village` is `None` |
| `GOONZU_DRAIN_MS` | `5000` | teardown drain bound |

Exit codes: `0` = `TornDown`, `1` = runtime error (stage printed), `2` = configuration error.
No address may be hardcoded anywhere in the driver.

## 6. Acceptance

1. **Five gates green** (run from a clean clone at the delivery SHA, fresh `target/`):

   ```
   RUSTFLAGS="-D warnings" cargo check --all-targets
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   cargo test
   cargo doc --no-deps
   ```
2. **Stage observability:** a failing stage is reported as that stage; success ends at
   `Stage::TornDown`. A test proves an injected failure at a chosen stage surfaces it.
3. **Teardown on every path:** success, mid-flow error, Ctrl+C seam, and server close all call
   `terminate` and leave zero orphaned tasks (reuse 5.3's seams; do not invent a real
   `GenerateConsoleCtrlEvent`, which would signal cargo's console group).
4. **No credential leakage:** a test asserts the password string never appears in
   `DriverError`'s `Display` output or in a rendered log record at `INFO`.
5. **No new parsing:** `driver.rs` calls delivered parsers only; a review grep shows no
   `from_le_bytes`/offset arithmetic in the driver.
6. Test count strictly greater than the 194 baseline; recorded in the delivery note.

## 7. Forbidden shortcuts

Each of these is a documented Phase I failure mode; a single occurrence fails the row.

- `todo!()` / `unimplemented!()` / `unwrap()` / `expect()` in non-test driver paths.
- Swallowing an unexpected opcode, or "continue anyway" on a parse error.
- Skipping a stage because the previous one "probably worked".
- Defaulting a character slot, a world, or a village ref when the data is absent.
- Any number in output that does not come from a parsed struct.
- `sleep` used to "wait for" a frame instead of a bounded read.
- Editing `TASK_MATRIX.md` or claiming `PASSED`.
- Reporting a live/ngrok run as if it were a gate.

## 8. Evidence required (Verifier)

`evidence/6.1/` containing `head.txt` (`git rev-parse HEAD`, empty `git status --porcelain`,
clone path), `check.log`, `clippy.log`, `fmt.log`, `test.log` (with the `test result:` line),
`doc.log`, `env.txt` (rustc/cargo/rustfmt versions, UTC date, OS), `SHA256SUMS`.
Nothing handwritten: every number in the matrix row must be copyable from a log line.

---

## 9. Return blocks (append; do not rewrite the sections above)

### Delivery note — Implementer

```
Commit: 7ab2321 (governance prerequisite: 13cbaf2 - RCA policy decision, resolution (a))
Branch: step/6.1-driver
Files: src/client/driver.rs (new, 747 lines) · src/client/driver/tests.rs (new,
  231 lines, 12 tests) · src/client/mod.rs (+3: module docs + `pub mod driver;`)
Local gates (implementer-run, NOT evidence): check ok · clippy ok ·
  fmt ok (0 diffs) · test 206/206 · doc 0 warnings (0 warning lines)
Deviations from this WO:
  - Section 3 `src/main.rs` NOT implemented — still the Step 1.1 scaffold, so
    Definition of Done item 3 ("main.rs reduced to config + logging + runtime +
    exit codes") cannot close. Reason: out of the blocker's scope. The RCA
    recorded "the only pending edit is the from_env binding fix", and this
    takeover was scoped to that fix plus resuming the gates. Left for the next
    implementer; flagged here so it is not discovered at verdict time.
  - Two mechanical gate fixes beyond the named `from_env` fix, both required to
    resume the gates and neither touching protocol or logic:
    `clippy::needless_lifetimes` on `game_link_mut` (explicit `'a` elided) and
    two rustfmt diffs in `src/client/driver/tests.rs` (formatted via the Phase 2
    leaf-file rule — `driver.rs` / `driver/tests.rs` named directly, never
    `mod.rs`; `git status` confirms no unrelated file changed).
  - Authorship: this delivery was written by the Acting QC Lead, who is also the
    signing role. S1/S2 therefore bar a `PASSED` verdict from this actor; row
    6.1 moves to IN_PROGRESS pending an independent second actor.
  - Evidence bundle (section 8) NOT produced — S4 requires a clean clone at the
    delivery SHA. The numbers above are working-tree runs and are explicitly
    NOT evidence.
Findings raised: none as F-61-NN (the write-guard incident is resolved in
  docs/rca/2026-10-27-step-6.1-write-guard-conflict.md at 13cbaf2; the
  outstanding main.rs gap is recorded under Deviations above)
```

### Adversarial findings — Adversary (optional for 6.1, mandatory on request)

```
F-61-01 | <claim> | <how it was falsified> | <repro path>
```

### Verdict — Verifier

```
Verdict: PASSED | QC_REVIEW | REJECTED
Gates (clean clone at <sha>, run <UTC timestamp>): check <ok> · clippy <ok> ·
  fmt <0 diffs> · test <n>/<n> · doc <0 warnings>
Citations re-derived: <n> checked, <n> corrected
Findings: <closed/open>
Baseline after merge: <n>/<n> @ <sha>
```

## 10. Definition of Done

- [ ] Five gates green from a clean clone at the delivery SHA; evidence bundle committed.
- [ ] `DriverOutcome` proves every stage; `Stage::TornDown` only on clean teardown.
- [ ] `main.rs` reduced to config + logging + runtime + exit codes.
- [ ] Branch pushed, working tree clean, this file's blocks filled.
- [ ] Verifier's verdict recorded; only then does `TASK_MATRIX.md` row 6.1 move.
