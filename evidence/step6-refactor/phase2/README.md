# Step 6 — Phase 2: T-1 / T-2 Surveys (both N/A, evidence-only)

**Result: evidence-only. `git diff --stat -- src tests` is empty at this boundary —
no code byte moved.**

Both surveys document *why no structural motion is in scope, on this branch, at
this commit* — per Step 5 Phase 2 precedent (survey READMEs only).

## T-1 — Extract cohesive loop phases from `flow()`: **N/A (structurally satisfied)**

The Step 6 Pre-Implementation Audit Report's core finding: the main loop structure
is complete. Verified on this branch (`src/client/driver.rs`):

| Seam | Line |
|---|---|
| `async fn flow(` | `:597` |
| `async fn front_phase(` | `:536` |
| `async fn game_phase(` | `:629` |
| `async fn in_world_phase(` | `:762` |

`flow()` is an 8-line dispatcher over three extracted phase functions (Steps 4/5
did the extraction — `in_world_phase` byte-identical move in Step 5 Phase 3,
7251cbc). No further slicing of cohesive functions is warranted: T-1's structural
requirement holds, so Phase 2 records N/A rather than inventing motion.

## T-2 — Hoist in-loop scratch buffers: **N/A (already hoisted)**

| Site | Scratch | State |
|---|---|---|
| `src/auth/front.rs` | `[0u8; 4096]` read scratch | **hoisted above the loop** — comment at `:306`: "scratch is hoisted above the loop - one stack slot, reused by…" (port-chosen read buffer documented at `:291`) |
| `src/game/entry.rs` | `[0u8; 4096]` read scratch | **hoisted above the loop** — comment at `:500` (doc at `:478`) |

Both in-loop buffers are already single-slot, loop-invariant allocations. Nothing
left to hoist within Step 6's scope; T-2 records N/A.

## Scope linkage

With T-1 satisfied and T-2 already hoisted, no extraction/hoist motion exists for
Step 6 — confirming the WO §1 scope pick (Option B = C3 + C4 only). The remaining
planned code motion is exactly Phase 3 (`copy_credential`) and Phase 4
(`gate_frame`).

## Boundary check

- `git diff --stat -- src tests` → empty (clean tree, `c7d9eb8`).
- Gates from Phase 1 (`../phase1/`) still the operative green boundary:
  5/5 EXIT=0, **337/337**.
