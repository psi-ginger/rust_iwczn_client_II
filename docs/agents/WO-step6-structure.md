# WO — Step 6 Structural Refactoring: Lean Findings Sweep (C3 + C4)

**Status:** `DRAFT` for Custodian alignment — definition-before-implementation.
No `src/**` byte may move before this scope is accepted.
**Date:** 2026-11-04 · **Branch:** `refactor/step6-structure`, cut from `main` @ `0cd1788`
**Baseline (re-derived this session):** `cargo check --all-targets` exit 0 under
`RUSTFLAGS="-D warnings"`; `cargo test` exit 0 at **336/336** (303 lib + 11 fixtures +
22 e2e; 5 ignored = 1 operator demo + 4 `live_integration`).
**Reference:** Step 6 Pre-Implementation Audit Report (2026-11-04) +
`docs/audit/2026-10-30-step-7.0-lean-structural-audit.md`.

---

## 1. Scope decision — Option B, scoped to C3 + C4

| Finding | State on `0cd1788` (grounded) | Disposition |
|---|---|---|
| **C3** `copy_credential` | still duplicated: `src/auth/login.rs:254`, `src/game/login.rs:181` — bodies identical modulo error type + const name | **IN SCOPE** |
| **C4** `gate_frame` | still duplicated: `src/game/dispatcher.rs:426`, `src/game/world_entry.rs:381` — same (opcode, exact-len) contract, different error enums | **IN SCOPE** |
| 7.0 D1, C1, C2 | closed (`dispatch_frame` gone; readers single-defined in `src/protocol/bytes.rs`) | done |
| S1 (68 test-only `pub`) | open; 7.0 marks **optional**; wide blast radius; naive narrowing breaks cross-module `test_support` imports | **deferred** — separate hygiene step if directed |
| G1 (ticker catch-up depth) | open, but fail-path **falsified**: zero intervals filtered at `event_loop.rs:356-360` — nesting style only | **excluded** |
| Option A (`local_listener` ×3 in auth group) | open; test-only hygiene | **deferred** — rides a future structural step |
| P-5 uiX fixture gap | open (`src/game/char_info/tests.rs` — Step 4 scope) | **deferred** — separate follow-up |
| Phase III 7.3 | undefined (`CONTRIBUTING_PHASE3.md:185-186`; no `WO-7.3`) | blocked by definition-before-implementation |

**Why B:** C3/C4 were assigned "ride-with-7.1" by the 7.0 audit and never landed;
both are already behavior-pinned on **both** sides by the existing 336 suite (§3), so
the sweep is provably regression-free; the audit's core finding (T-1 structurally
satisfied — `flow()` is 8 lines, the legs are extracted) removes any further
extraction motive, so the remaining code-health debt is exactly these two findings.

**Scope-discovery discrepancy (rides Phase 0):** `src/protocol/bytes.rs:4-5` claims the
"C1–C4" recommendations were "applied under the Custodian's Step 7.1 directive", but
only D1+C1+C2 landed (`TASK_MATRIX.md` 7.1 row) — C3/C4 are still duplicated. The
module doc must state the truth at every commit that carries it (Phase 0 corrects it
to "C1–C2; C3/C4 in Step 6"; Phase 3 updates it again when C3 lands here).

---

## 2. Design (grounded in the current code)

### C3 — one credential copier

- **Today:** both sites NUL-cut (`position(|&b| b == 0)`), refuse `len >= 20` with the
  identical `{ field: &'static str, len: usize }` payload, copy + zero-fill, and
  `debug_assert_eq!(slot.len(), <20-byte const>)`. Error variants
  `LoginError::CredentialTooLong` / `GameLoginError::CredentialTooLong` are shape-identical.
- **Move:** `pub(crate) fn copy_credential(slot, field, value) -> Result<(), CredentialTooLong>`
  plus `pub(crate) struct CredentialTooLong { field, len }` into `src/protocol/bytes.rs`
  (the C1/C2 shared-wire-byte precedent). The 5 call sites (3 auth + 2 game) convert
  explicitly with `.map_err` — the repo's documented idiom over `#[from]` when
  blanket-impl overlap would arise (`src/client/event_loop.rs:236-239`).
- **Unchanged:** both public error enums and their variant shapes (tests match on them:
  `auth/login/tests.rs:183`, `game/login/tests.rs:236`); both module consts with their
  citations; call-site module docs.
- **Citations:** the shared doc carries `Lib\commonutil.cpp:6-16` (StringCchCopy/MStrCpy)
  plus auth's uninitialized-stack note; aggregate `*.cpp`/`*.h` count across touched
  files must not decrease.

### C4 — one frame gate, two thin wrappers

- **Today:** dispatcher uses early-return `if`s, world_entry a `match`; the only
  delta is `opcode: expected_cmd` vs `opcode: cmd` — equal on every path because the
  opcode guard returns first, so the shared core may use either (disclosed).
- **Move:** new `src/game/gate.rs`, declared as a **private** `mod gate;` in
  `src/game/mod.rs`: `pub(crate) enum GateViolation { UnexpectedOpcode, Truncated, TrailingBytes }`
  (field names identical to both existing enums) + `pub(crate) fn gate_frame(...) -> Result<(), GateViolation>`.
- **Each site keeps a thin `gate_frame` wrapper** whose body is an explicit `.map_err`
  match into its own error type — so **every existing call site stays byte-identical**
  (the Steps 3/4/5 checklist discipline) and 7.0's "share core, not blind dedup"
  (audit line 33) is honored.
- **Unchanged:** `DispatchError` / `WorldEntryError` variants; all parser logic; the
  inline gates in `auth::{login,world_list}`, `char_info`, `entry` are **out of scope**
  (7.0 defined C4 as exactly these two functions).

---

## 3. Phases (series pattern; all five gates exit 0 at every boundary)

| Phase | Content |
|---|---|
| **0 baseline + doc truth** | evidence-only over `0cd1788`: head/env + 5 gate logs, 336/336; one-line comment correction in `src/protocol/bytes.rs` module doc ("C1–C4 applied" → "C1–C2 applied; C3/C4 in Step 6") |
| **1 characterization** | **Gap scan executed → documented N/A** (recorded in evidence). Pins already exist on both sides: C3 too-long (`auth/login/tests.rs:162` table ×3 fields ×3 lengths; `game/login/tests.rs:216` + assert at `:434`), NUL-cut + zero-fill (`auth/login/tests.rs:111` @ `:135-139`; `game/login/tests.rs:183`), C4 all three arms (`dispatcher/tests.rs:224,556,660,888`; `world_entry/tests.rs:398,503`). **If the Verifier disputes a pin, the additive test lands here — before any motion; the test count grows only in this phase.** |
| **2 surveys** | T-1 **N/A** (loop structure complete per audit — no extraction in scope); T-2 **N/A** (no in-loop scratch in scope). Evidence-only READMEs, per Step 5 Phase 2 precedent. |
| **3 C3 motion** | one commit: shared copier in `bytes.rs`, 5 explicit `.map_err` conversions, module-doc truth update; logic byte-identical; count unchanged |
| **4 C4 motion** | one commit: `game/gate.rs` + `mod gate;` + two thin wrappers; call sites untouched; count unchanged |
| **5 probes + evidence** | probe matrix below (each: one committed file mutated, named test only, exit 101, restore byte-clean + `git status --porcelain -- src tests` empty); 3× determinism at the final count; final 5 gates; committed-blob hash table (L-2); `summary.md` + unsigned audit-report draft; prose self-audit (counts copied from tables, push status from `git`) |

### Phase 5 probe matrix

| # | Mutation | Named test(s) that must FAIL (exit 101) |
|---|---|---|
| PR-1 | shared copier bound `>= 20` → `> 20` | `credentials_that_do_not_fit_are_refused_not_truncated`, `a_credential_without_room_for_its_terminator_is_refused` |
| PR-2 | shared copier drops the NUL cut (`position(...)` → `raw.len()`) | `strings_stop_at_their_terminator_and_zero_fill_the_rest`, `login_request_payload_is_byte_exact_at_literal_offsets` |
| PR-3 | shared gate skips the opcode check | `server_info_rejects_foreign_opcodes_and_wrong_lengths`, `village_struct_rejects_foreign_opcodes_and_wrong_lengths` |
| PR-4 | shared gate swaps `Truncated`/`TrailingBytes` arms | `a_char_update_length_boundary_is_refused_exactly`, `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` |

---

## 4. Invariants & write guard

- **Zero regression:** 336/336 held at every boundary (Phase 1 is N/A → the count may
  not change); `RUSTFLAGS="-D warnings"` check, `clippy --all-targets -D warnings`,
  `fmt --check` 0 diffs, `cargo test`, `cargo doc --no-deps` 0 warnings.
- **Write guard:** `src/protocol/bytes.rs` (C3 + the doc line), `src/auth/login.rs`,
  `src/game/login.rs`, `src/game/dispatcher.rs`, `src/game/world_entry.rs`,
  **new** `src/game/gate.rs`, `src/game/mod.rs` (+1 line), `evidence/step6-refactor/`.
  Nothing else: no `src/net/**`, no fixtures, no `Cargo.*`, no `TASK_MATRIX.md`
  (Verifier-only), no prose files.
- **F-07:** the shared cores are production code pinned by pre-existing hand-derived
  expectations; no expectation re-derived from a parser under test; no fixture edits.
- **Public API:** error enum variants unchanged; no new public exports (the `gate`
  module is private; the shared `CredentialTooLong` struct is `pub(crate)`).
- **Citations:** aggregate `*.cpp`/`*.h` references across touched files: net no-loss.

## 5. Acceptance (Verifier re-derives independently)

1. Clean clone @ delivery SHA: all 5 gates exit 0, **336/336**, ignored set = 5 unchanged.
2. Pure-motion check: shared copier/gate bodies byte-identical to their pre-move donors
   (modulo the disclosed const/error-type tokens); C4 call sites literally untouched;
   wrapper checklist counts recorded on both sides.
3. Probes 4/4 exit 101 on the named tests; each restore byte-clean.
4. Citations net no-loss; blob-hash table matches committed bytes; prose self-audit clean.
