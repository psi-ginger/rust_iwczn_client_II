# Multi-Agent Task & Execution Matrix

> [!IMPORTANT]
> **READ BEFORE EXECUTING:**
> 1. Check the table below first. Find the **FIRST** step with status `PENDING`.
> 2. Implement **ONLY THAT STEP**. Do not touch or jump to future steps.
> 3. Read the exact scope and acceptance criteria in `CONTRIBUTING.md`.
> 4. Status updates and verification ticks are strictly audited and managed by the QC Lead.

---

## Progress Tracker (Phase II)

| Step | Target Milestone | Status | Verified Commit | Tests Baseline | Audited By / Notes |
| :---: | :--- | :---: | :---: | :---: | :--- |
| **1.1** | Cargo Scaffold & Minimal Workspace | `PASSED` | `d7e5fa1` | 1/1 | QC Lead (Aun) / Clean minimal tokio/bytes deps |
| **1.2** | GoonZu Wire Header (5-byte LE) | `PASSED` | `5ff9bd4` | 12/12 | QC Lead (Aun) / 5-byte LE wire truth verified; RCA filed |
| **1.3** | Cipher Primitives (Link & Session) | `PASSED` | `236229f` | 20/20 | QC Lead (Aun) / Bit-exact ITZ rolling cipher; full legacy vectors |
| **2.1** | TCP Session & Timeout Management | `PASSED` | `afcee87` | 30/30 | QC Lead (Aun) / Bounded dial/read timeouts; wire-faithful Nagle |
| **2.2** | Framed Packet Decoder | `PASSED` | `19cd996` | 47/47 | QC Lead (Aun) / Sticky poison guards; pull-time cipher |
| **2.3** | Framed Packet Encoder | `PASSED` | `1330a20` | 53/53 | QC Lead (Aun) / Outbound cipher framing; C++ vector match |
| **2.4.1** | Scan & Refactor MOD-01 | `PASSED` | `72ea2e0` | 53/53 | QC Lead (Aun) / Decoupled cipher/header tests & fixtures; tick() dedup |
| **2.4.2** | MOD-01 Continuation / Risk & RCA | `PASSED` | `e6bc650` | 53/53 | QC Lead (Aun) / Comprehensive RCA & R1-R10 residual risk posture filed |
| **2.5.1** | Scan & Refactor MOD-02 | `PASSED` | `1a0d2ff` | 53/53 | QC Lead (Aun) / Decoupled fixtures & tests; unified declared_at guard |
| **2.5.2** | MOD-02 Continuation / Risk & RCA | `PASSED` | `9704ed9` | 53/53 | QC Lead (Aun) / Comprehensive RCA & R1-R10 residual risk posture filed |
| **3.1** | Front Connect & SecretKey Handshake | `PASSED` | `e2ce7fd` | 63/63 | QC Lead (Aun) / Opcode 30000; session seeds payload[2]/[4] |
| **3.2** | WorldList & Notice Request | `PASSED` | `c61256e` | 76/76 | QC Lead (Aun) / Strict 428B WorldInfo; no trailing drops; F-01/F-02/F-04 folded |
| **3.3** | Front Login & Auth Key Acquisition | `PASSED` | `c85fd44` | 85/85 | QC Lead (Aun) / 4-byte auth_key captured; strict response parse; 85/85 tests |
| **4.1** | Game Connect with Immediate Link Cipher | `PASSED` | `abd0ab4` | 91/91 | QC Lead (Aun) / Port 4010; inbound link cipher [0x12, 0x34] from byte 0; F-02 idle timeout preserved |
| **4.2** | ClientCharUnique Handshake (Opcode 1) | `QC_REVIEW` | `c72ecdb` | 98/98 | Evidence re-audit 2026-10-26 (P-13/P-14): baseline corrected 100/100 → 98/98 (measured at c72ecdb, CAR-2026-10-25-42A1); F-42-01 seed log, F-42-02 doc links, F-42-04 pre-handshake decision closed @8b3dd51; F-42-03 (P-12) held open by QC → not PASSED. Strict 8-byte parse; session cipher keying & send gate; previous record retired as unreliable |
| **4.3** | Game Login Submission (Opcode 2) | `PASSED` | `92e0d18` | 106/106 | Evidence re-audit 2026-10-26: 106/106 at this commit, 130/130 current run; F-43-01 cite fixed and F-43-02 acceptance rewritten to the real `game: login ok for ...` line @8b3dd51. 48-byte cltLogInInfo; 53B session-ciphered frame; auth_key carried |
| **4.4** | Character List & Full MakeCharInfo | `PASSED` | `e089632` | 120/120 | Evidence re-audit 2026-10-26: 120/120 at this commit, 130/130 current run; F-44-01 doc link closed @8b3dd51 (cargo doc 0 warnings). 984B roster; 592B MakeCharInfo; field-exact offsets; refuse LZF |
| **5.1** | Non-Blocking Async Event Loop | `PASSED` | `8b3dd51` | 130/130 | Evidence re-audit 2026-10-26: P-01–P-05 resolved (links, drain parity, gamemode decision record, +3 falsifying tests, virtual-time flake removal); 130/130 incl. 4 high-load passes; cargo doc 0 warnings |
| **5.2** | In-Game Heartbeat Keepalive | `PENDING` | - | - | Acceptance: 10s ping ticker active 60s+ |
| **5.3** | Graceful Teardown & Signal Handling | `PENDING` | - | - | Acceptance: Clean socket close on SIGINT |

---
*Status Enum: `PENDING` | `IN_PROGRESS` | `QC_REVIEW` | `PASSED`*
(`QC_REVIEW` = implementation and evidence recorded, but a required finding is still open — such a row is **not** `PASSED`; the first `PENDING` row remains the next implementable step.)

> **2026-10-26 evidence re-audit (QC directives P-13, P-14):** rows 4.2–5.1 were re-verified from the current run — `cargo check` / `cargo clippy -- -D warnings` clean, **130/130** tests (including 4 passes under high CPU load), `cargo doc --no-deps` **0 warnings** — and the previous QC record for those rows was retired as unreliable. Only evidence measured at the stated commits (4.2: 98/98 at `c72ecdb`, CAR-2026-10-25-42A1; 4.3: 106/106; 4.4: 120/120) or in this run is recorded above. Findings state: P-01–P-11, P-13, P-14 resolved; **P-12 (F-42-03) held open by QC** — revisit in the 5.1 verification pass.
