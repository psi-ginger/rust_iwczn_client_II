# Step 6 — Phase 1: Characterization (gap scan + added login framing test)

**Result: 336 → 337 (+1), all 5 gates exit 0.**

## Gap scan (recorded outcome of the WO §3 scan)

Both C3 sites and all three C4 arms were already behavior-pinned by the existing
suite, but the WO left room for one additive characterization test, so Phase 1 adds
a concrete successful-framing case for the login dialog's default credentials.

## Added test (the only test-count change in Step 6)

`src/auth/login/tests.rs` — `short_credentials_are_terminated_and_zero_filled_at_every_field`
(inserted after `login_request_payload_is_byte_exact_at_literal_offsets`):

- Builds `login_request_payload("test2", "test", "A1B2C3D4E5F6")` — ID **`test2`**
  (5 bytes), Password **`test`** (4 bytes), a valid 12-hex MAC.
- Asserts the full 60-byte frame equals a zero-filled buffer with `test2\0` at
  `0..6`, `test\0` at `20..25`, the MAC at `40..52` (`FrontMsg.h:344-347`).
- Terminator assertions at `payload[5]` (userid) and `payload[24]` (userpw).
- Zero-fill assertions on all three tails (`6..20`, `25..40`, `52..60`) —
  `StringCchCopy` semantics (`Lib/commonutil.cpp:6-16`), MAC shape
  (`Client.cpp:7286`).

The test passed on its first run:
`test auth::login::tests::short_credentials_are_terminated_and_zero_filled_at_every_field ... ok`.

## Pre-existing pins (survey, no action needed)

| Concern | Pin |
|---|---|
| C3 too-long refusal (auth) | `auth/login/tests.rs:162` `credentials_that_do_not_fit_are_refused_not_truncated` (3 fields × 3 lengths) |
| C3 too-long refusal (game) | `game/login/tests.rs:216` `a_credential_without_room_for_its_terminator_is_refused` |
| C3 NUL-cut + zero-fill (auth) | `auth/login/tests.rs:111` `login_request_payload_is_byte_exact_at_literal_offsets` |
| C3 NUL-cut + zero-fill (game) | `game/login/tests.rs:183` `strings_stop_at_their_terminator_and_zero_fill_the_rest` |
| C4 opcode arm (world_entry) | `world_entry/tests.rs:398` + `:503` |
| C4 truncated/trailing arms | `dispatcher/tests.rs:224,556,660,888`; `world_entry/tests.rs:398,503` |

## Gates at the Phase 1 boundary

| File | Gate | Exit |
|------|------|------|
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` | EXIT=0 |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` | EXIT=0 |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (0-byte log = clean) | EXIT=0 |
| `04-cargo-test.exit/.log` | `cargo test` — **337/337** (304 lib + 11 fixtures + 22 e2e) | EXIT=0 |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` | EXIT=0 |

Ignored set unchanged: 5 (1 lib operator-window demo + 4 `live_integration`).
