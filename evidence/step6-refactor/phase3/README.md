# Step 6 — Phase 3: C3 Motion (`copy_credential` consolidation)

**Result: 5 call sites → 1 shared core. Count held at 337, all 5 gates exit 0.**

## What moved

| Before (2 copies) | After (1 core) |
|---|---|
| `src/auth/login.rs` local `fn copy_credential` (19 lines, `Result<(), LoginError>`) | `src/protocol/bytes.rs` `pub(crate) fn copy_credential` → `Result<(), CredentialTooLong>` + `pub(crate) struct CredentialTooLong { field, len }` + private `const CREDENTIAL_FIELD_LEN: usize = 20` |
| `src/game/login.rs` local `fn copy_credential` (14 lines, `Result<(), GameLoginError>`) | same |

Both donor bodies were identical modulo the const name and error type —
NUL cut at first `0` byte, refuse `len >= 20`, copy + zero-fill,
`debug_assert_eq!(slot.len(), …20…)`.

## Call-site conversion (explicit `.map_err`, repo idiom)

- `src/auth/login.rs` `login_request_payload` — 3 sites
  (`userid`, `userpw`, `macaddress`) →
  `.map_err(|e| LoginError::CredentialTooLong { field: e.field, len: e.len })?`
- `src/game/login.rs` `game_login_payload` — 2 sites
  (`szID`, `szPassword`) →
  `.map_err(|e| GameLoginError::CredentialTooLong { field: e.field, len: e.len })?`

No `From` impl added (blanket-impl overlap rule documented at
`src/client/event_loop.rs:236-239`).

## Unchanged (verified in the diff)

- Both public error enums and their variant shapes (`CredentialTooLong { field, len }`
  on each) — tests still match on them (`auth/login/tests.rs:183`,
  `game/login/tests.rs` too-long asserts).
- Module consts `CREDENTIAL_LEN` (`auth/login.rs:167`, used in the
  `FrontMsg.h:344-347` error display) and `PLAYER_NAME_LEN` (`game/login.rs:141`,
  used in the `CommonHeader.h:575` error display + `char_info`/`dispatcher`) —
  no dead code.
- Public function signatures of `login_request_payload` / `game_login_payload`.

## Citations (net no-loss)

- `Lib\commonutil.cpp:6-16` (`StringCchCopy`/`MStrCpy`) moved from the game
  donor doc into the shared doc; auth's uninitialized-stack note moved with it.
- Module doc of `bytes.rs` widened to name the copier; both module docs of the
  callers keep their own `commonutil.cpp`/`FrontMsg.h`/`CommonHeader.h` refs.
- New refs added in `bytes.rs`: `FrontMsg.h:344-347`, `CommonHeader.h:575`,
  `event_loop.rs:236-239`.

## Gates at the Phase 3 boundary

| File | Gate | Exit |
|------|------|------|
| `01-cargo-check.exit/.log` | `RUSTFLAGS=-D warnings cargo check --all-targets` | EXIT=0 |
| `02-cargo-clippy.exit/.log` | `cargo clippy --all-targets -- -D warnings` | EXIT=0 |
| `03-cargo-fmt.exit/.log` | `cargo fmt --check` (0-byte log = clean) | EXIT=0 |
| `04-cargo-test.exit/.log` | `cargo test` — **337/337** (304 lib + 11 fixtures + 22 e2e) | EXIT=0 |
| `05-cargo-doc.exit/.log` | `cargo doc --no-deps` | EXIT=0 |

Named pins through the shared core (from `04-cargo-test.log`):
`credentials_that_do_not_fit_are_refused_not_truncated ok`,
`a_credential_without_room_for_its_terminator_is_refused ok`,
`strings_stop_at_their_terminator_and_zero_fill_the_rest ok`,
`login_request_payload_is_byte_exact_at_literal_offsets ok`,
`short_credentials_are_terminated_and_zero_filled_at_every_field ok`.
