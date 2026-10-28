# evidence/live-fix - independent verification of the LIVE-01 fix (Verifier, 2026-10-28)

Delivery verified: `fix/live-loginmany-npc` @
`701963ac51376191adc4f72c78d6e1e308123f7a` (single commit, author
`goonzu-rs`; S1 satisfied — the verifying signer `psi-ginger` authored
none of the range `af4ab01..701963a`).

## S4 gates — clean clone straight from origin at the delivery SHA

Fresh `git clone git@github.com:psi-ginger/rust_iwczn_client_II.git`,
checkout `701963a`, fresh `target/`, five mandatory gates:

| Gate | Log | Result |
| :--- | :--- | :--- |
| `cargo check --all-targets` | `check.log` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | `clippy.log` | exit 0 |
| `cargo fmt --check` | `fmt.log` | exit 0 (0 bytes = 0 diffs) |
| `cargo test` | `test.log` | exit 0 — **211 lib + 9 e2e = 220/220** |
| `cargo doc --no-deps` | `doc.log` | exit 0 (0 warnings) |

`head.txt` / `env.txt`: SHA binding + toolchain. Clone porcelain empty.

## Adversarial review (no blocking findings)

- Walker: min-68 gate before any head read; per-record `remaining < stride`
  refusals naming offset/shortfall/demanded stride; `uiType` bit dispatch
  (592 PC / 68 NPC); `UnknownRecordType` kept loud for a widened field.
- No PC regression: `decode_record` = `decode_head` + `fill_pc_tail`, the
  exact prior field set; opcode-5 path length-checked before use; the driver
  consumes `DispatchOutcome::Entered` only through `debug!`, compatible.
- Dispatcher: NPC records counted-and-skipped with a falsifiability note
  (zero count ⇒ the stride dispatch mis-read the batch).
- Fixtures F-07-clean: hand-placed literals; hermetic 2,312-B / 34-record
  live-shape unit test; the e2e double now leads opcode 6 with an NPC record.
- **L-1 (non-blocking, correction owed by the implementer lane):**
  `tests/e2e_loopback/frames.rs` `make_npc_record` doc comment cites live
  rec[00] "y 338"; the capture (`evidence/live/decode2.log`, `w12` =
  `0x00027852`) gives uiY = low 10 bits = **82**. No assertion depends on it.
- **Self-correction (Verifier):** the earlier RCA/matrix decimal
  "8492->8533" was mis-divided; the log's hex `0x2134..0x2155` is
  **8500..8533**, as the delivery's docs state. Corrected in the RCA.

## Live acceptance (clone binary, live tunnels, wire-verified roles)

| Criterion | Result | Evidence |
| :--- | :--- | :--- |
| (a) pass `Stage::InWorld`, absorb the live NPC-led opcode-6 broadcast | **PROVEN** — 3 consecutive green sessions (`char_unique` 11/12/13); no refusal anywhere | `drv_green_acceptance.log`, `drv_green_ctrlc_attempt.log` |
| (b) survive >= 3 in-world heartbeat intervals | **MEASURED** — `cap/c2s.bin` walked on the wire: 53-B login + 9-B select + 3 x 5-B heartbeats at +10/+20/+30 s (4 in the prior run) | `cap/c2s.bin` (frame walk printed by the runner) |
| (c) graceful teardown to `Stage::TornDown`, exit code 0 | **PROVEN via the clean-close teardown** — `session complete stage=TornDown`, **exit 0**. The literal Ctrl+C *console event* is undeliverable in this ConPTY-hosted environment — proven at the kernel32 level, not inferred | `drv_green_acceptance.log`, `ctrl_c_investigation.md`, `probe_fire.log` |

`s2c.bin` (50,996 B) matches the pre-fix capture byte-count: the server's
world-entry broadcast is deterministic.

## Files

| File | What |
| :--- | :--- |
| `drv_green_acceptance.log` | THE green run: world entry, 40 s in-world, `stage=TornDown`, exit 0, embedded evidence lines |
| `drv_green_ctrlc_attempt.log` | The first acceptance attempt + the live Ctrl+C investigation (driver alive, link ESTABLISHED, no teardown) |
| `cap/c2s.bin`, `cap/s2c.bin` | Raw wire captures of the green run (the heartbeat frames are in c2s) |
| `probe_fire.log` | The kernel32 control experiment (raw handler never fires) |
| `relay/main.rs` | Capture relay, now with the graceful-FIN watchdog (`fin_after_secs`) |
| `relay/ctrlc_send.rs`, `relay/ctrl_probe.rs` | The Ctrl+C sender / raw-handler probe used in the investigation |
| `run_driver_acceptance.ps1` | Sanitized reproduction runner (credentials from operator env) |
| `check/clippy/fmt/test/doc.log`, `env.txt`, `head.txt` | The S4 gate bundle |
| `SHA256SUMS` | Checksums of every file here |
