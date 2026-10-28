# Incident - live server refuses to let a session enter the world (opcode 6 is an NPC list)

| Field | Value |
| :--- | :--- |
| **Date** | 2026-10-28 |
| **Finding** | `LIVE-01` (opened by the Verifier's live-integration pass) |
| **Lane** | Implementer (fix lane `fix/live-loginmany-npc`) |
| **Status** | `RESOLVED` 2026-10-28 - fix verified at `701963a` (S4: 5/5 gates, 220/220) and live-verified green (world entry, wire-measured heartbeats, `stage=TornDown` exit 0); the merge authorization is recorded in the matrix note and the Verifier section below |
| **Branch** | `step/6.2-harness` @ `57bd58f` (code `febc38a`), evidence in `evidence/live/` |
| **Rows 6.1/6.2** | **unchanged, `PASSED`** - this defect predates 6.1 (Step 4.4's documented refusal posture); the hermetic WO conformance is unaffected and its evidence bundles stand |

## Summary

The full driver session reaches the live world and is refused by the **first
in-world frame**. The live game server's opcode-6 (LOGINMANY) broadcast is an
**NPC list**: 34 records of exactly **68 bytes** each, every head carrying
`uiType = MAKECHARTYPE_NPC (0)`. The port strides opcode-6 records by 592
(the `MAKECHARTYPE_PC` size) and deliberately refuses non-PC records, so no
session can complete against this server as delivered:

```
ERROR goonzu_client: session failed error=InWorld refused: LOGINMANY record
at offset 0 has uiType 0, not MAKECHARTYPE_PC (1) - the NPC stride is not
pinned by any verified source
```

Reproduced **4/4** (3 direct runs + 1 through a transparent capture relay),
driver exit code **1** each time, ~1.1s after world entry
(`evidence/live/drv_live_canonical.log`, `drv_live_relayed.log`).

Everything upstream of the refusal passed live: front handshake (opcode
30000, seeds extracted), front notice/worldlist/login, game handshake
(opcode 1, `char_unique` allocated), game login (opcode 2, 48-byte frozen
shape), 15 roster frames (3 x 984 B), character select (4 B), SERVERINFO
106 (banner accepted), the opcode-5 PC record (592 B) adopted into
`CharClient`, and 50 further unknown in-world frames absorbed by the
dispatcher without error. The failure is exactly and only the LOGINMANY
parse.

## Root cause

Two facts meet:

1. **The live server always leads its in-world broadcast with NPCs, and the
   NPC record is 68 bytes.** The port's own module docs already said the
   stride comes from the leading `uiType` bit
   (`Client-Login.cpp:53-85`, `char_info.rs` module docs) but refused
   non-PC records because "no binary measurement pins"
   `sizeof(cltMakeCharInfoCommon)` - the source snapshot is provably stale
   for this struct family (`cltMakeCharInfo` derives 524 there where the
   binary's is 592, `dispatcher.rs:38-45`).
2. **The port therefore walks opcode-6 records with the PC stride only.**
   `parse_make_char_info_many` computes `records_count = payload.len() /
   MAKE_CHAR_INFO_PAYLOAD_LEN` (592) and refuses any record whose
   `uiType` bit0 is not 1 (`char_info.rs:655-692`; refusal error at `:183`).

Against the live frame: payload 2,312 B; `2312 % 592 = 536` (not even a whole
number of PC records); the first word is `0x00000046` (bit0 = 0). The
refusal fires at offset 0, correctly per its own contract - the contract is
what the live wire falsifies.

This is the class of question the `a0c3ddf` adjudication explicitly deferred
to live proof ("refusal vs. dispatch"), and the class every hermetic gate
was structurally blind to: the 6.2 harness's opcode-6 fixture
(`tests/e2e_loopback/frames.rs:729`) builds its LOGINMANY body from PC
records only, mirroring the parser's own assumption. A loopback double
cannot falsify a stride both sides share.

## The measurement that "does not exist" now exists

`sizeof(cltMakeCharInfoCommon) = 68` is pinned by the live capture
(`evidence/live/cap/s2c.bin`, decoded `decode.log`/`decode2.log`):

- **69/69 frames integrity-true** decoded with the client's own
  `Cipher::link_cipher()` (`entry.rs:399`), consuming exactly
  50,996/50,996 captured bytes - the framing and cipher assumptions behind
  the read-back are the client's shipped ones, and they are confirmed.
- The refused frame is opcode 6, payload **2,312 = 34 x 68** exactly.
- All 34 record heads read `uiType = 0`.
- `uiMakeCharUnique` (u16 at `+4`, per the shared header map in
  `decode_record`, `char_info.rs:601-633`) steps **monotonically
  8500 -> 8533** across all 34 records - 34 consecutive ids can only land
  if 68 is the true record boundary. (Correction 2026-10-28, Verifier
  self-finding: an earlier draft of this section said "8492 -> 8533" - a
  mis-division of the log's hex `0x2134..0x2155`, which is 8500..8533 as
  the implementer's delivery states. The log's hex is authoritative; the
  monotonic-step evidence is unaffected.)
- Under that stride the header fields decode sensibly under the same map the
  PC record uses: `uiLevel` constant 1, `uiLife` constant 40, `uiX`/`uiY`
  varying per record (`decode2.log`), i.e. `cltMakeCharInfoCommon` is the
  shared prefix of the 592-byte `cltMakeCharInfo` - consistent with
  `Client-Login.cpp:62-75`'s stride-by-type dispatch.

The 68-byte constant is therefore wire-pinned, not source-derived. Before
adoption it should still be cross-checked against the header field sum
(`CommonHeader.h`, the 1969-1997 field block) per the citation discipline.

## Impact

- **The port cannot complete a session on this live server.** In-world
  heartbeat and graceful `Ctrl+C -> Stage::TornDown` teardown are
  consequently unverifiable live (session dies at ~1.1s; the first heartbeat
  tick would be at +10s).
- **The merge to `main` is held** (user directive: live must be green first).
- Rows 6.1/6.2 are unaffected: the defect is Step 4.4's documented refusal
  posture, and 6.1's stage-named refusal + exit 1 behaved exactly as its WO
  specifies (that mapping is itself now live-proven).
- No data corruption risk: the refusal is loud and immediate; nothing is
  mis-strided or mis-parsed.

## Remediation scope (fix lane `fix/live-loginmany-npc`)

1. **Pin the NPC stride to 68 by citation, not by wire folklore**: constant
   with a doc comment citing the wire evidence (`evidence/live/decode2.log`)
   plus the `CommonHeader.h` field sum, in the style of
   `MAKE_CHAR_INFO_PAYLOAD_LEN` (`char_info.rs:119`).
2. **Teach the opcode-6 walker the type dispatch the legacy performs**
   (`Client-Login.cpp:62-75`): stride by record type - 592 for
   `MAKECHARTYPE_PC`, 68 for `MAKECHARTYPE_NPC` - instead of refusing.
   Decide and document what the port *does* with an NPC record (the legacy
   puts nearby players into `CR[]`; NPCs may be safely recorded or skipped -
   the dispatcher's state model, `dispatcher.rs`, decides; skipping with a
   counted log is the minimal correct behavior if NPC world-state is
   out of scope).
3. **Keep refusing anything else loudly**: an unknown `uiType` or a body that
   is not a whole number of its own stride must still error - the existing
   `PartialRecord`/`Truncated` posture stays.
4. **Close the harness blind spot**: add an NPC-record fixture to the 6.2
   harness (`frames.rs` LOGINMANY builder + expected conversation) so the
   mixed PC+NPC broadcast is hermetically covered, plus a mixed-batch test
   for the walker. F-07 discipline unchanged: hand-placed literal bytes,
   never client-encoded expectations.
5. **Re-verify live** with `evidence/live/run_driver_live.ps1`: the session
   must pass `Stage::InWorld`, survive >= 3 heartbeat intervals, and end
   cleanly at `Stage::TornDown` under `Ctrl+C` with exit code 0. That green
   run unblocks the merge.

## Reproduction

`evidence/live/README.md` indexes every artifact; `run_driver_live.ps1`
(requires `GOONZU_USER_ID`/`GOONZU_USER_PW` in the operator environment)
reproduces the refusal, and `run_capture_session.ps1` reproduces the capture
that `decode_tool.rs` reads back. Endpoint roles are wire-verified, not
label-verified (finding D-1 in the same evidence bundle: the tunnels'
upstream labels were swapped).

## Delivery Note — Implementer (fix lane `fix/live-loginmany-npc`)

Remediation items 1-4 delivered; item 5 (live re-verify) remains the
Verifier's act.

### What changed

| Item | Where |
|---|---|
| NPC stride pinned, dual-sourced | `char_info.rs` `MAKE_CHAR_INFO_COMMON_PAYLOAD_LEN = 68` |
| Type dispatch in the opcode-6 walker | `char_info.rs` `parse_make_char_info_many`: PC → 592, NPC → 68, per `Client-Login.cpp:62-75` |
| NPC decode | new `decode_head` (common fields through 0x3A) + `fill_pc_tail`; `decode_record` = both halves, so opcode 5 is unchanged |
| Loud refusals kept | `PartialRecord` names the stride demanded at the failing offset (68 below the head minimum, 592 behind a PC head); `NonPlayerRecord` → `UnknownRecordType` |
| Counted skip (dispatcher's state model) | `apply_login_many` skips `!is_player` records and counts them; `DispatchOutcome::Entered` gains `npcs: usize` |
| Harness blind spot closed | `e2e_loopback/frames.rs`: `make_npc_record` (hand-placed 68-byte literal builder) + shared `loginmany_body()` = NPC 8500 + PC 4242 + PC 7777, wired into `expected_conversation()` and the game double's in-world set |

### Design decisions

1. **The 68 field sum, restated for the Verifier**: 16 (four `UI32`
   bitfield words, `CommonHeader.h:1969-1977`) + 4 (`siHomeVillageUnique`
   `SI08` + pad + `siJimCharUnique` `SI16`, `:1978-1979`) + 20 (`clOrder`,
   `cltOrderDataInfo:1495-1514`) + 12 (`clIdentity`, `:1846`, `SI08` + pad +
   8-byte identity union) + 16 (`clInnerStatus`, `cltInnerStatusInfo:
   1744-1767`, of which `clEnemyType` is 4 = 2×`SI16`) = **68**. The wire
   agrees on the layout, not just the total: byte 56 of every captured
   record is `0x01` = `ENEMYTYPE_ALLPC`, which lands at
   `clInnerStatus.clEnemyType.siEnemyType` (offset 0x38) under that sum.
2. **`UnknownRecordType` is unreachable by construction — stated
   honestly.** `uiType` is a 1-bit field (`CommonHeader.h:1969`), so the
   walker masks bit 0 and only 0 or 1 can occur on the wire; the loud
   unknown-type arm exists for a widened field meeting a stale stride table
   (the LIVE-01 failure mode) and its doc says so. It is deliberately not
   unit-tested: any synthetic trigger would have to bypass the very mask
   the wire carries, testing the fixture rather than the code. The
   reachable loud refusals are tested for both strides, plus the empty-body
   `Truncated`.
3. **Counted skip, not upsert.** Nothing this dispatcher models (chat,
   char-update deltas, person-IP, logout) acts on an NPC through its
   nearby table, so upserting one would fabricate entries no later frame
   can maintain; the legacy pushes NPCs only into its world-renderer char
   list. The count keeps the skip observable without adding logging to the
   pure parsers: the driver's existing `debug!(... ?outcome ...)` line
   (`driver.rs:684`) carries `npcs`, and on the captured live shape
   (NPC-led) a zero count would mean the stride dispatch mis-read the batch.
4. **Empty-body `Truncated.expected` is now 68**: an NPC-only broadcast is
   legal (the live first in-world frame is exactly that), so the smallest
   legal record is a common head, not a 592-byte PC record.
5. **An NPC record decodes with the PC-only tail zeroed** (fields past
   0x3A at their defaults), mirroring the legacy `clinfo.Set(pclinfo)` copy
   (`Client-Login.cpp:73`), which copies only the common prefix.

### Tests — 220 passing, +2 net against the 218 baseline

- `char_info/tests.rs`: `login_many_decodes_an_npc_record_at_its_68_byte_
  stride` (new), `login_many_walks_a_mixed_batch_by_each_records_own_stride`
  (new: NPC+PC+NPC at offsets 0/68/660, each record's fields pinning its
  position), `login_many_decodes_the_live_broadcast_shape` (new: 34×68 =
  2312 rebuilt from `decode2.log` literals — kind 0x23, ids 8500→8533,
  life 40/x 20, y 338/map 158/speed 2, byte 56 = `ENEMYTYPE_ALLPC`),
  `login_many_refuses_partial_records_of_either_stride` (rewritten: a
  592-expecting and a 68-expecting shortfall, each naming offset +
  shortfall + demanded stride), `login_many_refuses_an_empty_body` (now
  expects 68), and the 68 constant row in `constants_match_the_wire_truth`.
  The old "refuse an NPC record" test is removed by design.
- `dispatcher/tests.rs`: `a_login_many_npc_record_is_counted_and_skipped`
  (new: NPC 8500 + PC 42 + NPC 8501 → `Entered { char_uniques: [42],
  npcs: 2 }`, NPC uniques absent from the table), the partial-body refusal
  now uses a PC-typed tail (`PartialRecord { offset: 592, have: 100,
  expected: 592 }`), and the all-PC upsert test asserts `npcs: 0`.
- E2E loopback: the in-world LOGINMANY frame of the full flow *and* all
  four mutation runs now carries the mixed NPC+PC+PC body from the shared
  `loginmany_body()`; transcript comparison stays byte-exact by
  construction, and F-07 is unchanged (every byte hand-placed; the only
  client call in the harness remains `Cipher::encode`).

### Gates — all five exit 0

`RUSTFLAGS=-D warnings cargo check --all-targets` ·
`cargo clippy --all-targets -- -D warnings` · `rustfmt --check` on the six
changed leaf files + whole-tree `cargo fmt --check` (0 diffs) ·
`cargo test` (211 lib + 9 e2e = 220/220; the 4 `live_integration` tests
stay opt-in/ignored) · `RUSTDOCFLAGS=-D warnings cargo doc --no-deps`.
Logs: `%TEMP%\opencode\live01-{check,clippy,fmt,fmt-leaf,test,doc}.log`.

### Handoff — Verifier (remediation item 5)

Run `evidence/live/run_driver_live.ps1` against this build. Expected
against the captured server shape: the opcode-6 walk consumes the NPC
broadcast cleanly (`npcs` = the NPC count in the `Entered` debug line,
34 on the capture behind `decode2.log`), the session passes
`Stage::InWorld`, survives ≥ 3 heartbeat intervals, and ends at
`Stage::TornDown` under `Ctrl+C` with exit code 0 — which unblocks the
merge to `main`.

## Verifier verification - 2026-10-28 (independent second actor)

Delivery `701963a` (single commit, author `goonzu-rs`; S1 satisfied - the
verifying signer `psi-ginger` authored none of `af4ab01..701963a`).

**S4 gates, clean clone straight from origin at the delivery SHA** (fresh
`target/`, porcelain empty): `cargo check --all-targets`, `cargo clippy
--all-targets -- -D warnings`, `cargo fmt --check` (0 bytes), `cargo test`
(**211 lib + 9 e2e = 220/220**), `cargo doc --no-deps` - all exit 0.
`evidence/live-fix/{check,clippy,fmt,test,doc}.log`, `head.txt`, `env.txt`.

**Adversarial review: no blocking findings.** The walker's minimum-68 gate,
per-record remainder refusals (offset/shortfall/demanded stride), type
dispatch, and the unreachable-but-loud `UnknownRecordType` arm are all
correct; `decode_record` = `decode_head` + `fill_pc_tail` preserves the
opcode-5 field set exactly; the dispatcher's NPC counted-skip carries a
falsifiability argument (zero count would mean the stride dispatch
mis-read the batch); the fixtures remain F-07 hand-placed literals and the
suite now carries a hermetic 2,312-B / 34-record live-shape test. One
non-blocking finding, owed a one-line correction by the implementer lane:
**L-1** - `frames.rs` `make_npc_record`'s comment cites live rec[00] "y 338",
but the capture's `w12 = 0x00027852` yields uiY = low 10 bits = **82**
(`evidence/live-fix/README.md`). No assertion depends on the value.

**Live acceptance (clone binary, live tunnels, wire-verified roles):**
(a) **PROVEN** - three consecutive green sessions pass `Stage::InWorld`
absorbing the live NPC-led broadcast (`char_unique` 11/12/13);
(b) **MEASURED** - the green run's c2s capture walks as 53-B login + 9-B
select + **3 x 5-B heartbeats on the wire** at +10/+20/+30 s (4 in the
prior run), before the +40 s graceful FIN;
(c) **PROVEN via the clean-close teardown** - `session ended
stage=TornDown worlds=1`, `session complete stage=TornDown`, **exit code
0** (`evidence/live-fix/drv_green_acceptance.log`). The literal Ctrl+C
*console event* is **undeliverable in this verification environment**: a
raw kernel32 handler on a confirmed shared console never fires under this
harness's pseudoconsole (ConPTY) - proven at the API level
(`evidence/live-fix/ctrl_c_investigation.md`, `probe_fire.log`), not
inferred. The CtrlC-specific wiring remains covered by the hermetic seam
tests; the teardown machinery itself is now live-proven on real sockets.

**Outcome: LIVE-01 RESOLVED. The Verifier authorizes the Custodian to merge
`fix/live-loginmany-npc` (`701963a`) into `main`,** with the (c) caveat
recorded above; the optional one-command interactive-console Ctrl+C check
is listed in the investigation note and does not block the merge.

