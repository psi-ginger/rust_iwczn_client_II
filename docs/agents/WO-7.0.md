# Work Order - Step 7.0: Codebase Lean & Structural Audit (pre-Phase-III)

## 1. Provenance, lane hats, baseline

Filed 2026-10-30 by the Acting QC Lead / Verifier session (`psi-ginger`) in the
**governance-actor role under the Custodian's directive** ("inject a pre-7.1
lean-and-clean pass before we start implementing the asset decoders"; precedent
`fc5ffb1` — Verifier as acting governance actor, Custodian-directed, no status
written). No Scribe session was active; the `TASK_MATRIX.md` edit is made under
directive with this disclosure. This work order does not amend
`CONTRIBUTING_PHASE3.md`; it is definition-before-implementation for an
**audit-only** step that precedes Step 7.1.

- **Baseline:** `main` @ `ee44877` (Phase III governance + WO-7.1 commit,
  docs-only atop `606da1a`; `git diff 606da1a..ee44877 -- src/ tests/` empty,
  i.e. the audited code is exactly the Phase II gate-verified tree, **225/225**,
  five gates exit 0 — `docs/audit/2026-10-29-phase-2-final-sign-off-report.md`).
- **Branch:** none — this step is docs + audit deliverable only; no code branch
  is cut, nothing under `src/` or `tests/` is created, modified or deleted.
- **Lanes:** governance actor (this session, under directive) -> Verifier for
  the status verdict on the audit deliverable. No Implementer: **no code is
  written in this step** (see §4 non-goals).
- **Write guard:** this step touches exactly `TASK_MATRIX.md` (row 7.0),
  `docs/agents/WO-7.0.md` (this file), and the audit report it commissions
  (`docs/audit/2026-10-30-step-7.0-lean-structural-audit.md`). `src/**` and
  `tests/**` are read-only observation targets.

## 2. Scope

A lean-and-structural audit of the Phase II codebase (`src/`, 41 `.rs` files,
~18.2k lines at the baseline) executed BEFORE Phase III expands the footprint
with `src/graphics/**`, so that bloat or architectural debt is neither carried
forward nor replicated by the asset steps. Four checks, each with a concrete
pass condition:

1. **File size & monolith check** — inventory every `src/**/*.rs` with its
   line count; flag production files (non-`tests.rs`) over a stated threshold
   for review; distinguish "large but cohesive" from "large because N concerns
   live in one file" by listing the file's items (structs/impls/fns).
2. **Hidden test bloat** — find every inline `#[cfg(test)]` suite embedded in
   production files (i.e. tests not already in `foo/tests.rs` submodule files),
   and every test-support construct (fixtures, builders, `#[cfg(test)]` helpers)
   living in production modules; state per occurrence whether segregating it
   into the sibling `tests.rs` submodule is warranted or the inline placement
   is idiomatic.
3. **Structural / OOP readiness** — identify procedural "god functions" and
   deeply nested routines that should become encapsulated structs/traits or
   domain models before Phase III grows the surface: name the function, its
   line span, its responsibilities, and the candidate decomposition (types +
   boundaries), with the recommendation keyed to which Phase III step would
   benefit (7.1 decoders, 7.2 render loop).
4. **Dead-weight scan** — `pub` items unreachable from the binary/lib surface,
   duplicated helpers across modules, and any `#[allow]`/`todo!()`/`unimplemented!()`
   markers that would violate the no-stub invariant as Phase III lands.

## 3. Acceptance criteria (what the delivered report must contain)

- A **file-size table** (every `src/**/*.rs`, line count, test-vs-production),
  with the threshold stated explicitly and the flagged files justified item by
  item. Counts are measured, never estimated.
- Per flagged file: an **item inventory** (the file's top-level items with line
  spans) proving the diagnosis, and a concrete **refactoring recommendation**
  (target module/type layout, e.g. "split dispatcher match arms into
  `src/game/dispatcher/handlers.rs` per opcode family") — actionable, not
  "consider refactoring".
- A **hidden-test-bloat list** with file:line for every inline `#[cfg(test)]`
  and a segregate/keep recommendation each, following the repo's existing
  convention (`foo/tests.rs` submodules — see `src/game/dispatcher/tests.rs`).
- A **god-function list** (name, file:line span, measured line count, nested
  depth or responsibility count) with the encapsulation proposal (struct/trait
  names, state to own, seams to expose) and the Phase III step it serves.
- A **dead-weight section** (or an explicit "none found" with the commands run
  so the claim is reproducible).
- Findings that imply code changes are filed as **recommendations** with a
  priority (pre-7.1 / ride-with-7.1 / ride-with-7.2 / optional), never applied:
  this step's write guard is docs-only.
- Every number in the report is copyable from a command output recorded in the
  report itself (S5: prose is not evidence — the report carries the raw
  inventories it was derived from).

## 4. Non-goals

- No code change of any kind (`src/**`, `tests/**` read-only; a rewrite is a
  finding for a future Implementer step, never an edit here).
- No style re-litigation: `cargo fmt`/`clippy` verdicts are already recorded
  green at the baseline; this audit is structure, not formatting.
- No performance benchmarking, no dependency audit beyond noting bloat-relevant
  facts, no MSRV/toolchain changes.
- This step does not modify `CONTRIBUTING_PHASE3.md` or WO-7.1.

## 5. File plan

| artifact | lane | note |
| :--- | :--- | :--- |
| `TASK_MATRIX.md` row 7.0 | governance actor under directive | inserted before 7.1; status `PENDING` (S3: only the Verifier sets status) |
| `docs/agents/WO-7.0.md` | governance actor under directive | this file |
| `docs/audit/2026-10-30-step-7.0-lean-structural-audit.md` | this session (audit) | the commissioned report; §3 is its acceptance checklist |

## 6. Forbidden shortcuts

- Editing any file outside the §5 plan (notably `src/**` — "fixing while
  auditing" is exactly the bloat this step exists to prevent).
- Hand-typed line counts or item spans without the command output that produced
  them (S5).
- Vague recommendations ("clean up dispatcher.rs") without a concrete target
  layout.
- Marking the matrix row anything but `PENDING` in this step.

## 7. Evidence required (Verifier, on the audit deliverable)

The report at `docs/audit/2026-10-30-step-7.0-lean-structural-audit.md` satisfies
§3: a Verifier checking it must be able to re-run the embedded commands on the
baseline tree and reproduce every count. No `evidence/7.0/` gate bundle is
required (no code changed; the baseline gates remain the Phase II record).

## 8. Return blocks (append; do not rewrite the sections above)

### Audit report - governance actor

Delivered 2026-10-30 by the governance actor (`psi-ginger`) under the Custodian's directive.
The commissioned report is at `docs/audit/2026-10-30-step-7.0-lean-structural-audit.md` and
satisfies every §3 checklist item:

- **File-size table** (all 41 `src/**/*.rs`, production vs test-support, threshold stated at
  700 production lines ≈1.9× mean) with per-flagged-file item inventories proving the
  diagnosis (§1).
- **Hidden-test-bloat list** with file:line for every `#[cfg(test)]` and a segregate/keep call
  each — verdict: clean, only the idiomatic 10-line `lib.rs` suite is inline; keep (§2).
- **God-function list** with measured spans, nesting depths, and encapsulation proposals keyed
  to Phase III steps 7.1/7.2 (§3-§4).
- **Dead-weight section** with reproducible commands: `dispatch_frame` (dispatcher.rs:1068) is
  the one genuinely dead item (zero callers anywhere); 68 unit-test-only `pub` + 8 over-exposed
  internal types are visibility bloat, not dead code (§5).
- Findings filed as **recommendations with priorities** (pre-7.1 / ride-with-7.1 /
  ride-with-7.2 / optional) — none applied (§6).
- **Scope:** zero bytes changed under `src/**` and `tests/**` (verified via `git status`).
  Matrix row 7.0 left `PENDING` for the Verifier (S3).

### Verdict - Verifier

## 9. Definition of done

1. `TASK_MATRIX.md` carries row 7.0 before 7.1 with `PENDING` status and a
   pointer to this work order.
2. The audit report exists and satisfies every §3 checklist item, with measured
   numbers and reproducible commands.
3. Zero bytes changed under `src/**` and `tests/**`.
