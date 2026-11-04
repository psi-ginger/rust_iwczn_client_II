# Step 6 — Phase 5: Adversary Probes, Determinism, Final Gates

**Result: 4/4 probes EXIT=101 with byte-clean restores; 3× determinism at
337/337; final 5 gates all EXIT=0; doc 0 warnings.**

## Adversary probe matrix (one probe per moved invariant)

| # | Mutation (one committed file) | WO-named test(s) | Outcome |
|---|---|---|---|
| PR-1 | `src/protocol/bytes.rs` shared copier bound `len >= CREDENTIAL_FIELD_LEN` → `len >` | `credentials_that_do_not_fit_are_refused_not_truncated`, `a_credential_without_room_for_its_terminator_is_refused` | **EXIT=101, 2/2 FAILED** |
| PR-2 | `src/protocol/bytes.rs` drop the NUL cut (`position(...)` line → `let len = raw.len();`) | `strings_stop_at_their_terminator_and_zero_fill_the_rest`, `login_request_payload_is_byte_exact_at_literal_offsets` | **EXIT=101, 2/2 FAILED** |
| PR-3 | `src/game/gate.rs` invert the opcode check (`cmd != expected_cmd` → `cmd == expected_cmd`) | `server_info_rejects_foreign_opcodes_and_wrong_lengths`, `village_struct_rejects_foreign_opcodes_and_wrong_lengths` | **EXIT=101, 2/2 FAILED** |
| PR-4 | `src/game/gate.rs` swap the `Truncated`/`TrailingBytes` arms | `a_char_update_length_boundary_is_refused_exactly`, `a_chat_body_that_disagrees_with_its_si_data_size_is_refused` | **EXIT=101, 1/2 FAILED** — see disclosure |

Every mutation **compiled** (each run got as far as executing tests); each was
caught by the named test's own hand-derived expectation, not by a panic or a
compile error.

## Disclosure — PR-4's second named target passes (WO probe-target error)

`a_chat_body_that_disagrees_with_its_si_data_size_is_refused` did **not** fail
under PR-4, and correctly so: the chat body's `Truncated`/`TrailingBytes`
results come from `parse_chat`'s **own inline declared-size gate**
(`src/game/dispatcher.rs:499` and `:506`, against `siDataSize`), which never
routes through `gate_frame`. C4's scope (WO §2) is exactly the two `gate_frame`
functions; `parse_chat`'s inline contract was always out of scope, so its arms
are untouched by the mutation and the test rightly still passes. The WO
probe matrix listed this test in error.

Mutation effectiveness on **both shared arms** is proven by the target that did
fail — `a_char_update_length_boundary_is_refused_exactly` asserts the shared
gate's `Truncated` at `len-1` **and** `TrailingBytes` at `len+1` (and `Ok` at
the exact length); under the swap both length assertions failed. No coverage
was lost: the chat test continues to pin `parse_chat`'s inline contract on the
unmutated tree.

## Restore proof (all four probes)

For each probe, after the run:
- `git restore <mutated file>`
- `git status --porcelain -- src tests` → **empty**
- `git diff --exit-code -- <mutated file>` → **EXIT=0**

Saved as `P-0X-restore.log` next to `P-0X.{log,exit,diff}` (`P-0X.diff` records
the exact mutation applied). Final `git status --porcelain -- src tests` at
bundle time: empty.

## Determinism (3× full `cargo test`, final tree)

| Run | Exit | Result |
|---|---|---|
| `determinism-1` | EXIT=0 | **337/337** (304 lib + 11 fixtures + 22 e2e), 5 ignored |
| `determinism-2` | EXIT=0 | **337/337**, 5 ignored |
| `determinism-3` | EXIT=0 | **337/337**, 5 ignored |

## Gates (final tree, all 5)

| File | Gate | Exit |
|------|------|------|
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` | EXIT=0 |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` | EXIT=0 |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (0-byte log = clean) | EXIT=0 |
| `04-cargo-test.exit/.log` | `cargo test` — **337/337**, 5 ignored | EXIT=0 |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` — 0 warning lines | EXIT=0 |

## Committed-blob hash table (L-2)

`blob-hashes.txt` — `git rev-parse HEAD:path` over all 8 write-guard src files
at Phase 4's commit `5a6d523` (generated before this bundle commit; the
Probes mutated working copies only and restored byte-clean).

## Layout

```
P-01..P-04:      probe log, exit (101), diff (the mutation), restore proof
determinism-1..3: full-suite runs (log + exit)
01..05:           final gate logs + exits
blob-hashes.txt:  committed blob hashes at 5a6d523
README.md:        this file
```
