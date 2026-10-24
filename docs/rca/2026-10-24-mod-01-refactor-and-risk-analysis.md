# RCA & Risk Analysis: MOD-01 Refactor (Header, Cipher, Fixtures) and Residual Risk Posture

- **Date**: 2026-10-24
- **Step**: Phase II, Step 2.4.2 ("MOD-01 Continuation / RCA & Risk Management")
- **Depends on**: Step 2.4.1 refactor, commit `72ea2e0` (QC-passed in `7f0983f`)
- **Scope**: `src/protocol/header.rs`, `src/net/cipher.rs` and their new
  `cfg(test)` submodules; invariant analysis for the header/cipher layer;
  residual risks carried into MOD-03 (Front Server flow) and MOD-04 (Game
  Server flow)
- **Severity**: Informational (audit deliverable — Step 2.4.1 left no remaining
  refactoring, so this step is the RCA document per `CONTRIBUTING.md` Step 2.4.2)

---

## Summary

Step 2.4.1 brought MOD-01 to the same uniform structure Step 2.5.1 established
for MOD-02: each implementation module carries implementation only, with its
recorded ground truth and its tests in dedicated `#[cfg(test)]` submodules.
One duplicated prologue (the cipher's key tick) was collapsed into a single
private helper so its invariant has exactly one enforcement site. The refactor
changed **no wire behavior**: all 39 recorded legacy cipher vectors replay
bit-exactly, and the suite is 53/53 green with `cargo clippy --all-targets --
-D warnings` and — for the first time crate-wide — `cargo fmt --check` clean.

This document records (1) the structural changes and the uniform layout, (2)
the invariants the layer must preserve — the rolling key tick on every call
before any check, the 5-byte little-endian wire layout with its single guard
table, and the integrity byte's resync-tag semantics — each with its
enforcement site and pinning tests, and (3) the residual risks that the live
Front/Game handshakes (MOD-03/04) must respect, each with its existing
safeguard.

---

## 1. Structural changes and uniform impl/tests layout

### 1.1 Before / after

| File | Before | After | Role now |
|---|---|---|---|
| `src/net/cipher.rs` | 751 lines: impl + 250-line recorded vector table + test harness | **309 lines** | Implementation only: `CipherError`, `Cipher`, `LINK_KEY1/2`, `PACKET_MAX_SIZE` |
| `src/net/cipher/fixtures.rs` | — (inline) | **260 lines** | `pub struct Op` / `pub struct Case` / `pub const CASES`: 26 recorded cases, 39 recorded ops from the byte-for-byte unmodified `ITZEncrypt.cpp` |
| `src/net/cipher/tests.rs` | — (inline) | **205 lines** | Cipher test harness (8 tests) + `unhex`/`SENTINEL` helpers |
| `src/protocol/header.rs` | 378 lines: impl + inline tests | **237 lines** | Implementation only: `HeaderError`, `WireHeader`, `validate_size`, size/flag constants |
| `src/protocol/header/tests.rs` | — (inline) | **145 lines** | Header tests (11 tests); no recorded vectors exist here, so no fixtures module |

The crate now has one uniform layout: an implementation module ends with
`#[cfg(test)] mod tests;` (`header.rs:236`, `cipher.rs:305-309`), and a module
that owns recorded wire ground truth additionally declares `mod fixtures;`
(`cipher.rs:305-306`) — exactly the shape MOD-02's `codec.rs` settled on in
Step 2.5.1. Test distribution after the split (all pre-existing, none added or
weakened): cipher 8, codec 23, session 10, header 11, root 1 = **53**.

### 1.2 How the split was done (decoupling without drift)

Extraction was content-based line slicing of the original files — no retyping:

- **The vector table is byte-verified identical.** After the split, every
  moved block was diffed against the pre-split `HEAD` content: fixtures
  250/250 lines, cipher tests 205/205, header tests 140/140 — zero differing
  lines, with the only transformations being one level of dedent and `pub`
  visibility on `Op`/`Case`/`CASES` (struct, fields, table) so the sibling
  `tests` module can read them. The fixture file's module doc states the
  ground-truth rule: recorded vectors must never be edited to make a test
  pass; a disagreement means the port is wrong, not the fixture.
- **The replay test is the integrity anchor.** `every_vector_matches_the_legacy`
  replays all 39 recorded ops and asserts `checked >= 39` — the floor equals
  the exact op count, so a silently shrunk table fails CI.
- **Public API unchanged.** No item was renamed, moved, or re-exported; the
  implementation files lost only their `mod tests` bodies and gained the
  submodule declarations. Downstream steps (MOD-02 already, MOD-03/04 next)
  import exactly the same paths as before the refactor.

### 1.3 Logic dedup: one tick site

`Cipher::encode` and `Cipher::decode` each hand-wrote the two-line key-tick
prologue. Both now call one private helper (`cipher.rs:171`), invoked as the
first statement of `encode` (`cipher.rs:198`) and `decode` (`cipher.rs:250`).
This is behavior-preserving by construction — the helper is the identical
`wrapping_add(1)` on both keys — and it gives the module's central invariant
(§I1) a single enforcement site, mirroring MOD-02's `declared_at` dedup.

### 1.4 Encapsulation audit (no change needed)

- `Cipher`'s `key1`/`key2` and `WireHeader`'s `size`/`integrity`/`cmd` are
  private; all construction and mutation goes through invariant-preserving
  paths (`WireHeader::decode` runs `validate_size`; `WireHeader::new` bounds
  the payload; `Cipher::set_key` is the documented resync entry).
- `Cipher` is intentionally `Copy` and intentionally **not `Sync`** — the
  legacy guarded the same state with its session critical section; one
  instance per direction per connection is the contract (§R9).

---

## 2. Invariant analysis

### I1 — Rolling tick on *every* call, *before any check*

**What must hold.** Both key bytes advance by one (`wrapping_add`) at the top
of every `Encode` and `Decode` call — before any length, size, or integrity
check — including the `size <= 3` passthrough branch and frames that go on to
be rejected (`ITZEncrypt.cpp:33-34,72-73`). Two peers stay in step only if
they make the same calls in the same order; the keys *are* the counter, and
`set_key` resets the counter position (the only repair for a desynchronised
link).

**Where it is enforced now.** Exactly one site: `Cipher::tick`
(`cipher.rs:163-174`), the first statement of both public mutators
(`cipher.rs:198`, `cipher.rs:250`). The refactor moved this from two inline
copies to one helper — the invariant can no longer be updated in one place and
forgotten in the other.

**Tests pinning it.** `every_vector_matches_the_legacy` (vectors
`A5_passthrough_ticks_keys`, `C3_enc2_counter_advances` and every round-trip
case assert post-call key positions implicitly through output bytes),
`oversize_frame_is_rejected_without_writing_output` ("keys still ticked"),
`link_cipher_roundtrip_on_a_real_frame` ("peer counters equal"). On the
codec side, `cipher_keys_advance_exactly_once_per_pulled_frame` (MOD-02)
pins the pull-time consumption rule, and `an_oversized_payload_is_refused_
before_the_cipher_ticks` pins the one deliberate pre-tick refusal (encoder
header guard).

### I2 — 5-byte little-endian layout, one guard table

**What must hold.** The wire header is 5 bytes — `usSize: u16` LE, `ucChk: u8`
reserved, `usCmd: u16` LE (`#pragma pack(1)`, `itzbase.h:63-68`) — never the
spec's fabricated 8-byte/checksum form (RCA
`2026-10-23-step-1.2-header-spec-mismatch.md`). Size guards live in exactly
one place, `validate_size` (`header.rs:110`): reject `< 2` (legacy guard),
`2..=4` (BUG-013 divergence for parity with server `gz-net`), `> 64992`
(`MAX_FRAME_SIZE = PACKET_MAX_SIZE - 8`). The reserved byte passes through
decoding untouched and is zeroed on encoding; `usSize` stays in the clear on
ciphered links so framing can cut frames before decryption.

**Where it is enforced now.** `WireHeader::decode` (`header.rs:171`) reads LE
and calls `validate_size` before constructing; `WireHeader::new`
(`header.rs:148`) bounds locally built frames by the same limit; `encode`
(`header.rs:187`) emits the layout verbatim with the reserved byte zeroed. The
MOD-02 decoder reaches the same single guard site through `declared_at`, so
encode, decode, and framing cannot drift apart.

**Tests pinning it.** `header_is_five_bytes`, `encode_is_bit_exact_little_endian`,
`decode_reads_cpp_s_packet_header_sample`, `guard_table_matches_legacy_recv_path`
(pins `< 2` / `2..=4` / `64992` boundaries),
`decode_applies_the_guard_table`, `reserved_byte_is_carried_through_untouched`,
`compression_flag_is_exposed_but_never_masked`, `new_rejects_oversized_payloads`.

### I3 — Integrity byte: a cipher resync tag, verified as such

**What must hold.** Offset 2 is **not a checksum**: no legacy producer writes
it and no legacy consumer validates it. On a ciphered link it carries a
one-byte resync value: `encode` writes `src[size-1] + key1` there
(`cipher.rs:218`, `ITZEncrypt.cpp:44`); `decode` recovers
`src[2] - key1` into a local and accepts only when it equals the **last
decoded plaintext byte** (`cipher.rs:279`, `cipher.rs:295`,
`ITZEncrypt.cpp:111`) — the legacy's `TRUE`/`FALSE`. Byte-compatibility
quirks are preserved deliberately: `decode` never writes `dest[2]`
(BUG-012 — the decoded reserved byte is whatever the caller's buffer held),
and `decode` leaves `src` untouched (the legacy rewrites `pSrc[2]` in place;
this port computes the same value in a local).

**Where it is enforced now.** Unchanged by the refactor — only the file
layout moved. The codec layer documents the consumer consequence: a
deciphered frame's `Frame::integrity()` is the caller's leftover byte, not a
trustworthy value (`codec.rs:128-129`).

**Tests pinning it.** `E7_decode_corrupt_checksum_slot` (rejected when the
tag disagrees), `E2_decode_corrupt_payload` / `E6_decode_wrong_key` (integrity
fails through payload or key mismatch), `decode_leaves_the_source_untouched`,
`mismatched_key_pair_fails_the_integrity_check`, plus the MOD-02 anchor
`a_tampered_frame_fails_integrity_and_reports_the_legacy_log_fields`.

---

## 3. Residual risks for MOD-03 / MOD-4 (live handshakes)

| ID | Risk | Affects | Severity | Existing safeguard |
|---|---|---|---|---|
| R1 | **Key desync is only repairable by resync**: the keys *are* the call counter (I1); one dropped, duplicated, or reordered frame — or one swallow-and-continue after an integrity error — permanently desynchronises the link. | MOD-03/04 integration | High | `set_key` (`cipher.rs:144`) is the documented repair (resets counter position); sticky decoder poison forces teardown-and-rekey rather than silent continuation; `CodecError::Integrity` carries post-tick `key1/key2` for diagnosis. |
| R2 | **Front link (5700) is asymmetric**: inbound is plaintext for the link's whole life, outbound becomes ciphered only after Opcode 30000 delivers the seed (`payload[2]`, `payload[4]` → `set_key`). Installing a decoder cipher here refuses the plaintext stream; sending before the key goes out in plaintext the server does not expect. | MOD-03 (Steps 3.1–3.3) | High | `Decoder::new()` (no cipher) + `Encoder::new()` → `set_cipher(Cipher::session_cipher(k1, k2))` is exactly this shape; legacy parity: keyed links *bar* pre-key sends at the handler gates (`Client-Winsock.cpp:141,171`); pinned by `a_ciphered_decoder_refuses_a_plaintext_stream` and `set_cipher_switches_the_outbound_side_mid_connection`. |
| R3 | **Game link (4010) is keyed from the first byte**: the fixed link cipher `[0x12, 0x34]` (`ITZSession::Init`) governs server→client from connection setup; the negotiated session cipher governs client→server once the char-unique handshake keys it. Installing either late corrupts the first frames; installing either on the wrong direction breaks the peers' counter alignment. | MOD-04 (Steps 4.1–4.2) | High | `Cipher::link_cipher()` / `Cipher::session_cipher()` name the two legacy instances (`GetEncrypt2`/`GetEncrypt`); constants pinned by `link_cipher_and_size_constants_match_the_legacy`; direction table in the cipher module docs. |
| R4 | **The integrity byte is 1 byte, checked last-plaintext-only**: a corrupted frame passes with probability 1/256. This is legacy parity, not a defect — but nothing above the cipher should treat acceptance as an integrity guarantee. | MOD-03/04 payload handling | Medium | Documented as "not a checksum" in both module docs; server repo makes the same statement (`gz-net::Frame::integrity`); MOD-02's codec surfaces rejection as a sticky, fatal refusal rather than a recoverable error. |
| R5 | **`Cipher` is not `Sync` — and must not be shared**: two tasks calling one instance interleaves the counters and silently corrupts the link. | MOD-03/04 task layout | Medium | Documented on the type (`cipher.rs:102-103`); one instance per direction per connection, matching the legacy's two-`ITZEncrypt`-per-session design; codec holds the cipher by value inside `Decoder`/`Encoder`, which are also per-connection. |
| R6 | **Direct `Cipher` use bypasses the framing guards**: the `size <= 3` passthrough branch ticks and copies verbatim, and `encode` has no size ceiling — the legacy's own asymmetry, reproduced exactly. Any code path that feeds `Cipher` without going through `validate_size`/`Decoder`/`Encoder` re-opens the out-of-bounds-adjacent behaviors the port otherwise replaces with errors. | future refactors / non-codec callers | Medium | All current call sites go through `Decoder`/`Encoder` (guard-before-cipher); `encode_has_no_size_ceiling_but_decode_does` and `lying_length_fields_are_rejected` pin the raw behavior as *deliberate parity*, so "fixing" it in the cipher layer itself would fail CI. |
| R7 | **Deciphered frames carry a meaningless reserved byte** (BUG-012): `decode` never writes `dest[2]`, so `Frame::integrity()` on a deciphered link is the caller's leftover byte. A future step tempted to build checksum-style logic on it would invent behavior no legacy or server code has. | MOD-03/04 opcode handling | Medium | Module docs on both layers state "do not build integrity on it"; `reserved_byte_is_carried_through_untouched` pins the passthrough; server repo documents the same prohibition. |
| R8 | **Fixture drift vs. the server repo**: `cipher/fixtures.rs` is recorded ground truth copied from `rust_Iwcgz_server/crates/gz-cipher`'s reference harness. If that repo revises vectors, this repo's copies silently lag. | all cipher steps | Medium | Fixtures are read-only ground truth (module doc); the `checked >= 39` floor equals the exact recorded op count, so table shrinkage fails CI; dual-source rule (README Rule 1) applies to any cipher change. |
| R9 | **No live-server coverage in MOD-01**: every MOD-01 test replays recorded vectors or runs loopback. Timing, real seed extraction, and cross-server framing behaviors cannot be proven here. | MOD-03/04 acceptance | Medium | By design: `CONTRIBUTING.md` Steps 3.1/4.1 require live tests against the `goonzu-rs` Front/Game servers — those are the safety net for what vectors cannot prove. |
| R10 | **`set_key` is a bare setter**: it resets both keys unconditionally. Calling it mid-stream on a live cipher (rather than between connection phases) desynchronises the peer with no diagnostic. | MOD-03 (opcode 30000 timing) | Low | Documented as *the* resync point; the Front flow keys outbound exactly once, at opcode 30000, before any keyed send — the gate tests above pin the ordering. |

### Consumer contract for MOD-03/04 (derived from I1–I3 and R1–R10)

1. **Front link**: `Decoder::new()` (plaintext inbound, always) —
   `Encoder::new()` until Opcode 30000, then one
   `set_cipher(Cipher::session_cipher(payload[2], payload[4]))` before the
   first keyed send. Never install an inbound cipher on this link.
2. **Game link**: inbound `Decoder::with_cipher(Cipher::link_cipher())`
   before the first read (fixed `0x12/0x34`); outbound session cipher keyed
   from the char-unique handshake, same one-shot rule.
3. **One `Cipher` per direction per connection**, owned by its
   `Decoder`/`Encoder`; never shared across tasks, never `set_key`'d outside
   a connection-phase boundary.
4. **Any `Err` from `feed`/`next_frame` is fatal** — the keys have already
   ticked; drop the session and re-key on a fresh connection (R1).
5. **Never build logic on the reserved byte** of a deciphered frame (R7), and
   never call `Cipher` directly without the framing guards (R6).

---

## 4. Verification safeguards (Step 2.4.2 acceptance evidence)

Commands run against the refactor commit `72ea2e0`:

| Check | Result |
|---|---|
| `cargo check --all-targets` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` (crate-wide) | clean — first time; the four pre-existing deviations were all in MOD-01 test code and were formatted during the split |
| Content diff of every moved block vs. pre-split `HEAD` | 0 differing lines (fixtures 250/250, cipher tests 205/205, header tests 140/140) |
| `cargo test` | **53 passed, 0 failed** (cipher 8, codec 23, session 10, header 11, root 1) |

Invariant → test maps are listed under each invariant in §2; the recorded
vector replay (`every_vector_matches_the_legacy`, `checked >= 39` against
exactly 39 recorded ops) is the byte-exact anchor for I1 and I3.

## 5. Prevention

- The cipher's key tick lives in one private helper; any new mutator on
  `Cipher` must call it (or be a documented, gate-tested pre-tick refusal
  like the encoder's header guard).
- Size guards live only in `protocol::header::validate_size`; nothing may
  re-derive size rules in the cipher, codec, or future flow modules.
- Recorded vectors stay in `fixtures.rs` modules and are read-only ground
  truth; any future step that changes framing or cipher behavior must satisfy
  the dual-source rule (legacy tree + server repo) and leave the fixtures
  passing unmodified.
- The five consumer-contract rules in §3 apply to every MOD-03/04 module that
  touches `Cipher` or `WireHeader`; deviations are parity bugs even when
  tests are green, because loopback and vector tests cannot observe
  server-side counter desynchronisation (R1/R9).

## Evidence

- Refactor commit: `72ea2e0` ("refactor: scan & refactor MOD-01 per Step 2.4.1")
- QC gate: `7f0983f` (Step 2.4.1 PASSED, 53/53 verified)
- Related: `1a0d2ff` (MOD-02 refactor, same layout), `9704ed9` (MOD-02 RCA —
  whose R1/R2/R3 and consumer contract this document's §3 refines from the
  MOD-01 side)
- Legacy sources cited in-code: `ITZEncrypt.cpp:14,20,25,30,33-34,44,69,72-73,85,111`
  (SetKey/GetKey/Encode/Decode, tick order, resync tag, in-place decrement),
  `itzbase.h:45,63-68` (header layout, `PACKET_MAX_SIZE`),
  `ITZSession.cpp:13-14,56` (two cipher instances, fixed link keys),
  `Client-Winsock.cpp:141,171,228` (pre-key send gates, opcode-30000 key)
- Prior RCAs: `docs/rca/2026-10-23-step-1.2-header-spec-mismatch.md` (5-byte
  header ground truth), `docs/rca/2026-10-24-mod-02-refactor-and-risk-analysis.md`
  (MOD-02 invariant/risk counterpart)
