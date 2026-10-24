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
| **4.1** | Game Connect with Immediate Link Cipher | `PENDING` | - | Target: ≥85 | Port 4010 with `[0x12, 0x34]` link cipher |
| **4.2** | ClientCharUnique Handshake (Opcode 1) | `PENDING` | - | Session cipher key initialization |
| **4.3** | Game Login Submission (Opcode 2) | `PENDING` | - | Valid auth_key from 3.3; verified in server log |
| **4.4** | Character List & Full MakeCharInfo | `PENDING` | - | 592-byte exact wire parse; zero stubs |
| **5.1** | Non-Blocking Async Event Loop | `PENDING` | - | 100ms timeout select; no idle socket stalls |
| **5.2** | In-Game Heartbeat Keepalive | `PENDING` | - | 10s ping ticker active 60s+ |
| **5.3** | Graceful Teardown & Signal Handling | `PENDING` | - | Clean socket close on SIGINT |

---
*Status Enum: `PENDING` | `IN_PROGRESS` | `QC_REVIEW` | `PASSED`*
