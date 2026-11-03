# Step 5 — Phase 5: Evidence Bundle, Adversary Probes, Determinism

**Verdict: complete — 4 adversary probes all fail their named test at
EXIT=101 under mutation and restore the tree byte-identically; 3×
determinism runs each hold 336/336 (exit 0); all 5 gates green on the
final tree.**

## Adversary probe matrix (one probe per moved invariant)

Every mutation is applied to source, the **named** test is run, the
failure is captured, then `git checkout -- <file>` restores the file and
`git status --porcelain -- src tests` is verified **empty** before the
next probe. No fixture or expectation file is ever edited. Each probe
carries `P-0X.log` (full output), `P-0X.exit`, `P-0X-mutation.diff`
(the exact bytes changed) and `P-0X-restore.txt` (post-restore status +
the committed blob hash the file returned to).

| Probe | Moved invariant | Mutation (at `df316a3`) | Named test | Exit |
|---|---|---|---|---|
| P-01 | `adopt_local` wiring (T-4e — closes the Step 4 §7 blind spot) | delete `dispatcher.adopt_local(&char_client);` (`driver.rs:771`, inside `in_world_phase`) | `world_entry_adoption_is_wired_before_the_in_world_loop_starts` | **101** |
| P-02 | stage label of the T-1e-extracted `in_world_phase` body | `DriverError::refused(Stage::InWorld, …)` → `Stage::CharClient` (`driver.rs:797`, the `LoopError::Handler` arm) | e2e `a_flipped_byte_in_a_chat_frame_is_refused_at_the_in_world_stage` | **101** |
| P-03 | dispatcher routing boundary — the `RESPONSE_CHAT` arm | delete `RESPONSE_CHAT => Ok(DispatchOutcome::Chat(parse_chat(cmd, payload)?)),` (`dispatcher.rs:872`) | `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | **101** |
| P-04 | dispatcher fall-through boundary — foreign opcode ⇒ `Unhandled`, never an error | `other => Ok(DispatchOutcome::Unhandled { cmd: other })` → `other => Err(DispatchError::UnexpectedOpcode { expected: RESPONSE_CHAT, cmd: other })` (`dispatcher.rs:881`) | `a_foreign_opcode_is_unhandled_not_an_error` | **101** |

### Failure reproduction (from the logs — each test caught the mutation itself; every mutation *compiled*)

- **P-01:** `assertion 'left == right' failed: world entry adopts the local avatar exactly once`
  (`src\client\driver\tests.rs:581`) — `hits.len()` = 0, not 1. This is
  the same deletion the Step 4 Verifier ran green at 334/334 (Step 4 §4,
  "adopt_local blind spot"); Phase 1's own pre-move check
  (`phase1/06-gap-mutation-check.exit` = `EXIT=101`) is its baseline.
- **P-02:** `expected a Refused{InWorld} refusal, got Refused { stage: CharClient, detail: "game opcode 118 body truncated: expected 35 bytes, got 34" }`
  (`tests\e2e_loopback.rs:796`) — the mutated stage label surfaces
  verbatim through the refusal, so the label of the moved body is pinned.
- **P-03:** `called 'Result::unwrap_err()' on an 'Ok' value: Unhandled { cmd: 118 }`
  (`src\game\dispatcher\tests.rs:232`) — with the arm gone the chat
  opcode falls through to the unhandled boundary instead of parsing.
- **P-04:** panic at `src\game\dispatcher\tests.rs:932:44` —
  `state.dispatch(999, &[]).unwrap()` on the now-`Err` fall-through
  (`.unwrap()` on `Err(UnexpectedOpcode { expected: 118, cmd: 999 })`).

### Restore proof

After every probe: `git status --porcelain -- src tests` printed nothing
(recorded in `P-0X-restore.txt` alongside the `git diff --stat` and the
`HEAD:<file>` blob the file returned to). Final post-probe state:
`src/tests` clean; the only untracked path is the new
`evidence/step5-refactor/phase5/` directory itself.

## Determinism (3× full `cargo test`)

| Run | lib | fixtures | e2e | ignored | Exit |
|---|---|---|---|---|---|
| `04-cargo-test-run1` | 303 | 11 | 22 | 5 (1 lib + 4 `live_integration`) | **0** |
| `04-cargo-test-run2` | 303 | 11 | 22 | 5 | **0** |
| `04-cargo-test-run3` | 303 | 11 | 22 | 5 | **0** |

Each run: **336/336 passed, 0 failed** — counts copied from the
`test result:` lines, never hand-computed. Same counts as Phases 1–4:
Phase 5 adds **zero** tests (probes + determinism only).

## Gates (final tree, all 5)

| Gate | Exit | Log |
|------|------|-----|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | **0** | `01-cargo-check.log` |
| `cargo clippy --all-targets -- -D warnings` | **0** | `02-cargo-clippy.log` |
| `cargo fmt --check` | **0** | `03-cargo-fmt.log` |
| `cargo test` (runs 1–3 above) | **0** | `04-cargo-test-run{1,2,3}.log` |
| `cargo doc --no-deps` | **0** | `05-cargo-doc.log` |

## Layout

```
evidence/step5-refactor/phase5/
  01..03, 05-*.log|.exit   final 5-gate capture
  04-cargo-test-run{1..3}  determinism runs
  probes/
    P-0X.log               named-test output under mutation
    P-0X.exit              EXIT=101
    P-0X-mutation.diff     the exact mutation applied
    P-0X-restore.txt       post-restore status (empty) + returned blob hash
```
