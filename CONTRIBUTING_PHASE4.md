# GoonZu Client Rust — Phase 4 Governance (Step 8 onward)

> **Scope of this document.** `CONTRIBUTING.md` governs Steps 1.1–5.4 (the Phase II
> micro-step catalog — a closed, audited record of delivered work). `CONTRIBUTING_PHASE2.md`
> governs Step 6 (end-to-end orchestration — closed and audited). `CONTRIBUTING_PHASE3.md`
> governs Step 7.1 onward (Phase III: asset loading & client rendering — closed and audited).
> **This document governs Step 8.0 onward — Phase IV: User Input & Interactive Control**,
> which maps raw OS/window events into game client actions and feeds them through the Step 7.4
> command channel (`ClientCommand` → `CommandSender` → `run_with_commands`). It was established
> 2026-11-06 by the Custodian's Phase IV Charter v0.2 sign-off (with the Q5 correction below),
> on the signed-off Phase III baseline (`origin/main` @ `0458e95`, all 32 matrix rows `PASSED`,
> five gates re-run green at **360/360** this session —
> `docs/audit/2026-11-06-phase-3-final-sign-off-report.md` §9, which declares that Phase IV
> "may be defined under a fresh charter").

---

## Inherited verbatim from `CONTRIBUTING.md` / `CONTRIBUTING_PHASE2.md` / `CONTRIBUTING_PHASE3.md`

The following apply **unchanged** to every Phase IV step. The earlier documents are canonical;
this list is the pointer, not a fork:

1. **Core philosophy** — zero-evasion, precision micro-steps; zero tolerance for stubbing,
   fake mocks, or unverified loops (`CONTRIBUTING.md`, philosophy section).
2. **The five mandatory gates** — `RUSTFLAGS=-D warnings cargo check --all-targets`,
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (stock rustfmt defaults,
   no `rustfmt.toml` in this repo), `cargo test`, `cargo doc --no-deps` (0 warnings). A gate
   that is not exit-0 blocks the row (`CONTRIBUTING.md`; gate commands recorded at
   `CONTRIBUTING_PHASE2.md:238-239`).
3. **The three execution invariants** (`CONTRIBUTING.md`, "Execution Invariants for AI
   Agents") — never jump steps; never hide failures; never stub (in Phase IV the third
   invariant reads: never stub an input path — an event that cannot be translated is
   **discarded explicitly** as a typed no-op, never a panic, never a fabricated command).
4. **The audit discipline** — dual-source citations with accurate `file:line`; Haru/CAPA
   discipline (a spec-vs-code divergence is a finding, not a silent fix); S5 (prose is not
   evidence); F-07 (an expectation is never derived from the code under test); status is the
   Verifier's to set, in a `docs(qc)` commit only, and new rows start `PENDING`
   (`CONTRIBUTING_PHASE2.md`, QC-discipline sections).
5. **The multi-agent execution model and separation of duty** — Implementer authors code but
   never writes the matrix or its own `PASSED`; Verifier is the only status-writer; the Scribe
   owns `CONTRIBUTING*` and edits them only under directive with disclosure (precedents
   `fc5ffb1`, `b6b48a7`); the Adversary lane is mandatory whenever a row's load-bearing claim
   is a negative ("no queue", "refuses", "parity with legacy") (`CONTRIBUTING_PHASE2.md`,
   "Multi-agent execution model").
6. **Definition-before-implementation** — a step's scope and acceptance are written into this
   document (or a work order citing it) **before** any code exists (`CONTRIBUTING_PHASE2.md`).
7. **S4 clean-clone evidence; S7 no force-push of cited SHAs; repo language English only;
   credentials never committed** (standing governance).
8. **F-7.4-01 (whole-tree exhaustiveness)** — before adding any variant to any enum
   (`InputEvent`, `Intent`, `ClientCommand`, `TickMessage`, or anything else), grep the whole
   tree (`src/**` **and** `tests/**`) for exhaustive matches; a forced edit to a READ-ONLY file
   to satisfy the compiler is a **disclosed finding**, never a silent change
   (`CONTRIBUTING.md`, lessons).
9. **Live runs are evidence, never a gate** (`CONTRIBUTING.md:28-46`).

---

## Phase IV charter decisions (binding, as signed off 2026-11-06)

These are the Custodian's rulings on Charter v0.2. They are **not** suggestions; each is
enforced by the DoD of the step(s) named.

### P4-C1 — The direct-feed architecture, zero intermediate queue (Q1 + charter §2–§3)

The input pipeline is exactly `InputEvent → Intent → ClientCommand → CommandSender::send`,
synchronous, feeding the **already-shipped** Step 7.4 receiving half
(`commands.rs:54` "clone it into whatever produces input"; `event_loop.rs:478/534`;
`driver.rs:823`). There is **no intermediate queue**: the tokio unbounded mpsc is the only
buffer (`WO-7.4.md` D7, `:147` — input is human-rate; a bounded/managed channel silently loses
intent and was rejected at design time). Explicitly **prohibited ports** from the legacy tree:
`cltKeyManager` 50-slot array (`Client/Key/key.h:5,59-63`), `Push` move-coalescing
(`key.cpp:18-42`), `Pop` FIFO shift (`key.cpp:44-65`), the `cltKey` struct (`key.h:11`),
`PushKey`/`GameWParam`/`GameMouseX` globals (`Client.h:2424`, `MouseOperation.cpp:203,232`).
The absence of any such type in `src/input/**` is a DoD item (grep-verified in evidence).
Module home: **`src/input/`** top-level (Q1 ruling).

### P4-C2 — The parity line: UX + math must match; machinery must not (Q2/Q3/Q4 + charter §3)

**MUST match (user experience & math):**

- `screen_to_tile` — the exact inverse of the already-ported forward transform
  `draw_center_xy` (`src/graphics/screen.rs:147` ← `mapdraw.cpp:36-52`), built from legacy
  `TransFromScreenToRealXY` (`common/Map/MapClient/Map-Client.cpp:183-188`) +
  `GetRealTileByRealDot` (`common/Map/MapCommon.cpp:963-965`), anchored at
  `GetScreenRX/RY = 0` (`Client.cpp:1005`; **Q4: no camera scrolling in Phase IV**).
- Keybindings: **Enter** → chat focus/flush, **Insert** → sit toggle, **Escape** → close/cancel
  (legacy arms cited in each WO's expectation table).
- Click-to-move tile picking and click-target selection (left click).

**DO NOT replicate (legacy boilerplate):** queue arrays/caps/coalescing (P4-C1), frame-counter
gates, 2004-era buffer management. Legacy *UX-visible* quirks are stated and matched per
P3-5; legacy *machinery* is replaced by the modern equivalent and the replacement is
documented as a port decision.

**Q2 ruling:** the sit toggle uses the frozen `Order::SitDown` (240 kind-2, `action.rs:99-100`);
**the server handles toggle logic**. The legacy `GAMEMSG_REQUEST_SITDOWN` opcode
(`mainfunction.cpp:1243,1249`) is **not** introduced — no new opcode, ever, outside a
protocol-phase WO.

**Q3 ruling:** chat UI is a **minimal line buffer** (focus + type + Enter flush). No
InterfaceMgr widget/dialog framework port.

### P4-C3 — Latency budget (Q5, corrected at sign-off)

The input-to-wire enqueue budget is **≤ 16 ms** (one frame at 60 FPS) per event, with a hard
**strictly sub-tick ceiling of < 50 ms**. The draft's 200 ms figure was **rejected** at
sign-off: high-speed combat (e.g. rapid sword attack cadence) requires sub-tick responsiveness;
tokio mpsc in-memory throughput makes the budget trivially achievable. Enforcement, in two
layers:

- **Structural (every step):** no `.await`, no channel hop, no intermediate buffer between
  `InputEvent` and `CommandSender::send` — synchronous pure-function translation (P4-C1).
- **Measured (Step 8.4 evidence):** end-to-end input→wire latency recorded in
  `evidence/8.4/` from a live session; the in-gate proxy is the WO-8.0 batch test (§9 T10).

A timing test carries an explicitly documented margin (batch size vs budget) in its WO; if
gate load ever threatens the margin, the finding is disclosed and adjudicated — never silently
relaxed.

### P4-C4 — Headless gate tests (P3-3 inherited, input instance)

No Phase IV unit or gate test may construct a Win32 object, open a window, or require a
display. Gate tests build `InputEvent` values directly (scripted mock window events); the
`wm_to_input` decode (Step 8.3) takes raw `wparam`/`lparam` **integers** so it is unit-testable
without a message. The Win32 `WindowProc` arms and any on-screen behavior are operator-only
(`#[ignore]`d) precedent from Step 7.2.

---

## Verification gates for a Phase IV step

A Phase IV row may be marked `PASSED` only when, in addition to the five mandatory gates
(all exit-0, evidence in `evidence/8.x/` on a clean clone at the delivery SHA):

1. **Parity pins (P4-C2):** every MUST-match item has a named test — a keybind-table test per
   legacy arm (each row citing `file:line`), `screen_to_tile` round-trip against
   `draw_center_xy` plus hand-computed goldens, click-to-move producing the frozen
   `Order::Move` bytes.
2. **Named refusals (D4-inherited):** off-map / out-of-`u16` tile, empty chat, over-cap chat
   (`MAX_CHAT_BYTES`, `action.rs:53`), unknown VK, pre-in-world events — each refused **by
   name**, never clamped, never panicked (refusal = typed no-op or typed `None`).
3. **Headlessness (P4-C4):** the gate suite runs with no window/Win32/display/network; every
   load-bearing input assertion is on scripted `InputEvent` values.
4. **Zero-queue audit (P4-C1):** the Verifier's evidence includes a grep over `src/input/**`
   showing no queue/ring/cap/coalescing type, and the WO's file plan matches the delivered
   diff.
5. **Latency (P4-C3):** the structural claim holds (no await/buffer in the enqueue path) and
   the WO's latency test is green; end-to-end figures land in `evidence/8.4/` (evidence, not a
   gate for earlier steps).
6. **Adversary probes:** ≥ 2 (default 3–4) named mutation probes per code row, each failing its
   named test at exit-101 with pristine restore and a green post-probe baseline
   (`WO-6.3.md` §9 ratification; `evidence/7.x/mutation-probes.md` discipline).

---

## Multi-agent execution model (Phase IV)

Lane hats, separation of duty, the Verifier-only status rule, S1/S3/S4/S5/S7, Haru/CAPA and
the Adversary-mandatory trigger are inherited unchanged (item 5 above). Phase IV
clarifications:

- **Lane sequence per code step:** Implementer → Verifier + **Adversary** — every row's
  load-bearing claim is "parity with legacy" or "refuses/discards", both dischargeable by
  verified mutation probes (§ verification gates item 6).
- **`src/**` write-guard:** each step writes only the files its WO lists; **`src/client/**` is
  READ-ONLY in every Phase IV step** (Step 7.4 delivered the receiving half; a defect there is
  a finding `F-8.x-NN`, never an edit — a forced compiler-mandated edit follows the F-7.4-01
  disclosure pattern, item 8). `src/game/**` stays frozen at 7.3, `src/graphics/**` is touched
  only by 8.1 (read-only imports) and 8.3 (`window.rs` WM_* arms only).
- **Merge:** fast-forward only (or `--no-ff` under explicit Custodian directive with
  disclosure, 7.3/7.4 precedent), on Custodian approval, per `CONTRIBUTING_PHASE2.md:256`.
- **`TASK_MATRIX.md` and this document:** status cells Verifier-only (`docs(qc)`); this
  document Scribe-only under directive; an Implementer never writes either.

---

## Steps in this phase

| Step | Title | Work order | Status |
|:---:|:---|:---|:---|
| 8.0 | Input core — `InputEvent`/`Intent`, keybinds, validation (zero-queue direct feed) | `docs/agents/WO-8.0.md` | `PENDING` (authoritative status: `TASK_MATRIX.md` Phase IV tracker) |
| 8.1 | Mouse — `screen_to_tile` inverse + click-to-move / target selection | `docs/agents/WO-8.1.md` (to author at opening) | `PENDING` |
| 8.2 | Keyboard/hotkeys — Enter/Insert/Escape bindings wired to intents | `docs/agents/WO-8.2.md` (to author at opening) | `PENDING` |
| 8.3 | Driver wiring — `wm_to_input` arms → `CommandSender` on the pump thread | `docs/agents/WO-8.3.md` (to author at opening) | `PENDING` |
| 8.4 | Live acceptance — moving, chatting, interacting + latency evidence | `docs/agents/WO-8.4.md` (to author at opening) | `PENDING` |

Steps are strictly sequential: 8.1/8.2 depend on 8.0; 8.3 wires 8.0–8.2 to the live pump; 8.4
accepts the whole. A WO is authored **before** its step's first code byte
(definition-before-implementation); status is authoritative only in `TASK_MATRIX.md`.

---

## Non-goals (whole phase)

No `winit` or any new crate (zero-new-crates precedent, WO-7.2 R1; `Cargo.toml` untouched
unless a WO names a feature add with disclosure). No InterfaceMgr widget/dialog framework, no
chat rendering beyond the line buffer (Q3). No camera scrolling (Q4). No map-edit `WM_CHAR`
path, no login/gateway input (in-world only), no gamepad/touch/IME. No new opcodes or wire
layouts (240/117 stay WO-7.3 §8.3-frozen). No live-server run as a gate. Arrow keys remain
unbound (legacy block is dead code, `MouseOperation.cpp:273-522`); right- and double-click
parity is deferred beyond 8.1's scope note.
