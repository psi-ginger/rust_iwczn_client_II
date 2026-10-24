# Multi-Agent Task & Execution Matrix

> [!IMPORTANT]
> **READ BEFORE EXECUTING:**
> 1. Check the table below first. Find the **FIRST** step with status `PENDING`.
> 2. Implement **ONLY THAT STEP**. Do not touch or jump to future steps.
> 3. Read the exact scope and acceptance criteria in `CONTRIBUTING.md`.
> 4. Status updates and verification ticks are strictly audited and managed by the QC Lead.

---

## Progress Tracker (Phase II)

| Step | Target Milestone | Status | Verified Commit | QC Gate / Notes |
| :---: | :--- | :---: | :---: | :--- |
| **1.1** | Cargo Scaffold & Minimal Workspace | `PASSED` | `d7e5fa1` | Clean minimal tokio/bytes deps; verified by QC |
| **1.2** | GoonZu Wire Header (5-byte LE) | `PASSED` | `5ff9bd4` | 5-byte LE wire truth verified; 12/12 tests pass; RCA filed |
| **1.3** | Cipher Primitives (Link & Session) | `PASSED` | `236229f` | Bit-exact ITZ rolling cipher; full legacy vectors match; 20/20 tests |
| **2.1** | TCP Session & Timeout Management | `PASSED` | `afcee87` | Bounded dial/read timeouts; wire-faithful Nagle; 30/30 tests |
| **2.2** | Framed Packet Decoder | `PASSED` | `19cd996` | Sticky poison guards; pull-time cipher; 47/47 tests pass |
| **2.3** | Framed Packet Encoder | `PASSED` | `1330a20` | Outbound cipher framing; C++ vector match; 53/53 tests pass |
| **3.1** | Front Connect & SecretKey Handshake | `PENDING` | - | Opcode 30000; session seed extraction |
| **3.2** | WorldList & Notice Request | `PENDING` | - | Full WorldInfo parse without dropping bytes |
| **3.3** | Front Login & Auth Key Acquisition | `PENDING` | - | 4-byte auth_key captured in state |
| **4.1** | Game Connect with Immediate Link Cipher | `PENDING` | - | Port 4010 with `[0x12, 0x34]` link cipher |
| **4.2** | ClientCharUnique Handshake (Opcode 1) | `PENDING` | - | Session cipher key initialization |
| **4.3** | Game Login Submission (Opcode 2) | `PENDING` | - | Valid auth_key from 3.3; verified in server log |
| **4.4** | Character List & Full MakeCharInfo | `PENDING` | - | 592-byte exact wire parse; zero stubs |
| **5.1** | Non-Blocking Async Event Loop | `PENDING` | - | 100ms timeout select; no idle socket stalls |
| **5.2** | In-Game Heartbeat Keepalive | `PENDING` | - | 10s ping ticker active 60s+ |
| **5.3** | Graceful Teardown & Signal Handling | `PENDING` | - | Clean socket close on SIGINT |

---
*Status Enum: `PENDING` | `IN_PROGRESS` | `QC_REVIEW` | `PASSED`*
