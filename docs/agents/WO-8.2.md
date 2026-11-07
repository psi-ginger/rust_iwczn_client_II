# Work Order - Step 8.2: Keyboard Hotkeys — Auto-Repeat Guard, Dialog Close, Line Editing

**Status:** `ACCEPTED` — definition-before-implementation satisfied by this WO's commit on
`main` (Scribe lane, under the Custodian's Step 8.2 directive, 2026-11-07) **before** any
`src/**` byte moves. The Implementer may pull and begin.
**Date:** 2026-11-07 · **Revision:** Rev 1 (initial, Scribe lane under directive).
**Branch:** `step/8.2-keyboard-hotkeys`, to be cut from `main` at this WO's commit.
**Baseline (re-derived this session):** `main` @ `7d76490`, porcelain clean, all five mandatory
gates exit-0 — `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` under
`RUSTFLAGS=-D warnings`, `cargo fmt --check` 0 diffs, `cargo test` **371/371** (335 lib +
11 `asset_fixtures` + 25 `e2e_loopback`; 1 ignored lib + 4 ignored `live_integration`),
`cargo doc --no-deps` 0 warnings.
**Reference:** `TASK_MATRIX.md` Phase IV tracker (row 8.2 `PENDING`, est. +12–16, lanes
Implementer → Verifier + Adversary) · `CONTRIBUTING_PHASE4.md` (P4-C1…C4; non-goals: arrow keys
remain unbound) · `docs/agents/WO-8.0.md` (§7/§8/§9 contract this step hardens; §14 verdicts) ·
`docs/agents/WO-8.0.md` finding F-8.0-01 (§11 grep disposition applied here) ·
`src/game/action.rs` (`Order::SitDown`, `MAX_CHAT_BYTES`) · `docs/agents/WO-6.3.md` §9
(mutation-probe precedent).

---

## 1. Provenance, governance, lane hats

- **Registration & the two scope rulings (Custodian, 2026-11-07).**
  1. **Sequence disclosure:** row 8.1 (mouse) is still `PENDING` and first in sequence, but it
     is **being implemented in a parallel lane on another machine** (Custodian ruling); this
     directive proceeds with 8.2 directly. The matrix is untouched by this WO — status is the
     Verifier's to set, per `TASK_MATRIX.md` header rule 4.
  2. **Overlap resolution:** Step 8.0 already delivered and tested the three bindings
     (`VK_RETURN` focus/flush, `VK_INSERT` → frozen `Order::SitDown`, `VK_ESCAPE` →
     `CloseDialog`; T1–T11). Re-delivering them would be redundant, so the Custodian ruled
     WO-8.2's scope = **hardening + legacy gaps**: (a) an auto-repeat guard, (b) an Escape
     dialog-close state machine per the legacy `NEXIT_DLG` close/cancel semantics, (c) chat-line
     editing (Backspace) + control-character filtering, (d) explicit pins for unbound arrow keys
     and the legacy no-op keys. Est. +12–16 tests, matching the matrix row.
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every load-bearing claim here
  is "parity with legacy" or "discards/refuses", dischargeable by verified mutation probes
  (§12), per `CONTRIBUTING_PHASE4.md` verification gates item 6.
- **Write guard:** only the files in §6. Everything else in `src/**`, `tests/**`,
  `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`, `GoonZu_Source/**`, the other WOs and
  `docs/**` is READ-ONLY — a defect found there is a finding `F-8.2-NN`, never an edit (P3-6;
  F-7.4-01 disclosure pattern if the compiler ever forces one).
- **Parallel-lane guarantee (8.1):** the mouse arms of `translate`
  (`InputEvent::Move | LeftDown | LeftUp` → `None`, `keybinds.rs:93-96`) and
  `InputEvent::Move/LeftDown/LeftUp` (`event.rs:10-31`) are **byte-for-byte untouched** by this
  delivery — a DoD item (§13.5), because another lane is writing 8.1's mouse behavior against
  this shared `src/input/` surface.
- **Source-of-record split.** This step adds **no new wire bytes**: 240/117 layouts stay
  `WO-step7-in-world-actions.md` §8.3 (frozen), the sending half stays `src/client/commands.rs`
  (7.4, frozen — `src/client/**` READ-ONLY for all of Phase IV), the chat cap stays
  `action.rs:53`, and `Intent`/`InputEvent`/`validate` stay exactly as 8.0 delivered them
  (§7.3). This WO owns only the `UiState` extension (§7.1) and the `translate` behavior
  changes (§7.2), each row traced to a legacy `file:line` or a named frozen constant (F-07).
- **S1:** author ≠ signer. This WO's author writes none of the delivery code; the Implementer
  delivers, the Verifier signs, the Adversary probes.

## 2. Discovery record

### 2.1 What Step 8.0 delivered (the surface this step hardens) — as of `7d76490`

| Piece | Delivered behavior | Where |
|---|---|---|
| `translate` guard | `!in_world → None`, checked first for every event | `src/input/keybinds.rs:34-36` |
| `VK_RETURN` arm | chat closed → `chat_open=true`, `ChatFocus`; open → take line, `SendChat`, close | `keybinds.rs:39-53` |
| `VK_ESCAPE` arm | chat open → cancel (clear+close, `None`); else → `Some(CloseDialog)`, **no state change** | `keybinds.rs:54-66` |
| `VK_INSERT` arm | chat open → swallowed `None`; else → `Some(SitToggle)` | `keybinds.rs:67-76` |
| unknown `KeyDown` | `_ => None`, no state change | `keybinds.rs:77-79` |
| `KeyUp` | `.. => None`, accepted and ignored (no state change) | `keybinds.rs:81-83` |
| `CharInput` | chat open → `line.push(ch)` **uncapped, any char**; else no-op; `None` always | `keybinds.rs:84-92` |
| mouse arms | `Move \| LeftDown \| LeftUp` → `None` — **8.1's territory, untouched here** | `keybinds.rs:93-96` |
| `Intent` | `MoveTo/AttackTarget/ChatFocus/SendChat/SitToggle/CloseDialog` — complete surface | `src/input/intent.rs:9-33` |
| `UiState` | `in_world`, `chat_open`, `line` + `#[derive(Default)]` | `intent.rs:41-49` |
| `validate` | refuses empty/over-cap chat, out-of-`u16` tiles; `CloseDialog`/`ChatFocus` → `None` | `src/input/validate.rs` |
| tests | **T1–T11** (412 lines), all green; every `UiState` literal uses `..Default::default()` | `src/input/tests.rs` |
| callers | **zero** uses of `translate`/`validate`/`InputEvent` outside `src/input/**` (tree grep 2026-11-07) — 8.3 will be the first | — |

### 2.2 Constraints discovered (each forces a design choice or a documented divergence)

1. **Three delivered tests pin `UiState` identity and must stay byte-identical (§10).**
   - **T3** (`tests.rs:91-112`) asserts that a lone `KeyDown ESCAPE` leaves `ui` **equal to
     `UiState { in_world: true, ..Default::default() }`** — so the Escape arm may not set *any*
     new state field on its "nothing open" path.
   - **T7** (`tests.rs:194-221`) asserts "hover tracking must not mutate `UiState` either" —
     `Move`/clicks may not touch the new guard field.
   - **T5** (`tests.rs:135-157`) asserts discarded input (unknown `KeyDown`, `KeyUp`, clicks,
     chat-closed `CharInput`) leaves a fresh `UiState` untouched.
   These three pin the *shape* of the auto-repeat guard below (D9): the guard field is set only
   by the `RETURN`/`INSERT` arms, cleared only by their matching `KeyUp`, a pushed character, or
   a *different* key's `KeyDown`; `Move`/clicks never touch it; the Escape arm never touches it.
2. **The auto-repeat divergence is real and wire-visible.** WO-8.0 §2.2-4 bound all three keys
   uniformly on `KeyDown` (legacy fired Return/Insert on `WM_KEYUP`,
   `mainfunction.cpp:1202/:1218/:1229`) and claimed "one action per key press is the
   user-visible contract either way". Under OS auto-repeat (repeated `WM_KEYDOWN`, one
   `WM_KEYUP` at release) that claim does not hold:
   - **held `VK_INSERT`** → `SitToggle` on every repeat → `validate` → `Order::SitDown`
     **enqueued on every repeat** (~30/s) — wire spam and stand/sit flicker (legacy: exactly one
     order per physical press);
   - **held `VK_RETURN`** → chat line oscillates open (flush) / closed (re-focus) at repeat
     rate (wire-safe only because an empty line is refused — the UX still flickers);
   - **held `VK_ESCAPE`** → repeated `CloseDialog` signals — `validate` refuses them
     (`validate.rs`, D2), so **no wire traffic** and the state transitions are idempotent.
   Guarding `RETURN`/`INSERT` restores the legacy one-action-per-press contract while keeping
   8.0's low-latency `KeyDown` binding (firing on `KeyUp` instead would add release latency and
   break T1/T10 — rejected).
3. **Legacy keyboard arms 8.0 did not cover** (this step's pin inventory): `VK_F11`
   (`mainfunction.cpp:1287`, empty), `VK_F12` (`:1292`, commented debug only), `VK_F8` in
   `WM_SYSKEYDOWN` (`:1378`, `SI32 a = 0`), `VK_LWIN/VK_RWIN` (`:1344-1350`, minimize window —
   OS window management, **not** input-layer work), arrow cases commented out
   (`:1205-1217`) and the dead `MouseOperation.cpp:273-522` block. None may be bound (charter
   non-goal; F8/F11/F12 are no-ops in legacy too).
4. **Legacy dialog-close paths** (the E16 grounding): `ItemInventory.cpp:1479-1481`
   (`VK_ESCAPE` → `Hide()`), `PrivateChatDlg.cpp:413-415` and `PartyDlg.cpp:411-413`
   (Escape clears the focused chat input), `NExitDlg.cpp:163-166` (close button →
   `DeleteDialog()`), and `mainfunction.cpp:1304` (Escape with nothing open →
   `CreateInterface(NEXIT_DLG)`, idempotent via `if (! IsCreate())`, `NExitDlg.cpp:63`).
   The port has no widget framework (Q3), so the keyboard side owns the **close** transition
   over a single `dialog_open` flag; the open/render direction defers with the dialog layer
   (divergence (d), §8).
5. **Legacy line editing came free from edit controls.** The chat bar is a Windows `EDIT`
   control (`CNChatBarDlg::SetInputEditBoxFocus`, `NChatBarDlg.cpp:715`) and the legacy's own
   interface control handles `VK_BACK` (`InterfaceMgr/Interface/Edit.cpp:1024`) and `VK_DELETE`
   (`:672`). The port's line buffer is a caret-less `String` (Q3): Backspace pops the last char
   — the edit-control behavior with the caret at end, which is where the caret always is while
   typing appends. Forward `VK_DELETE` needs a caret model → deferred with the chat widget
   (divergence (b), §8).
6. **F-8.0-01 disposition applied (Scribe lane).** WO-8.0 §11's zero-queue grep was adjudicated
   a defective pattern (bare `ring` substring-matches `String`; `cltKey` appears in mandated doc
   prose). This WO's §11 audit line is the **corrected** pattern: machinery subset
   `VecDeque|coalesc|MAX_KEY` plus word-bounded `\bring\b` — satisfiable while the contract
   text is delivered verbatim.
7. **Latency (P4-C3) unchanged.** The guard is two field comparisons and one store per event —
   no `.await`, no buffer, no allocation; the chain stays synchronous (D1). T10 (the 100-event
   `< 16 ms` batch) re-pins the budget with the guard in place; **no new latency test** is
   added, because no new structural cost exists to measure (rationale recorded here and in
   T10's unchanged doc comment).

### 2.3 Legacy semantics inherited (why these behaviors exist)

| Legacy behavior | Citation |
|---|---|
| Return/Insert fired on `WM_KEYUP` — one action per physical press, auto-repeat harmless | `mainfunction.cpp:1202/:1218/:1229` |
| Escape with nothing open → create the exit dialog (idempotent) | `mainfunction.cpp:1304`; `NExitDlg.cpp:63` (`IsCreate()` guard) |
| Escape closes/hides the active dialog | `ItemInventory.cpp:1479-1481` (`Hide()`), `NExitDlg.cpp:163-166` (`DeleteDialog()`) |
| Escape clears the focused chat input | `PartyDlg.cpp:411-413`, `PrivateChatDlg.cpp:413-415` |
| Backspace/Delete edit the focused line | `Edit.cpp:1024` / `:672`; chat bar focus `NChatBarDlg.cpp:715` |
| `VK_F11` / `VK_F12` / `VK_F8` are no-op arms | `mainfunction.cpp:1287` / `:1292` / `:1378` |
| Win-keys minimize the window (country-switch gated) — OS management, not input | `mainfunction.cpp:1344-1350` |
| Arrow-key movement: commented out / dead block — arrows unbound | `mainfunction.cpp:1205-1217`; `MouseOperation.cpp:273-522` |
| `GetFrame() > 40` warm-up gate → the `in_world` boolean (8.0 D4, unchanged) | `mainfunction.cpp:221,236,254,288,302,1367` |

## 3. Scope decision — this delivery

**In:**

1. `src/input/intent.rs` — `UiState` gains exactly two fields: `dialog_open: bool` and
   `repeat_vk: Option<u32>` (§7.1). **`Intent` is not touched** (F-7.4-01: extend behavior, not
   the enum — no new variant is needed; `CloseDialog` already carries the close signal).
2. `src/input/keybinds.rs` — `VK_BACK` constant + the D9 auto-repeat guard in the
   `VK_RETURN`/`VK_INSERT` arms (+ `KeyUp`/other-`KeyDown` bookkeeping), the D10 dialog-close
   transition in the `VK_ESCAPE` arm, the D11 `VK_BACK` arm, and the D11 control-character
   filter in `CharInput` (§7.2). The mouse arms (`keybinds.rs:93-96`) are untouched.
3. `src/input/mod.rs` — one re-export line gains `VK_BACK` (API-surface consistency with the
   three re-exported 8.0 constants).
4. `src/input/tests.rs` — **append** T28–T39 (§9). T1–T11 remain byte-identical (§10, §13.2).
5. Twelve tests: baseline **371 → 383** expected (+12, inside the matrix row's est. +12–16;
   the delivered number is copied from `test.log`).
6. `docs/agents/WO-8.2.md` — this document, committed **before** any code.

**Out:** see §5. Explicitly unchanged: `src/input/event.rs`, `src/input/validate.rs`,
`src/lib.rs`, `src/client/**`, `src/game/**`, `tests/**`, `Cargo.*`.

## 4. Architectural decisions (binding for this WO)

- **D9 — the auto-repeat guard: a single-key consecutive-press window.**
  `UiState.repeat_vk: Option<u32>` records the `VK_RETURN`/`VK_INSERT` press whose release has
  not yet been seen. Rules, exhaustively:
  1. `KeyDown { vk: VK_RETURN | VK_INSERT }`: if `repeat_vk == Some(vk)` → **auto-repeat →
     `None`, no state change** (the fix: held keys fire once per physical press, restoring the
     legacy `WM_KEYUP` contract while keeping 8.0's `KeyDown` binding). Otherwise set
     `repeat_vk = Some(vk)` and run the 8.0 arm unchanged (including the swallowed
     Insert-while-chating row — setting the guard there is harmless and correct: the key *is*
     down).
  2. `KeyUp { vk }`: clear `repeat_vk` iff it equals `vk` (matching release only — a second
     key's release must not reopen the first key's window). Still returns `None` (8.0 row).
  3. `CharInput { ch }` that is **pushed** (chat open, `!ch.is_control()`, D11): clear
     `repeat_vk` — a character the player actually typed ends the repeat context (this is also
     what keeps 8.0's T1/T10 sequences — which omit `KeyUp` — green: their typed characters
     end the window between presses).
  4. `KeyDown` of any **other** key (the `_` arm): clear `repeat_vk` — pressing a different key
     ends the window (this clause is what keeps T10's script — `…Enter, F1, Move…` with no
     `KeyUp` — firing on the next cycle; T10 is its pin).
  5. `Move`/`LeftDown`/`LeftUp`: **never touch** `repeat_vk` (T7's "hover must not mutate
     `UiState`" principle; pointer motion must not affect key state).
  6. **`VK_ESCAPE` is deliberately unguarded** (set nothing, check nothing): Escape produces no
     wire command ever (`validate` refuses `CloseDialog`, D2) and its state transitions are
     idempotent — the legacy idiom for exactly this is the `IsCreate()` guard
     (`NExitDlg.cpp:63`). T3 pins that a lone Escape leaves `UiState` untouched; T30 pins the
     unguarded repeats. Guarding Escape would break a delivered, signed-off test for zero harm
     reduction (divergence (a), §8).
  7. **`VK_BACK` is deliberately unguarded**: auto-repeat backspace is the correct editing UX
     (T32 pins it). Editing keys are commands-with-repeat; action keys are
     commands-once-per-press.
  Documented limitation (divergence (e), §8): the window is *single-key consecutive* detection,
  not a full key-state matrix — a second key press or a typed character ends it, so exotic
  two-key hold combinations (hold Insert while typing) are outside the model, exactly as they
  were outside legacy's `WM_KEYUP`-based model.
- **D10 — the Escape dialog-close state machine (close direction only).**
  `UiState.dialog_open: bool` models "the active dialog" (Q3: one flag, no framework). Escape,
  in priority order: (1) chat open → cancel the line (8.0 row, unchanged — focused input wins);
  (2) dialog open → `dialog_open = false`, return `Some(CloseDialog)` — the close transition;
  (3) neither → `Some(CloseDialog)` with **no state change** (8.0's E4 row, byte-for-byte —
  T3's identity assert is the pin). `validate` continues to refuse `CloseDialog` (UI-only,
  D2). Other keys are **not** gated by `dialog_open` (E17: legacy popups are custom-drawn in
  the game window and `mainfunction.cpp`'s `WM_KEYDOWN` switch keeps processing — non-modal).
  The flag is written by tests today and by the future dialog layer; **no dialog opener exists
  in this step** (divergence (d), §8).
- **D11 — chat-line editing: Backspace + control-character filter, caret-less.**
  `KeyDown { vk: VK_BACK }` (`0x08`, new constant; legacy cite `Edit.cpp:1024`): chat open →
  `ui.line.pop()` (char-wise, UTF-8 safe — never panics, empty line is a no-op); chat closed →
  no-op. Always `None` (state change only). `CharInput` gains a filter: **control characters
  never enter the line** (`ch.is_control()` → not pushed, no state change) — the legacy `EDIT`
  class never inserted them either (Enter/Backspace are consumed as *commands*: `VK_RETURN` is
  the flush command, `VK_BACK` the edit command). This also makes the D9 guard robust to
  whether 8.3's `WM_CHAR` decode emits `'\r'` repeats for a held Enter: filtered characters do
  not end the repeat window, so held Enter cannot oscillate under either decode policy.
  Forward delete (`VK_DELETE`, `Edit.cpp:672`) and caret movement are **not** bound — they need
  a caret model, deferred with the chat widget (Q3).
- **D12 — surface stability: `Intent`, `InputEvent`, and `validate` do not change.**
  No new `Intent` variant (the `CloseDialog` signal already exists), no `InputEvent` variant
  (the events needed — `KeyDown`/`KeyUp`/`CharInput` — exist), no `validate` change
  (`CloseDialog` already → `None`; `SendChat`/`SitDown` paths untouched). F-7.4-01's
  whole-tree exhaustiveness grep is therefore trivially preserved, and the 8.1 parallel lane's
  shared surface (`event.rs`, the mouse arms) stays byte-identical.

## 5. Non-goals

- **No mouse-path changes of any kind** (parallel 8.1 lane): the `Move`/`LeftDown`/`LeftUp`
  arms stay `None`, `event.rs` untouched, T7 untouched.
- No `screen_to_tile`, no click→intent (8.1), no `window.rs`/`WM_*` decode (8.3), no
  `tests/e2e_loopback` wire assertions (8.3), no live runs (8.4).
- No `Intent`/`InputEvent`/`validate` changes (D12); no dialog **open** direction or rendering
  (Q3 — the future dialog layer writes `dialog_open`); no widget framework (Q3).
- No forward-delete/caret model (D11), no Escape auto-repeat guard (D9.6), no guard on
  `VK_BACK` (D9.7), no arrow-key bindings (charter non-goal), no F8/F11/F12/Win-key bindings
  (E19), no modifier-key modeling (the `InputEvent` surface has no modifier state; `WM_CHAR`
  already carries the composed result).
- No queue machinery of any kind (P4-C1); no new crates (`Cargo.toml` untouched), no new
  opcodes (Q2), no wire-byte changes; no `in_world` production wiring (8.3).
- No modification of T1–T11 (§10), no matrix/status edits (Verifier), no `README.md`,
  `CONTRIBUTING*`, `GoonZu_Source/**`, or other-WO edits.

## 6. File plan (ownership — one step, one Implementer, one file set)

| # | File | Change |
|---:|---|---|
| 1 | `docs/agents/WO-8.2.md` | NEW — this document, committed **before** any code |
| 2 | `src/input/intent.rs` | EDIT — `UiState` gains `dialog_open` + `repeat_vk` with docs (§7.1); `Default` derive unchanged (both new fields derive); module/`Intent` docs untouched |
| 3 | `src/input/keybinds.rs` | EDIT — `VK_BACK` const; D9 guard + bookkeeping; D10 dialog-close arm; D11 `VK_BACK` arm + `CharInput` filter; module-doc divergence note (§7.2) |
| 4 | `src/input/mod.rs` | EDIT — one re-export line: add `VK_BACK` |
| 5 | `src/input/tests.rs` | EDIT — **append only** T28–T39 (§9); T1–T11 byte-identical |
| 6 | `evidence/8.2/` | NEW at verification, written by the Verifier only (§11) |

**READ-ONLY:** `src/input/event.rs` and `src/input/validate.rs` (unchanged this step — the
shared 8.0 contract), all of `src/client/**`, `src/game/**`, `src/graphics/**`, `src/auth/**`,
`src/net/**`, `src/protocol/**`, `src/main.rs`, `src/lib.rs`, all of `tests/**`,
`Cargo.toml`/`Cargo.lock`, `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `README.md`,
`GoonZu_Source/**`, every other file in `docs/**`, every existing `evidence/**` directory. A
defect found there is a finding, never an edit (P3-6).

## 7. API & wiring spec (the shared contract)

### 7.1 `src/input/intent.rs` — `UiState` (the only struct change)

```rust
/// Translator-local UI state — the legacy dialog flags' modern replacement,
/// owned by the input layer; nothing else mutates it in Phase IV.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiState {
    /// In-world stage reached (8.3 sets it; replaces `GetFrame() > 40`, D4-guard).
    pub in_world: bool,
    /// Chat line open (Enter toggles).
    pub chat_open: bool,
    /// The typed line; cleared on flush and on cancel (Q3: line buffer only).
    /// Control characters never enter it (8.2 D11); Backspace pops it (D11).
    pub line: String,
    /// An active dialog is open (8.2 D10) — the close direction of the
    /// legacy `NEXIT_DLG`/per-dialog Escape contract (`ItemInventory.cpp:1479`).
    /// Written by tests today and by the future dialog layer (Q3); Escape clears it.
    pub dialog_open: bool,
    /// Auto-repeat guard (8.2 D9): the `VK_RETURN`/`VK_INSERT` press not yet
    /// released. A repeated `KeyDown` of the same key while this is `Some(vk)`
    /// is an OS auto-repeat and is discarded; cleared by the matching `KeyUp`,
    /// by a pushed typed character, or by a different key's `KeyDown`.
    /// `VK_ESCAPE`/`VK_BACK` deliberately never set it (D9.6/D9.7).
    pub repeat_vk: Option<u32>,
}
```

`Intent` is **unchanged** (verbatim as delivered — `intent.rs:9-33`). `Default` stays
`#[derive(Default)]` (`false`/`false`/empty/`false`/`None`); a manual impl would trip
`clippy::derivable_impls` under `-D warnings` (8.0 delivery-note precedent).

### 7.2 `src/input/keybinds.rs` — constants and the full `translate` table

```rust
/// `VK_BACK` — the chat-line edit key (8.2 D11; legacy edit-control handling
/// `InterfaceMgr/Interface/Edit.cpp:1024`). Auto-repeat pops repeatedly by
/// design (D9.7 — editing is the one place repetition is the UX).
pub const VK_BACK: u32 = 0x08;
```

`translate(event, &mut ui) -> Option<Intent>` — signature unchanged; guard order:
`!in_world → None` first (D4, unchanged), then the `match`. **Every row is tested (§8/§9):**

| `InputEvent` (after the `!in_world` guard) | `UiState` effect | Result |
|---|---|---|
| `KeyDown { vk: VK_RETURN }`, `repeat_vk == Some(VK_RETURN)` | unchanged | `None` — auto-repeat discarded (D9.1) |
| `KeyDown { vk: VK_RETURN }`, otherwise, chat closed | `repeat_vk = Some(VK_RETURN)`, `chat_open = true`, `line` cleared | `Some(ChatFocus)` |
| `KeyDown { vk: VK_RETURN }`, otherwise, chat open | `repeat_vk = Some(VK_RETURN)`, `chat_open = false`, `line` taken (cleared) | `Some(SendChat { line })` |
| `KeyDown { vk: VK_INSERT }`, `repeat_vk == Some(VK_INSERT)` | unchanged | `None` — auto-repeat discarded (D9.1) |
| `KeyDown { vk: VK_INSERT }`, otherwise, chat closed | `repeat_vk = Some(VK_INSERT)` | `Some(SitToggle)` |
| `KeyDown { vk: VK_INSERT }`, otherwise, chat open | `repeat_vk = Some(VK_INSERT)` | `None` (typing state swallows it — 8.0 row) |
| `KeyDown { vk: VK_ESCAPE }`, chat open | `chat_open = false`, `line` cleared (**`repeat_vk` untouched** — D9.6) | `None` (cancel — 8.0 row) |
| `KeyDown { vk: VK_ESCAPE }`, chat closed, `dialog_open` | `dialog_open = false` | `Some(CloseDialog)` — the close transition (D10) |
| `KeyDown { vk: VK_ESCAPE }`, neither open | **unchanged** (T3 identity) | `Some(CloseDialog)` — 8.0 E4 row |
| `KeyDown { vk: VK_BACK }`, chat open | `line.pop()` (no-op when empty) | `None` (D11) |
| `KeyDown { vk: VK_BACK }`, chat closed | unchanged | `None` |
| `KeyDown { other }` (unknown, F-keys, arrows, Win-keys, `VK_DELETE`, …) | `repeat_vk = None` (ends the window — D9.4); nothing else | `None` |
| `KeyUp { vk }`, `repeat_vk == Some(vk)` | `repeat_vk = None` | `None` (D9.2) |
| `KeyUp { vk }`, otherwise | unchanged | `None` (8.0 row) |
| `CharInput { ch }`, chat open, `!ch.is_control()` | `line.push(ch)`, `repeat_vk = None` (D9.3) | `None` (state change only) |
| `CharInput { ch }`, chat open, `ch.is_control()` | **unchanged** — never pushed (D11) | `None` |
| `CharInput { ch }`, chat closed | unchanged | `None` (8.0 row) |
| `Move { .. }` / `LeftDown` / `LeftUp` | **unchanged — never touches `repeat_vk`** (D9.5; 8.1's arms) | `None` (D8) |
| **any**, `!in_world` | unchanged (guard checked first — no bookkeeping) | `None` (D4) |

`validate` (§7.4 of WO-8.0) is **unchanged** and restated for reference only: `CloseDialog` /
`ChatFocus` → `None`; `SitToggle` → `Order::SitDown`; `SendChat` → refuse empty/over-cap else
`Chat { mode: Normal }`. `event.rs` unchanged.

### 7.3 Unchanged surfaces (explicit)

`Intent` (all six variants), `InputEvent` (all six variants), `validate`, `src/lib.rs`, the
`mod.rs` pipeline docs (only the `pub use` line gains `VK_BACK`). Any delivered byte differing
from these is a write-guard violation (§13.5).

## 8. Expectation table (the F-07 source for §9)

E1–E12 (WO-8.0 §8) remain in force and unmodified — T1–T11 keep asserting them. The new rows:

| # | Behavior (as tested) | Provenance |
|---|---|---|
| **E13** | Held `INSERT` fires exactly one `SitToggle` until the matching `KeyUp`; held `ENTER` cannot oscillate the chat line (repeats discarded while `repeat_vk` matches, incl. interleaved filtered `'\r'` chars); typed characters / a different key press / the matching `KeyUp` end the window; re-press fires again | Legacy fired on `WM_KEYUP` → one action per physical press (`mainfunction.cpp:1202/:1218/:1229`); 8.0 bound `KeyDown` (`WO-8.0` §2.2-4) — D9 restores the contract; P4-C3: unguarded Insert repeats would enqueue `Order::SitDown` (`action.rs:94`, kind 2 `:69`) on every repeat |
| **E14** | Backspace pops the last char while chat is open (empty line no-op, chat-closed no-op); backspace auto-repeat pops repeatedly (deliberate, D9.7); an edited line flushes whole through the chain | Legacy edit controls: `Edit.cpp:1024` (`VK_BACK`), chat-bar focus `NChatBarDlg.cpp:715`; caret-at-end behavior of the caret-less port (Q3) |
| **E15** | Control characters (`'\r'`, `'\b'`, `'\t'`, `'\u{1}'`) never enter the line; printable characters do (incl. non-ASCII: `'é'`, `'世'`) | Legacy `EDIT` class consumes Enter/Backspace as commands and never inserts control chars; port decisions: `VK_RETURN` = flush command, `VK_BACK` = edit command (D11) |
| **E16** | Escape with `dialog_open` → close it (`dialog_open = false` + `Some(CloseDialog)`, `validate` → `None`); chat-open takes priority over the dialog; neither open → 8.0's E4 row byte-for-byte | `ItemInventory.cpp:1479-1481` (`Hide()`), `NExitDlg.cpp:163-166` (`DeleteDialog()`), `mainfunction.cpp:1304` + idempotent `NExitDlg.cpp:63`; chat priority: focused-input-wins (`PartyDlg.cpp:411-413` clears the input, not the dialog) |
| **E17** | `dialog_open` does not swallow game keys: `INSERT` still sits (→ frozen `Order::SitDown`), `RETURN` still focuses chat while a dialog is open | Legacy popups are custom-drawn in the game window; `mainfunction.cpp`'s `WM_KEYDOWN` switch (`:1282`) runs regardless of an open dialog — non-modal |
| **E18** | Arrow VKs (`0x25`/`0x26`/`0x27`/`0x28`), down and up, → discarded, `UiState` unchanged | `mainfunction.cpp:1205-1217` commented out; `MouseOperation.cpp:273-522` dead block; `CONTRIBUTING_PHASE4.md` non-goals ("Arrow keys remain unbound") |
| **E19** | `F8` (`0x77`), `F11` (`0x7A`), `F12` (`0x7B`), `LWIN` (`0x5B`), `RWIN` (`0x5C`) → discarded, `UiState` unchanged | Legacy no-op arms `mainfunction.cpp:1378` / `:1287` / `:1292`; Win-key minimize `:1344-1350` **not ported** (window management is outside the input layer) |

**Stated divergences (Haru/CAPA — documented, never silent):** (a) `VK_ESCAPE` auto-repeat is
**not** guarded — its repeats emit only idempotent, wire-refused `CloseDialog` signals
(`validate` refuses; legacy idiom `NExitDlg.cpp:63`), and guarding would break delivered T3;
(b) forward `VK_DELETE` and caret movement are not bound — caret-less line buffer (Q3),
deferred with the chat widget; (c) the Win-key minimize (`mainfunction.cpp:1344-1350`) is not
ported — window management is not the input layer's; (d) the `NEXIT_DLG` **open** direction
(`mainfunction.cpp:1304` `CreateInterface`) is not modeled — 8.2 delivers the close contract;
the open/render direction and the writer of `dialog_open` defer with the dialog layer (Q3), and
with nothing open Escape keeps 8.0's `CloseDialog` signal; (e) the repeat window is
single-key consecutive detection, not a full key-state matrix (D9 limitation).

## 9. Test invariants (T28–T39) and the baseline

Pattern authorities: as WO-8.0 §9 (colocated unit tests, pure-function tables, typed-refusal
assertions). F-07: expectations from §8 only. All tests build `InputEvent` values directly —
no window, no Win32, no network (P4-C4). **T1–T11 are not modified** (§10).

| # | Invariant | File | Phase |
|---|---|---|---|
| **T28** | `held_insert_fires_once_until_released` — E13: five consecutive `KeyDown INSERT` → exactly **one** `SitToggle` (intents counted; `repeat_vk == Some(VK_INSERT)` after the first), repeats return `None` with `UiState` otherwise untouched; `KeyUp { INSERT }` clears the guard; a fresh `KeyDown INSERT` fires a second `SitToggle` | `src/input/tests.rs` | unit |
| **T29** | `held_enter_does_not_oscillate_the_chat_line` — E13/E15: `KeyDown RETURN` → `ChatFocus` + `repeat_vk == Some(VK_RETURN)`; three `CharInput '\r'` → line stays `""`, guard **stays set** (filtered chars don't end the window); a repeat `KeyDown RETURN` → `None` and the chat **stays open** (no flush/reopen flicker); `KeyUp { RETURN }` clears; type `'h'`; `KeyDown RETURN` → `SendChat { "h" }` + closed | `src/input/tests.rs` | unit |
| **T30** | `escape_repeats_are_deliberately_unguarded` — E13-divergence (a)/E4: three consecutive `KeyDown ESCAPE` on a fresh state → three `Some(CloseDialog)`, `UiState` byte-identical after each (T3's identity, generalized) — pinning the *deliberate* non-guard so a future "helpful" Escape guard must revise this WO | `src/input/tests.rs` | unit |
| **T31** | `backspace_edits_the_chat_line` — E14: open, type `"hi"` → `VK_BACK` → `"h"` → `VK_BACK` → `""` → `VK_BACK` on empty → `""` (no panic, no change); type `'o'`; `KeyDown RETURN` → `SendChat { "o" }` | `src/input/tests.rs` | unit |
| **T32** | `backspace_autorepeat_is_intentional` — E14/D9.7: open, type `"abc"`, three **consecutive** `KeyDown VK_BACK` (no `KeyUp`) → `""` — editing keys are not latched (deliberate asymmetry vs T28) | `src/input/tests.rs` | unit |
| **T33** | `control_characters_never_enter_the_chat_line` — E15: open; `CharInput '\r'`/`'\b'`/`'\t'`/`'\u{1}'` → line `""` after each, `None`; then `'é'` + `'世'` → line grows to those chars exactly (`line.len() == 5` bytes) | `src/input/tests.rs` | unit |
| **T34** | `escape_closes_the_active_dialog` — E16: `dialog_open = true` → `KeyDown ESCAPE` → `Some(CloseDialog)` + `dialog_open = false`; `validate(&CloseDialog) == None` (UI-only, D2); `repeat_vk` untouched (`None`) | `src/input/tests.rs` | unit |
| **T35** | `chat_cancel_takes_priority_over_the_dialog` — E16: `chat_open` + `dialog_open` both set, line typed → `KeyDown ESCAPE` → `None`, chat closed + line cleared, **`dialog_open` still true** (focused input wins) | `src/input/tests.rs` | unit |
| **T36** | `dialog_state_does_not_swallow_game_keys` — E17: `dialog_open = true` → `KeyDown INSERT` → `SitToggle` → `validate` → `Order::SitDown`; `KeyDown RETURN` → `ChatFocus` (chat opens; dialog still open) | `src/input/tests.rs` | unit |
| **T37** | `arrow_keys_remain_unbound` — E18: `KeyDown` and `KeyUp` of `0x25`/`0x26`/`0x27`/`0x28` → `None` each, `UiState` byte-identical (fresh state) | `src/input/tests.rs` | unit |
| **T38** | `legacy_no_op_keys_are_discarded` — E19: `KeyDown 0x77`/`0x7A`/`0x7B`/`0x5B`/`0x5C` → `None` each, `UiState` byte-identical (fresh state) | `src/input/tests.rs` | unit |
| **T39** | `the_edited_line_flushes_through_the_full_chain` — E14+E16 integration: through `translate → validate → sender.send` (the §8.0 `feed` helper, reused): open, type `"cat"`, `VK_BACK` → `"ca"`, flush → exactly one `ClientCommand::Chat { mode: Normal, text: "ca" }` enqueued; then `dialog_open` + `KeyDown ESCAPE` → `Some(CloseDialog)` but `validate` refuses → **drains to exactly 1** | `src/input/tests.rs` | unit |

**Baseline: 371 → 383 expected** (+12 = T28–T39, all lib tests; `asset_fixtures` 11 and e2e 25
untouched; the 5 ignored tests preserved). The matrix cell is copied from the delivered
`test.log`, never from this prose. T10's `< 16 ms` batch runs with the guard in place (D9 is
O(1) — §2.2-7) and is expected green unchanged.

## 10. Forbidden shortcuts

- **Modifying T1–T11 in any way** — they are the signed-off 8.0 contract; this delivery
  *appends* to `tests.rs` only (a diff hunk touching lines 1–412 is a violation).
- Touching the mouse arms (`keybinds.rs:93-96`), `event.rs`, `validate.rs`, `lib.rs`, or any
  READ-ONLY file of §6 (the 8.1 parallel lane depends on them) — finding first, P3-6.
- Any queue/ring/capacity/coalescing structure in `src/input/**` (P4-C1); any new `Intent`/
  `InputEvent` variant (D12, F-7.4-01); new crates (`Cargo.toml` untouched), new opcodes (Q2).
- Guarding `VK_ESCAPE` or `VK_BACK` (D9.6/D9.7 — T30/T32 pin the deliberate behavior);
  binding arrows/F-keys/Win-keys/`VK_DELETE` (E18/E19, §5); opening or rendering dialogs
  (Q3 — `dialog_open` is cleared, never set, by `translate`).
- Deriving an expectation from `translate`/`validate` under test (F-07) — §8 rows only;
  clamping, coercing, or panicking where §7 names a typed result; silently relaxing T10's
  margin if it flakes (disclose, P4-C3).
- A `.await`, channel hop, or buffer in the enqueue path (D1/D7); a latency test without its
  documented margin; live-server runs promoted to gates.
- Touching `TASK_MATRIX.md` / `CONTRIBUTING*` (Verifier/Scribe only), `README.md`,
  `GoonZu_Source/**`, any other WO or audit doc, any existing `evidence/**`.

## 11. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/8.2/` standing bundle: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`,
  `doc.log`, `env.txt`, `summary.md`, `mutation-probes.md`, `zero-queue-grep.txt`,
  `f7401-postcheck.txt`, `write-guard.txt`, `t1-t11-unchanged.txt`, `SHA256SUMS` — hashes over
  the committed (LF) bytes (6.3 L-2 lesson); the matrix `Tests Baseline` cell is copied from
  `test.log` (**383** expected: 347 lib + 11 + 25).
- **Zero-queue audit (corrected pattern — F-8.0-01 disposition, §2.2-6):**
  `grep -rinE 'VecDeque|coalesc|MAX_KEY|\bring\b' src/input/` → no matches captured in
  `zero-queue-grep.txt` (P4-C1, Phase IV gate 4). The bare-`ring`/`cltKey`-in-prose defect of
  WO-8.0 §11 does not recur: the word-bounded form and the machinery subset are both
  satisfiable while §7's contract text is delivered verbatim.
- **T1–T11 unchanged (§13.2):** `t1-t11-unchanged.txt` — `git diff <base> <delivery> --
  src/input/tests.rs` shows **only additions after line 412** (zero deletions, zero hunks in
  lines 1–412), plus `cargo test --lib input::tests` green at 23/23.
- **F-7.4-01 post-check:** `f7401-postcheck.txt` — `Intent`/`InputEvent` byte-identical to the
  8.0 delivery (`git diff` empty for those enums' definitions); no new exhaustive match outside
  `src/input/**`.
- Write-guard confirmation: `git diff --stat` limited to §6; `src/client|game|graphics|auth|
  net|protocol`, `src/lib.rs`, `event.rs`, `validate.rs`, `tests/**`, `Cargo.*`, matrix,
  contributing, README and the legacy tree untouched — **and the mouse arms byte-identical**
  (`git diff` hunk list for `keybinds.rs` contains no change at lines 93-96).
- Four mutation probes recorded (§12): mutated diff, failing output, pristine restore, porcelain
  CLEAN after each, baseline re-run green afterwards.

## 12. Mutation probes (Adversary — each must fail its named test loudly)

| # | Mutation | Named failing test | Restore |
|---:|---|---|---|
| 1 | `translate`: delete the D9.1 block-checks — the `repeat_vk == Some(vk) → None` early-outs in the `VK_RETURN` and `VK_INSERT` arms (auto-repeat flows again) | **T28** (five Insert presses → five `SitToggle`) | `git checkout --` |
| 2 | `translate`: remove the D11 control filter — `line.push(ch)` unconditionally while chat is open | **T33** (line gains `'\r'`/`'\b'`/`'\t'`) | `git checkout --` |
| 3 | `translate`: the Escape dialog-close branch forgets `ui.dialog_open = false` (falls through to the plain E4 signal) | **T34** (`dialog_open` still `true`) | `git checkout --` |
| 4 | `translate`: bind an arrow — `VK_UP` (or the `_` arm before it) returns `Some(Intent::SitToggle)` | **T37** (`KeyDown 0x26` must be discarded) | `git checkout --` |

Each probe: apply one semantic break to the pristine tree, run the named test, record exit 101
and the failing assertion, restore, verify `git status --porcelain` clean — then re-run the full
gate set to the delivered baseline (`WO-6.3.md` §9 / `evidence/7.x/mutation-probes.md`
discipline).

## 13. Definition of done

A Step 8.2 delivery is `PASSED` only when, on a clean S4 clone at the delivery SHA:

1. All five mandatory gates exit-0 at the delivered baseline from `test.log` (**383** expected:
   347 lib + 11 fixtures + 25 e2e), `fmt` 0 diffs, `doc` 0 warnings, porcelain clean.
2. **T1–T11 byte-identical** (only appended T28–T39 in `tests.rs`; `t1-t11-unchanged.txt`
   clean) and all 23 `input::tests` green.
3. Every decision D9–D12 is pinned by a failing-if-corrected test or probe (T28–T39 +
   probes 1–4; T10 additionally pins the D9.4 other-key clause and the O(1) latency claim).
4. All four §12 probes show their named test failing at exit 101, with pristine restores and a
   green post-probe baseline.
5. Write guard holds: only §6 files changed; **the mouse arms (`keybinds.rs:93-96`),
   `event.rs`, `validate.rs`, `src/client/**`, matrix, contributing, README, `src/game/**`
   and the legacy tree untouched** (the 8.1 parallel-lane guarantee; row update, if any, is
   the Verifier's).
6. F-07 holds: every expectation traces to an E-row in §8 (E13–E19 new, E1–E12 inherited), and
   every E-row traces to a legacy `file:line` or a frozen constant — never to the code under
   test.
7. Zero-queue audit green (the corrected §11 pattern, no matches) and the D1/D7 structural
   claim holds (no `.await`/buffer in the enqueue path; T10 green with its margin documented).
8. No new crates, no new opcodes, no wire-byte changes, no `Intent`/`InputEvent`/`validate`
   changes; no window/Win32/network in any gate test; no live run as a gate.

## 14. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

**Branch:** `step/8.2-keyboard-hotkeys` · **Delivery SHA:** `26c07a7` (parent `987a3b7`, the
WO commit) · **Delivered:** 2026-11-07 · **Baseline from `test.log`: 383/383** (347 lib +
11 `asset_fixtures` + 25 `e2e_loopback`; 1 ignored lib + 4 ignored `live_integration` preserved).

**Five gates** (`RUSTFLAGS=-D warnings`, all exit 0, run twice: pre-probe and post-probe
baseline — identical both times):

| Gate | Result |
|---|---|
| `cargo check --all-targets` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --check` | exit 0, 0 diffs |
| `cargo doc --no-deps` | exit 0, 0 warnings |
| `cargo test` | **383/383** (347 + 11 + 25), 0 failed |
| `cargo test --lib input::tests` | **23/23** (T1–T11 inherited + T28–T39) — matches §9 expected |

**T28–T39 individual runs:** all 12 green in the appended block (counted in the 23/23 above);
T10's 100-event `< 16 ms` batch green with the D9 guard in place (no margin relaxed).

**T1–T11 byte-identical (DoD 13.2):** `git diff -U0 987a3b7..26c07a7 -- src/input/tests.rs`
shows exactly one hunk — `@@ -412,0 +413,431 @@` — zero deletions anywhere in the file
(`NO_DELETIONS`); lines 1–412 untouched.

**Write guard (DoD 13.5):** `git diff --name-only 987a3b7..26c07a7` = the four §6 source
files only (`intent.rs`, `keybinds.rs`, `mod.rs`, `tests.rs`). `event.rs`, `validate.rs`,
`src/client/**`, `src/game/**`, `src/lib.rs`, `tests/**`, `Cargo.*`, matrix, contributing,
README and the legacy tree: **NONE**. Mouse arms byte-identical
(`InputEvent::Move { .. } | InputEvent::LeftDown { .. } | InputEvent::LeftUp { .. } => None,`
identical at base and delivery — the 8.1 parallel-lane guarantee).

**F-7.4-01:** `Intent`/`InputEvent` definitions byte-identical to the 8.0 delivery (only
`UiState` gained two documented fields, as §7.1 specifies).

**Probes 4/4** (each against the committed tree; pristine restore + porcelain clean after
each; full gate set re-run green at 383 afterwards):

| # | Mutation | Named test | Result |
|---:|---|---|---|
| 1 | delete the D9.1 block-checks in `VK_RETURN`/`VK_INSERT` | T28 | **exit 101** — `E13/D9.1: auto-repeat 1 must be discarded, not re-fired` (`left: Some(SitToggle)`), restored, clean |
| 2 | `line.push(ch)` unconditional (drop `!ch.is_control()`) | T33 | **exit 101** — `E15: the line stays empty after '\r'`, restored, clean |
| 3 | drop `ui.dialog_open = false` in the Escape close branch | T34 | **exit 101** — `E16/D10: the dialog is closed`, restored, clean |
| 4 | bind arrow `0x26 => Some(SitToggle)` | T37 | **exit 101** — `E18: arrow KeyDown 0x26 must be discarded` (`left: Some(SitToggle)`), restored, clean |

**Disclosed edits / findings:** none — no `F-8.2-NN` findings, no out-of-plan edits; one
compile fix during authoring (`'\b'` → `'\x08'`, Rust has no backspace escape — inside the
appended test block) and five `cargo fmt` line-joins, all at lines ≥ 450 (appended region).
One process incident: probe 1 was first run against the *uncommitted* tree, and its
`git checkout --` restore reverted the whole implementation; it was re-applied, gates
re-verified at 383, committed as `26c07a7`, and **all four probes were then re-run against
the committed tree** (the table above is the committed-tree run).

### Verdict - Verifier

**Status: PASSED** — formally verified 2026-11-07 (+07:00) on a fresh S4 clean clone of
`git@github.com:psi-ginger/rust_iwczn_client_II.git` at the delivery tip **`937c9ab`**
(delivery SHA `26c07a7`, parent/base `987a3b7` = the Scribe WO-8.2 commit; `26c07a7..937c9ab`
= this WO's section-14 note alone, docs-only — `src/`, `tests/`, `Cargo.*` byte-identical, so
gating the tip fully characterizes the delivery). All five gates green in **both** runs
(run 1 pre-probe, run 2 post-probe on the restored pristine tree):

| Gate | Command | Run 1 | Run 2 |
|---|---|---:|---:|
| 1 | `RUSTFLAGS=-D warnings cargo check --all-targets` | 0 | 0 |
| 2 | `cargo clippy --all-targets -- -D warnings` | 0 | 0 |
| 3 | `cargo fmt --check` | 0 — 0 diffs | 0 — 0 diffs |
| 4 | `cargo doc --no-deps` | 0 — 0 warnings | 0 — 0 warnings |
| 5 | `cargo test` | 0 — **383 passed / 0 failed** | 0 — **383 passed / 0 failed** |

**Baseline:** 371 → **383** (+12 = T28–T39, all lib; 347 lib + 11 `asset_fixtures` + 25
`e2e_loopback`); the 5 ignored tests preserved (1 lib + 4 `live_integration`); 0 doc-tests;
`cargo test --lib input::tests` = **23/23** (T1–T11 + T28–T39). Matrix `Tests Baseline` copy
target **383/383**, taken from `test.log` only. `TASK_MATRIX.md` untouched — row 8.2 stays
`PENDING`: this verdict and `evidence/8.2/` are produced for Custodian acceptance; no matrix
flip, commit or push is authorized by the verification directive (8.0 round-1 precedent).

**DoD (§13) items 1–8, audited:**

1. Five gates exit-0 at 383 in both runs, fmt 0 diffs, doc 0 warnings, porcelain clean
   before/after every run — **hold**.
2. T1–T11 byte-identical: `t1-t11-unchanged.txt` — exactly one hunk `@@ -412,0 +413,431 @@`
   (pure insertion after line 412), zero deletion lines in the file; all 23 `input::tests`
   green — **hold**.
3. D9–D12 each pinned by a failing-if-corrected test or probe: D9 → T28/T29/T30/T32 + probe 1
   (and T10 for the D9.4 other-key clause + the O(1)/`< 16 ms` claim, green with its margin
   documented and unrelaxed); D10 → T34/T35/T36 + probe 3; D11 → T31/T32/T33/T39 + probe 2;
   D12 → `f7401-postcheck.txt` (`Intent`/`InputEvent` byte-identical, `validate` untouched) +
   T37/T38 + probe 4 — **hold**.
4. §12 probes **4/4**: each one semantic break → named test FAILED at **exit 101** with the
   tabled E-row assertion (T28 `E13/D9.1 … left: Some(SitToggle)`; T33 `E15: the line stays
   empty after '\r'`; T34 `E16/D10: the dialog is closed`; T37 `E18: arrow KeyDown 0x26 …
   left: Some(SitToggle)`) → `git checkout --` → porcelain CLEAN; post-probe full gate re-run
   green at 383 — **hold** (transcripts + two disclosed process notes in
   `evidence/8.2/mutation-probes.md`).
5. Write guard: `987a3b7..26c07a7` = exactly the four §6 files (519 insertions / 9 deletions,
   `tests.rs` 431/0); `26c07a7..937c9ab` = this WO only; **mouse arms byte-identical** (13
   hunks, machine-checked 0 overlaps of old lines 93–96; base line 93 == delivery line 163);
   `event.rs`/`validate.rs`/`lib.rs`, forbidden paths and the legacy tree all empty — **hold**.
6. F-07: 104 assertion lines, 120 E-row citations covering **E1–E19** (E1–E12 inherited,
   E13–E19 new); each E-row traces to a legacy `file:line` or a frozen constant, never the
   code under test — **hold**.
7. Zero-queue audit green: both components of the corrected pattern return **0 matches**
   (machinery subset + word-bounded `ring`, run as `git grep` on the delivery commit; the
   host's PS/grep backslash quirk and the `cmd /c` `|` blockage are disclosed in
   `zero-queue-grep.txt`). D1/D7 structural claim holds — production `.await` = 0 (only
   `mod.rs:16` doc prose), async code only in the two pre-existing 8.0 harnesses plus T39's
   test body; T10 green with its margin documented — **hold**.
8. No new crates (`Cargo.*` untouched), no new opcodes, no wire-byte changes, no
   `Intent`/`InputEvent`/`validate` changes (§13.8; `Intent` enum byte-identical — all three
   `intent.rs` hunks start at old line ≥ 40, the enum at line 9 precedes every hunk), no
   window/Win32/network in any gate test (grep over `src/input/**` = 0 matches), no live run
   as a gate — **hold**.

**Findings:** none — no `F-8.2-NN` findings, no out-of-plan edits (consistent with the
Implementer's note). Two **verifier process notes**, disclosed in `mutation-probes.md`:
the first probe-4 attempt was invalid (script array-literal bug dropped the `_ => {` line →
compile error at exit 101, not a test failure) — detected, restored, re-run cleanly; the
first probe-1 run produced the intended break with a whitespace-only artifact — re-run for
the exact 6-deletion diff, with the post-probe gate set re-run after the final probe run.

**Evidence:** `evidence/8.2/` — 21 files (§11 standing set of 14 incl. `t1-t11-unchanged.txt`
and `f7401-postcheck.txt`, + `gate-exits.txt` + `postprobe/` run 2), `SHA256SUMS` verified
**20/20** over the delivered LF/no-BOM bytes (6.3 L-2 lesson). Artifacts left **uncommitted**
on the branch working tree pending Custodian acceptance.

**Integration considerations (Step 8.1 + Step 8.2 → main, sequentially, before Step 8.3):**

- **Lineage / order:** `step/8.1-mouse-move` (tip `cada5ed`) cuts at `449d60f`;
  `step/8.2-keyboard-hotkeys` (tip `937c9ab`) cuts at `987a3b7` = current `origin/main`.
  Merge **8.1 first**: main-side `src/**` is unchanged since `449d60f` (both Scribe commits
  are docs-only), so 8.1's six files merge clean; the one content reconciliation at merge 1
  is `docs/agents/WO-8.1.md` — both sides *added* it from the `449d60f` base (main/Scribe
  blob `7f80e36` vs 8.1-branch blob `f5df5b7`) — keep the Scribe definition **and** the
  delivery note + verdict.
- **Second merge (8.2):** merge-base with the post-8.1 main is `987a3b7`, and both sides
  have since touched the four shared files **`src/input/{intent,keybinds,mod,tests}.rs`** —
  expect conflicts: `tests.rs` certain (both append at the same anchor after line 412);
  `intent.rs` near-certain (both extend `UiState`); `keybinds.rs`/`mod.rs` may auto-merge if
  their hunks are disjoint (8.1's click arms sit at old `:93-96`+, 8.2's keyboard hunks at
  old `:10-89`) — resolve whatever git flags, keeping T1–T11 byte-identical and the mouse
  arms byte-identical. `WO-8.2.md` should take the 8.2 side automatically (main unchanged
  since `987a3b7`) unless main moves it meanwhile; `TASK_MATRIX.md` is untouched by both
  branches — no conflict there.
- **T-number collision (resolved at the sequential merge):** WO-8.1 numbers its new tests
  **T12-T27**; this WO's **T12-T23** were renumbered **T28-T39** in the merge commit that
  brought `step/8.2-keyboard-hotkeys` into `main` (Custodian dual sign-off directive), so
  every T-number above now matches `tests.rs` as merged - Step 8.3 can cite T-numbers unambiguously.
- **Test arithmetic:** sequential baselines 371 → **387** → **399** (363 lib + 11 + 25).
  Re-verify the combined count from `test.log` at the merged tree, never from prose.
- **After both merges:** run the full five-gate set at the combined **399** before Step 8.3
  wiring, and re-run the write-guard / F-07 / zero-queue audits against the merged tree if
  any conflict resolution touched `src/input/**`. Both verdict bundles
  (`evidence/8.1/`, `evidence/8.2/`) stay uncommitted until Custodian acceptance; the row
  flips and any push are the governance step that follows.
