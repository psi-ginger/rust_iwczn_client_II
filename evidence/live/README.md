# evidence/live — live-server integration verification (Verifier, 2026-10-28)

Branch `step/6.2-harness` @ `57bd58fffb6680ea7b7c8596acd2a5859d2dc919`
(code = `febc38a5b895a0559475b945dac393aeb457113f`; HEAD is docs-only over it).
Live integration is **evidence, never a gate** (`CONTRIBUTING_PHASE2.md`):
nothing here changes any row's status; rows 6.1/6.2 stay `PASSED` on their
S4 clean-clone gate bundles (`evidence/6.1/`, `evidence/6.2/`).

The live backend was reached through two ngrok TCP tunnels. **Their labels
were swapped relative to the wire (finding D-1)**; every run below uses the
wire-verified roles: `GOONZU_LIVE_FRONT`/`GOONZU_FRONT_ADDR` =
`0.tcp.ap.ngrok.io:14195`, `GOONZU_LIVE_GAME`/`GOONZU_GAME_ADDR` =
`0.tcp.ap.ngrok.io:27911` (see `env.txt`).

Account credentials (`GOONZU_USER_ID`/`GOONZU_USER_PW`/`GOONZU_CHAR_SLOT`)
were supplied by the operator at run time and are **deliberately not
recorded anywhere in this bundle** — not in the logs (scanned), not in the
scripts (they read the operator's environment).

## File index

| File | What it is |
| :--- | :--- |
| `live_asdirected.log` | `cargo test --test live_integration -- --ignored` with the tunnels **as originally labelled**: 0/4 failed in 0.17s with `UnexpectedOpcode{cmd:18503}` (front dial hitting game) and `Codec(Integrity{size:10, cmd:30000})` (game dial hitting front) — the D-1 evidence. |
| `live_corrected.log` | Same suite with the corrected roles: **4/4 green in 70.12s** — front handshake (seeds `0x1b,0xd6`), game handshake (link cipher `0x12/0x34` confirmed against the real server, `char_unique` allocated, server version 1), WAN event loop (3 ticks / 0 frames), heartbeat soak **7/7 over 70.01s** at exact 10s cadence, never dropped. |
| `drv_live_canonical.log` | **Canonical LIVE-01 reproduction.** Full driver session via the env config (front handshake → notice → worldlist → front login → game login → 15×984-B roster → select → SERVERINFO 106 → opcode-5 PC record → `CharClient` → in-world), refused at the first in-world frame (opcode 6). The **process exit code (1)** is appended inside the file. |
| `drv_live_relayed.log` | Same session, game leg routed through the transparent capture relay below: reproduced identically (refusal is not an artifact of the direct path). |
| `decode.log` | Offline read-back of `cap/s2c.bin`, frame walk: **69/69 frames integrity-true** with the client's own `Cipher::link_cipher()`, consuming exactly 50,996/50,996 bytes (any wrong framing/cipher assumption breaks this). |
| `decode2.log` | Record-grade read-back of the refused opcode-6 frame: payload **2,312 B = exactly 34 records × 68 B**, all 34 heads `uiType=0` (`MAKECHARTYPE_NPC`), `uiMakeCharUnique` at `+4` stepping **monotonically 8492→8533** across all 34 records, `2312 % 592 = 536 ≠ 0`. This is the pin for `sizeof(cltMakeCharInfoCommon) = 68`. |
| `cap/s2c.bin`, `cap/c2s.bin` | Raw captured bytes of the relayed session: 50,996 B server→client, 62 B client→server. The decode logs consume these exactly. |
| `decode_tool.rs` | Source of the offline read-back (run as a temporary, deleted test — see its header for the re-run recipe). |
| `relay/Cargo.toml`, `relay/main.rs` | The transparent TCP relay used for the capture (std-only, forwards untouched while appending each direction to its own file). |
| `run_driver_live.ps1` | Sanitized reproduction runner for the canonical driver run (credentials read from the operator environment). |
| `run_capture_session.ps1` | Sanitized reproduction runner for the relayed capture session. |
| `env.txt`, `head.txt` | Toolchain/endpoint context and SHA binding, matching the `evidence/6.x/` convention. |
| `SHA256SUMS` | Checksums of every file in this directory. |

## Findings recorded from this run

- **D-1 (operational, resolved by correction):** the tunnel labels given for
  the run were swapped. `:14195` speaks FRONT (plaintext `usSize=10,
  cmd=30000` `FRONTMSG_RESPONSE_SECRETKEY`); `:27911` speaks GAME (ciphered
  13-byte `CLIENTCHARUNIQUE`). Evidence: `live_asdirected.log` (both failure
  signatures) vs `live_corrected.log` (4/4 with corrected roles).
- **LIVE-01 (defect in the port, opened 2026-10-28):** the live game server's
  first in-world frame (opcode 6, LOGINMANY) is an **NPC list**: 34 records
  of **68 bytes** (`sizeof(cltMakeCharInfoCommon)`, now pinned by the wire),
  every head `uiType = MAKECHARTYPE_NPC (0)`. The port strides opcode-6
  records by 592 (PC) and deliberately refuses non-PC records
  (`char_info.rs:655-692`, refusal at `:183`; posture at
  `dispatcher.rs:38-45`), so a live session **cannot complete** as delivered.
  Reproduced 4/4 (3 direct + 1 relayed). The hermetic suite was blind because
  its opcode-6 fixture is PC-only (`tests/e2e_loopback/frames.rs:729`).
  Full analysis and remediation scope: `docs/rca/2026-10-28-live-opcode6-npc-stride.md`.

## What is green (live)

Handshake layer (both legs), the `0x12/0x34` link-cipher deployment
assumption, WAN event loop, 70s heartbeat soak, the entire staged flow up to
and including `Stage::CharClient`, dispatcher absorption of a 50-frame
in-world burst of unknown opcodes, and WO-6.1's exit-code mapping (exit 1,
failing stage named).

## What is NOT proven (blocked by LIVE-01)

The in-world heartbeat (the session dies ~1.1s after entry; first tick would
be at +10s — the standalone soak covers the protocol) and the graceful
`Ctrl+C → Stage::TornDown` live teardown. Both need a session that survives
opcode 6.
