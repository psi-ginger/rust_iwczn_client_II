# Step 6 — Phase 4: C4 Motion (`gate_frame` core consolidation)

**Result: 2 donor bodies → 1 shared core + 2 thin wrappers. Count held at 337,
all 5 gates exit 0. Call sites byte-identical (zero hunks at any call site).**

## What moved

| Donor (before) | After |
|---|---|
| `src/game/dispatcher.rs:426` `fn gate_frame` — early-return `if` chain, `Result<(), DispatchError>` | `src/game/gate.rs` `pub(crate) fn gate_frame` → `Result<(), GateViolation>`; dispatcher keeps a **thin wrapper** whose signature and call sites are unchanged |
| `src/game/world_entry.rs:381` `fn gate_frame` — `match payload.len()` form, `Result<(), WorldEntryError>` | same shared core; world_entry keeps its **thin wrapper** |

- New file `src/game/gate.rs`: `pub(crate) enum GateViolation { UnexpectedOpcode { expected, cmd }, Truncated { opcode, expected, got }, TrailingBytes { opcode, expected, got } }` — field names identical to both caller enums — plus the core body.
- `src/game/mod.rs`: `+1` line — `mod gate;` (private; no new public export).
- Each wrapper is an explicit `.map_err(|v| match v …)` one-for-one into its own error type (the repo's conversion idiom, `event_loop.rs:236-239`).

## Disclosures

1. **`opcode` field source.** The dispatcher donor filled `opcode` with
   `expected_cmd`; the world_entry donor filled it with `cmd`. The shared core
   uses `expected_cmd` — equal to `cmd` on every path, because the opcode guard
   returns first. No observable change.
2. **Style delta absorbed.** Donors differed only as `if`-chain vs `match`;
   the core is the early-return form (dispatcher's), which is behavior-equal.
3. **Wrapper signatures byte-identical** to the donors (params, order, return
   type) — no call site was edited (verified: the diff contains no hunks at any
   `gate_frame(` call site).

## Unchanged (verified in the diff)

- `DispatchError` / `WorldEntryError` enums and all variant shapes.
- All parser bodies; `auth::{login,world_list}` inline gates, `char_info`,
  `entry` inline gates — out of scope per WO §2 (7.0 defined C4 as exactly
  these two functions).

## Citations

- Donor wrapper docs retained (their sentences extended with the wrapper
  note); `gate.rs` adds no legacy `*.cpp`/`*.h` refs, so aggregate citations
  across touched files are unchanged (net no-loss holds — no legacy citation
  was deleted anywhere in this phase).

## Gates at the Phase 4 boundary

| File | Gate | Exit |
|------|------|------|
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` | EXIT=0 |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` | EXIT=0 |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (0-byte log = clean) | EXIT=0 |
| `04-cargo-test.exit/.log` | `cargo test` — **337/337** (304 lib + 11 fixtures + 22 e2e) | EXIT=0 |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` | EXIT=0 |

Named C4 pins through the shared core (from `04-cargo-test.log`):
`server_info_rejects_foreign_opcodes_and_wrong_lengths ok`,
`village_struct_rejects_foreign_opcodes_and_wrong_lengths ok`,
`a_char_update_length_boundary_is_refused_exactly ok`,
`a_chat_body_that_disagrees_with_its_si_data_size_is_refused ok`.
