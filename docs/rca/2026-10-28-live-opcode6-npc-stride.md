# Incident - live server refuses to let a session enter the world (opcode 6 is an NPC list)

| Field | Value |
| :--- | :--- |
| **Date** | 2026-10-28 |
| **Finding** | `LIVE-01` (opened by the Verifier's live-integration pass) |
| **Lane** | Implementer (fix lane `fix/live-loginmany-npc`) |
| **Status** | `OPEN` - live verification against the real server blocked; merge to `main` held |
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
  8492 -> 8533** across all 34 records - 34 consecutive ids can only land
  if 68 is the true record boundary.
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
