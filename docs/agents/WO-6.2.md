# Work Order — Step 6.2: Hermetic E2E Loopback Harness

| | |
|---|---|
| **Step** | 6.2 — Hermetic E2E Loopback Harness (`CONTRIBUTING_PHASE2.md:113-124`) |
| **Lane** | Implementer (dual-hat: the implementer also drafts this WO, as in 6.1 — recorded deviation, §1.3) |
| **Branch** | `step/6.2-harness`, cut from `step/6.1-driver` @ `f3c256f` (recorded deviation, §1.2) |
| **Baseline** | `f3c256f` — suite 209/209, five gates green (check 0 · clippy 0 · fmt 0 · test · doc 0) |
| **Frozen API** | Step 6.1 driver (`src/client/driver.rs`, WO-6.1 §4) — consumed, not modified |
| **Depends on** | Step 6.1 delivered (`f3c256f`); row 6.1 remains `IN_PROGRESS` pending the Verifier's sign-off |

## 1. Deviations recorded up front

1. **Branch base.** `CONTRIBUTING_PHASE2.md:252` cuts step branches from `main`. Step 6.1 has
   not merged to `main` yet (it awaits the independent Verifier verdict), so this branch is cut
   from `step/6.1-driver` @ `f3c256f`. The 6.1→main merge is the Custodian's act after the row
   is signed `PASSED`; this branch then rides that merge.
2. **Cadence override.** The cadence rule (`CONTRIBUTING_PHASE2.md`, lanes) says no lane opens
   the next step until the previous row is `PASSED`. Row 6.1 is `IN_PROGRESS`; the project
   owner (user) explicitly authorized opening 6.2 in parallel with 6.1's verification. This
   WO records the override. Nothing in this step writes `PASSED` on any row — sign-off stays
   with the Verifier (A) and QC.
3. **Dual-hat authorship.** Same as 6.1: the implementer drafts this WO and then implements
   it. The Verifier should treat the §9 delivery note as self-reported until independently
   reproduced.

## 2. Scope (verbatim authority: `CONTRIBUTING_PHASE2.md:113-124`)

- **Path:** `tests/e2e_loopback.rs` (new integration test) + `tests/e2e_loopback/**`.
- **Scope:** in-process fake front service and fake game service speaking the full conversation
  byte-exactly and driving `driver::run` end to end, including in-world dispatcher frames
  (6/11/118/200/215) and heartbeat observation, then a server-side close that must surface as
  a clean teardown.
- **Acceptance:**
  1. the full-flow test passes with no network beyond loopback;
  2. byte-exact assertions cover **every** server→client frame sent and **every**
     client→server frame received in the flow;
  3. mutation tests prove at least one detection per stage (front, game login, world entry,
     in-world);
  4. the harness never calls a parser/encoder under test to build its expectations (F-07);
  5. all five gates green.

## 3. Non-goals

- No soak, no scaled cadence, no robustness fuzzing — that is Step 6.3 (`:126-135`).
- No live-server anything: reachability and cipher-key agreement with a real deployment stay
  live-evidence territory (`:81-83`).
- No changes under `src/**`. The driver API is frozen (WO-6.1 §4); if the harness needs a seam
  the driver does not offer, that is a 6.1 defect to report, not a license to edit.
- No `TASK_MATRIX.md` edits (never the implementer's file).
- No new dependencies. `tokio` (with dev-only `test-util`), `tracing`, `tracing-subscriber`
  are already present.

## 4. File plan (ownership: `CONTRIBUTING_PHASE2.md:203-218`)

| File | Role | Writer |
|---|---|---|
| `tests/e2e_loopback.rs` | test root: session orchestration, full-flow test, mutation tests, transcript assertions | Implementer (single writer) |
| `tests/e2e_loopback/frames.rs` | **the only byte source**: hand-placed wire literals/builders with per-field citations; `frame()`/`encode_link()`/`read_ciphered()` transport mirrors; expected-conversation table | Implementer (shared fixture, single writer) |
| `tests/e2e_loopback/front_double.rs` | fake front service: SECRETKEY on accept, then notice/worldlist/login conversation, byte-exact validation of every client frame | Implementer |
| `tests/e2e_loopback/game_double.rs` | fake game service: opcode-1 handshake, game login, 15 roster frames, select-char, SERVERINFO + opcode-5, village round-trip, in-world frame set, heartbeat observation, FIN | Implementer |

Split by double; no file is written by two lanes. Root is the only test-entry file.

## 5. Design

### 5.1 The conversation, both legs

Front leg (plaintext server→client; session-ciphered client→server, seeds `0xA7/0x5C` chosen
by the double in its own SECRETKEY frame):

```text
accept → 30000 SECRETKEY (10 B wire)            [unsolicited, front.rs:5-6]
       ← REQUEST_NOTICE    (5 B)   op 2         [world_list.rs:134-135]
       → RESPONSE_NOTICE   (21 B)  op 1001, 16-byte notice text
       ← REQUEST_WORLDLIST (25 B)  op 3, szAccountID[20] = cfg.user_id
       → RESPONSE_WORLDLIST(435 B) op 1002, usCount 1 + one 428-byte WORLDINFO
       ← REQUEST_LOGIN     (65 B)  op 1, userid/pw/mac 20 B each
       → RESPONSE_LOGIN    (113 B) op 1000, 108-byte payload, siAuthKey captured
       ← (client drops the front connection — driver.rs:574-576)
```

Game leg (link-ciphered server→client, fixed `0x12/0x34` installed at construct,
`entry.rs:399`; session-ciphered client→server, keys `0x7E/0xAD` chosen by the double in the
opcode-1 handshake):

```text
accept → op 1  (13 B) char_unique 4242 + keys 0x7E/0xAD + version   [entry.rs:251-256]
       ← op 2  (53 B) siAuthKey == the front's, szID == the response's userid [login.rs:216-219]
       → op 3 ×15 (989 B each) roster, slot 2 occupied                [char_info.rs:87-95]
       ← op 4  (9 B)  SELECT_CHAR slot 2                              [world_entry.rs:340-354]
       → op 106 (53 B) SERVERINFO, siServiceArea = 0x08               [world_entry.rs:155-197]
       → op 5  (597 B) MakeCharInfo, uiMakeCharUnique == 4242         [char_info.rs:460-543]
       ← op 1101 (9 B) VILLAGESTRUCT ref                              [world_entry.rs:357-375]
       → op 1102 (27 B) VillageStruct, non-negative village           [world_entry.rs:552-560]
       → in-world: op 6 (2×592 PC records), 11 (logout), 118 (chat),
         200 (char update), 215 (person IP)                           [dispatcher.rs:16-28]
       ← heartbeat op 100 (5 B wire, `05 00 00 64 00`) ≥1             [heartbeat/tests.rs:74]
       ⨯ double FINs → loop sees EOF → Cause::ServerDisconnect → terminate → Ok(TornDown)
```

### 5.2 F-07 posture (`CONTRIBUTING_PHASE2.md:64-83`)

- **Expectations are hand-placed plaintext bytes**, each field written at its literal offset
  with the legacy/server authority cited inline — the same dual-sourced fixtures the
  per-module suites already use, lifted as independent byte sources (e.g. `SECRETKEY_FRAME`
  from `world_list/tests.rs:22`, the heartbeat image from `heartbeat/tests.rs:74`, the
  builders' arithmetic from `dispatcher/tests.rs` and the packed layouts from `char_info.rs`,
  `world_entry.rs`, `login.rs`). No client parser (`parse_notice`, `WorldList::from_frame`,
  `parse_login_response`, `parse_char_list_entry`, `parse_make_char_info`, `dispatch`, …) is
  ever called to construct an expected byte.
- **The cipher is transport, not expectation.** The doubles encode server→client frames with
  `Cipher::link_cipher()` / `Cipher::session_cipher()` and decode client→server frames the
  same way the real services do (`GetEncrypt()->Decode`, `FrontClientProtocol.cpp:85`; the
  server port's `main.rs:704` uses this same `Cipher`). The expectation is the plaintext
  literal table; the cipher call is the wire transform both real peers perform, and its
  correctness is 1.3's suite's job. What the harness must never do — and does not — is derive
  an expected *ciphertext* from the client's own encoder.
- **Client→server frames** are checked byte-for-byte after decode against independent
  constants (opcode/usSize/payload literals with their own citations), never against
  "whatever the encoder produced".
- **Mutation tests** (§7) prove the pipeline notices a single flipped byte at each stage.

### 5.3 Clock strategy (real clock, deliberate)

The in-world heartbeat cadence is fixed at 10 s on the driver path (`driver.rs:674` hardcodes
`heartbeat_ticker()`; no knob exists, and inventing one is out of scope). Pausing tokio's
clock is **not** available for this test: `driver::run` dials through
`FrontLink::connect`/`GameLink::connect`, whose 5 s connect window races tokio's
virtual-time auto-advance against the kernel handshake — the failure mode and its statistics
are written up in `src/client/event_loop/tests.rs` (module docs, "QC follow-up
2026-10-27"), and that module's fix (`from_stream` with a timerless dial) is deliberately not
exposed by the driver, which is the production path. The full-flow test therefore runs on the
real clock and waits out exactly one heartbeat (~10 s). Mutation tests fail before the
in-world loop and cost milliseconds.

### 5.4 Observable outcome

`DriverOutcome` carries no cause; the teardown cause is asserted from the emitted `INFO`
record ("session ended", `driver.rs:510-520`): the full-flow test captures tracing output via
a scoped thread-local subscriber (`tracing-subscriber` is already a dependency; the
current-thread test runtime keeps the capture hermetic) and asserts the record renders
`stage=TornDown` and `cause=Some(ServerDisconnect)`.

## 5b. Frozen API consumed (WO-6.1 §4 — read-only)

`DriverConfig{front,game,user_id,password,mac_address,char_slot,village_ref,drain_bound}` ·
`driver::run(&DriverConfig,&Shutdown) -> Result<DriverOutcome,DriverError>` ·
`DriverOutcome{stage,worlds,char_unique,char_slot,village}` · `Stage` (13 variants) ·
`DriverError::{Cancelled,Refused,Stage}` · `Shutdown::new()/trigger(Cause)`.

Test configuration: `char_slot = 2` (must lie in `0..SELECT_CHAR_SLOT_MAX` = 0..3,
`world_entry.rs:189` — the driver's own validation only caps at `CHAR_SLOTS`, so a
higher slot would pass config validation and then fail at send), `village_ref = Some(7)`
(in `1..MAX_VILLAGE_NUMBER`), credentials of ≤19 bytes so every C-string field fits with a
terminator.

## 6. Transcript assertion (acceptance item 2)

Both doubles record every frame they send and every frame they receive as
`(leg, direction, plaintext)` entries plus the raw wire length. The full-flow test rebuilds
the **entire expected conversation** as a literal table in `frames.rs` and asserts the
transcript equals it entry-for-entry, byte-for-byte, both directions — one assertion that
fails loudly on any added, dropped, reordered or altered frame on either leg.

## 7. Mutation tests (acceptance item 3)

Each mutation flips exactly one byte in one server→client frame (the double's `Mutation`
option) and the test asserts both that the client refuses at the expected stage and that the
harness observed the corrupted frame on its transcript:

| Stage | Corruption | Expected observation |
|---|---|---|
| front | `usCount` of the world-list response 1→2 (frame shorter than declared) | `DriverError::Stage{stage: WorldList}` (Truncated) |
| game login | first byte of the occupied roster entry's `szName` → NUL (slot reads empty) | `DriverError::Refused{stage: CharList, ..}` ("slot 2 holds no character") |
| world entry | `uiMakeCharUnique` low byte of the opcode-5 record (≠ handshake 4242) | `DriverError::Refused{stage: CharClient, ..}` ("belongs to another character") |
| in-world | `siDataSize` of the chat body +1 (declared size disagrees) | `DriverError::Refused{stage: InWorld, ..}` (dispatcher refusal) |

## 8. Forbidden shortcuts

- No client parser/encoder called to build an expectation (F-07, restated §5.2).
- No test dials anything but `127.0.0.1` ephemeral ports; nothing hardcodes a non-loopback
  address (F-01).
- No `#[ignore]`, no feature flags, no `src/**` edits, no new dependencies.
- No sleeps as synchronization: the doubles sequence on socket reads, not on timers (the one
  wall-clock wait is the protocol's own 10 s heartbeat tick).
- Never `mod.rs` for rustfmt (leaf files only); never `2>&1` on gates; commits via temp file
  + `git commit -F` with the trailer block (`:255-259`).

## 9. Return blocks (append; do not rewrite the sections above)

### Delivery note — Implementer

_(appended after implementation)_

### Adversarial findings — Adversary

_(optional unless requested)_

### Verdict — Verifier

_(independent; reproduces all five gates and the byte-exactness claims)_

## 10. Definition of Done

- [x] WO committed before code, with baseline and deviations.
- [ ] `tests/e2e_loopback.rs` + `tests/e2e_loopback/{frames,front_double,game_double}.rs` delivered.
- [ ] Full flow green on loopback: front → game → world entry → in-world → heartbeat →
      server FIN → `Ok(DriverOutcome{stage: TornDown})` with the `ServerDisconnect` cause
      logged.
- [ ] Transcript == expected conversation, byte-exact, both directions, every frame.
- [ ] Four mutation tests, one per stage, each observing a flipped byte.
- [ ] Five gates green: `RUSTFLAGS=-D warnings cargo check --all-targets` ·
      `cargo clippy --all-targets -- -D warnings` · `cargo fmt --check` · `cargo test` ·
      `cargo doc --no-deps`.
- [ ] Delivery note appended to §9 with gate evidence; commit carries the trailer block;
      pushed to `step/6.2-harness`.
