# Step 5 — Phase 4: T-3e Test-Support Consolidation

**Verdict: complete — the client group's duplicated test helpers now live
once, the two cross-group wire literals import from their original donors
byte-identically, all 5 gates green, test count locked at 336/336.**

Counterpart to Step 3's `auth/test_support.rs` (T-3) and Step 4's
`game/test_support.rs` (T-3g). One new file, seven touched files, **test
code only** — zero production (`non-test`) edits.

## What moved

| Duplicate | Was at | Now | Count after |
|---|---|---|---|
| `local_listener` | `event_loop/tests.rs`, `heartbeat/tests.rs`, `teardown/tests.rs` (3 copies) | `src/client/test_support.rs` (new), `pub(crate)` | **1** in the client group |
| `HANDSHAKE_PAYLOAD` (+`KEY1`/`KEY2`) | `heartbeat/tests.rs` redefinition | imported from `crate::game::test_support` (the Step 4 donor) | **1** tree-wide |
| `SECRETKEY_FRAME` | `driver/tests.rs` redefinition | imported from `crate::auth::test_support` (the Step 3 donor) | **1** tree-wide |

Diff (`00-diff-stat.txt`, new file excluded from `git diff` until add):

```
 src/auth/mod.rs                |  2 +-
 src/client/driver/tests.rs     | 10 ++++++----
 src/client/event_loop/tests.rs |  9 --------
 src/client/heartbeat/tests.rs  | 22 +++++-----------
 src/client/mod.rs              |  3 +++
 src/client/teardown/tests.rs   | 10 --------
 src/game/mod.rs                |  2 +-
 7 files changed, 21 insertions(+), 37 deletions(-)
```

Plus `src/client/test_support.rs` (new, 30 lines: header, one import, the
verbatim `local_listener`).

## F-07 compliance

- **Zero redefinitions (verified by grep after the edit):**
  - `async fn local_listener` in the client group → **1** hit, `test_support.rs:23`.
  - `const HANDSHAKE_PAYLOAD` tree-wide → **1** hit, `game/test_support.rs:52`.
  - `const SECRETKEY_FRAME` tree-wide → **1** hit, `auth/test_support.rs:18`.
- **Byte-identical payload donors (verified, `True` both):**
  - removed heartbeat `HANDSHAKE_PAYLOAD` `[0x2A,0x00,0x00,0x00,0x7E,0xAD,0x01,0x00]`
    == `game/test_support.rs` donor.
  - removed driver `SECRETKEY_FRAME` `[0x0A,0x00,0x00,0x30,0x75,0x00,0x00,0xA7,0x00,0x5C]`
    == `auth/test_support.rs` donor.
  Nothing was re-derived from a parser under test; the literals are the
  same hand-placed bytes the Step 3/Step 4 audits already verified.
- **Citation docs moved verbatim / no citation lost:**
  - `local_listener`'s doc (`/// Binds a loopback listener on an
    ephemeral port.`) moved with it, identical in all three old copies.
  - heartbeat's import carries a comment pointing at the donor's citation
    docs (`game.rs:98-110`, `game.rs:81-85`) — the payload's own citations
    live with the donor, unchanged.
  - driver's import carries a comment preserving **both** citations: its
    own `FrontMsg.h:114` (the wire image, as `auth::front`'s tests replay
    it) and the donor's `FrontMsg.h:132-141` (struct layout). Neither was
    deleted.
  - The `local_listener` F-01 note (loopback-only binding) is restated in
    the new module header: every importing suite still binds
    `127.0.0.1:0` only.

## Cross-group visibility change

`game/mod.rs` and `auth/mod.rs`: `#[cfg(test)] mod test_support;` →
`#[cfg(test)] pub(crate) mod test_support;` — the minimal widening that
lets `client` suites import the donors. Test-only (`#[cfg(test)]`), so no
production surface changed. `client/mod.rs` gains the same
`#[cfg(test)] mod test_support;` declaration the other two groups carry.

## Deliberately NOT consolidated (surveyed)

- `frame()` / `encode_link()` in `heartbeat` and `event_loop` tests: bodies
  are identical to the game donor but each copy carries a **different
  citation doc** (heartbeat: "pre-cipher image of anything the client
  sends"; event_loop: "the pre-cipher image the server writes through
  `Cipher::encode`"). Their suite-local docs are load-bearing F-07
  annotations; merging them would either lose a citation or rewrite prose
  beyond a move. Out of the approved T-3e scope (checkpoint: `local_listener`
  ×3 + the two wire literals).
- `driver/tests.rs`'s `frame()`: explicitly documented as *not* imported
  ("Written out here rather than imported so this unit test shares no code
  with the Step 6.2 harness") — an intentional isolation decision from
  Step 4.
- `teardown/tests.rs`'s `plaintext_frame()`: a differently-named,
  differently-documented helper (one call site), not a copy of `frame()`.
- `auth/*` and `net/session` suites still define their own
  `local_listener`: they were out of scope for Step 5 (Step 3's bundle did
  not consolidate them either).

## Gates (all 5, this boundary)

| Gate | Exit | Log |
|------|------|-----|
| `RUSTFLAGS=-D warnings cargo check --all-targets` | **0** | `01-cargo-check.log` |
| `cargo clippy --all-targets -- -D warnings` | **0** | `02-cargo-clippy.log` |
| `cargo fmt --check` | **0** | `03-cargo-fmt.log` |
| `cargo test` | **0** | `04-cargo-test.log` |
| `cargo doc --no-deps` | **0** | `05-cargo-doc.log` |

**Test count held: 336/336** — 303 lib + 11 fixtures + 22 e2e; 5 ignored
preserved (1 lib + 4 `live_integration`). Consolidation only: no test
added, removed, or renamed — only where helpers are *defined* changed.
