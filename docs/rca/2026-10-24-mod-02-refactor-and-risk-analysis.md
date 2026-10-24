# RCA & Risk Analysis: MOD-02 Refactor (Session, Codec, Fixtures) and Residual Risk Posture

- **Date**: 2026-10-24
- **Step**: Phase II, Step 2.5.2 ("MOD-02 Continuation / RCA & Risk Management")
- **Depends on**: Step 2.5.1 refactor, commit `1a0d2ff` (QC-passed in `9eb7b28`)
- **Scope**: `src/net/session.rs`, `src/net/codec.rs` and their new `cfg(test)`
  submodules; invariant analysis for the transport/codec layer; residual risks
  carried into MOD-03 (Front Server flow) and MOD-04 (Game Server flow)
- **Severity**: Informational (audit deliverable — Step 2.5.1 left no remaining
  refactoring, so this step is the RCA document per `CONTRIBUTING.md` Step 2.5.2)

---

## Summary

Step 2.5.1 split the two MOD-02 implementation files so each now carries only
implementation, moved every test and recorded wire fixture into dedicated
`#[cfg(test)]` submodules, and collapsed one duplicated guard walk into a
single private helper. The refactor changed **no wire behavior**: the recorded
legacy fixtures, the C++ encoder captures, and the cipher key-tick invariants
all pass byte-identically (53/53 tests, `cargo clippy --all-targets -- -D
warnings` clean).

This document records (1) the structural changes and how the decoupling was
achieved without touching ground truth, (2) the invariants that the new layout
must preserve — sticky decoder poisoning, pull-time cipher with one key tick
per frame, the 5-byte little-endian header and its guard table — and where each
is now enforced and tested, and (3) the residual risks the Front/Game server
flows (MOD-03/04) must respect, each with its existing safeguard.

---

## 1. Structural changes and decoupling

### 1.1 Before / after

| File | Before | After | Role now |
|---|---|---|---|
| `src/net/codec.rs` | 2181 lines: impl + two fixture tables + ~1450-line test harness | **737 lines** | Implementation only: `CodecError`, `SendError`, `Frame`, `Decoder`, `Encoder` |
| `src/net/codec/fixtures.rs` | — (inline) | **407 lines** | `pub mod legacy` + `pub mod legacy_send`: recorded wire ground truth |
| `src/net/codec/tests.rs` | — (inline) | **1085 lines** | Codec test harness (23 tests) |
| `src/net/session.rs` | 474 lines: impl + inline tests | **260 lines** | Transport only: timeouts, `SessionError`, `SessionTimeouts`, `Session` |
| `src/net/session/tests.rs` | — (inline) | **213 lines** | Session tests (10 tests) |

The layout is now uniform across the crate: an implementation module declares
its test half at the bottom (`#[cfg(test)] mod tests;`), and the codec
additionally isolates its recorded vectors in `mod fixtures;`. The test-count
split (codec 23, session 10, cipher 8, header 11, root 1 = 53) matches the
pre-refactor suite exactly — no test was added, dropped, or weakened.

### 1.2 How the split was done (decoupling without drift)

The extraction was content-based line slicing of the original files, not a
retyping, so every moved byte is verbatim:

- **Fixture tables are byte-verbatim.** `codec/fixtures.rs` is the original
  `legacy` / `legacy_send` blocks copied line-for-line; the only textual edits
  are (a) removing one level of 4-space indentation and (b) `mod` → `pub mod`
  so the sibling `tests` module can reach them. The `#[test]`
  `fixture_is_intact_and_divergences_are_pinned` still asserts unique case
  names, `CASES.len() >= 15`, and the exact four-entry `DIVERGENT` list
  (BUG-013's `declared_2/3/4` + BUG-014's `ring_smaller_than_frame`) — a moved
  or mangled fixture fails immediately.
- **Tests moved verbatim, imports rewired only.** `codec/tests.rs` differs from
  the original inline block only in indentation and two `use` paths
  (`super::fixtures::legacy::{...}`, `super::fixtures::legacy_send`).
  `session/tests.rs` is a straight dedent.
- **Implementation files kept their module docs and public API unchanged.**
  `codec.rs` lost only the `#[cfg(test)] mod tests { ... }` tail and gained the
  two submodule declarations (`codec.rs:733-737`); `session.rs` ends the same
  way (`session.rs:259-260`). No public item was renamed, moved, or re-exported,
  so downstream steps (MOD-03/04) import exactly the same paths as before.

### 1.3 Logic dedup: one guard site

`Decoder::scan` (feed-time walk) and `Decoder::next_frame` (pull-time cut) each
re-derived the guarded `usSize` by hand — the same 2-byte little-endian read,
the same `validate_size` call, the same poison-on-failure. Both now call one
private helper (`codec.rs:432-445`):

- `fn declared_at(&mut self, at) -> Result<Option<usize>, CodecError>` reads the
  declared size at offset `at`, returns `Ok(None)` while fewer than 2 size
  bytes are buffered, and — on a guard failure — stores the error in
  `self.failure` **and** returns it.
- `scan` (`codec.rs:454-463`) walks frames with `while let Some(declared) =
  self.declared_at(at)?`; `next_frame` (`codec.rs:481-487`) opens with
  `self.declared_at(0)?`.

This is behavior-preserving by construction: the guard runs on exactly the same
bytes at the same moments as before (feed-time eager validation, plus the
pull-time re-check that keeps the invariant independent of `feed` being the
only byte source), the poison write happens in one place, and
`next_frame` still returns the *original* stored error for an already-refused
decoder because both entry points check `self.failure` first
(`codec.rs:418`, `codec.rs:482`).

### 1.4 Encapsulation audit (Step 2.5.1 finding: no change needed)

- `Decoder` fields (`buf`, `cipher`, `failure`) and `Encoder`'s `cipher` are
  private; construction goes through `new`/`with_cipher`/`set_cipher`, and
  frames leave only through `Frame::from_validated_bytes`
  (`codec.rs:273`), which is private and only reachable after the guard table
  and (when keyed) the cipher have both accepted the bytes.
- `Session` exposes `stream()` (`session.rs:241`) deliberately — the framing
  layer (Step 2.2/2.3) needs the raw `TcpStream` for reads/writes — but its
  fields are private and timeout configuration goes through `SessionTimeouts`.
- No public field, no leaky internal, no invariant that a caller can now break
  from outside. Nothing was changed here.

---

## 2. Invariant analysis

### I1 — Sticky decoder poisoning (refusal is permanent and total)

**What must hold.** A guard-table failure (or a cipher integrity failure)
refuses the decoder **for the rest of its life**: subsequent `feed`/`next_frame`
calls return the *original* error, `clear()` discards bytes but never the
refusal (`codec.rs:548-554`), and a bad header anywhere in the stream means
**nothing** is handed out — not even good frames queued ahead of it. This
mirrors the legacy, which tears the connection down rather than resuming.

**Where it is enforced now.** Exactly one write site for `self.failure` on the
header path — `declared_at` (`codec.rs:441`) — plus the two cipher-path sites
in `next_frame` (`codec.rs:517`, `codec.rs:522`). Both public entry points
short-circuit on `self.failure` first. The refactor's dedup *reduced* the
number of places that could forget to poison from two to one.

**Tests pinning it (pre-existing, still green).**
`the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks`,
`a_good_frame_ahead_of_a_bad_header_is_never_delivered`,
`clear_discards_bytes_but_keeps_a_refusal`,
`a_tampered_frame_fails_integrity_and_reports_the_legacy_log_fields`,
plus the full recorded-fixture replay `decoder_agrees_with_the_compiled_legacy_on_every_recorded_read`.

### I2 — Pull-time cipher, exactly one key tick per frame

**What must hold.** Framing runs ahead of decryption: `usSize` is in the
clear, so headers validate without a key (`codec.rs:492-494`). The inbound
cipher runs **once**, at pull time, over the whole cut frame; both keys advance
on *entry* to `decode`, even when the frame is then refused
(`ITZEncrypt.cpp:72-73`; `cipher.rs` module docs). Consequently an integrity
failure is sticky — the keys already moved, so continuing would desynchronize
permanently. Plaintext mode never touches bytes or keys.

**Where it is enforced now.** `next_frame` (`codec.rs:496-525`): the cut
happens before the cipher, `cipher.decode` is called once per pulled frame, and
the post-tick key position is captured into `CodecError::Integrity` for the
legacy's log fields. The refactor moved this code only insofar as
`declared_at` replaced the inline header read above it; the cipher block is
unchanged. On the outbound side, `Encoder::encode` ticks at encode time, before
any flush (`codec.rs` encoder docs) — one tick per encoded frame, matching
`ITZEncrypt::Encode`.

**Tests pinning it.**
`cipher_keys_advance_exactly_once_per_pulled_frame`,
`ciphered_frames_reassemble_across_irregular_splits_and_bursts`,
`a_plaintext_decoder_never_touches_its_bytes_or_keys`,
`a_ciphered_decoder_refuses_a_plaintext_stream`,
`encoded_frames_round_trip_through_the_decoder_in_both_modes`,
`an_oversized_payload_is_refused_before_the_cipher_ticks`,
`encoder_reproduces_every_recorded_cxx_capture_byte_for_byte`.

### I3 — 5-byte little-endian header and the single guard table

**What must hold.** The wire header is 5 bytes (`usSize: u16` LE, `ucChk: u8`
reserved, `usCmd: u16` LE), never 8 and never checksum-bearing (RCA
`2026-10-23-step-1.2-header-spec-mismatch.md`). Size guards live in exactly one
place — `protocol::header::validate_size` — with: reject `< 2`, reject `2..=4`
(the BUG-013 divergence for parity with server `gz-net`), reject `> 64992`
(`MAX_FRAME_SIZE = PACKET_MAX_SIZE - 8`, `header.rs:45-52`). The decoder never
re-derives these rules; `declared_at` is the only decode-path caller.

**Where it is enforced now.** `declared_at` performs the LE read
(`u16::from_le_bytes([buf[at], buf[at+1]])`, `codec.rs:436`) and delegates to
`validate_size` (`codec.rs:437`). After the refactor there is **no second
copy** of the size logic anywhere in MOD-02 — previously the same read+guard
pair existed twice. The reserved byte passes through the decoder untouched and
is zeroed on encode (then overwritten by the cipher's resync tag on keyed
links).

**Tests pinning it.** `protocol::header` guard-table tests (11),
`fixture_is_intact_and_divergences_are_pinned` (pins `HEADER_SIZE == 5`-era
behavior via the recorded vectors and the exact divergence list),
`compression_flag_and_reserved_byte_pass_through_the_decoder`.

### I4 — Bounded transport (session)

**What must hold.** Connect and every read are bounded (5 s default policy,
7 s legacy number selectable via `SessionTimeouts::legacy_connect`,
`session.rs:71-86`); EOF reads as `Ok(0)`, not an error; no `TCP_NODELAY`
(the legacy netlib never sets it — default Nagle is the wire-faithful choice);
`LEGACY_CLOSE_THROTTLE` (600 ms) is reserved for a future reconnect state
machine. The read timeout is a documented port-added guard with no legacy
counterpart.

**Where it is enforced now.** Unchanged by the refactor — only the test module
moved out. **Tests pinning it:** the 10 `net::session` tests
(`connect_times_out_on_a_black_holed_address`, `read_times_out_when_the_peer_sends_nothing`,
`eof_is_zero_not_an_error`, `no_nodelay_is_set_parity_with_the_legacy`,
`legacy_connect_window_is_selectable`, …).

---

## 3. Residual risks for MOD-03 / MOD-04

| ID | Risk | Affects | Severity | Existing safeguard |
|---|---|---|---|---|
| R1 | **Asymmetric cipher mode on the Front link**: Front Server traffic is *plaintext inbound* while *outbound becomes ciphered* after Opcode 30000 (`ResponseSecretkey`). Installing a `Decoder` cipher there would refuse the plaintext stream. | MOD-03 (Step 3.1) | High | `Decoder::new()` (no cipher) + `Encoder::set_cipher` (`codec.rs:644`) is exactly this shape; `a_ciphered_decoder_refuses_a_plaintext_stream` documents the failure mode; encoder docs (`codec.rs:579-588`) state the outbound-only key arrival. |
| R2 | **Game link is keyed from the first byte**: fixed link cipher `[0x12, 0x34]` must be installed *before* the first read (`Decoder::with_cipher` at construction), or the first frames decode as garbage/`Integrity` failures. | MOD-04 (Step 4.1) | High | `Decoder::with_cipher` / `Encoder::with_cipher`; `a_ciphered_decoder_refuses_a_plaintext_stream` and `link_cipher_roundtrip_on_a_real_frame` (MOD-01) pin the keyed mode. |
| R3 | **Key-tick desync is unrecoverable**: any dropped, duplicated, or reordered frame — or any swallow-and-continue after an integrity error — permanently desynchronizes both peers. | MOD-03/04 integration | High | Sticky poison (I2) forces teardown; `CodecError::Integrity` carries the legacy log fields (`size`, `cmd`, post-tick `key1/key2`) for diagnosis; consumers must treat `Err` from `feed`/`next_frame` as fatal to the connection, never retry. |
| R4 | **Buffer growth if the caller does not drain**: `Decoder::buf` is bounded only by drain discipline — whole frames wait until `next_frame` is called. A bursty server plus a stalled consumer grows memory without a hard cap. | MOD-03/04 runtime | Medium | `buffered()`/`is_empty()` expose pressure; module docs state the bounded-while-drained contract; no legacy ring exists to overflow against (divergence documented). Future: a drain-per-read-loop discipline in the consumer step. |
| R5 | **`0x8000` compression flag is passed through, not handled**: framing exposes `is_compressed()` but never masks or decompresses. Opcode dispatch in MOD-03/04 must branch *before* clearing the flag, and LZF decompression is not yet implemented anywhere in this crate. | MOD-04 (opcode handling) | Medium | Flag round-trip pinned by `compression_flag_and_reserved_byte_pass_through_the_decoder`; header docs (`header.rs:54-56`) cite the legacy's test-before-clear order (`GameProtocol.cpp:109,141`). |
| R6 | **Mid-connection `set_cipher` frame boundary**: frames encoded before the key are plaintext-headered, frames after are keyed from tick one. If MOD-03 sends application traffic between connect and Opcode 30000, it goes out in a mode the server may not expect. | MOD-03 (Step 3.1/3.3) | Medium | Legacy parity: keyed links *bar* pre-key sends at the handler gates (`Client-Winsock.cpp:141,171`, encoder docs); `set_cipher_switches_the_outbound_side_mid_connection` pins the switch semantics. MOD-03 must not send pre-key unless the legacy didn't. |
| R7 | **Fixture drift vs. the server repo**: `codec/fixtures.rs` is recorded ground truth copied from `rust_Iwcgz_server`'s harnesses. If the server repo revises vectors or guard behavior, this repo's fixtures silently lag. | all framing steps | Medium | Fixtures must never be edited to make a test pass (module doc); `fixture_is_intact_and_divergences_are_pinned` fails on any shrink/list change; dual-source rule (README Rule 1: legacy tree + server repo) applies to any framing change. |
| R8 | **No live-server coverage in MOD-02**: every codec/session test runs against loopback or recorded vectors. Real Front/Game servers may still surface timing, Nagle-interaction, or fragmentation behaviors no fixture contains. | MOD-03/04 acceptance | Medium | By design: `CONTRIBUTING.md` Steps 3.1/4.1 acceptance criteria require live tests against `goonzu-rs` servers — those are the safety net for what loopback cannot prove. |
| R9 | **`LEGACY_CLOSE_THROTTLE` is inert**: the 600 ms close→re-dial floor exists as a constant only; nothing enforces it until a reconnect/state-machine step lands. | future reconnect logic | Low | Documented at `session.rs:86` and in the module doc table as "(for a future state-machine step)"; a future step must wire it rather than invent a new number. |
| R10 | **Split-module wiring is `cfg(test)`-gated**: `fixtures`/`tests` submodules compile only under test; a broken path or signature there is invisible to `cargo check` without `--all-targets`. | CI hygiene | Low | Verification standard for every MOD step: `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` on touched files, `cargo test` (53/53) — all run and green for `1a0d2ff`. |

### Consumer contract for MOD-03/04 (derived from I1–I3 and R1–R6)

1. **Front link (MOD-03)**: `Decoder::new()` (plaintext in) — `Encoder::new()`
   until Opcode 30000, then `encoder.set_cipher(Cipher::session_cipher(k1, k2))`.
   Never install a decoder cipher on this link.
2. **Game link (MOD-04)**: both sides keyed at construction with the fixed link
   cipher before the first read.
3. **Any `Err` from `feed`/`next_frame` is fatal** — drop the session and
   re-dial (respecting `LEGACY_CLOSE_THROTTLE` if/when a state machine exists).
   Never attempt in-place recovery; the poison is deliberate parity.
4. **Drain after every read**: `feed` → loop `next_frame` until `Ok(None)`, so
   R4 cannot accumulate.
5. **Check `is_compressed()` before opcode dispatch**; do not mask `0x8000`
   until the decompression step exists.

---

## 4. Verification safeguards (Step 2.5.2 acceptance evidence)

Commands run against the refactor commit `1a0d2ff`:

| Check | Result |
|---|---|
| `cargo check --all-targets` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` (MOD-02 files) | clean |
| `cargo test` | **53 passed, 0 failed** (codec 23, session 10, cipher 8, header 11, root 1) |

Invariant → test map is listed under each invariant in §2; the divergence list
and fixture integrity are pinned by `fixture_is_intact_and_divergences_are_pinned`,
so any silent change to recorded ground truth or the reviewed BUG-013/BUG-014
divergences fails CI immediately.

## 5. Prevention

- Future MOD steps that touch framing must keep **one** guard site: extend
  `validate_size` / `protocol::header`, never inline a size rule into
  `Decoder`/`Encoder`.
- Recorded vectors stay in `codec/fixtures.rs` and are treated as read-only
  ground truth; disagreement with them means the port is wrong, not the
  fixture.
- Any consumer added in MOD-03/04 must obey the five consumer-contract rules
  in §3; deviations are parity bugs even when tests are green, because no
  loopback test can observe server-side key desynchronization (R3/R8).

## Evidence

- Refactor commit: `1a0d2ff` ("refactor: scan & refactor MOD-02 per Step 2.5.1")
- QC gate: `9eb7b28` (Step 2.5.1 PASSED, 53/53 verified)
- Prior MOD-02 steps: `afcee87` (2.1), `19cd996` (2.2), `1330a20` (2.3)
- Legacy sources cited in-code: `ITZRecvBuffer.cpp:112-225` (feed/scan),
  `Client-Winsock.cpp:866-916` (pull), `ITZEncrypt.cpp:33-44,72-73` (tick,
  resync tag), `itzbase.h:45,63-68` (header, `PACKET_MAX_SIZE`),
  `ITZSession.cpp:213-243` (throttle, connect deadline)
- Prior RCA: `docs/rca/2026-10-23-step-1.2-header-spec-mismatch.md` (5-byte
  header ground truth this analysis builds on)
