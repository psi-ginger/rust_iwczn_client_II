# Step 3 Refactoring — Adversary Mutation Probes (Phase 5, P-1…P-7)

Branch: `refactor/step3-structure` (base `f5ec238`, head after Phase 4 `56d197a`).
Each probe applied **one** mutation to a committed file, ran **only** the named
verification test, recorded exit **101** with that test in `FAILED`, then
restored with `git checkout -- <file>` and confirmed `git status --porcelain --
src tests` empty (byte-clean) before the next probe. All seven fired.

| Probe | File / site | Mutation | Named test | Exit |
|-------|-------------|----------|------------|------|
| P-1 | `src/auth/front.rs` `FrontLink::send` | removed the `NotKeyed` gate (`if self.key.is_none() { return Err(...); }`) | `send_before_the_handshake_is_refused_and_writes_nothing` | 101 |
| P-2 | `src/net/cipher.rs` `Cipher::decode` | `self.tick()` moved from entry to the tail (after the key-consuming body, before `Ok(integrity == …)`) | `every_vector_matches_the_legacy` | 101 |
| P-3 | `src/net/codec.rs` `Decoder::declared_at` | removed `self.failure = Some(err);` (refusal no longer sticky) | `the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks` | 101 |
| P-4 | `src/net/codec.rs` `Decoder::new` | `cipher: None` → `cipher: Some(Cipher::link_cipher())` | `a_plaintext_decoder_never_touches_its_bytes_or_keys` | 101 |
| P-5 | `src/net/codec.rs` `Decoder::next_frame` | extra `cipher.decode(&raw, &mut throwaway)` after the real decode (keys tick twice per frame) | `cipher_keys_advance_exactly_once_per_pulled_frame` | 101 |
| P-6a | `src/client/driver.rs` `front_phase` | `parse_notice` refusal labelled `Stage::FrontLogin` instead of `Stage::WorldList` | e2e `a_short_notice_response_is_refused_at_the_world_list_stage` (G-2) | 101 |
| P-6b | `src/client/driver.rs` `front_phase` | `login.capture()` refusal labelled `Stage::WorldList` instead of `Stage::FrontLogin` | e2e `a_rejected_login_result_is_refused_at_the_front_login_stage` (G-1) | 101 |
| P-7 | `src/auth/front.rs` `FrontLink::next_frame` | `self.decoder.feed(&buf[..n])` → `self.decoder.feed(&buf[..])` | `handshake_reassembles_a_frame_split_across_reads` | 101 |

## Observed failures (verbatim highlights)

- **P-1** — `auth::front::tests::send_before_the_handshake_is_refused_and_writes_nothing ... FAILED`,
  panicked at `src\auth\front\tests.rs:299:48`; result `3 passed; 1 failed; 297 filtered out`.
- **P-2** — `net::cipher::tests::every_vector_matches_the_legacy ... FAILED`,
  panicked at `src\net\cipher\tests.rs:46:17`,
  `assertion 'left == right' failed: C4_enc2_roundtrip op 1: accepted`.
- **P-3** — `net::codec::tests::the_guard_table_refuses_each_bad_size_at_feed_time_and_sticks ... FAILED`,
  panicked at `src\net\codec\tests.rs:414:9` (`decoder.failure()` sticky assert).
- **P-4** — `net::codec::tests::a_plaintext_decoder_never_touches_its_bytes_or_keys ... FAILED`,
  panicked at `src\net\codec\tests.rs:703:38`.
- **P-5** — `net::codec::tests::cipher_keys_advance_exactly_once_per_pulled_frame ... FAILED`,
  panicked at `src\net\codec\tests.rs:607:5`.
- **P-6a** — `expected a Stage{WorldList} refusal, got Stage { stage: FrontLogin, source: UnexpectedNoticeLength { len: 15 } }`
  (`tests\e2e_loopback.rs:613`).
- **P-6b** — `expected a Stage{FrontLogin} refusal, got Stage { stage: WorldList, source: Rejected { result: 2 } }`
  (`tests\e2e_loopback.rs:554`).
- **P-7** — `auth::front::tests::handshake_reassembles_a_frame_split_across_reads ... FAILED`,
  panicked at `src\auth\front\tests.rs:223:38` (seed mismatch: trailing buffer
  zeros joined the frame).

## P-2 placement note (two-step execution, recorded honestly)

The literal reading — `self.tick()` moved from `decode`'s entry to just after
the size-guard block — was applied first and ran **EXIT=0** against
`every_vector_matches_the_legacy`: no recorded vector drives `decode` down an
error path, and every accepted vector still ticks before the key-consuming
body, so guard-relative placement is invisible to the vector set (the tick-on-
refusal guarantee is pinned by the separate
`oversize_frame_is_rejected_without_writing_output`, which was not the named
target). The probe was therefore escalated to the placement that makes tick
*timing* observable — `self.tick()` relocated after the integrity/rolling
computation — which fails the named test at exit 101 as specified (output
above). The guard-relative variant's EXIT=0 observation is recorded here so
the deviation is explicit rather than silent.

## Restore discipline

After every probe: `git checkout -- <mutated file>`, then
`git status --porcelain -- src tests` returned **empty** (no residue in
tracked sources or tests). After P-7 the whole tree was verified clean
(`git status --porcelain` showed only the untracked `evidence/` bundle), and
the subsequent 3× full `cargo test` runs (all exit 0, 327 tests) plus the five
final gate logs in `final/` were captured on the restored tree.
