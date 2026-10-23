# GoonZu Client Rust (Phase II) - Precision Engineering Specification

## Core Philosophy: Zero-Evasion, Precision Micro-Steps
In Phase II, we abandon monolithic module delivery in favor of strictly bounded **Micro-Steps (e.g. 1.1 -> 1.2 -> 1.3)**.
Each step has an isolated scope, strict acceptance criteria, and zero tolerance for stubbing (`// not parsed in detail`), fake mocks, or unverified loops.

---

## The WBS Micro-Milestone Roadmap

### [MOD-01: Foundation & Binary Primitives]
* **Step 1.1: Cargo Scaffold & Minimal Workspace**
  - Path: `Cargo.toml`, `src/lib.rs`, `src/main.rs`
  - Scope: Minimal, high-performance dependencies (`tokio` with rt/net/macros/time, `bytes`, `tracing`, `tracing-subscriber`, `thiserror`). No bloated frameworks.
  - Acceptance: `cargo check` and `cargo test` pass cleanly.
* **Step 1.2: GoonZu Wire Header & Checksum**
  - Path: `src/protocol/header.rs`
  - Scope: Parse and serialize the standard 8-byte wire header (`size: u16`, `checksum: u16`, `opcode: u16`, `param: u16`). Implement the exact C++ checksum algorithm.
  - Acceptance: Unit test asserting bit-exact checksum against C++ sample byte arrays.
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
  - Scope: Read streaming bytes into a ring/bytes buffer, extract 8-byte headers, verify size and checksum, apply inbound cipher (if active), and emit full packet frames.
  - Acceptance: Unit test decoding fragmented streams (split packets) and multi-frame bursts.
* **Step 2.3: Framed Packet Encoder (Packets -> Wire)**
  - Path: `src/net/codec.rs`
  - Scope: Take opcode, param, and payload; prepend 8-byte header; compute checksum; apply outbound cipher (if active); flush to socket.
  - Acceptance: Unit test asserting exact wire byte output against pre-recorded C++ packet captures.

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
* **Step 4.3: Game Login Submission (Opcode 2)**
  - Path: `src/game/login.rs`
  - Scope: Send Opcode 2 (`RequestLogin`, 48 bytes) enciphered with session cipher, carrying account name and 4-byte `auth_key` from Step 3.3.
  - Acceptance: Server logs confirm: `game: 127.0.0.1:xxxx: game login for account "..."` without "unknown account" rejection.
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
  - Acceptance: Server logs continuously record heartbeat/ping packets over 60 seconds without dropping connection.
* **Step 5.3: Graceful Teardown & Signal Handling**
  - Path: `src/client/teardown.rs`
  - Scope: Clean disconnection handling on `Ctrl+C` or server disconnect signal. Closes sockets cleanly without leaking tasks.
  - Acceptance: Client terminates cleanly with zero orphaned tokio background tasks.

---

## Execution Invariants for AI Agents
1. **Never jump steps:** Do not implement Step 1.3 until Step 1.2 is committed and verified.
2. **Never hide failures:** If a wire offset does not match, log the raw hex diff. Do not fabricate dummy values or mask errors.
3. **Never stub wire parsers:** Every packet parser must validate full payload lengths.
