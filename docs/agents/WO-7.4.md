# Work Order - Step 7.4: Driver & Event-Loop Action Wiring

**Status:** `DRAFT` for PsiGinger review — definition-before-implementation. No `src/**` byte may
move before this scope is accepted and this WO is committed on `main`.
**Date:** 2026-11-05 · **Revision:** Rev 1 (initial draft).
**Branch:** to be cut from `main` @ `4e4ba5b` on acceptance.
**Baseline (re-derived this session):** S4 clean clone at `main` @ `4e4ba5b`
(`4e4ba5b19d721f6edd27b7086869c6a587d4de3f`), porcelain clean, all five mandatory gates exit-0 —
`cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` under
`RUSTFLAGS=-D warnings`, `cargo fmt --check` 0 bytes, `cargo test` **352/352** (319 lib +
11 `asset_fixtures` + 22 `e2e_loopback`; 5 ignored = 1 operator demo + 4 `live_integration`),
`cargo doc --no-deps` 0 warnings. The `src/**`, `tests/**` and `Cargo.*` tree at `4e4ba5b` is
byte-identical to the gated `2436a54` (`git diff --name-only 2436a54 4e4ba5b -- src/ tests/
Cargo.toml Cargo.lock` is empty — `4e4ba5b` is docs+evidence only), so `evidence/7.3/` applies to
this tree unchanged.
**Reference:** `TASK_MATRIX.md:96` (row 7.4 `PENDING`, lanes Implementer → Verifier + Adversary,
row claim byte-exact) · `CONTRIBUTING_PHASE3.md:179-195` (steps table + Verifier/append rule) ·
`docs/agents/WO-step7-in-world-actions.md` (frozen; §8.3 goldens = wire source of record) ·
`docs/agents/WO-6.3.md` §9 (mutation-probe precedent) · `evidence/7.3/` (baseline bundle).

---

## 1. Provenance, governance, lane hats

- **Registration.** Row 7.4 was opened by the Verifier in the single directive-disclosing
  `docs(qc)` commit `4e4ba5b` (2026-11-05), together with the retroactive 7.3 registration, the
  7.1/7.2 mirror sync and the option-(b) scoping note — all three disclosed in that commit
  message. This WO is authored next, before any code (definition-before-implementation).
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every load-bearing claim in
  this WO is "byte-exact" or "stops/refuses", dischargeable by a verified mutation probe (§12),
  per `CONTRIBUTING_PHASE3.md` multi-agent model and the 6.3 ratification precedent.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs and
  `docs/audit/**` is READ-ONLY — a defect found there is a finding `F-7.4-NN`, never an edit
  (P3-6).
- **Source-of-record split.** This step adds **no new wire bytes**: the 240/117/241 layouts stay
  `WO-step7-in-world-actions.md` §8.3 (frozen — never edited by this WO), the semantics stay the
  legacy tree plus the Step 7.3 API (`src/game/action.rs`, `src/game/dispatcher.rs`, both frozen
  at `2436a54`). This WO owns only the **wiring contract** (§7) and the test goldens restated in
  §8, each traced back to §8.3 or a named constant (F-07).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 The in-world surface as shipped (7.1-7.3)

| Piece | As of `4e4ba5b` | Where |
|---|---|---|
| Loop events | `LoopEvent::{Frame, Ticked}` only; the `select!` has exactly two arms — a read bounded by `READ_POLL_BUDGET` (100 ms) and the earliest ticker deadline | `src/client/event_loop.rs:203-216`, `:384-397` |
| Wake / loop | `Wake::{Idle, Frame, Closed, Failed, Ticks}`; `run` → `run_with_shutdown` → the loop | `event_loop.rs:244-257`, `:309-319`, `:346-450` |
| Driver chain | `run` → `run_with_watcher` → `flow` → `front_phase` / `game_phase` → `in_world_phase` | `src/client/driver.rs:458-469`, `:597-604`, `:629`, `:762` |
| In-world handler | heartbeat arm, then one blanket dispatch record `debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched")` | `driver.rs:778-789` (record `:786`) |
| Handler errors | `InWorldError::{Dispatch, Heartbeat}`; `LoopError::Handler` → `DriverError::refused(Stage::InWorld, detail)` — **the refusal path this step reuses** | `driver.rs:728-738`, `:793-800` |
| Outbound today | only the 10 s heartbeat (`heartbeat_ticker`, `is_heartbeat`) — the in-world loop has no other send path | `src/client/heartbeat.rs:192`, `:206` |
| Frozen 7.3 API | `send_order(&mut GameLink, &Order) -> Result<(), GameError>` (`action.rs:133`); `send_chat(&mut GameLink, ChatMode, &str) -> Result<(), ActionError>` (`:223`); `ActionError::{EmptyChat, ChatTooLong, Send}` (`:170-191`); `DispatchOutcome::Ordered { char_unique, target, order }` report-only (`dispatcher.rs:788-798`) | `src/game/**` @ `2436a54` |
| Harness | `run_driver` runs `driver::run` only (`e2e_loopback.rs:106`); `spawn_game_with` injects a `Tail` (`:154`); `Tail::HeartbeatThenFin` asserts the **first** in-world C2S frame is a heartbeat (`game_double.rs:389-411`); `Transcript`/`Entry` + `expected_conversation` give the byte-exact conversation pattern (`frames.rs:127-166`, `e2e_loopback.rs:335-354`); `debug_capture_subscriber` captures DEBUG records (`e2e_loopback.rs:258`); `CHAR_UNIQUE = 4242` (`frames.rs:411`) | `tests/**` |

### 2.2 Constraints discovered (each forces a disclosed edit or a design choice)

1. **Source-pin locator.** `src/client/driver/tests.rs:568-604`
   (`world_entry_adoption_is_wired_before_the_in_world_loop_starts`) locates the in-world loop
   start by the literal `run_with_shutdown(` (`:596-603`). The call site becomes the new
   `run_with_commands(` (§7.3), so the locator string is renamed — same position semantics
   (adoption must still precede the loop).
2. **Watcher-seam call.** `src/client/driver/tests.rs:434-451` calls
   `run_with_watcher(&cfg, &shutdown, { …watcher factory… })`; the seam fn gains the commands
   parameter (§7.3), so the call appends `, None` (production parity: no channel).
3. **Exhaustive `LoopEvent` match.** `src/client/event_loop/tests.rs:251-265`
   (`frames_are_delivered_while_tickers_keep_firing`) matches `LoopEvent` exhaustively
   (`:255-258`); it gains a `LoopEvent::Command(_)` arm that treats the event as impossible
   under `run` (`run` wires `None`, §7.2) and fails loudly if it ever arrives.
4. **Import/name hygiene.** `driver.rs:54` drops `run_with_shutdown` (no longer called there).
   `driver.rs` must **not** import the event loop's `run_with_commands` by name — it collides
   with the driver's own new `pub fn run_with_commands`; call it by full path
   (`crate::client::event_loop::run_with_commands`). Only one exhaustive `LoopEvent` match
   exists in the tree (item 3); `soak.rs:206,217` counts the blanket record's exact message
   string (§7.3 keeps it byte-identical); the storm cycle carries **no 241** (`game_double.rs:520-530`),
   so no existing count includes an `Ordered` record.

### 2.3 Legacy semantics inherited (why commands exist)

Input → `cltOrder::MakeOrder` (`common/Order/order.cpp:131`) → `cltClient::OrderOperation`
(`Client.cpp:3284-3304`) → `GAMEMSG_REQUEST_USERORDER` (**240**) and the chat path through
`CHATMODE_*` (`MsgType-System.h:1085-1094`) — semantics already inherited and cited by
WO-7.3 §3.2/§8. This step wires the modern equivalents (`Order`, `ChatMode`) to the loop; it
invents no opcode and no legacy byte layout.

## 3. Scope decision — this delivery

**In:**

1. A new `src/client/commands.rs`: `ClientCommand` (`Order` / `Chat`), `CommandSender` /
   `CommandReceiver` newtypes + `channel()`, `CommandError`, and
   `ClientCommand::execute(&self, &mut GameLink) -> Result<(), ActionError>`.
2. The event loop: `LoopEvent::Command`, one new `select!` arm, `Wake::{Command,
   CommandsClosed}`, and `pub run_with_commands(link, tickers, shutdown, Option<CommandReceiver>,
   handler)`; existing `run` / `run_with_shutdown` keep their exact signatures and behavior.
3. The driver: thread `Option<CommandReceiver>` through the private chain, expose
   `pub run_with_commands(cfg, shutdown, CommandReceiver)`, handle `LoopEvent::Command` in the
   in-world handler, add `InWorldError::Action`, and give `DispatchOutcome::Ordered` its own
   report record.
4. `Cargo.toml`: tokio `"sync"` feature (mpsc) — precedent: Step 5.3 added `"signal"`.
5. Eight tests (§9): 3 event-loop, 2 command-channel, 3 end-to-end. Baseline **352 → 360**.

**Out:** see §5.

## 4. Architectural decisions (binding for this WO)

- **D1 — single writer, same task.** Commands are consumed *on the loop task*: the new `select!`
  arm yields a `LoopEvent`, the handler awaits `execute` on the very `&mut GameLink` the loop
  owns. No second task ever holds the link — the link cipher ticks once per encoded frame
  (`net/cipher.rs:23-30`), so two writers would race the counter.
- **D2 — additive API only.** `event_loop::run` and `event_loop::run_with_shutdown` keep their
  signatures and delegate with `commands = None`; `driver::run` keeps its signature and passes
  `None`; the only new public entries are `event_loop::run_with_commands` and
  `driver::run_with_commands`. Existing callers (including the four `run_with_shutdown` call
  sites in `teardown/tests.rs`) are untouched. A `None` channel adds exactly one permanently
  pending `select!` arm — no ordering, timing or behavior change for any existing test.
- **D3 — a closed channel disables the arm; it never stops and never spins the loop.**
  `command_wait(&mut Option<CommandReceiver>)` answers `pending()` for `None`, and a `recv()`
  that returns `None` (every sender dropped) becomes `Wake::CommandsClosed`, which sets
  `commands = None` — from then on the arm is pending forever. The loop keeps running (dropping
  the input layer must not kill a session) and never busy-polls (a pending arm is not a
  resolved arm).
- **D4 — refusals stop the session, on the existing path.** `execute` returns
  `Result<(), ActionError>`; the handler's Command arm propagates with `?` into the new
  `InWorldError::Action(#[from] ActionError)`, and the untouched
  `LoopError::Handler → DriverError::refused(Stage::InWorld, detail)` carries it
  (`driver.rs:796-798`). **No new `DriverError` variant, no warn-and-continue.** A future input
  layer validates before it enqueues (empty / oversized chat never reach the channel); the
  in-world stop is the last line of defense, not the user experience.
- **D5 — `Ordered` gets a dedicated report record; everything else stays byte-identical.**
  The frame arm becomes an explicit (non-wildcard) match over all six `DispatchOutcome`
  variants: `Ordered` logs
  `debug!(char_unique, ?target, "in-world order echo reported")`; the other five keep
  `debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched")` — the exact string and
  fields the existing e2e record test (`e2e_loopback.rs:1037-1073`) and the soak counters
  (`soak.rs:206,217`) assert verbatim. No wildcard, so a future `DispatchOutcome` variant forces
  a conscious decision at this site.
- **D6 — tokio `"sync"`.** Added to `[dependencies]` (`Cargo.toml:17`) with a comment sentence
  in the Step 5.3 `signal` precedent's style; `tokio::sync::mpsc` is feature-gated.
- **D7 — unbounded channel.** `mpsc::unbounded_channel`; `CommandSender::send(&self, …)` is a
  **synchronous** call that fails only when closed (`CommandError::Closed`), so a future input
  callback can enqueue without awaiting. Drain cost is one command per `select!` iteration: a
  ready arm resolves the poll immediately, so a backlog never waits out the 100 ms read budget.
  Input is human-rate; a bounded channel would force a drop/block policy that silently loses
  intent — rejected.
- **D8 — handler order: heartbeat → Command → Frame.** `is_heartbeat` keys on the ticker name
  and cannot match `LoopEvent::Command` anyway; the explicit order documents that a ticker fire
  is never mistaken for a command, and the dispatch record stays last.

## 5. Non-goals

- **No input/UI layer** (keyboard/mouse/window → enqueue). That is a later step; this WO fixes
  only the channel contract it will use.
- **No new opcodes, payloads, cipher/frame-header changes** — hence **no `goonzu-rs` mirror
  obligation** is raised here (mirror duties stay WO-7.3 §8.4).
- **No state write from `Ordered`** — report-only; position authority stays opcode 200
  (WO-7.3 D8/T11).
- **No new `DriverError` / `LoopError` variants**, no INFO-level per-command logging (debug
  only), no heartbeat cadence change, no `biased` in the `select!` (it would reorder the
  existing read/ticker arms and disturb behavior pinned by Step 5.1/5.3 tests).
- **No mid-session socket-write-failure e2e** (nondeterministic: TCP buffers writes). The
  `GameError → ActionError::Send` normalization is pinned pre-handshake instead (T5).
- No edits to `src/game/**` (7.3 frozen), no live-server run as a gate, no graphics/window work.

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change |
|---:|---|---|
| 1 | `docs/agents/WO-7.4.md` | NEW — this document, committed **before** any code |
| 2 | `src/client/commands.rs` | NEW — §7.1 |
| 3 | `src/client/commands/tests.rs` | NEW — T4, T5 |
| 4 | `src/client/mod.rs` | EDIT — one `//!` Step 7.4 bullet + `pub mod commands;` |
| 5 | `src/client/event_loop.rs` | EDIT — §7.2 (additive) |
| 6 | `src/client/event_loop/tests.rs` | EDIT — T1-T3 + the disclosed exhaustive-match arm (§2.2-3) |
| 7 | `src/client/driver.rs` | EDIT — §7.3 |
| 8 | `src/client/driver/tests.rs` | EDIT — **only** the two disclosed mechanical lines (§2.2-1, §2.2-2) |
| 9 | `Cargo.toml` | EDIT — `"sync"` + comment (D6) |
| 10 | `tests/e2e_loopback.rs` | EDIT — `run_driver_with_commands` helper + T6-T8 |
| 11 | `tests/e2e_loopback/game_double.rs` | EDIT — `Tail::Commands` + its tail fn |
| 12 | `tests/e2e_loopback/frames.rs` | EDIT — three hand-placed golden constants (§8) |
| 13 | `evidence/7.4/` | NEW at verification, written by the Verifier only (§11) |

**READ-ONLY:** `src/game/**`, `src/auth/**`, `src/net/**`, `src/main.rs`, `src/lib.rs`,
`tests/live_integration.rs`, `tests/e2e_loopback/{front_double,robustness,soak}.rs`,
`TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, every other file in
`docs/**`, every existing `evidence/**` directory. A defect found there is a finding, never an
edit (P3-6).

## 7. API & wiring spec (the shared contract)

### 7.1 `src/client/commands.rs` (new)

```rust
//! Step 7.4: the in-world command channel — how an input layer (later step)
//! asks the loop to send on the player's behalf (WO-7.4 §7).

/// One action the in-world loop may send (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCommand {
    /// A 240 order — the frozen 7.3 `Order` rides unchanged.
    Order(Order),
    /// A 117 chat: the mode travels exactly as given (no coercion, D4).
    Chat { mode: ChatMode, text: String },
}

impl ClientCommand {
    /// Encodes and sends this command on `link`.
    ///
    /// # Errors
    /// `ActionError::EmptyChat` / `ActionError::ChatTooLong` (the frozen 7.3
    /// chat refusals, never a clamp) and `ActionError::Send` — the send gate
    /// or the socket, normalized from `GameError` by `#[from]` (T5).
    pub async fn execute(&self, link: &mut GameLink) -> Result<(), ActionError> {
        match self {
            ClientCommand::Order(order) => send_order(link, order).await?,
            ClientCommand::Chat { mode, text } => send_chat(link, *mode, text).await?,
        }
        Ok(())
    }
}

/// Why a command could not be queued (the receiver is gone).
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// The session dropped its receiver (ended, or never wired).
    #[error("the command channel is closed (the session is over)")]
    Closed,
}

/// The sending half — clone it into whatever produces input (D7).
#[derive(Debug, Clone)]
pub struct CommandSender { /* tokio::sync::mpsc::UnboundedSender<ClientCommand> */ }

impl CommandSender {
    /// Queues `command`; `Err(CommandError::Closed)` once the session is over.
    pub fn send(&self, command: ClientCommand) -> Result<(), CommandError> { … }
}

/// The receiving half — moved into the session as `Option<CommandReceiver>`.
#[derive(Debug)]
pub struct CommandReceiver { /* tokio::sync::mpsc::UnboundedReceiver<ClientCommand> */ }

impl CommandReceiver {
    /// `Some(command)`, or `None` once every sender is gone (D3).
    pub(crate) async fn recv(&mut self) -> Option<ClientCommand> { … }
}

/// A fresh unbounded channel (D7).
pub fn channel() -> (CommandSender, CommandReceiver) { … }
```

Imports: `crate::game::action::{ActionError, ChatMode, Order, send_chat, send_order}`,
`crate::game::entry::GameLink`, `tokio::sync::mpsc`, `thiserror`. Every `pub` item gets a doc
comment; `execute` gets an `# Errors` section (house style).

### 7.2 `src/client/event_loop.rs` (additive)

- `LoopEvent` gains `Command(ClientCommand)` (documented as "a queued player command from
  [`crate::client::commands`]"). The `Debug` derive keeps working because `ClientCommand` is
  `Debug`.
- The private `Wake` enum gains `Command(ClientCommand)` and `CommandsClosed`.
- New private helper (mirrors `ticker_wait`):

  ```rust
  async fn command_wait(commands: &mut Option<CommandReceiver>) -> Option<ClientCommand> {
      match commands {
          Some(receiver) => receiver.recv().await,
          // D2/D3: no channel wired — this arm never resolves, so `run` and
          // `run_with_shutdown` behave exactly as they do today.
          None => std::future::pending().await,
      }
  }
  ```

- The existing `tokio::select!` (`event_loop.rs:384-397`) gains **one** arm, staying
  non-biased (§5):

  ```rust
  cmd = command_wait(&mut commands) => match cmd {
      Some(command) => Wake::Command(command),
      None => Wake::CommandsClosed,
  },
  ```

- Two new `match wake` arms (next to the existing ones at `:399-448`):

  ```rust
  Wake::Command(command) => {
      let flow = handler(link, LoopEvent::Command(command))
          .await
          .map_err(LoopError::Handler)?;
      if flow == Flow::Break { return Ok(()); }
  }
  // D3: disable the arm for the rest of the session; the loop keeps running.
  Wake::CommandsClosed => commands = None,
  ```

  (The `&mut commands` borrow held by the `select!` statement ends with that statement; the
  assignment happens in the following `match wake`.)
- New public entry — `run_with_shutdown`'s body moves here verbatim, plus the parameter:

  ```rust
  pub async fn run_with_commands<F, E>(
      link: &mut GameLink,
      tickers: &[Ticker],
      shutdown: &Shutdown,
      commands: Option<CommandReceiver>,
      mut handler: F,
  ) -> Result<(), LoopError<E>>
  where
      F: AsyncFnMut(&mut GameLink, LoopEvent) -> Result<Flow, E>,
      E: std::error::Error + 'static,
  ```

- `run_with_shutdown` keeps its signature and becomes
  `run_with_commands(link, tickers, shutdown, None, handler).await`; `run` is untouched and
  still delegates to `run_with_shutdown`. Update the module `//!` docs and the doc comments on
  `run_with_shutdown` / `LoopEvent` / `Wake` for the new flavour; `# Errors` stays "as `run`"
  plus the closed-arm semantics (a closed channel is **not** an error — D3).

### 7.3 `src/client/driver.rs`

- Imports: `use crate::client::commands::CommandReceiver;`, add `DispatchOutcome` to the
  `crate::game::dispatcher` import (`:60`), add `use crate::game::action::ActionError;`, and
  drop `run_with_shutdown` from the `event_loop` import (`:54`).
- The private chain threads `commands: Option<CommandReceiver>` (each has exactly one call
  site): `run_with_watcher(…, commands)` → `flow(…, commands)` → `game_phase(…, commands)` →
  `in_world_phase(…, commands)`. Doc comments on those four fns gain one clause naming the
  pass-through (they document their signatures today).
- Public surface:

  ```rust
  pub async fn run(cfg: &DriverConfig, shutdown: &Shutdown) -> Result<DriverOutcome, DriverError> {
      run_with_watcher(cfg, shutdown, spawn_ctrl_c_watcher, None).await   // unchanged contract
  }

  /// Step 7.4: [`run`] with a command channel wired into the in-world loop.
  /// # Errors  — as [`run`].
  pub async fn run_with_commands(
      cfg: &DriverConfig,
      shutdown: &Shutdown,
      commands: CommandReceiver,
  ) -> Result<DriverOutcome, DriverError> {
      run_with_watcher(cfg, shutdown, spawn_ctrl_c_watcher, Some(commands)).await
  }
  ```

  (`commands` is the **last** parameter of `run_with_watcher`, so the watcher-seam test appends
  `, None` — §2.2-2. The event loop's fn is called by full path
  `crate::client::event_loop::run_with_commands(…)` — §2.2-4.)
- `InWorldError` gains the third variant:

  ```rust
  /// A queued player command could not be encoded or sent (D4).
  #[error(transparent)]
  Action(#[from] ActionError),
  ```

- The in-world handler keeps its order (D8) and gains the Command arm plus the explicit
  dispatch match:

  ```rust
  if is_heartbeat(&event) {
      send_heartbeat(link).await?;
      debug!("heartbeat sent");
      return Ok(Flow::Continue);
  }
  if let LoopEvent::Command(command) = &event {
      command.execute(link).await?;            // -> InWorldError::Action (D4)
      return Ok(Flow::Continue);
  }
  if let LoopEvent::Frame(frame) = &event {
      let outcome = dispatcher.dispatch(frame.cmd(), frame.payload())?;
      match &outcome {
          DispatchOutcome::Ordered { char_unique, target, .. } => {
              debug!(char_unique, ?target, "in-world order echo reported");   // D5
          }
          DispatchOutcome::Unhandled { .. }
          | DispatchOutcome::Chat(_)
          | DispatchOutcome::Entered { .. }
          | DispatchOutcome::Removed { .. }
          | DispatchOutcome::Applied { .. } => {
              debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched");
          }
      }
  }
  Ok(Flow::Continue)
  ```

  The blanket arm is **byte-identical** to today's record (`driver.rs:786`): same fields, same
  message — the existing record test and the soak counters depend on it verbatim. The
  `LoopError::Handler → DriverError::refused(Stage::InWorld, error.to_string())` mapping at
  `:796-798` is untouched and becomes the refusal carrier for T7 (`InWorldError::Action` is
  `#[error(transparent)]`, so `detail` is `ActionError`'s own Display).

### 7.4 `Cargo.toml`

```toml
tokio = { version = "1", features = ["rt", "net", "macros", "time", "io-util", "signal", "sync"] }
```

plus one comment sentence in the block at `:8-12` in the Step 5.3 style: `sync` added by Step
7.4 for `tokio::sync::mpsc`, the in-world command channel — still tokio-only as a direct
dependency.

## 8. Wire goldens for this step (the F-07 expectation source)

Tests assert **these literals**; a golden may never be regenerated from the encoder under test,
and `WO-step7-in-world-actions.md` §8.3 is never edited — its letters are cited as the origin.

| # | Image (hex, wire order) | Provenance |
|---|---|---|
| **G1** | `0A 00 00 F0 00 00 64 00 C8 00` | WO-step7 §8.3-**A** (frozen): `usSize=10`, `ucChk=0`, `cmd=240`, payload `kind 00 \| 64 00 \| C8 00` = Move, x=100 LE, y=200 LE |
| **G2** | `08 00 00 75 00 03 68 69` | §8.3-**D** layout (mode byte then UTF-8 text; no `siDataSize`, no NUL) with mode `03` = `ChatMode::Town` (`action.rs:149`, semantics citation), text `"hi"` = `68 69`; `usSize=8 = 6+2` |
| **G3** | `0E 00 00 F1 00 92 10 00 00 00 64 00 C8 00` | §8.3-**H** layout (9-byte payload: `char_unique` i32 LE, `kind` u8, `x` u16 LE, `y` u16 LE; `usSize=14`) with `CHAR_UNIQUE = 4242` (`frames.rs:411`) → 4242 = 0x1092 → LE `92 10 00 00` (hand-computed: 4096+146), kind `00` Move, x=100, y=200 |
| **G4** | `frame(OP_CHARUPDATE, &char_update_body(CHAR_UNIQUE as i16))` | the harness's own fixture — byte-identical to the 200 the in-world set already writes (`frames.rs:850`); it is *stimulus*, and the load-bearing T8 claim is the log record, not its bytes |

Hand-placed in `tests/e2e_loopback/frames.rs` as named constants (e.g. `MOVE_COMMAND_WIRE`,
`CHAT_TOWN_WIRE`, `ORDER_ECHO_241_WIRE`) with these § citations in their doc comments.

## 9. Test invariants (T1-T8) and the baseline

Pattern authorities: paused-clock loopback (`event_loop/tests.rs`, `dial()`, `local_listener()`),
the pre-handshake send-gate pattern (`src/game/action/tests.rs:298-323`), the byte-exact
conversation pattern (`e2e_loopback.rs:269-354`), the DEBUG-record pattern
(`e2e_loopback.rs:1037-1073`). F-01 loopback-only; F-07 expectations from §8 / frozen citations
only.

| # | Invariant | File | Phase |
|---|---|---|---|
| **T1** | `queued_commands_reach_the_handler_and_the_loop_keeps_running` — with `run_with_commands(…, Some(rx), …)` under a paused clock: two queued commands arrive **in order, payload-exact** (`Order::Move { x: 100, y: 200 }`, then `Chat { mode: Town, text: "hi" }`), and the loop keeps running — at least one ticker fire — before the handler `Break`s | `event_loop/tests.rs` | unit |
| **T2** | `dropping_every_sender_never_stops_the_loop` — every sender dropped before the loop starts: the session runs to three ticker fires and returns `Ok` (D3). Fails if a closed channel ends the loop early (0 fires) or errors | `event_loop/tests.rs` | unit |
| **T3** | `a_handler_error_on_the_command_path_stops_the_loop` — a handler `Err` raised while handling `LoopEvent::Command` surfaces as `LoopError::Handler`, not `LoopError::Link` and not a swallow | `event_loop/tests.rs` | unit |
| **T4** | `sending_after_the_receiver_is_gone_reports_closed` — drop the receiver, then `CommandSender::send` → `Err(CommandError::Closed)` (the closed contract in the other direction) | `commands/tests.rs` | unit |
| **T5** | `a_send_gate_failure_is_reported_as_an_action_send_error` — `execute` on a freshly connected, **pre-handshake** `GameLink` (loopback) → `Err(ActionError::Send(GameError::NotHandshaken))`, zero bytes written: pins the `GameError → ActionError::Send` normalization inside `execute` (7.3 pinned `send_order`/`send_chat` themselves; this pins the new wrapper) | `commands/tests.rs` | unit |
| **T6** | `queued_commands_travel_the_wire_byte_exact` — two commands enqueued before the run; full transcript == `expected_conversation` + **G1** + **G2** (both `Dir::C2S`, in that order), then a clean teardown (`Stage::TornDown`, no corruption). Needs `Tail::Commands` because the default tail asserts the first in-world C2S frame is a heartbeat (`game_double.rs:396-405`) | `e2e_loopback.rs` | e2e |
| **T7** | `an_empty_chat_command_refuses_the_session_without_a_frame` — `Chat { mode: Normal, text: "" }` → the driver returns `DriverError::Refused { stage: Stage::InWorld, detail }` with `detail` containing `chat text is empty` (the frozen `ActionError::EmptyChat` Display, `action.rs:175`); **no opcode-117 frame anywhere on the transcript**; the double observes `closed_early` and `heartbeats == 0` (the session died long before the 10 s tick) | `e2e_loopback.rs` | e2e |
| **T8** | `an_order_echo_is_reported_and_the_next_delta_stays_local` — one Move command (G1 out), the double answers with the 241 echo **G3** and then a second 200 **G4**: the log carries `in-world order echo reported` with `char_unique=4242 target=Local`, and the **last** `opcode=200 outcome=Applied { char_unique: 4242, target: Local }` record occurs *after* it (the `Ordered` consumption is report-only and does not poison the dispatcher — the next delta still resolves `Local`); transcript == `expected_conversation` + G1 (C2S) + G3 (S2C) + G4 (S2C); clean teardown | `e2e_loopback.rs` | e2e |

**Baseline: 352 → 360** — 319 + 5 new lib tests (T1-T5) = 324 lib, 11 `asset_fixtures`,
22 + 3 new e2e (T6-T8) = 25 e2e; the 5 ignored tests are preserved. The number is copied from
`test.log`, never from prose.

## 10. Forbidden shortcuts

- Editing anything outside §6 to make a test pass (finding first, P3-6).
- Deriving a golden from the encoder, dispatcher or driver under test (F-07) — §8 literals
  only; editing `WO-step7-in-world-actions.md` (frozen).
- A second task writing the link (D1), or `biased` in the `select!` (§5).
- Stopping **or** spinning the loop when the channel closes (D3) — the arm must become
  permanently pending.
- Swallowing a command error (warn + continue), or adding a `DriverError`/`LoopError` variant
  for one (D4) — the existing `Refused{InWorld}` path carries it.
- Changing `run` / `run_with_shutdown` signatures or behavior (D2), or the blanket dispatch
  record's fields/message (existing e2e + soak assert them verbatim).
- Routing `Ordered` through the blanket record, or logging the dedicated record at another
  level or with other fields than §7.3 (D5).
- Coercing `ChatMode`, clamping chat, or adding validation inside `execute` beyond what the
  frozen `send_chat` already refuses (D4; WO-7.3 D6).
- Touching `src/game/**` (7.3 frozen), `TASK_MATRIX.md` / `CONTRIBUTING*.md` (Verifier only),
  `README.md`, `GoonZu_Source/**`, any other WO or audit doc.
- Any external network, GPU, window or display in a gate test; any live-server run promoted to
  a gate (live = evidence only).

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/7.4/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
  `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`, `SHA256SUMS` — hashes over the
  committed (LF) bytes per the 6.3 L-2 lesson; the matrix `Tests Baseline` cell is copied from
  `test.log` (**360** expected: 324 + 11 + 25).
- Write-guard confirmation: `git diff --stat` limited to §6; `src/game/**`, matrix, contributing,
  README, legacy tree and the other test modules untouched.
- Disclosed-edit confirmation: the diff on `driver/tests.rs` and `event_loop/tests.rs` contains
  **exactly** the three mechanical hunks of §2.2 (locator rename, `, None` argument, one match
  arm) and nothing else.
- Five mutation probes recorded (§12): mutated diff, failing output, pristine restore,
  porcelain CLEAN after each, baseline re-run green afterwards.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test(s) | Restore |
|---:|---|---|---|
| 1 | In `command_wait`, replace `receiver.recv().await` with `std::future::pending().await` — the command arm never fires (commands are queued but never delivered) | **T1** (loop ends at peer EOF with an empty `seen`), **T6** (no command frames; the tail instead observes heartbeats → transcript mismatch) | `git checkout --` |
| 2 | In `ClientCommand::execute`, force the chat mode: match `Chat { text, .. }` and call `send_chat(link, ChatMode::Normal, text)` — mode passthrough broken | **T6** (G2's mode byte `03` arrives as `01`) | `git checkout --` |
| 3 | Delete the dedicated `Ordered` arm — merge it into the blanket record | **T8** (`in-world order echo reported` never logged) | `git checkout --` |
| 4 | `Wake::CommandsClosed => return Ok(())` — a closed channel stops the session (D3 violated) | **T2** (0 ticker fires instead of 3) | `git checkout --` |
| 5 | In the handler's Command arm, replace `command.execute(link).await?` with `warn!(…)` + continue — a refusal is swallowed (D4 violated) | **T7** (driver returns `Ok` instead of `Refused{InWorld}`; `closed_early` false, a heartbeat arrives) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit 101
and the failing assertion, restore, verify `git status --porcelain` clean — then re-run the full
gate set to the 360 baseline (6.3/7.3 probe discipline, `evidence/7.3/mutation-probes.md`).

## 13. Definition of done

A Step 7.4 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at **360** tests from `test.log` (324 lib + 11 fixtures +
   25 e2e), `fmt` 0 diffs, `doc` 0 warnings, porcelain clean.
2. T1-T8 all present and green; the three §2.2 disclosed hunks are the only changes to
   pre-existing tests.
3. Every decision D1-D8 is pinned by a failing-if-corrected test (T1-T8 + probes 1-5).
4. All five §12 probes show their named test failing at exit 101, with pristine restores and a
   green post-probe baseline.
5. Write guard holds: only §6 files changed; matrix, contributing, README, `src/game/**` and the
   legacy tree untouched (row update, if any, is the Verifier's).
6. F-07 holds: every expectation traces to §8, and §8 traces to WO-step7 §8.3 or a named
   constant — never to code under test.
7. No new wire bytes; no `goonzu-rs` mirror obligation raised; the blanket dispatch record and
   `run`/`run_with_shutdown` are byte-for-byte unchanged in behavior.
8. No external network in any test; no live run as a gate; no position write, no new opcode.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

- **Branch:** `step/7.4-driver-action-wiring` (cut from `origin/main` @ `c7717c1`), delivery SHA
  `b72ec33` — implementation only; this note lands as a follow-up docs commit on the same branch.
- **Baseline:** 352 → **360** (+8: T1-T3 `event_loop/tests.rs`, T4-T5 `commands/tests.rs`,
  T6-T8 `e2e_loopback.rs`).

**Gates at the delivery SHA (and re-run post-probe on the pristine tree):**

| Gate | Command | Result |
|---|---|---|
| 1 | `RUSTFLAGS=-D warnings cargo check --all-targets` | exit 0 |
| 2 | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 3 | `cargo fmt --check` | exit 0 — 0 diffs |
| 4 | `cargo doc --no-deps` | exit 0 — 0 warnings |
| 5 | `cargo test` | exit 0 — **360 passed / 0 failed** (324 lib + 11 fixtures + 25 e2e) |

T1-T8 all present and green, run individually as well as in the full suite:
`queued_commands_reach_the_handler_and_the_loop_keeps_running`,
`dropping_every_sender_never_stops_the_loop`,
`a_handler_error_on_the_command_path_stops_the_loop`,
`sending_after_the_receiver_is_gone_reports_closed`,
`a_send_gate_failure_is_reported_as_an_action_send_error`,
`queued_commands_travel_the_wire_byte_exact`,
`an_empty_chat_command_refuses_the_session_without_a_frame`,
`an_order_echo_is_reported_and_the_next_delta_stays_local` — each `1 passed; 0 failed`.

**§2.2 disclosed edits, as applied:**

1. `driver/tests.rs:603` locator renamed `run_with_shutdown(` → `event_loop::run_with_commands(`
   — **deviation from the §2.2-1 literal:** written with the `event_loop::` module path, because
   the bare `run_with_commands(` matches the driver's own `pub async fn run_with_commands`
   *definition* before the adoption site, defeating the locator's purpose. Same position
   semantics (adoption still precedes the loop).
2. `driver/tests.rs` watcher-seam call appends `, None` — rustfmt forces the 4th argument to
   reflow the call to multi-line (unavoidable under `fmt` 0-diffs); position semantics unchanged.
3. `event_loop/tests.rs` `frames_are_delivered_while_tickers_keep_firing` gains the
   `LoopEvent::Command(_) => panic!(…)` arm (file diff: 175 insertions, 0 deletions).

**Finding F-7.4-01 (user-approved, disclosed):** `tests/live_integration.rs:263` is a **second**
exhaustive `LoopEvent` match that §2.2-4 missed (the file is READ-ONLY in §6). One
`LoopEvent::Command(_) => panic!(…)` arm with an F-7.4-01 comment was added; without it `E0004`
(non-exhaustive match) fails every gate. Forced hunk, called out in the `b72ec33` commit body and
here. Write guard otherwise holds: only §6 rows 2-12 changed; `src/game/**`, matrix,
CONTRIBUTING, README and the legacy tree untouched.

**§12 mutation probes: 5/5** — each: one semantic break → named test fails at **exit 101** →
`git checkout --` → `git status --porcelain` clean → full gate re-run green at 360:

| # | Mutation | Named test | Failure observed |
|---:|---|---|---|
| 1 | `command_wait` Some arm → `std::future::pending().await` | T1 (+T6) | T1 exit 101: `seen == []` (empty); T6 exit 101: double budget `Elapsed` + driver `ConnectionReset` |
| 2 | `execute` chat arm forced to `send_chat(link, ChatMode::Normal, text)` | T6 | exit 101: transcript mismatch — mode byte `01` on the wire vs golden G2's `03` (byte-exact left/right dump) |
| 3 | `Ordered` arm merged into the blanket dispatch record | T8 | exit 101: `the 241 echo must be reported by its dedicated record, not the blanket dispatch record (D5)` |
| 4 | `Wake::CommandsClosed => return Ok(())` | T2 | exit 101: `left: 0, right: 3` ticker fires (loop stopped at channel close, D3 violated) |
| 5 | handler Command arm swallows via `warn!(…)` + continue | T7 | exit 101 (~30 s): `expect_err` got `Stage { InWorld, ConnectionReset }` instead of `Refused { InWorld }` — no refusal, D4 violated |

Post-probe full gate re-run on the restored tree: all five gates exit 0, `cargo test` = **360**
(324 + 11 + 25), porcelain clean.

### Verdict - Verifier

_(to be filled at verification: S4 clone SHA, five gate exits, 360-from-`test.log` copy,
write-guard diff, disclosed-edit diff, probe summary, `evidence/7.4/` bundle reference.)_
