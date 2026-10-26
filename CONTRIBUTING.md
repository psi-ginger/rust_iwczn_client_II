# GoonZu Client Rust (Phase II) - Precision Engineering Specification

## Core Philosophy: Zero-Evasion, Precision Micro-Steps
In Phase II, we abandon monolithic module delivery in favor of strictly bounded **Micro-Steps (e.g. 1.1 -> 1.2 -> 1.3)**.
Each step has an isolated scope, strict acceptance criteria, and zero tolerance for stubbing (`// not parsed in detail`), fake mocks, or unverified loops.

---

## Mandatory Gate Set (applies to every step)

Every step, and every QC follow-up, must pass **all five** gates before it can be marked `PASSED`. A gate that is not run, or that cannot run, is **not** a pass — record it as unverified instead.

| Gate | Command | Requirement |
| :--- | :--- | :--- |
| Compile | `cargo check --all-targets` | 0 errors, 0 warnings (`RUSTFLAGS=-D warnings`) |
| Lint | `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| Format | `cargo fmt --check` | 0 diffs (rustfmt 1.x, default style — no `rustfmt.toml` in this repo) |
| Tests | `cargo test` | 100% pass; baseline count recorded in `TASK_MATRIX.md` |
| Docs | `cargo doc --no-deps` | 0 warnings, 0 errors |

> **rustfmt (added 2026-10-26, QC):** the format gate was previously absent — `rustfmt` was not
> installed and no `rustfmt.toml` existed, so no formatting result has ever been recorded for this
> repo. It is now a required gate. Install with `rustup component add rustfmt`; apply with
> `cargo fmt`; verify with `cargo fmt --check`. Style is stock rustfmt defaults — do **not**
> introduce a `rustfmt.toml` to reformat history, and never let `cargo fmt` alter logic (it is
> whitespace/collapse only; review `git diff` after every `cargo fmt`).

### Live integration (opt-in, not part of the hermetic gate)

`tests/live_integration.rs` runs the real handshakes against a **reachable** server. It exists
because the 131 loopback tests are circular for one assumption: they encode with this crate's own
`Cipher`, so they cannot detect a mismatch between the client's hardcoded link key
(`0x12, 0x34`) and the game service's *configured* link key (`cfg.link_key`, SEC-014,
`services/game/src/main.rs:680`). Only a live handshake proves that pairing.

The three tests are `#[ignore]`d so `cargo test` stays network-free. Run them when a server is
reachable:

```
GOONZU_LIVE_FRONT=host:frontport GOONZU_LIVE_GAME=host:gameport \
    cargo test --test live_integration -- --ignored --nocapture
```

Endpoints come from the environment (tunnel addresses are per-session — never hardcode them).
A live pass is **evidence**, not a gate: it is never required for a step to be `PASSED`, and a
step must not be blocked on a server being up.

---

## The WBS Micro-Milestone Roadmap

### [MOD-01: Foundation & Binary Primitives]
* **Step 1.1: Cargo Scaffold & Minimal Workspace**
  - Path: `Cargo.toml`, `src/lib.rs`, `src/main.rs`
  - Scope: Minimal, high-performance dependencies (`tokio` with rt/net/macros/time, `bytes`, `tracing`, `tracing-subscriber`, `thiserror`). No bloated frameworks.
  - Acceptance: `cargo check` and `cargo test` pass cleanly.
* **Step 1.2: GoonZu Wire Header (5-Byte)**
  - Path: `src/protocol/header.rs`
  - Scope: Parse and serialize the standard 5-byte wire header (`usSize: u16` total frame length, `ucChk: u8` reserved byte, `usCmd: u16` opcode), little-endian, per `GoonZu_Source/NetworkLib/ITZNetLib/itzbase.h:63-68` and the server wire truth (`rust_Iwcgz_server/crates/gz-net`: `HEADER_SIZE = 5`). There is **no checksum**: `ucChk` is never computed or validated by any legacy producer/consumer; treat it as reserved (the cipher reuses it as a resync tag on encrypted links). Enforce the legacy recv guards: reject declared size `< 2` and `> PACKET_MAX_SIZE - 8` (64992), and reject `2..=4` as too small to hold a header (BUG-013, parity with server `gz-net`). Opcode bit 15 (`0x8000`) is the LZF compression flag, owned by the decompressing consumer, exposed via a helper.
  - Acceptance: Unit tests asserting bit-exact little-endian header byte layout (encode + decode round-trip against `sPacketHeader` sample arrays) and the full size-guard table.
  - Note: The original spec of this step (8-byte header with `checksum: u16` + `param: u16`, "exact C++ checksum algorithm") did not match the legacy or server wire truth — no such header or algorithm exists. See `docs/rca/2026-10-23-step-1.2-header-spec-mismatch.md`.
* **Step 1.3: GoonZu Cipher Primitives (Link & Session)**
  - Path: `src/net/cipher.rs`
  - Scope: Implement `Cipher` table and byte-level rolling cipher:
    - Fixed Link Cipher (`[0x12, 0x34]`) used for initial Game Server ingress.
    - Dynamic Session Cipher initialized from seed bytes `(key1, key2)`.
  - Acceptance: Reversible encryption/decryption unit test matching server `goonzu-rs` test vectors.

---

### [MOD-02: Framed Transport & Session Layer]
* **Step 2.1: TCP Session & Timeout Management**
  - Path: `src/net/session.rs`
  - Scope: Asynchronous TCP socket wrapper with bounded connect and read timeouts (default 5000ms).
  - Acceptance: Test attempting connection to an open/closed local port handling timeouts gracefully without blocking.
* **Step 2.2: Framed Packet Decoder (Stream -> Packets)**
  - Path: `src/net/codec.rs`
  - Scope: Read streaming bytes into a ring/bytes buffer, extract 5-byte headers, validate declared size against the guard table, apply inbound cipher (if active), and emit full packet frames.
  - Acceptance: Unit test decoding fragmented streams (split packets) and multi-frame bursts.
* **Step 2.3: Framed Packet Encoder (Packets -> Wire)**
  - Path: `src/net/codec.rs`
  - Scope: Take opcode and payload; prepend the 5-byte header (`usSize`, reserved byte, `usCmd`); apply outbound cipher (if active); flush to socket.
  - Acceptance: Unit test asserting exact wire byte output against pre-recorded C++ packet captures.
* **Step 2.4.1: Scan & Refactor MOD-01**
  - Scope: Scan all files in MOD-01 (`src/protocol/`, `src/net/cipher.rs`) for excessive size, mixed responsibilities, or lack of OOP modularity. Refactor cleanly.
  - Acceptance: 53/53 tests pass, `cargo clippy --all-targets -- -D warnings` clean.
* **Step 2.4.2: MOD-01 Continuation / RCA & Risk Management**
  - Scope: If Step 2.4.1 has remaining refactoring, finish it. If complete, produce an RCA & Risk Management document covering MOD-01 modularity, invariants, and residual risks.
  - Acceptance: All tests green, clean audit report.
* **Step 2.5.1: Scan & Refactor MOD-02**
  - Scope: Scan all files in MOD-02 (`src/net/session.rs`, `src/net/codec.rs`) for excessive size, mixed responsibilities, or lack of OOP modularity. Refactor cleanly.
  - Acceptance: 53/53 tests pass, `cargo clippy --all-targets -- -D warnings` clean.
* **Step 2.5.2: MOD-02 Continuation / RCA & Risk Management**
  - Scope: If Step 2.5.1 has remaining refactoring, finish it. If complete, produce an RCA & Risk Management document covering MOD-02 transport/codec separation and residual risks.
  - Acceptance: All tests green, clean audit report.

---

### [MOD-03: Front Server Flow (Port 5700)]
* **Step 3.1: Front Server Connection & SecretKey Handshake**
  - Path: `src/auth/front.rs`
  - Scope: Connect to Front Server (5700). Receive Opcode 30000 (`ResponseSecretkey`). Extract `(payload[2], payload[4])` seed. Set outbound session cipher. Note: Inbound from Front Server is **Plaintext**.
  - Acceptance: Live test against `goonzu-rs` Front Server establishing ciphered outbound channel.
* **Step 3.2: WorldList & Notice Request**
  - Path: `src/auth/world_list.rs`
  - Scope: Send Opcode 5 (`RequestWorldlist`). Receive and fully parse `WorldInfo` frames without dropping trailing bytes.
  - Acceptance: Wire-exact parsing of server name, status, and IP/port fields verified against server response.
* **Step 3.3: Front Login & Auth Key Acquisition**
  - Path: `src/auth/login.rs`
  - Scope: Send front authentication request. Receive response containing the 4-byte `auth_key`. Extract and store `auth_key` in client state.
  - Acceptance: Successful front handshake obtaining valid `auth_key` verified by server logs.

---

### [MOD-04: Game Server Flow (Port 4010)]
* **Step 4.1: Game Server Connect with Immediate Link Cipher**
  - Path: `src/game/entry.rs`
  - Scope: Connect to Game Server (4010). Immediately initialize inbound cipher with fixed Link Cipher `[0x12, 0x34]`.
  - Acceptance: Inbound packet readable without cipher corruption error.
* **Step 4.2: ClientCharUnique Handshake (Opcode 1)**
  - Path: `src/game/entry.rs`
  - Scope: Receive Opcode 1 (`ResponseClientcharunique`). Parse unique ID and session seeds `payload[4], payload[5]`. Initialize outbound session cipher.
  - Acceptance: Client logs displaying correct session seeds matching Game Server console logs.
  - Seed log (QC P-08): every successful `GameLink::handshake` (`src/game/entry.rs`, the real
    success path — not startup code) emits at INFO via `tracing`:
    `client handshake succeeded: session_seed = (0xRR, 0xRR), char_unique = <id>` (once per
    connection; idempotent re-entry does not re-log). Purpose: connection verification and
    troubleshooting — `(0xRR, 0xRR)` is the `key1, key2` pair printed exactly as the server
    console prints it (`game: <peer>: charunique=<id> -> (0xRR, 0xRR)`, timestamp-prefixed by
    `logln!`, `services/game/src/main.rs`), so any session can be traced back to its server-side
    keying line in later QC runs.
* **Step 4.3: Game Login Submission (Opcode 2)**
  - Path: `src/game/login.rs`
  - Scope: Send Opcode 2 (`RequestLogin`, 48 bytes) enciphered with session cipher, carrying account name and 4-byte `auth_key` from Step 3.3.
  - Acceptance: Server logs confirm `game: login ok for "<user>" (account <id>, auth_key <key>): <n> character(s)` (timestamp-prefixed by `logln!`, `services/game/src/auth.rs`) with no `game login for unknown account "<user>"` rejection. This is the actual success line; the client tracks the same literal in `src/game/login.rs` module docs (`game: login ok for ... (account ..., auth_key ...)`).
* **Step 4.4: Character List & Detail Parsing (592-byte MakeCharInfo)**
  - Path: `src/game/char_info.rs`
  - Scope: Parse character list frames and full `MakeCharInfo` binary layout. Every single field (name, level, job, hp, mp, coordinates) mapped to exact byte offsets. No stub comments allowed.
  - Acceptance: Exact wire struct validation with byte-offset unit test.

---

### [MOD-05: Non-Blocking Event Loop & Heartbeat]
* **Step 5.1: Non-Blocking Async Event Loop**
  - Path: `src/client/event_loop.rs`
  - Scope: Event loop using `tokio::select!` with a strict 100ms read timeout (`tokio::time::timeout`). Prevents blocking on idle sockets.
  - Acceptance: Loop processes incoming frames while continuing background tickers without latency.
* **Step 5.2: In-Game Heartbeat / Keepalive**
  - Path: `src/client/heartbeat.rs`
  - Scope: 10-second recurring ticker firing keepalive pings over the active game session.
  - Acceptance: A live game connection stays up for 60s+ under the heartbeat without the server dropping it; the frame is header-only opcode 100 (`usSize` 5, no payload), session-ciphered, uncompressed, byte-exact to the legacy `cltMsg(GAMEMSG_REQUEST_HEARTBEAT, 0, NULL)`. Proven by the live soak in `tests/live_integration.rs` (7 ticks / 70s, connection never dropped) plus the byte-exact wire test.
    > *Rewritten 2026-10-27 (QC, as in F-43-02): the original acceptance asked that "server logs continuously record heartbeat/ping packets." That clause cannot hold — the game service **deliberately mutes** opcode 100's recv log (`services/game/src/main.rs:1106-1112`: "muted, they dominated the recv log for no signal"). The connection-survival observable is the real criterion; the muted log is a server-side signal-to-noise decision, not a client defect.*
* **Step 5.3: Graceful Teardown & Signal Handling**
  - Path: `src/client/teardown.rs`
  - Scope: Clean disconnection handling on `Ctrl+C` or server disconnect signal. Closes sockets cleanly without leaking tasks.
  - Acceptance: Client terminates cleanly with zero orphaned tokio background tasks.

---

## Execution Invariants for AI Agents
1. **Never jump steps:** Do not implement Step 1.3 until Step 1.2 is committed and verified.
2. **Never hide failures:** If a wire offset does not match, log the raw hex diff. Do not fabricate dummy values or mask errors.
3. **Never stub wire parsers:** Every packet parser must validate full payload lengths.
