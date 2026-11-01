# Step 4 — Structural Refactor: execution record

Branch `refactor/step4-structure`, created from clean `main` @
`e6ee8a92b3df07c4781b1a2be0c8915e27216b0c` (Phase 0 baseline). Final
source HEAD before this bundle: `e5643f7`.

Environment (`baseline/00-env.txt`): rustc 1.99.1 (58cf8f9c3), cargo
1.99.1, git 2.55.0.windows.5, host `x86_64-pc-windows-gnu`,
`core.autocrlf=true`.

## Phases

| Phase | Commit | What moved | Evidence |
|---|---|---|---|
| 0 baseline | *(none — evidence only)* | 5 gates over `e6ee8a9`, tree verified clean | `baseline/` |
| 1 characterization | `383ba3b` | G-1 … G-7 + 3 additive mutation arms, **no production code** | `phase1/` |
| 2 T-2g | `3175a2d` | `GameLink::next_frame` read scratch hoisted above the loop | `phase2/` |
| 3 T-1g | `5857d5e` | `game_phase` extracted verbatim from `flow` (35 insertions, 2 deletions) | `phase3/` |
| 4 T-3g | `e5643f7` | `src/game/test_support.rs` consolidates the triplicated game-suite helpers | `phase4/` |
| 5 verification | *(this bundle)* | 11 mutation probes, 3× determinism runs, all 5 gates | `phase5/` |

## Gates — every boundary, all exit 0

Exit codes captured with delayed expansion
(`cmd /v:on /c "… > file 2>&1 & echo EXIT=!ERRORLEVEL! > file.exit"`);
the plain `%ERRORLEVEL%` form is parse-time expanded and reports the
*previous* command's code — verified wrong during Phase 0 and rejected.

| Boundary | check | clippy `-D warnings` | fmt --check | test | doc |
|---|---|---|---|---|---|
| 0 baseline | 0 | 0 | 0 | 0 | 0 |
| 1 (`383ba3b`) | 0 | 0 | 0 | 0 | 0 |
| 2 (`3175a2d`) | 0 | 0 | 0 | 0 | 0 |
| 3 (`5857d5e`) | 0 | 0 | 0 | 0 | 0 |
| 4 (`e5643f7`) | 0 | 0 | 0 | 0 | 0 |
| 5 final | 0 | 0 | 0 | 0 (×3 runs) | 0 |

## Test counts (copied verbatim from `test result:` lines)

| Boundary | lib | main | fixtures | e2e | live | doc | executed | ignored |
|---|---|---|---|---|---|---|---|---|
| 0 baseline | 300 | 0 | 11 | 16 | 0 | 0 | **327** | 5 |
| 1 | 301 | 0 | 11 | 22 | 0 | 0 | **334** | 5 |
| 2 | 301 | 0 | 11 | 22 | 0 | 0 | **334** | 5 |
| 3 | 301 | 0 | 11 | 22 | 0 | 0 | **334** | 5 |
| 4 | 301 | 0 | 11 | 22 | 0 | 0 | **334** | 5 |
| 5 run 1 / 2 / 3 | 301 | 0 | 11 | 22 | 0 | 0 | **334 ×3** | 5 |

Ignored: 1 lib operator-window test + 4 `live_integration` (F-01: no live
server in this environment). The three Phase 5 runs are byte-identical in
counts (`phase5/04-cargo-test-run{1,2,3}.log`), each exit 0.

Phase 1 is the only boundary that grows the suite (+7: G-1 … G-7).
Phases 2–4 are motion-only: counts hold at 334.

## Adversary mutation probes (Phase 5)

One probe applied to the working tree at a time, `cargo test <named test>`
run and captured, then `git checkout -- <file>` restored and
`git status --porcelain` verified — after **every** probe the only entry
was the untracked `evidence/` directory, i.e. the tree returned to the
committed bytes.

| # | Mutation | Named test | Exit | Evidence |
|---|---|---|---|---|
| P-1 | `GameLink::send`'s `NotHandshaken` gate removed | `game::entry::tests::send_before_the_handshake_is_refused_and_writes_nothing` | 101 | `probes/P-01.*` |
| P-2 | handshake keys the outbound encoder with `link_cipher()` | `game::entry::tests::handshake_keys_the_outbound_channel_and_ciphers_every_frame_after_it` | 101 | `probes/P-02.*` |
| P-3 | construct-time decoder keyed `session_cipher(0x7E,0xAD)` (`entry.rs:399`) | `game::entry::tests::the_link_cipher_is_installed_before_the_first_read` | 101 | `probes/P-03.*` |
| P-4 | roster name read `80` → `84` | `game::char_info::tests::char_list_fields_read_their_literal_offsets` | 101 | `probes/P-04.*` |
| P-5 | `uiX` mask `0x3FF` → `0x7FF` (+1 bit) | `game::char_info::tests::every_bitfield_mask_stops_at_its_declared_width` | **0 — NEUTRAL** | `probes/P-05.*`, `P-05b.*`, `P-05c.*` |
| P-6 | loginmany walk stride forced to 592 for every record | `game::char_info::tests::login_many_walks_a_mixed_batch_by_each_records_own_stride` | 101 | `probes/P-06.*` |
| P-7 | region gate inverted | `game::world_entry::tests::the_region_gate_drops_a_mismatched_banner_without_storing_it` | 101 | `probes/P-07.*` |
| P-8 | `apply_login`'s `my_char_unique` gate removed | `game::world_entry::tests::an_opcode_5_record_for_another_character_is_not_adopted` **and** e2e `a_flipped_byte_in_the_opcode_5_record_is_refused_at_the_char_client_stage` | 101 / 101 | `probes/P-08a.*`, `P-08b.*` |
| P-9 | `is_in_world` ignores `village_struct_pending` | `game::world_entry::tests::entry_completes_only_when_banner_record_and_village_agree` | 101 | `probes/P-09.*` |
| P-10 | `Stage::LinkKey` mislabeled `Stage::GameLogin` (3 sites, `driver.rs:640-645`) | e2e `a_foreign_first_game_frame_is_refused_at_the_link_key_stage` (G-1) | 101 | `probes/P-10.*` |
| P-11 | `read_roster` bound `CHAR_SLOTS` → `CHAR_SLOTS - 1` | e2e `a_full_session_conversation_is_byte_exact_and_tears_down_cleanly` | 101 | `probes/P-11.*` |

13 of 14 probe runs failed their named test at exit 101. Every probe
compiled successfully before running (each log begins `Compiling` /
`Finished`), so no exit 101 is a build failure in disguise.

## Disclosures (honest findings)

### P-5 is neutral — a fixture gap, confirmed three ways

The `uiX` mask widening (`& 0x3FF` → `& 0x7FF`, `char_info.rs:616`) is
**not detected by any test in the suite**:

- `probes/P-05.log` — the named test passes (exit 0) with the mutation
  compiled in.
- `probes/P-05b-lib-wide-recheck.log` — the whole lib target passes:
  `ok. 301 passed; 0 failed; 1 ignored` (exit 0).
- `probes/P-05c-all-targets-recheck.log` — **all 334 tests pass** with
  the mutation applied (exit 0).

Root cause: the fixture never parks 1s in the two reserved bits above
`uiX`. `make_char_payload` builds the word as
`life_x = r.life | (u32::from(r.x) << 20)` (`src/game/char_info/tests.rs:137`)
and the `if reserved_bits_set` block (`tests.rs:141-149`) ORs reserved
bits into `type_kind`, `unique_level` and `y_map_speed` **only** — never
into `life_x` — although its own comment (`tests.rs:142-145`) claims
"the 2 bits above uiX" are parked. Bits 30-31 of the `uiLife|uiX` word
therefore stay 0, so widening the mask absorbs zeros and
`parsed.x == 0x3FF` holds under both masks. The test's *intent* (assert
the reserved bits cannot leak into `uiX`) is not exercised for this one
field; the other five assertions in it (level, life, total_move_speed,
make_char_kind, make_char_unique) are backed by genuinely parked bits.

Per Step 4's invariant **no fixture edits**, the fixture was *not*
changed to make the probe bite. Correcting `make_char_payload` to park
bits 30-31 of `life_x` is a follow-up recommendation, not part of this
step.

### T-2g hoist is known-neutral motion

Phase 2's scratch hoist has no failing probe: reverting
`let mut buf = [0u8; 4096]` back into the loop changes no observable
behavior (the decoder accumulates across reads either way). Its warrant
is the verbatim review against the Step 3 T-2 precedent
(`auth/front.rs:303-319`) plus the split-frame reassembly test — not a
mutation probe. Recorded here so the absence of a probe is not mistaken
for an oversight.

### `adopt_local` removal is a documented blind spot

Deleting `dispatcher.adopt_local(&char_client)` (`driver.rs:725`) passes
the whole suite: G-4 pins only the `DispatcherState::new` unique argument
(opcode 200 flips `Local` → `Unmatched` when *that* argument is wrong),
and nothing asserts `local_char()` after adoption. The call site is
protected only by T-1g's verbatim-motion checklist (counted: 1
`dispatcher.adopt_local` moved with the `game_phase` extraction). This is
the same blind spot Step 4's blueprint recorded; it remains open.

### The one non-verbatim token in T-1g

`game_phase` differs from the extracted `flow` body in exactly two lines:
`flow`'s doc sentence (rewritten) and the game-login call, which passed
`&auth` when `auth` was a local and passes `auth` now that the parameter
is already `&FrontAuth` — `clippy::needless_borrow` under
`-D warnings` denies the needless borrow. Documented in `game_phase`'s
own doc comment; no other byte moved (diff: 35 insertions, 2 deletions).

### Err-path log assertion (G-7)

`run_with_watcher` emits the terminal `session ended` record only for an
`Ok` outcome (`driver.rs:498-502`). G-7 therefore asserts the *absence*
of `teardown failed after an earlier failure` through
`assert_clean_aftermath` rather than the presence of the terminal record.

## Invariants held

- **F-07**: every expectation hand-placed with its legacy/server authority
  cited inline; no expected byte or ciphertext derived from the client's
  own parsers/encoders.
- **F-01**: every address introduced is loopback (`127.0.0.1:0` or the
  harness's own bound listener); no live server named.
- **No fixture edits**: Phase 1 changed test *harness* code only by
  **adding** mutation arms, constants and gated-seam plumbing
  (`Mutation::GameHandshakeForeign` / `ServerInfoRegion` /
  `VillageNegative`, `SERVICE_AREA_KOREA`, `server_info_payload_for_area`,
  `negative_village_struct_payload`, `ServerInfoGate`,
  `run_game_double_gated`, `spawn_game_gated`), each carrying its
  citation; existing fixtures and expectations were not modified.
- **One commit per phase**, forward-only (no rebase, no force-push; the
  branch has not been pushed yet).
- **Counts copied from `test result:` lines**, never hand-computed.
- **PowerShell/cmd capture** uses `cmd /v:on /c … !ERRORLEVEL!`.

## Committed-blob hashes (LF normalized — `git rev-parse HEAD:path`)

Base: `e6ee8a92b3df07c4781b1a2be0c8915e27216b0c` (main, Phase 0)
Head of the source work: `e5643f7` (Phase 4)

| Path | Blob |
|---|---|
| `src/client/driver.rs` | `6f6204cdcd6f8770c66c0d26b233118927047f48` |
| `src/client/driver/tests.rs` | `0247faceb2f997db7e06106f2ed2c74ea3786bf4` |
| `src/game/mod.rs` | `fb880b74d8b379aa89726c9bf521eccd65ba2ded` |
| `src/game/entry.rs` | `fc1ce321c1df4d47e8cae320e38bbc7e6fd9ef1d` |
| `src/game/test_support.rs` | `7ffa4141b0905edd61c8718374f4e52bdd2a1844` |
| `src/game/entry/tests.rs` | `f8eb9982688182cb837485ac84f0ec661192b738` |
| `src/game/login/tests.rs` | `010cc952ef14cbf13fb1a6b72f89096a678fab92` |
| `src/game/world_entry/tests.rs` | `6dfb47de10bc1bc616eb5504617592aa6b98ac1f` |
| `tests/e2e_loopback.rs` | `adf2b7ed0de99f9e0eea488db8d7c4016035342c` |
| `tests/e2e_loopback/frames.rs` | `8597bf3143fb7a54f8c5604093c2cc8c6c0d17b8` |
| `tests/e2e_loopback/game_double.rs` | `ef24e42940d776f6a03d9764445c067a1114df00` |
| `tests/e2e_loopback/robustness.rs` | `b63c838276a5b35e1c775226feefcd5970a8e43b` |

Hashes are over committed blobs (git's LF-normalized content), never the
CRLF working-tree bytes — the Step 3 §7 lesson.

## Commits

```
e5643f7 Step 4 Phase 4 (T-3g): one shared test_support for the game suites
5857d5e Step 4 Phase 3 (T-1g): extract the driver's game leg into `game_phase`
3175a2d Step 4 Phase 2 (T-2g): hoist the game link's read scratch out of the loop
383ba3b Step 4 Phase 1: characterize the game leg before touching it
```

## Layout

```
evidence/step4-refactor/
  baseline/   Phase 0: head, env, 5 gates, README
  phase1/     Phase 1 gates   phase4/  Phase 4 gates
  phase2/     Phase 2 gates   phase5/  final gates + 3 determinism runs
  phase3/     Phase 3 gates            + probes/P-*.log|.exit
  summary.md  this file
```
