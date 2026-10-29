# evidence/6.3/verify2 - second independent Verifier pass (2026-10-29)

This is the evidence bundle of a SECOND, fully independent Verifier audit of
Step 6.3. It sits beside (never inside) the primary `evidence/6.3/` bundle
filed by the parallel Verifier window in `2b8761e`; no file of that bundle
was touched. Provenance of the two passes is disjoint: this pass cloned
origin at the delivery SHA **`3285d07`** (the code commit) and ran its whole
audit before `2b8761e` appeared on the branch mid-run; the primary pass
cloned at the tip **`a5151ca`** (code + delivery note). The two commits
differ only by `docs/agents/WO-6.3.md` +55, so the audited code is identical.

## Contents

| file | what it is |
| :--- | :--- |
| `head.txt` / `head-status.txt` / `head-describe.txt` | clone identity: fresh `git clone` of origin, checkout `3285d07`, no `target/` at start, `git status --porcelain` EMPTY (captured before this evidence dir existed) |
| `check.log` `clippy.log` `fmt.log` `test.log` `doc.log` | the five gates, cold, from that clone - all exit 0; `fmt.log` 0 bytes |
| `env.txt` | toolchain + OS + tunnel state + soak run stamp |
| `soak-measurements.log` | hermetic soak re-run with `--nocapture` (the `soak measurements:` line) |
| `falsification-probe.txt` | mutation probe: the gate is load-bearing (see below) |
| `live-soak-driver.log` | driver log of the >=120 s live soak (2026-10-29) |
| `live-soak-wire-walk.txt` | acceptance results + c2s/s2c frame walks of the soak captures |
| `run_live_soak.ps1` | the soak runner (sanitized; credentials read from operator env, never stored) |
| `cap/c2s.bin` `cap/s2c.bin` | raw wire captures of the live soak (game leg via the capture relay) |
| `SHA256SUMS` | hashes of the exact committed (LF) bytes; verify with `sha256sum -c --ignore-missing` or `Get-FileHash` |

## Findings of this pass

1. **S4 reproduced independently:** clone at `3285d07`, fresh `target/`,
   all five gates exit 0, **225/225** (211 lib + 14 e2e; 4 live ignored).
   Every number in the matrix cell was copied from this bundle's `test.log`.
2. **Falsification probe (mutation run):** in the clone only, the 34-record
   NPC loop inside `loginmany_npc_led_body()` was deleted (PC-only batch,
   the 6.2 hermetic blind spot); call sites/imports kept so the suite still
   compiled. The soak then FAILED at its own assertion `soak.rs:223`
   ("expected >= 904 batches, counted 0", exit 101). The LIVE-01 regression
   gate is load-bearing, not decorative. Clone restored byte-identical
   afterwards (`git diff --stat -- tests/` empty).
3. **Live soak (2026-10-29, separate session from the primary pass):**
   S4-clone binary vs the wire-verified tunnels via the capture relay,
   FIN watchdog +135 s: **in-world 134.93 s >= 2 min**, **13 wire
   heartbeats** on the c2s capture (mean interval 10.38 s vs the server's
   10 s cadence), no partial frame in either direction (zero frame drops),
   `stage=TornDown worlds=1 exit 0`. The s2c capture additionally shows the
   trailing **2317-B frame = 34x68 + 5 header - the live NPC-led opcode-6
   broadcast absorbed without refusal** (LIVE-01 regression shape on the
   wire, live, at scale).
4. **Finding L-2 (non-blocking, evidence-hygiene): the primary bundle's
   `SHA256SUMS` does not verify against the committed bytes.** 5 of 14
   entries fail a fresh-clone check (`head.txt`, `env.txt`, `gates.txt`,
   `live_soak.txt`, `soak63_live.log`). Root cause diagnosed byte-level:
   for the first four the recorded hash is the hash of the **CRLF** bytes -
   the manifest was generated in a working tree whose files were CRLF
   (autocrlf) and git committed them LF-normalized; re-hashing the CRLF
   transform reproduces the recorded values exactly. `soak63_live.log`
   matches no EOL variant (LF, CRLF, either without trailing newline), i.e.
   its content diverged from what was hashed. Content of all five is
   genuine and consistent with the verifying files; the gate logs, caps and
   runner all verify directly. Disposition: non-blocking (evidence hygiene,
   not a gate result); the Custodian may regenerate the manifest over the
   committed bytes forward-only, per S7 no committed file is rewritten here.

This bundle's own `SHA256SUMS` is generated over the exact bytes committed
(LF, no normalization ambiguity), so it verifies from any fresh clone.
