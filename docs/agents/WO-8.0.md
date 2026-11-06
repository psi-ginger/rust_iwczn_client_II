# Work Order - Step 8.0: Input Core — `InputEvent`, `Intent`, Keybinds, Validation

**Status:** `ACCEPTED` — definition-before-implementation satisfied by the Custodian's Phase IV
Charter v0.2 sign-off (2026-11-06, `CONTRIBUTING_PHASE4.md` P4-C1…C4); this WO is committed on
`main` **before** any `src/**` byte moves. The next Implementer may pull and begin.
**Date:** 2026-11-06 · **Revision:** Rev 1 (initial, Scribe lane under directive).
**Branch:** `step/8.0-input-core`, to be cut from `main` at this WO's sign-off commit.
**Baseline (re-derived this session):** `main` @ `0458e95`, porcelain clean, all five mandatory
gates exit-0 — `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` under
`RUSTFLAGS=-D warnings`, `cargo fmt --check` 0 diffs, `cargo test` **360/360** (324 lib +
11 `asset_fixtures` + 25 `e2e_loopback`; 1 ignored lib + 4 ignored `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `TASK_MATRIX.md` Phase IV tracker (row 8.0 `PENDING`, lanes Implementer → Verifier
+ Adversary) · `CONTRIBUTING_PHASE4.md` (P4-C1 zero-queue, P4-C2 parity line, P4-C3 ≤ 16 ms /
< 50 ms, P4-C4 headless) · `src/client/commands.rs` (7.4 receiving half — frozen for this step) ·
`docs/agents/WO-7.4.md` D4/D7 · `src/game/action.rs` (`Order`, `ChatMode`, `MAX_CHAT_BYTES`) ·
`docs/agents/WO-6.3.md` §9 (mutation-probe precedent).

---

## 1. Provenance, governance, lane hats

- **Registration.** Rows 8.0–8.4 were opened and this WO authored in the Scribe lane by the
> Custodian's Phase IV Charter v0.2 sign-off directive (2026-11-06), in the same commit as
> `CONTRIBUTING_PHASE4.md` — definition-before-implementation, before any code.
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every load-bearing claim here
  is "refuses/discards" or "parity with legacy", dischargeable by verified mutation probes
  (§12), per `CONTRIBUTING_PHASE4.md` verification gates item 6.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs and
  `docs/**` is READ-ONLY — a defect found there is a finding `F-8.0-NN`, never an edit (P3-6;
  F-7.4-01 disclosure pattern if the compiler ever forces one).
- **Source-of-record split.** This step adds **no new wire bytes**: 240/117 layouts stay
  `WO-step7-in-world-actions.md` §8.3 (frozen), the sending half stays `src/client/commands.rs`
  (7.4, frozen — **`src/client/**` is READ-ONLY for all of Phase IV**), the chat cap stays
  `action.rs:53`. This WO owns only the **translator contract** (§7) and the expectation table
  in §8, each row traced to a legacy `file:line` or a named frozen constant (F-07).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 The receiving half as shipped (Step 7.4) — what this step plugs into

| Piece | As of `0458e95` | Where |
|---|---|---|
| `ClientCommand` | `Order(Order)` / `Chat { mode, text }`; "the mode travels exactly as given (no coercion)" | `src/client/commands.rs:17-27` |
| `execute` | async, on the loop task; `ActionError::{EmptyChat, ChatTooLong, Send}` | `commands.rs:37-44` |
| `CommandSender` | `Clone`, **synchronous** `send -> Result<(), CommandError::Closed>`; doc: *"clone it into whatever produces input (D7)"* | `commands.rs:54-70` |
| `channel()` | tokio unbounded mpsc; doc: *"sender is cloned into the input layer"* | `commands.rs:89-96` |
| Loop arm | `select!` → `Wake::Command` → `LoopEvent::Command` | `src/client/event_loop.rs:478`, `:534-535` |
| Driver handler | `command.execute(link).await?` → `InWorldError::Action` | `src/client/driver.rs:823-824` |
| D4 for input | "A future input layer validates before it enqueues (empty / oversized chat never reach the channel)" | `WO-7.4.md:130-132` |
| D7 for input | "Input is human-rate; a bounded channel would force a drop/block policy that silently loses intent — rejected" | `WO-7.4.md:143-148` |
| Frozen `Order` | `Move { x: u16, y: u16 }`, `Attack { target_char: u32 }`, `SitDown` (kind 2) | `src/game/action.rs:76-131`, `:63-74` |
| Chat | `ChatMode::Normal = 1` ordinary speech; `MAX_CHAT_BYTES = 255`; `encode_chat` refuses empty/over-cap, never clamps | `action.rs:148`, `:53`, `:198-214` |
| VK constants | **none exist anywhere in `src/**`** (tree grep 2026-11-06) — this WO defines them | §7.3 / D6 |

### 2.2 Constraints discovered (each forces a design choice or a documented divergence)

1. **F-7.4-01 pre-check.** `InputEvent`/`Intent` are new enums: at baseline no exhaustive match
   references them (grep `match …InputEvent|match …Intent` → empty). The grep is recorded in
   `evidence/8.0/` at verification; adding later variants (8.1/8.2) repeats it tree-wide.
2. **`src/client/**` is read-only (P4-C4/Phase IV write guard).** The translator depends only
   on the pub surface of `commands.rs` (`ClientCommand`, `CommandSender`) and `game::action`
   (`Order`, `ChatMode`, `MAX_CHAT_BYTES`) — all sufficient; no 7.4 file changes are needed or
   permitted.
3. **`src/lib.rs` needs the module** (`:34-39` lists `auth, client, game, graphics, net,
   protocol`): one `pub mod input;` in alphabetical position + one `//!` Phase IV bullet in the
   module list (row 8 of §6).
4. **Legacy fired Return/Insert on `WM_KEYUP`** (`mainfunction.cpp:1202`, cases `:1218`/
   `:1229`) and Escape on `WM_KEYDOWN` (`:1304`). The port binds **all** of them uniformly on
   `KeyDown` — one action per key press is the UX contract either way; the message-side
   difference is invisible to the user. Documented divergence (§8 note; Haru/CAPA — stated,
   never silent).
5. **Chat typing source.** Legacy typing lived inside the `CNChatBarDlg` edit control (a
   separate HWND receiving `WM_CHAR`), not the game `WindowProc` (whose `WM_CHAR` arm,
   `mainfunction.cpp:1261`, is map-edit only). The port has no controls, so typing enters as
   `InputEvent::CharInput` (decoded from `WM_CHAR` in 8.3) appended to the line buffer while
   chat is open — a stated port decision, not a legacy `WindowProc` port.
6. **Latency placement (P4-C3).** The ≤ 16 ms budget is enforced here structurally (no
   `.await`, no buffer between event and `send`) plus one in-gate batch test with a documented
   margin (§9 T10); end-to-end measurement is 8.4's evidence.

### 2.3 Legacy semantics inherited (why the bindings exist)

| Legacy behavior | Citation |
|---|---|
| `VK_RETURN` → focus the chat input box | `mainfunction.cpp:1218-1225` (`WM_KEYUP`) |
| Return in chat → send the typed line | `CNChatBarDlg` edit control → `cltClient::SendChatMessage` (`Client.cpp:3308`) |
| `VK_INSERT` → sit/stand **toggle**, state-checked (`GetCurrentOrder() == ORDER_SITDOWN`) | `mainfunction.cpp:1229-1249` (`WM_KEYUP`; sends `GAMEMSG_REQUEST_SITDOWN` — **not ported**, Q2 ruling) |
| `VK_ESCAPE` → close the active dialog / exit | `mainfunction.cpp:1304` (`WM_KEYDOWN`; `:1385` syskey) |
| `WM_MOUSEMOVE` → track position/hover only (no order) | `mainfunction.cpp:208` → `MouseMoveAction` (`Client.cpp:7687`) |
| Click arms gated until the loop is warmed up (`GetFrame() > 40`) | `mainfunction.cpp:221,236,254,288,302,1367` |
| Queue machinery (`cltKey`/`cltKeyManager` cap 50, coalescing) | `Client/Key/key.h:5,11,59`, `key.cpp:18,44` — **explicitly not ported** (P4-C1) |
| Arrow-key movement | `MouseOperation.cpp:287-520` inside a `/* … */` block (`:273`–`:522`) — dead code; arrows stay **unbound** |

## 3. Scope decision — this delivery

**In:**

1. `src/input/event.rs` — `InputEvent`, the raw OS-shaped value type (no `HWND`, no FFI).
2. `src/input/intent.rs` — `Intent` (game-level) + `UiState` (chat focus/line/in-world guard).
3. `src/input/keybinds.rs` — the three `VK_*` constants + `translate(event, &mut ui)` —
   the whole keybind table of §8 as code.
4. `src/input/validate.rs` — `validate(&intent) -> Option<ClientCommand>`: the D4 gate
   (bounds, chat cap, default mode) producing only frozen 7.3/7.4 types.
5. `src/input/mod.rs` — module docs + re-exports; `src/input/tests.rs` — T1–T11 (§9).
6. `src/lib.rs` — `pub mod input;` + one `//!` bullet (§2.2-3).
7. Eleven tests: baseline **360 → 371** (expected; the delivered number is copied from
   `test.log`).

**Out:** see §5.

## 4. Architectural decisions (binding for this WO)

- **D1 — direct feed, zero intermediate queue (P4-C1).** `translate` → `validate` →
  `CommandSender::send` is one synchronous call chain per event. No `VecDeque`, no ring, no
  capacity constant, no coalescing, no "temporary" buffer anywhere in `src/input/**` — the
  tokio unbounded mpsc is the only queue (it already exists, `commands.rs:93`). Its absence is
  a DoD item (grep-verified, §11).
- **D2 — two enum hops, both pure.** `InputEvent` (what the OS said) → `Intent` (what the
  player wants) → `ClientCommand` (what goes on the wire). UI state (`UiState`) is mutated only
  by `translate`; intents that only change UI (`ChatFocus`, `CloseDialog`, chat cancel) return
  `None` from `validate` and never enqueue.
- **D3 — validate before enqueue (WO-7.4 D4).** Negative/`> u16::MAX` tiles, empty chat, and
  lines over `MAX_CHAT_BYTES` (255, `action.rs:53`) are refused **before**
  `CommandSender::send` — the channel never sees them; the frozen execute-time refusals remain
  the last line of defense, not the user experience. Refusal = typed `None`/discarded intent,
  never a clamp, never a panic (`action.rs:198-214` style; `CONTRIBUTING_PHASE4.md` gate 2).
- **D4 — the in-world guard replaces `GetFrame() > 40`.** Every event is discarded while
  `UiState.in_world` is `false` — one boolean, no frame counter (P4-C2: machinery replaced,
  UX retained: no input before the game is up). 8.3 sets the flag at in-world stage entry.
- **D5 — the chat default mode is declared, not invented.** `SendChat` validates to
  `ClientCommand::Chat { mode: ChatMode::Normal, text }` (`action.rs:148`, "Ordinary speech",
  `MsgType-System.h:1085`) — channel *selection* UI is out of scope (Q3); downstream the mode
  travels uncoerced (`commands.rs:20`).
- **D6 — VK constants are defined fresh, in `src/input`.** The tree has no `VK_*` anywhere
  (§2.1); the three bound codes are named constants carrying their legacy case citations, plain
  `u32` values, zero FFI/crates (7.2 R1: `Cargo.toml` untouched).
- **D7 — latency (P4-C3): ≤ 16 ms per event, hard ceiling < 50 ms.** Enforced here by design
  (D1: no `.await`, no hop between event and `send`) and pinned by T10: 100 scripted events
  must complete translate+validate+enqueue **in under 16 ms total** — a batch-of-100
  within one frame is ≥ 10× headroom over the per-event budget (debug-mode enum + mpsc costs
  are µs-scale; margin rationale recorded in T10's doc comment and evidence). End-to-end
  input→wire figures are measured as 8.4 evidence.
- **D8 — moves never enqueue.** `InputEvent::Move` is hover/tracking material (8.1 uses it);
  `translate` returns `None` for it. This — plus OS-level `WM_MOUSEMOVE` coalescing — is why
  the legacy `key.cpp:18` coalescing problem cannot arise and must not be ported (P4-C1).

## 5. Non-goals

- **No queue machinery of any kind** (P4-C1): no `cltKeyManager`/`cltKey`/cap-50/coalescing/
  `Pop`-shift port — no such type may exist in `src/input/**`.
- No `screen_to_tile` math (8.1), no mouse click→intent mapping (8.1), no `window.rs`/`WM_*`
  decode (8.3), no `tests/e2e_loopback` wire assertions (8.3), no live runs (8.4).
- No edits to `src/client/**` (7.4 frozen receiving half), `src/game/**` (7.3 frozen),
  `src/graphics/**`, `tests/**`.
- No chat rendering/widget framework (Q3), no camera scroll (Q4), no arrow-key bindings
  (§2.3 dead code), no right/double-click (deferred), no new opcodes (Q2), no new crates (D6).
- No `in_world` production wiring (8.3) — `UiState.in_world` is set by tests here and by the
  pump wiring later.

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change |
|---:|---|---|
| 1 | `docs/agents/WO-8.0.md` | NEW — this document, committed **before** any code |
| 2 | `src/input/mod.rs` | NEW — module docs (pipeline diagram, charter cites) + re-exports + `#[cfg(test)] mod tests;` |
| 3 | `src/input/event.rs` | NEW — `InputEvent` (§7.1) |
| 4 | `src/input/intent.rs` | NEW — `Intent` + `UiState` (§7.2) |
| 5 | `src/input/keybinds.rs` | NEW — `VK_*` constants + `translate` (§7.3) |
| 6 | `src/input/validate.rs` | NEW — `validate` (§7.4) |
| 7 | `src/input/tests.rs` | NEW — T1–T11 (§9) |
| 8 | `src/lib.rs` | EDIT — one `pub mod input;` (`:34-39`, alphabetical) + one `//!` Phase IV bullet |
| 9 | `evidence/8.0/` | NEW at verification, written by the Verifier only (§11) |

**READ-ONLY:** all of `src/client/**`, `src/game/**`, `src/graphics/**`, `src/auth/**`,
`src/net/**`, `src/protocol/**`, `src/main.rs`, all of `tests/**`, `Cargo.toml`/`Cargo.lock`,
`TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, every other file in
`docs/**`, every existing `evidence/**` directory. A defect found there is a finding, never an
edit (P3-6).

## 7. API & wiring spec (the shared contract)

### 7.1 `src/input/event.rs`

```rust
/// One raw input fact, decoded from a window message (8.3) or built by a test.
/// Pure data: no HWND, no FFI, `Copy` — the legacy `cltKey`'s payload without
/// the legacy queue (`WO-8.0` D1: the struct survives, the queue does not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// Pointer moved to screen `(x, y)` (`WM_MOUSEMOVE`, `mainfunction.cpp:208`).
    /// Hover/tracking only — never enqueues (D8).
    Move { x: i32, y: i32 },
    /// Left button pressed at `(x, y)` (`mainfunction.cpp:232`; 8.1 owns clicks).
    LeftDown { x: i32, y: i32 },
    /// Left button released (`mainfunction.cpp:251`; 8.1).
    LeftUp { x: i32, y: i32 },
    /// A virtual-key press (`WM_KEYDOWN`, `mainfunction.cpp:1282`).
    KeyDown { vk: u32 },
    /// A virtual-key release (`WM_KEYUP`, `mainfunction.cpp:1202`) — every
    /// binding fires on `KeyDown` (§2.2-4); `KeyUp` is accepted and ignored.
    KeyUp { vk: u32 },
    /// A character for the chat line (`WM_CHAR` → line buffer; §2.2-5).
    CharInput { ch: char },
}
```

### 7.2 `src/input/intent.rs`

```rust
/// What the player wants, after keybind translation — the middle hop (D2).
/// Full surface declared now so 8.1/8.2 extend behavior, not the enum (F-7.4-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Walk to map tile `(x, y)` — screen math may be negative; the `u16`
    /// range check happens in `validate` (D3), never in the math.
    MoveTo { x: i32, y: i32 },
    /// Attack a resolved target (produced by 8.1's hit-test, not by `translate`).
    AttackTarget { target_char: u32 },
    /// Open the chat line (Enter, chat closed).
    ChatFocus,
    /// Flush the typed line (Enter, chat open) — validated to `ClientCommand::Chat` (D5).
    SendChat { line: String },
    /// Insert: sit toggle → frozen `Order::SitDown` (Q2: the server toggles).
    SitToggle,
    /// Escape with no dialog state: close/cancel signal (UI-only in 8.0).
    CloseDialog,
}

/// Translator-local UI state — the legacy dialog flags' modern replacement,
/// owned by the input layer; nothing else mutates it in Phase IV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    /// In-world stage reached (8.3 sets it; replaces `GetFrame() > 40`, D4-guard).
    pub in_world: bool,
    /// Chat line open (Enter toggles).
    pub chat_open: bool,
    /// The typed line; cleared on flush and on cancel (Q3: line buffer only).
    pub line: String,
}

impl Default for UiState { /* in_world: false, chat_open: false, line: String::new() */ }
```

### 7.3 `src/input/keybinds.rs`

```rust
/// Bound virtual keys — standard Win32 `VK_*` values (the legacy cases use the
/// SDK macros at these lines): RETURN `mainfunction.cpp:1218`,
/// INSERT `mainfunction.cpp:1229`, ESCAPE `mainfunction.cpp:1304`.
pub const VK_RETURN: u32 = 0x0D;
pub const VK_INSERT: u32 = 0x2D;
pub const VK_ESCAPE: u32 = 0x1B;

/// The whole keybind table as code: event + state → optional intent (D2).
/// Pure; the only side effect is `UiState`. Never panics (P4-C4 invariant 3).
pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> { … }
```

`translate` semantics — exhaustive, every row tested (§8):

| `InputEvent` (after the `!in_world → None` guard, D4) | `UiState` effect | Result |
|---|---|---|
| `KeyDown { vk: VK_RETURN }`, chat closed | `chat_open = true`, `line` cleared | `Some(ChatFocus)` |
| `KeyDown { vk: VK_RETURN }`, chat open | `chat_open = false`, `line` taken (cleared) | `Some(SendChat { line })` |
| `KeyDown { vk: VK_ESCAPE }`, chat open | `chat_open = false`, `line` cleared | `None` (cancel — UI-only) |
| `KeyDown { vk: VK_ESCAPE }`, chat closed | unchanged | `Some(CloseDialog)` |
| `KeyDown { vk: VK_INSERT }`, chat closed | unchanged | `Some(SitToggle)` |
| `KeyDown { vk: VK_INSERT }`, chat open | unchanged | `None` (typing state swallows it) |
| `KeyDown { other }` | unchanged | `None` (unknown keys discarded) |
| `KeyUp { … }` | unchanged | `None` (§2.2-4) |
| `CharInput { ch }`, chat open | `line.push(ch)` (uncapped — D3 refuses at flush, never truncates) | `None` (state change only) |
| `CharInput { ch }`, chat closed | unchanged | `None` |
| `Move { .. }` | unchanged (8.1 adds hover) | `None` (D8) |
| `LeftDown` / `LeftUp` | unchanged | `None` (8.1 owns clicks) |
| **any**, `!in_world` | unchanged | `None` (checked first) |

### 7.4 `src/input/validate.rs`

```rust
/// Intent → wire-level command, or `None` = typed refusal before enqueue (D3).
/// Produces only frozen 7.3/7.4 types; never coerces, never clamps.
pub fn validate(intent: &Intent) -> Option<ClientCommand> { … }
```

| `Intent` | Result |
|---|---|
| `MoveTo { x, y }`, `0 <= x <= u16::MAX` and `0 <= y <= u16::MAX` | `Some(ClientCommand::Order(Order::Move { x: x as u16, y: y as u16 }))` |
| `MoveTo` otherwise (negative or `> u16::MAX`) | `None` |
| `AttackTarget { target_char }` | `Some(ClientCommand::Order(Order::Attack { target_char }))` |
| `SitToggle` | `Some(ClientCommand::Order(Order::SitDown))` — Q2, kind 2, no payload |
| `SendChat { line }`, empty | `None` (mirrors `action.rs:202-204`; never reaches `send`) |
| `SendChat { line }`, `line.len() > MAX_CHAT_BYTES` (255) | `None` (mirrors `action.rs:205-210`; **never truncated**) |
| `SendChat { line }`, otherwise | `Some(ClientCommand::Chat { mode: ChatMode::Normal, text: line.clone() })` (D5) |
| `ChatFocus` / `CloseDialog` | `None` (UI-only, D2) |

Imports: `crate::client::commands::{ClientCommand, CommandSender}` (send is used by callers
and T10 only — `validate` itself stays pure), `crate::game::action::{ActionError-constants,
ChatMode, Order, MAX_CHAT_BYTES}`. Every `pub` item gets a doc comment (house style).

## 8. Expectation table (the F-07 source for §9)

Tests assert **these rows**; no expectation may be derived from `translate`/`validate` under
test. Each row is a legacy `file:line` (dual source: the citation + the frozen constant named).

| # | Behavior (as tested) | Provenance |
|---|---|---|
| **E1** | Enter, chat closed → `ChatFocus` (open) | `mainfunction.cpp:1218-1225` (Return focuses chat) |
| **E2** | Enter, chat open → `SendChat { line }` + close | `Client.cpp:3308` (`SendChatMessage`); Q3 line buffer |
| **E3** | Escape, chat open → cancel (clear, close, no intent) | `mainfunction.cpp:1304` (Escape exits the active input/dialog) |
| **E4** | Escape, chat closed → `CloseDialog` | `mainfunction.cpp:1304` / `:1385` |
| **E5** | Insert → `SitToggle` → `Order::SitDown` (kind 2, no payload) | `mainfunction.cpp:1229-1249` state-checked toggle; **Q2 ruling** → frozen `action.rs:99-100` |
| **E6** | Unknown `KeyDown` / any `KeyUp` / clicks → discarded, no panic | port decision (D2/D8); legacy `key.cpp` queue not ported (P4-C1) |
| **E7** | Pre-`in_world` events → discarded | `mainfunction.cpp:221,236,254,288,302,1367` (`GetFrame() > 40`) → D4 boolean |
| **E8** | `Move` stream → zero intents/commands | `mainfunction.cpp:208` → `MouseMoveAction` (`Client.cpp:7687`) is position-only |
| **E9** | Tile `-1` / `65536` refused; `0` and `65535` pass as `u16` | `action.rs:87-94` (`Order::Move { x: u16, y: u16 }`) — frozen type bound |
| **E10** | Empty / 256-byte line refused; 255-byte line passes; nothing truncated | `action.rs:53` (`MAX_CHAT_BYTES = 255`), `:198-214` (refuse-never-clamp) |
| **E11** | Flushed chat carries `ChatMode::Normal` (`= 1`) | `action.rs:148` (Ordinary speech, `MsgType-System.h:1085`) — D5 declared default |
| **E12** | 100 scripted events complete translate+validate+enqueue in `< 16 ms` | `CONTRIBUTING_PHASE4.md` P4-C3 (Q5 ruling: ≤ 16 ms / 1 frame @ 60 FPS) |

**Stated divergences (Haru/CAPA — documented, never silent):** (a) legacy fired Return/Insert
on `WM_KEYUP` (`mainfunction.cpp:1202/:1218/:1229`); the port binds `KeyDown` uniformly
(§2.2-4). (b) `VK_INSERT` no longer sends the dedicated `GAMEMSG_REQUEST_SITDOWN` opcode —
Q2 routes it to frozen `Order::SitDown`. (c) `GetFrame() > 40` becomes the `in_world` boolean.

## 9. Test invariants (T1–T11) and the baseline

Pattern authorities: colocated unit tests (`commands/tests.rs`), pure-function tables
(`src/game/action/tests.rs`), typed-refusal assertions (`action.rs:198-214` callers). F-07:
expectations from §8 only. All tests build `InputEvent`/`Intent` values directly — no window,
no Win32, no network (P4-C4).

| # | Invariant | File | Phase |
|---|---|---|---|
| **T1** | `enter_focuses_chat_then_flushes_the_typed_line` — E1: `KeyDown RETURN` → `ChatFocus` + `chat_open`; two `CharInput`s accumulate; second `KeyDown RETURN` → `SendChat { line: "hi" }` + closed + line emptied | `src/input/tests.rs` | unit |
| **T2** | `escape_while_typing_cancels_the_line_without_a_command` — E3: open, type, `KeyDown ESCAPE` → `None`, `chat_open = false`, `line` empty (cancel is UI-only) | `src/input/tests.rs` | unit |
| **T3** | `escape_with_chat_closed_emits_close_dialog` — E4: `KeyDown ESCAPE` → `CloseDialog`, and `validate` returns `None` for it (UI-only) | `src/input/tests.rs` | unit |
| **T4** | `insert_maps_to_the_frozen_sit_down_order` — E5: `KeyDown INSERT` → `SitToggle` → `validate` → `Some(ClientCommand::Order(Order::SitDown))` (Q2 pin) | `src/input/tests.rs` | unit |
| **T5** | `unknown_and_unbound_input_is_discarded_without_panic` — E6: unknown `KeyDown` (e.g. F1 `0x70`), `KeyUp { VK_RETURN }`, `LeftDown`, `LeftUp`, `CharInput` with chat closed — all `None`, `UiState` unchanged | `src/input/tests.rs` | unit |
| **T6** | `events_before_the_in_world_guard_are_discarded` — E7: with `in_world = false`, each of the six event kinds (incl. a valid Enter/Insert pair) → `None` and `UiState` byte-identical after | `src/input/tests.rs` | unit |
| **T7** | `move_streams_never_enqueue_a_command` — E8: 1 000 `Move` events through `translate` → 1 000 `None`, zero intents, zero `CommandSender::send` calls (D8: why no coalescing exists) | `src/input/tests.rs` | unit |
| **T8** | `move_to_refuses_tiles_outside_the_u16_range` — E9: `(-1, 0)`, `(0, -5)`, `(65_536, 0)`, `(0, 65_536)` → `validate` `None`; `(0, 0)` and `(65_535, 65_535)` → `Order::Move` with exact `u16`s | `src/input/tests.rs` | unit |
| **T9** | `chat_flush_refuses_empty_and_over_cap_lines_without_clamping` — E10: `""` → `None`; a 256-byte line → `None`; a 255-byte line → `Some`; the refused `Intent`'s line is unchanged (no truncation) | `src/input/tests.rs` | unit |
| **T10** | `the_enqueue_path_stays_within_the_16ms_frame_budget` — E12/D7: fresh `commands::channel()`; 100 scripted events (Enter/type/flush cycles + unknown keys + moves) run through `translate` → `validate` → `sender.send`, `Instant`-measured, **< 16 ms** total; afterwards the receiver drains to exactly the expected `Some`-count (all enqueued, none lost). Doc comment carries the margin rationale (batch-of-100 inside one frame = ≥10× per-event headroom); on any flake the finding is disclosed, never relaxed silently | `src/input/tests.rs` | unit |
| **T11** | `chat_flush_uses_the_declared_default_mode` — E11/D5: `SendChat { "hi" }` → `Chat { mode: ChatMode::Normal, text: "hi" }` exactly (mode `= 1`; no coercion anywhere in the hop) | `src/input/tests.rs` | unit |

**Baseline: 360 → 371 expected** (+11 = T1–T11, all lib tests; `asset_fixtures` 11 and e2e 25
untouched; the 5 ignored tests preserved). The matrix cell is copied from the delivered
`test.log`, never from this prose.

## 10. Forbidden shortcuts

- Any queue/ring/capacity/coalescing structure in `src/input/**` — including a "temporary"
  buffer between `translate` and `send` (D1/P4-C1); any port of `cltKey`/`cltKeyManager`/
  `key.cpp` logic.
- Editing `src/client/**`, `src/game/**`, `src/graphics/**`, `tests/**` or any READ-ONLY file
  of §6 to make a test pass (finding first, P3-6; F-7.4-01 only if the compiler forces it —
  disclosed).
- Deriving an expectation from `translate`/`validate` under test (F-07) — §8 rows only.
- Clamping an over-cap line or an out-of-range tile instead of refusing; coercing `ChatMode`;
  panicking/discarding-with-silence where §7 names a typed result (D3/D5).
- A `.await`, channel hop, or buffer in the enqueue path (D7), or a latency test without its
  documented margin (P4-C3).
- New crates (`Cargo.toml` untouched), new opcodes (Q2), arrow-key bindings (§2.3),
  `in_world` production wiring (8.3's job), live-server runs promoted to gates.
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*` (Verifier/Scribe only), `README.md`,
  `GoonZu_Source/**`, any other WO or audit doc.

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/8.0/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
  `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`, `zero-queue-grep.txt`,
  `f7401-precheck.txt`, `write-guard.txt`, `SHA256SUMS` — hashes over the committed (LF) bytes
  (6.3 L-2 lesson); the matrix `Tests Baseline` cell is copied from `test.log` (**371** expected:
  335 lib + 11 + 25).
- **Zero-queue audit:** `grep -inE 'VecDeque|ring|coalesc|MAX_KEY|cltKey' src/input/` → no
  matches, captured in `zero-queue-grep.txt` (P4-C1, Phase IV gate 4).
- **F-7.4-01 pre-check:** whole-tree grep showing no pre-existing exhaustive match over the new
  enums (`f7401-precheck.txt`), plus the `src/lib.rs` diff limited to the two §6-row-8 lines.
- Write-guard confirmation: `git diff --stat` limited to §6; `src/client|game|graphics|auth|
  net|protocol`, `tests/**`, `Cargo.*`, matrix, contributing, README and the legacy tree
  untouched.
- Four mutation probes recorded (§12): mutated diff, failing output, pristine restore, porcelain
  CLEAN after each, baseline re-run green afterwards.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test | Restore |
|---:|---|---|---|
| 1 | `validate`: clamp instead of refuse — for over-cap lines `line.truncate(MAX_CHAT_BYTES)` then return `Some(...)` | **T9** (256-byte line now enqueues) | `git checkout --` |
| 2 | `translate`: delete the `!in_world → None` guard (input flows before the world is up) | **T6** (pre-world Enter/Insert produce intents) | `git checkout --` |
| 3 | `translate`: unknown-key arm falls through to `Some(Intent::SitToggle)` instead of `None` | **T5** (F1 press emits `SitToggle`) | `git checkout --` |
| 4 | `validate`: `SendChat` forced to `ChatMode::Secret` (mode coercion, D5 violated) | **T11** (`mode: Normal` expected) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit 101
and the failing assertion, restore, verify `git status --porcelain` clean — then re-run the full
gate set to the delivered baseline (`WO-6.3.md` §9 / `evidence/7.x/mutation-probes.md`
discipline).

## 13. Definition of done

A Step 8.0 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at the delivered baseline from `test.log` (**371** expected:
   335 lib + 11 fixtures + 25 e2e), `fmt` 0 diffs, `doc` 0 warnings, porcelain clean.
2. T1–T11 all present and green; **no pre-existing test modified** (this step adds only new
   files plus the two-line `src/lib.rs` edit — unlike 7.4 there are no disclosed hunks unless
   F-7.4-01 forces one, which would be disclosed in the delivery note).
3. Every decision D1–D8 is pinned by a failing-if-corrected test or probe (T1–T11 + probes 1–4).
4. All four §12 probes show their named test failing at exit 101, with pristine restores and a
   green post-probe baseline.
5. Write guard holds: only §6 files changed; `src/client/**`, matrix, contributing, README,
   `src/game/**` and the legacy tree untouched (row update, if any, is the Verifier's).
6. F-07 holds: every expectation traces to an E-row in §8, and every E-row traces to a legacy
   `file:line` or a frozen constant — never to the code under test.
7. Zero-queue audit green (§11 grep, no matches) and the D7 structural claim holds (no
   `.await`/buffer in the enqueue path; T10 green with its margin documented).
8. No new crates, no new opcodes, no wire-byte changes; no window/Win32/network in any gate
   test; no live run as a gate.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

- **Branch:** `step/8.0-input-core` (cut from `main` @ `d1a5267`), delivery SHA **`fc1a7c7`**
  — implementation only; this note lands as a follow-up docs commit on the same branch.
- **Baseline:** 360 → **371** (+11 = T1–T11, all lib; `asset_fixtures` 11 and e2e 25
  untouched; 1 ignored lib + 4 ignored `live_integration` preserved), counted from `cargo
  test` output, never from prose.

**Gates at the delivery SHA, and re-run post-probe on the pristine tree (both runs green):**

| Gate | Command | Result |
|---|---|---|
| 1 | `RUSTFLAGS=-D warnings cargo check --all-targets` | exit 0 |
| 2 | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 3 | `cargo fmt --check` | exit 0 — 0 diffs |
| 4 | `cargo doc --no-deps` | exit 0 — 0 warnings |
| 5 | `cargo test` | exit 0 — **371 passed / 0 failed** (335 lib + 11 fixtures + 25 e2e) |

**T1–T11, each run individually** (`cargo test --lib <name>` → `1 passed; 0 failed`):
`enter_focuses_chat_then_flushes_the_typed_line`,
`escape_while_typing_cancels_the_line_without_a_command`,
`escape_with_chat_closed_emits_close_dialog`,
`insert_maps_to_the_frozen_sit_down_order`,
`unknown_and_unbound_input_is_discarded_without_panic`,
`events_before_the_in_world_guard_are_discarded`,
`move_streams_never_enqueue_a_command`,
`move_to_refuses_tiles_outside_the_u16_range`,
`chat_flush_refuses_empty_and_over_cap_lines_without_clamping`,
`the_enqueue_path_stays_within_the_16ms_frame_budget`,
`chat_flush_uses_the_declared_default_mode`.

**Disclosed edits:** none beyond §6 — no pre-existing test was modified (unlike 7.4 there
are no forced hunks); the whole delivery is the six new `src/input/**` files plus the
two-line `src/lib.rs` edit (exactly 2 insertions, 0 deletions: one `//!` Phase IV bullet +
one `pub mod input;` in alphabetical position).

**Findings:**

- **F-8.0-01 (disclosed, never edited — §1 write guard):** WO §11's literal zero-queue
  pattern `grep -inE 'VecDeque|ring|coalesc|MAX_KEY|cltKey' src/input/` cannot return "no
  matches" while §7's contract is delivered verbatim: `cltKey` appears in §7.1's mandated
  doc comment ("the legacy `cltKey`'s payload…", documenting what was *not* ported) and the
  bare `ring` alternation substring-matches the §7.2-mandated `String` type. Observed: 5
  matches — `event.rs:6` (the §7.1 citation), `intent.rs:28/:48` (the two mandated
  `String` fields), `intent.rs:40` (restates §7.2's `Default` sketch), `tests.rs:259`
  (T9's empty-line stimulus). The audit's *intent* holds: the machinery subset
  `VecDeque|coalesc|MAX_KEY` has **zero** matches, and the word-bounded variant
  (`\bring\b`) leaves only the §7.1 `cltKey` citation — no queue/ring/cap/coalescing type
  exists in `src/input/**` (six pure files, reviewed). Adjudication is the Verifier's.
- **F-7.4-01 pre-check (§2.2-1):** whole-tree grep at baseline was **empty** for matches
  over `InputEvent`/`Intent`; after delivery, matches occur only inside `src/input/**`
  (`keybinds.rs`, `validate.rs`, `tests.rs`) — no other exhaustive match in the tree.

**Implementation notes:** `UiState`'s `Default` is `#[derive(Default)]` with values
identical to §7.2's sketch (`false`/`false`/empty) — a manual impl risks
`clippy::derivable_impls` under `-D warnings`; `mod.rs` uses the disambiguated
`[`validate()`]` intra-doc link (module + re-exported fn share the name — the doc gate
required it). T10's margin rationale (batch-of-100 inside one frame = ≥10× per-event
headroom) is in its doc comment; no `.await`/buffer exists between `InputEvent` and
`CommandSender::send` (D7 structural claim), `Cargo.toml` untouched, no window/Win32/
network in any gate test (P4-C4), no expectation derived from the code under test (F-07:
every assertion cites its §8 E-row).

**§12 mutation probes: 4/4** — each: one semantic break → named test fails at **exit 101**
→ `git checkout --` → `git status --porcelain` clean → full gate re-run green at 371:

| # | Mutation | Named test | Failing assertion observed |
|---:|---|---|---|
| 1 | `validate`: over-cap line truncated then `Some(...)` | **T9** | `E10: a 256-byte line is refused, never truncated` — `left: Some(Chat{…255×"x"…})`, `right: None` |
| 2 | `translate`: the `!in_world → None` guard deleted | **T6** | `E7: KeyDown { vk: 13 } before in_world must be discarded` — `left: Some(ChatFocus)`, `right: None` |
| 3 | `translate`: unknown-key arm → `Some(SitToggle)` | **T5** | `E6: KeyDown { vk: 112 } must be discarded without panic` — `left: Some(SitToggle)`, `right: None` |
| 4 | `validate`: `SendChat` forced to `ChatMode::Secret` | **T11** | `E11/D5: the declared default travels uncoerced` — `left: Chat{mode: Secret}`, `right: Chat{mode: Normal}` |

Post-probe full gate re-run on the restored tree: all five gates exit 0, `cargo test` =
**371** (335 + 11 + 25), porcelain clean.

### Verdict - Verifier

**Verified 2026-11-07 (Verifier, S4 discipline). Verdict: `PASSED`.**

- **S4 clone:** fresh clone of `git@github.com:psi-ginger/rust_iwczn_client_II.git`, checkout at
  delivery SHA **`fc1a7c7`** (tree `30f9bfc`); `git status --porcelain` clean before and after every
  run. Branch tip `89b9a9d` is docs-only — `git diff --name-only fc1a7c7 89b9a9d` = this file alone —
  so `src/`, `tests/` and `Cargo.*` at the tip are byte-identical to the gated SHA.
- **Five gates × 2 runs (pre-probe and post-probe) — all exit 0:**

| Run | check (`-D warnings`) | clippy (`-D warnings`) | fmt | doc | test |
|---:|---:|---:|---:|---:|---|
| 1 (pre-probe) | 0 | 0 | 0 — 0 bytes | 0 — 0 warnings | 0 — **371 passed / 0 failed** |
| 2 (post-probe, restored tree) | 0 | 0 | 0 — 0 bytes | 0 — 0 warnings | 0 — **371 passed / 0 failed** |

- **`Tests Baseline` copy target:** **`371/371`** from `evidence/8.0/test.log` — 335 lib +
  11 `asset_fixtures` + 25 `e2e_loopback`, 5 ignored preserved (1 lib + 4 live), 0 failed. Matrix
  cell `360/360 + est. 10–14` → **`371/371`**.
- **Evidence:** `evidence/8.0/` — 19 hashed files + `SHA256SUMS`, manifest verified **19/19** over
  the committed LF bytes (UTF-8 no-BOM, CR=0, hashed from working bytes = blob bytes, L-2 lesson):
  the §11 standing set (`head.txt`, `check.log`, `clippy.log`, `fmt.log`, `doc.log`, `test.log`,
  `env.txt`, `summary.md`, `mutation-probes.md`, `zero-queue-grep.txt`, `f7401-precheck.txt`,
  `write-guard.txt`) plus `gate-exits.txt` and `postprobe/` (run 2), mirroring the 7.4 layout.
- **Write guard (DoD 5):** `write-guard.txt` — exactly the seven §6 files (6 NEW `src/input/**` +
  `src/lib.rs` 2 insertions / 0 deletions); every READ-ONLY class untouched (`src/client|game|
  graphics|auth|net|protocol`, `src/main.rs`, `tests/**`, `Cargo.*`, `TASK_MATRIX.md`,
  `CONTRIBUTING*`, `README`, `GoonZu_Source/**`, `docs/**`, existing `evidence/**`); delivered file
  set == §6 file plan; no pre-existing test modified.
- **Zero-queue (DoD 7, P4-C1):** `zero-queue-grep.txt` — machinery subset `VecDeque|coalesc|MAX_KEY`
  **0 matches**, whole-word `\bring\b` **0**, collection tokens in the five production files **0**
  (only T10's harness `Vec::with_capacity(100)` exists), async code in production **0** (doc prose +
  the two `#[tokio::test]` harnesses only). D7 structural claim holds: no `.await`/buffer between
  `InputEvent` and `CommandSender::send`; T10 green with its documented margin.
- **F-7.4-01 pre-check (DoD 2):** `f7401-precheck.txt` — **EMPTY**: variant-qualified search outside
  `src/input/**` = 0; the only bare mentions are prose (`src/game/action.rs:6` doc table) and the
  delivery's own `src/lib.rs:30` doc line. No compiler-forced edit, no disclosed hunks beyond §6.
- **Probes (DoD 4):** `mutation-probes.md` — **4/4** at exit 101, each failing its named test with
  the E-row assertion, then `git checkout --` + empty porcelain: P1 `E10` (`Some(Chat{255×"x"})` vs
  `None`), P2 `E7` (`Some(ChatFocus)` vs `None`), P3 `E6` (`Some(SitToggle)` vs `None`), P4 `E11/D5`
  (`Chat{mode: Secret}` vs `Chat{mode: Normal}`); post-probe gates green at 371.
- **F-07:** 43 assertions, 37 E-row citations covering **E1–E12** — every expectation is the WO's
  frozen §8 text (VK lines from `mainfunction.cpp:1218/1229/1304`, `MAX_CHAT_BYTES` from
  `action.rs`), never derived from the code under test.
- **Headlessness (DoD 8):** Win32/FFI tokens appear only in two doc lines asserting their absence;
  network tokens 0; `unsafe` 0; `Cargo.toml` untouched (no new crates), no `protocol/**` change (no
  new opcodes), no live run as a gate.

**F-8.0-01 — adjudicated, NON-BLOCKING (a §11 prose/pattern defect, not a delivery defect):** the
literal audit command `grep -inE 'VecDeque|ring|coalesc|MAX_KEY|cltKey' src/input/` returns 5
matches and can never return "none" while §7's contract is delivered verbatim — `event.rs:6` is the
§7.1-*mandated* `cltKey` citation (WO line 190 mandates that exact line), the bare `ring`
alternation substring-matches the §7.2-mandated `String` type (3 hits) plus T9's `String::new()`
stimulus; as written the command also needs `-r` (GNU grep refuses a bare directory operand). The
audit's *intent* — no queue/ring/cap/coalescing machinery (P4-C1) — holds on every machinery probe
above. **Disposition: the delivered code stands; the §11 grep line is defective and is deferred to
the Scribe lane** for correction (WO-7.1 §9 precedent: implementer honest, WO left unedited, defect
recorded rather than silently fixed). Two minor disclosures recorded in `evidence/8.0/summary.md`:
the delivery note's "word-bounded `\bring\b` leaves only the cltKey citation" is imprecise (plain
`\bring\b` matches nothing); and the Verifier's own per-probe metadata line was reworded after a
regex false positive on cargo's `error: test failed` harness marker (raw outputs unmodified — no
rustc `error[E…]` signature exists in any probe output).

**DoD §13 items 1–8:**

| # | Item | Result |
|---:|---|---|
| 1 | Five gates exit 0 at the delivered baseline, fmt 0 diffs, doc 0 warnings, porcelain clean | **PASS** — 371 (335+11+25), both runs |
| 2 | T1–T11 present and green; no pre-existing test modified | **PASS** — 11/11 in the `input::tests::` run (names listed in `mutation-probes.md`); `tests/**` untouched |
| 3 | D1–D8 pinned by failing-if-corrected tests or probes | **PASS** — D1: T7+grep · D2: T1–T3 · D3: T8/T9+P1 · D4: T6+P2 · D5: T11+P4 · D6: T4 · D7: T10+no-await grep · D8: T7 |
| 4 | 4 §12 probes fail their named test at exit 101, pristine restores, green post-probe baseline | **PASS** — 4/4, `mutation-probes.md` + `postprobe/` |
| 5 | Write guard holds (only §6 files; protected classes untouched) | **PASS** — `write-guard.txt` |
| 6 | F-07 holds (expectations from §8 E-rows / legacy citations, never the code under test) | **PASS** — E1–E12 all cited |
| 7 | Zero-queue audit green (§11 grep, no matches) and D7 structural claim holds | **PASS with F-8.0-01 adjudicated** — all machinery greps empty; the literal §11 pattern defect is dispositioned above, non-blocking |
| 8 | No new crates/opcodes/wire-byte changes; no Win32/network in gate tests; no live run as a gate | **PASS** |

**Verdict: `PASSED`.** Matrix direction (for the Verifier's `docs(qc)` governance commit, on
Custodian acceptance): row **8.0** → `PASSED`, Verified Commit **`fc1a7c7`** (branch tip `89b9a9d`,
docs-only §14 delivery note), Tests Baseline **`371/371`**, evidence **`evidence/8.0/`**. Row 8.0
stays `PENDING` until that acceptance: this block and the bundle are uncommitted at report time —
no matrix flip and no push executed by this verdict.
