# RETROACTIVE CAPA GAP ANALYSIS — MOD-01 & MOD-02
**Document Control Reference:** CAR-2026-10-24-RETRO01 (retrospective audit, extends the CAR-2026-10-24 lineage)
**Quality Standards:** ISO 9001:2015 Clauses 6.1 (Risk), 8.5.2 (Identification & Traceability), 8.5.6 (Control of Changes), 8.6 (Release of Products), 8.7 (Nonconforming Outputs), 9.1 (Observability), 10.2 (Corrective Action)
**Audit Standard:** Haru/CAPA — no speculative assumptions, no unverified mocks, no premature mutations; every claim below is grounded in a file that was actually read, at the line cited, during this audit.
**Date:** 2026-10-24
**Target:** `C:\Data\06_Client_Goonzu\rust_iwczn_client_II` @ baseline `a409121` (clean tree, branch `main`)
**Scope:** `protocol::header` (Step 1.2), `net::cipher` (Step 1.3), `net::session` (Step 2.1), `net::codec` (Steps 2.2/2.3) — sources, tests, and fixtures.
**Out of scope:** `auth::front` (Step 3.1 / MOD-03), `TASK_MATRIX.md` (QC Lead's — untouched).

---

## 1. Executive Summary & Verdict

**Verdict: PASS with observations. No wire-logic defect, no test-mock bias, no premature state
mutation, and no silent truncation was found in any MOD-01/MOD-02 module.** All ten findings are
operational-risk, documentation, or test-infrastructure grade; none requires a code change before
Step 3.2, and none was changed by this audit (Directive: *do not rewrite working code blindly*).

The suite is green on this baseline: **`cargo test` → 63 passed / 0 failed** (52 in MOD-01/02:
header 11, cipher 8, session 10, codec 23; plus 10 `auth::front` from Step 3.1 and 1 root). The
directive's "47/47" figure is a stale count from the Step 2.5 baseline and has been superseded —
recorded as finding **F-06** for traceability, not a regression.

The Haru lesson ("verification must be grounded in physical wire truth") was applied literally:
every parity claim in the four modules' documentation was re-verified this audit by reading the
**actual legacy C++ sources** in `GoonZu_Source` and the **actual server-rust sources** in
`rust_Iwcgz_server` (local clone `C:\Data\04_Server_Goonzu\goonzu-rs`, remote confirmed
`git@github.com:psi-ginger/rust_Iwcgz_server.git`). Section 7 is the verification ledger.

---

## 2. Audit Method — the Evidence Chain

The client port's ground truth is not self-referential. The chain, each link verified this audit:

```
Unmodified legacy C++ (GoonZu_Source)
  │  compiled & driven by reference harnesses
  ▼
rust_Iwcgz_server (goonzu-rs)
  crates/gz-cipher/reference/{harness.cpp, ground_truth.json, itzencrypt_ref.py}
  crates/gz-net/reference/{harness.cpp, emit_vectors.py}
  │  recorded vectors, copied byte-for-byte
  ▼
rust_iwczn_client_II fixtures (src/net/{cipher,codec}/fixtures.rs)
  │  replayed by tests against the port
  ▼
Port implementation (cipher.rs / codec.rs)
```

- `cipher/fixtures.rs` claims provenance from `gz-cipher/tests/data/vectors.rs`; both tables were
  compared this audit — same 26 cases (A1–E7), same names, same order.
- `codec/fixtures.rs` claims provenance from `gz-net/tests/data/vectors.rs`; the framing cases and
  the `DIVERGENT` list (`declared_2/3/4` SubHeader + `ring_smaller_than_frame` RingCapacity,
  `gz-net/tests/data/vectors.rs:191-202`) are identical to the client's copy.
- No fixture was ever derived from the Rust implementation under test, and anti-shrink assertions
  (`checked >= 39`, `legacy_send::CASES.len() == 11`, `skipped == 3`, `CASES.len() >= 15`) pin the
  tables against silent erosion.

---

## 3. Findings Register

| ID | Finding | Class | Severity | Code impact |
|---|---|---|---|---|
| F-01 | Two session tests are non-hermetic (depend on host network behaviour, not on the code) | Test infra | Low | none |
| F-02 | 5 s `DEFAULT_READ_TIMEOUT` is port-added; fatal handling on a quiet link would diverge from the legacy | Documented divergence / operational | Medium | none now; policy decision for Steps 4+ |
| F-03 | `Decoder::feed` has no high-water cap; unbounded growth if the caller never drains | Documented divergence / FMEA | Medium | none now; candidate guard for a future step |
| F-04 | Legacy sends `FRONTMSG_REQUEST_PINGTEST` **raw** on a keyed link; port `Encoder` has no raw-send path | Forward-looking parity gap | Low | none today (no call site) |
| F-05 | Doc citation drift: ring-size numbers and one `LogIn.cpp` line number | Documentation | Low | none |
| F-06 | "47/47" test baseline in directives is stale; actual is 63/63 | Traceability | Info | none |
| F-07 | Mock-bias review: **no** mirror-the-implementation tests found (positive assurance) | Verification | — | none |
| F-08 | Silent-truncation review: **no** silent truncation found; every truncation is parity-pinned | Verification | — | none |
| F-09 | Premature-mutation review: **no** premature state transition found; all pinned by tests | Verification | — | none |
| F-10 | Legacy-parity ledger: all wire citations re-verified against source (Section 7) | Verification | — | none |

---

## 4. Focus Area 1 — Mock Bias & Non-Falsifiable Tests

**Assessment: clean.** The MOD-01/02 suites test physical boundary constraints, not implementation
mirrors. Evidence, verified by reading each test:

1. **Recorded-vector replay, not self-fulfilling oracles.** `every_vector_matches_the_legacy`
   (`cipher/tests.rs:23`) replays 39 ops recorded from g++-compiled unmodified `ITZEncrypt.cpp`;
   `decoder_agrees_with_the_compiled_legacy_on_every_recorded_read` replays the `ITZRecvBuffer`
   captures chunk-for-chunk; `encoder_reproduces_every_recorded_cxx_capture_byte_for_byte`
   (`codec/tests.rs:767`) replays 11 C++ send captures through the full encoder. A mismatch in any
   of these indicts the port, never the fixture (`fixtures.rs:5-7` documents the rule).
2. **Expectation sources are independent of the unit under test.** `plaintext_frame()` is built from
   `WireHeader` (a separately vector-verified module), and `send_flushes_exact_wire_bytes_over_the_session`
   (`codec/tests.rs:1023`) builds its wire expectation from `Cipher::encode` directly — the encoder
   under test is compared against the composition of two primitives that are each pinned on their own.
3. **Real boundaries, not round numbers.** Guard edges 0/1/2/3/4/5/64992/64993/65535 are all driven
   (`codec/tests.rs:370-447`); the 64992-byte frame is completed end-to-end (`:336-367`); key-tick
   counts are asserted per pull (`:582-635`); refusal stickiness survives `clear()` (`:475-505`).
4. **The fixtures verify themselves.** `fixture_is_intact_and_divergences_are_pinned`
   (`codec/tests.rs:152`) checks every recorded packet's length against its own `usSize`, pins the
   divergence list by exact equality, and asserts a refused read never handed out a frame.
5. **Physical-time session tests.** The timeout tests wait on real elapsed time with real sockets
   (`session/tests.rs:47-113`), asserting both the lower bound (the window actually bounded the
   wait) and the upper bound (it cut the wait short) — the CAPA "Architectural Invariant 3" pattern.

**F-01 (Low) — two non-hermetic tests.** They are falsifiable (they can fail loudly), but their
outcome depends on the host environment rather than the code:
- `connect_times_out_on_a_black_holed_address` (`session/tests.rs:47`) dials `192.0.2.1` (RFC 5737
  TEST-NET-1) and expects a SYN black hole. On a network that answers — ICMP unreachable, RST,
  captive portal, corporate proxy — the error surfaces as `SessionError::Io` and the test fails
  even though the code is correct. The comment pins this to "the dev host"; that is exactly the
  assumption the Haru standard flags.
- `resolve_maps_localhost_to_loopback` (`session/tests.rs:194`) depends on the system resolver
  mapping `localhost` to loopback (near-universal, but not guaranteed — e.g. a hosts file pointing
  `localhost` elsewhere).

*Recommendation:* keep both on dev hosts; gate with `#[ignore = "env: needs black-hole routing"]`
(or an env-var opt-in) before any CI/parallel-agent migration. No behaviour change.

---

## 5. Focus Area 2 — Silent Drops & Truncations

**Assessment: clean.** Every place a byte could vanish was checked against the legacy, and each is
either exact or an explicitly pinned, documented parity decision:

- `Cipher::encode` reads exactly `usSize` bytes and ignores trailing input — byte-identical to the
  legacy (`ITZEncrypt.cpp:36,49-55` reads `pSrc` only up to `pSrc + usSize`); pinned by recorded
  case `C8_trailing_byte_ignored`.
- The `size <= 3` passthrough branches copy exactly `size` bytes (`ITZEncrypt.cpp:38-42,79-83`;
  port `cipher.rs:211-213,272-275`) — reproduced, not approximated; pinned by cases A1–A5/E4/E5.
- `Cipher::decode` never writes `dest[2]` (first legacy write is `dest[3]`, `ITZEncrypt.cpp:98-100`).
  The port initialises the destination to zeros, so the decoded reserved byte is a **defined 0**
  rather than the legacy's stale-buffer garbage (BUG-012) — documented (`cipher.rs:52-54`,
  `codec.rs:129-131`) and pinned by the SENTINEL-filled expectations in `cipher/tests.rs`.
- The encoder rebuilds `ucChk` as 0 where a legacy capture carried a nonzero value — safe because
  `Encode` never reads `src[2]` (its loop starts at offset 3, `ITZEncrypt.cpp:55`, and offset 2 is
  *computed* at `:44`); `encoder_rebuilds_every_recorded_plaintext_packet` still demands byte
  equality of everything else (`codec/tests.rs:851-866`).
- `Decoder::next_frame` cuts exactly `declared` bytes (`split_to`), never a byte more or less;
  `Session::read` returns the true count; `read_exact` fails loudly (`UnexpectedEof` → `Io`) rather
  than short-filling.
- A refused `Completion` commits **no** packet count in the legacy (`ITZRecvBuffer.cpp:163-180`
  returns before `m_siNumPacket += numPacket` at `:219`), so good frames queued behind a bad header
  die with the connection. The port reproduces this (poisoned decoder never hands out the queued
  frame) — pinned by the recorded `good_then_over_limit` case and spelled out in
  `a_good_frame_ahead_of_a_bad_header_is_never_delivered` (`codec/tests.rs:449`).

**F-03 (Medium) — unbounded decoder growth.** The legacy's ring was a *second* ceiling: once full,
`GetRecvParam` wrapped and refused (`ITZRecvBuffer.cpp:78-89`), so a stalled consumer ended in a
clean refusal. The deque has no such ceiling; `Decoder::feed` grows without limit if the caller
drains nothing. This is already documented in `codec.rs:117-119` — the finding elevates it from
"known" to "carried in the risk register": a hostile or buggy peer sending many legal-size frames
faster than the consumer pulls causes memory exhaustion instead of a clean disconnect.
*Recommendation:* at the state-box step, add a configurable high-water mark (e.g. N ×
`MAX_FRAME_SIZE`) that converts overflow into a sticky refusal — the legacy posture. Behaviour
change → requires QC gating; **not done here.**

---

## 6. Focus Area 3 — Premature State Mutations

**Assessment: clean.** No state transition runs ahead of verified wire arrival:

- **Cipher keys** advance only inside `Cipher::encode`/`decode` — i.e. at *encode time* (outbound)
  and at *pull time* (inbound), never at `feed` time. The legacy ticks inside `Encode`/`Decode` too
  (`ITZEncrypt.cpp:33-34,72-73`), and both orders are pinned: `feed` alone does not advance keys
  (`cipher_keys_advance_exactly_once_per_pulled_frame`, `codec/tests.rs:582`), and a refused encode
  (`PayloadTooLarge`) leaves keys untouched (`an_oversized_payload_is_refused_before_the_cipher_ticks`,
  `:984`) — the documented divergence from a legacy with no encode-time ceiling.
- **Decoder poison** is set only by an actual guard/integrity refusal, and only ever *after* the
  offending bytes have physically arrived; `clear()` cannot resurrect it (`ITZSession.cpp:181,305`
  clears buffers only to reset a live session, never to rescue a refused one).
- **Integrity failure** poisons *after* the tick, matching the legacy's key position at the moment
  of its `return false` teardown (`Client-Winsock.cpp:872-889`); the logged `key1/key2` are the
  post-tick values the legacy's own trailing `GetKey1/GetKey2` read (`:880-881`).
- **Session** returns from `connect` only after the OS handshake completes; `Session` carries no
  lifecycle state at all (the state box is a future step), so there is nothing that could flip
  early. `Encoder::send`'s tick-before-flush on an I/O failure matches the legacy, whose keys had
  already advanced inside `Encode` while filling the send buffer, independently of the socket write
  (`ITZSendBuffer.cpp:115-177` + ignored `FALSE` at `Client-Winsock.cpp:147,203`) — documented at
  `codec.rs:247-253,704-708`.

---

## 7. Focus Area 4 — Parity with the C++ Legacy and the Server Rust

### 7.1 Verification ledger (every citation re-read this audit)

| Port claim | Legacy/server truth (read & confirmed) |
|---|---|
| Header is 5 bytes, packed, LE | `itzbase.h:58-70` (`#pragma pack(1)`, `sPacketHeader` at 63-68) |
| `PACKET_MAX_SIZE = 65000` | `itzbase.h:44-46` |
| Guards: `< 2` and `> PACKET_MAX_SIZE - 8` (= 64992) | `ITZRecvBuffer.cpp:163-170,172-180` |
| Cipher tick-before-check, both keys, every call | `ITZEncrypt.cpp:33-34` (Encode), `:72-73` (Decode) |
| `size <= 3` passthrough verbatim, no integrity byte | `ITZEncrypt.cpp:38-42,79-83` |
| Integrity slot = last plaintext byte + key1 | `ITZEncrypt.cpp:44` |
| Rolling XOR/add loop over offsets 3.. | `ITZEncrypt.cpp:55-65` (enc), `:98-109` (dec) |
| Integrity check vs last decoded byte → TRUE/FALSE | `ITZEncrypt.cpp:111-113` |
| Decode ceiling `> PACKET_MAX_SIZE` → FALSE, keys already ticked | `ITZEncrypt.cpp:72-77` |
| Link keys `0x12,0x34`; two instances per session | `ITZSession.cpp:56` and `:13-14` |
| `ClearBuffer` on session reset/close only | `ITZSession.cpp:181-182,305-306` |
| Connect deadline 7000 ms; close throttle 600 ms | `ITZSession.cpp:238-243` and `:213-215` |
| Name resolution via `gethostbyname` | `ITZSession.cpp:120-134` |
| `Completion == FALSE` → `SESSION_STATE_DISCONNECTED` | `ITZNetIOCP.cpp:145-150` |
| Only server-side liveness watchdog (10 s) | `ITZNetIOCP.cpp:41-49` |
| Send ring: `GetEncrypt2()->Encode` when `bEncrypt` | `ITZSendBuffer.cpp:122-126` |
| Send ring full → `FALSE`, ignored by client | `ITZSendBuffer.cpp:141-146`; `Client-Winsock.cpp:147,203` |
| Client game ring 300000 | `Client-Winsock.cpp:92` |
| Send gates on key receipt (front/game) | `Client-Winsock.cpp:141,171`; `LogIn.cpp:311,989` |
| Compression ≥ 200 bytes, `usCmd |= 0x8000` set before Encode | `Client-Winsock.cpp:178-196` (flag at `:192`) |
| Key switch on opcode 30000 | `Client-Winsock.cpp:222-248` (SetKey at `:228`) |
| Consume order `GetFirstPacket → Decode → RemoveFirstPacket` | `Client-Winsock.cpp:864-918` (Remove at `:916`) |
| Legacy failure log fields (size, wire cmd, post-tick keys) | `Client-Winsock.cpp:874-884` |
| Compression flag tested before / cleared after decompress | `GameProtocol.cpp:109,141`; `Client-Winsock.cpp:896-912` |
| Server decodes client→server with negotiated `GetEncrypt` | `GameProtocol.cpp:76`; `FrontClientProtocol.cpp:85` |
| Front server→client is plaintext (client's plaintext front decoder) | `FrontClientProtocol.cpp:21` (factory defaults `headerType=0, bEncrypt=false`), plaintext sends at `:58` |
| Game server client-facing rings (BUG-014 enabler) | `GameProtocol.cpp:25` (30000/80000 `_CHINA`), `:27` (50000/50000) — see F-05 |
| `headerType == 1` has zero call sites tree-wide | All 18 `CreateFactory`/`CreateRecvBuffer` sites in `GoonZuWorld` (12), `Server\FrontServer` (4), `PatchChina` (1), `NetworkLib` test client (1) read; the only non-default arg list is `0, true` (`GameProtocol.cpp:25,27`) |
| No `TCP_NODELAY` on game/front links | zero hits tree-wide (`*.cpp`) |
| Only socket timeouts in the tree are on the billing socket | `NHanGameBillingProcess.cpp:453` ( SND ), `:500` ( RCV ) — sole hits |
| GM/log clients never key their links | `GMProtocol.cpp:98` factory uses defaults (`bEncrypt=false`) |
| Server-rust `validate_size` logic identical | `gz-net/src/lib.rs:169-180` — same three guards, same bounds |
| Server-rust session timings identical | `gz-net/src/session.rs:79-85` (7000/600; plus 10 000 ms handshake) |

### 7.2 Divergences (all pre-existing, all documented, all pinned)

BUG-013 (reject 2..=4), BUG-014 (no ring corruption), encode-time size guard, defined-zero `ucChk`
after decode (BUG-012), dropped-with-connection refused frames, no `headerType == 1` — each is
asserted in both directions by the divergence harness (`codec/tests.rs:59-144`) so none can drift
into silent agreement. The client and server Rust ports agree on every one; API naming differs
only cosmetically (`is_compressed` vs `compressed`; client `HeaderError` adds `BufferTooShort`).

### 7.3 New parity observations

- **F-04 (Low) — raw PINGTEST send.** The legacy client writes `FRONTMSG_REQUEST_PINGTEST` to the
  send buffer **without `Encode`** (`PatchChina/FrontProcess.cpp:125-137`), and the front server
  deliberately skips its `Decode` for exactly that opcode (`FrontClientProtocol.cpp:77`) — the one
  plaintext exception on an otherwise keyed link. The port's `Encoder` has no raw-send API. No
  current call site exists in this port, so there is no present defect; whoever implements ping
  test must bypass the cipher or the server will read garbage and `CloseSocket()` the session
  (`FrontClientProtocol.cpp:85-91`). *Recommendation:* carry this note into that step's design.
- **F-02 (Medium) — read timeout on quiet links.** The legacy's links block forever on receive
  (no `SO_RCVTIMEO` anywhere near them); the port's 5 s `DEFAULT_READ_TIMEOUT` is a deliberate,
  documented guard (`session.rs:26-33`). The front link is safe under normal operation — the front
  server writes a world-refresh at least every 3 s per client (`FrontClientProtocol.cpp:44-60`) —
  but the **game link** can legitimately go silent for longer than 5 s (idle map, no events). If a
  Step 4/5 receive loop treats `SessionError::ReadTimeout` as fatal, the port will drop connections
  the legacy kept alive. The distinct error type makes the correct policy expressible today.
  *Recommendation (FMEA FM-R2):* receive loops treat `ReadTimeout` as "nothing yet — keep
  waiting", optionally with a much longer total-idle ceiling; or make the read window per-link.
  No MOD-01/02 code change required now.
- **Server-rust items the client has not yet needed:** `HANDSHAKE_TIMEOUT` (10 s,
  `ITZNetIOCP.cpp:41-49`) and the `SessionState` box live in `gz-net::session`; the client's
  transport deliberately defers them (documented at `session.rs:54-56`). Flagged so the future
  state-box step ports the same constants rather than inventing new ones.

---

## 8. FMEA (clause 6.1) — additions and carry-overs

| ID | Failure mode | Effect | S | O | D | RPN | Mitigation / status |
|---|---|---|---|---|---|---|---|
| FM-R1 | Environment answers where TEST-NET-1 should black-hole | `connect_times_out…` test fails on CI/foreign hosts (false alarm) | 3 | 4 | 2 | 24 | F-01: `#[ignore]`-gate before CI migration |
| FM-R2 | Quiet game link silent > 5 s; caller treats `ReadTimeout` as fatal | Spurious disconnect where legacy waited | 6 | 3 | 2 | 36 | F-02: caller policy at Steps 4/5; error type already distinct |
| FM-R3 | Consumer stops draining; peer keeps sending legal frames | Unbounded memory growth → OOM instead of clean refusal | 7 | 2 | 3 | 42 | F-03: high-water guard at state-box step (QC-gated change) |
| FM-R4 | Future ping-test step enciphers PINGTEST | Server kills session on decode failure | 6 | 1 | 3 | 18 | F-04: design note carried into that step |
| FM-R5 | Directive text says "47/47"; suite now 63 | QC sign-off confusion | 2 | 5 | 1 | 10 | F-06: actuals recorded here; QC Lead updates TASK_MATRIX |
| FM-01…FM-06 | (server-repo movement-subsystem FMEAs) | — | — | — | — | — | Out of this audit's scope (different repo/subsystem) |

---

## 9. Recommendations (no code changed by this audit)

1. **QC Lead:** supersede the "47/47" figure with the measured **63/63** (MOD-01/02: 52) when
   updating `TASK_MATRIX.md`.
2. **Next doc pass (any worker):** correct `codec.rs`'s ring-size citation — the game server's
   client-facing factories pass `30000, 80000` (`_CHINA`) / `50000, 50000`, not "30000/50000"
   (F-05); `LogIn.cpp:310` is now line 311. The *essential* claim (recv ring < 64992 → BUG-014)
   holds. The same compressed wording exists upstream in `gz-net`'s docs — fix both together.
3. **Before any CI/agent-parallel test migration:** gate the two F-01 tests behind
   `#[ignore = "env"]` or an env-var opt-in.
4. **State-box step:** port `HANDSHAKE_TIMEOUT`/`SessionState` from `gz-net::session`; design the
   `ReadTimeout` caller policy (FM-R2) and the decoder high-water guard (FM-R3) into that step's
   QC gate.
5. **Ping-test step (if ever):** implement a raw-send path or deliberately skip the cipher for that
   opcode (FM-R4).

---

## 10. Traceability & Sign-off

- **Baseline audited:** `a409121cae88a8e7b9ab83cc5a39d9dc07a90937` (clean tree before and after;
  this audit changed only this file).
- **Verification commands run this audit:**
  - `cargo test` → `63 passed; 0 failed` (lib), doc-tests and bin target clean.
  - `cargo clippy --all-targets -- -D warnings` → clean (zero warnings).
- **Evidence sources read this audit (not quoted from memory):**
  - Legacy: `itzbase.h`, `ITZEncrypt.cpp`, `ITZRecvBuffer.cpp`, `ITZSendBuffer.cpp`,
    `ITZSession.cpp`, `ITZSessionFactory.h`, `ITZNetIOCP.cpp`, `Client-Winsock.cpp`,
    `GameProtocol.cpp`, `FrontClientProtocol.cpp`, `PatchChina/FrontProcess.cpp`,
    `LogIn.cpp`, `NHanGameBillingProcess.cpp` (grep-confirmed), plus tree-wide greps for
    `CreateFactory`/`CreateRecvBuffer`/`SetBuffer`/`SO_*TIMEO`/`TCP_NODELAY`.
  - Server-rust: `goonzu-rs` (= `rust_Iwcgz_server`, remote confirmed) `gz-net/src/{lib,session}.rs`,
    `gz-net/tests/data/vectors.rs`, `gz-cipher/tests/data/vectors.rs`, reference harness trees.
  - Client: all of `src/protocol/header*.rs`, `src/net/{cipher,session,codec}*.{rs}` incl. tests
    and fixtures.
- **Change control:** zero source lines, zero tests, zero fixtures modified. `TASK_MATRIX.md`
  untouched (QC Lead's document).

**Status: AUDIT COMPLETE — PASS WITH OBSERVATIONS (F-01…F-06 carried to the QC register).**
