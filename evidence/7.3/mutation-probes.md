# Step 7.3 mutation probes - re-run on `main` @ `2436a54` (2026-11-05)

Each probe applies one semantic break to the pristine tree, runs the WO-named test, records the failure, and restores with `git checkout --`; porcelain is checked clean after every probe. All five must fail their named test (Adversary lane, `CONTRIBUTING_PHASE3.md` multi-agent model; `docs/agents/WO-step7-in-world-actions.md` section 12). Baseline before/after: 352/352.

Clone: S4 clean clone at `2436a54` (tree identical to the verified delivery `eb54f55`).

## Probe 1 - swap Move x/y word order in `encode_order`

- Target: `src/game/action.rs` (`encode_order`, Move arm)
- Named tests: T2 `move_order_is_golden_a_byte_for_byte` + T4 `move_coordinates_survive_a_little_endian_round_trip`
- Result: exit 101 (expect 101)
  - `test game::action::tests::move_coordinates_survive_a_little_endian_round_trip ... FAILED`
  - `test game::action::tests::move_order_is_golden_a_byte_for_byte ... FAILED`
  - `assertion `left == right` failed: D5 — x at bytes 1..3, y at 3..5, both LE, for (100, 200): a word swap or the legacy superword packing changes these bytes`
  - `assertion `left == right` failed: WO §8.3-A payload: kind 00, x=0x0064 LE, y=0x00C8 LE (5 B)`
  - `test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 318 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
- Restore: probe1 : porcelain CLEAN

## Probe 2 - re-introduce a legacy `siDataSize` prefix on chat

- Target: `src/game/action.rs` (`encode_chat`, payload builder)
- Named test: T5 `chat_payload_is_golden_d_mode_then_utf8_text`
- Result: exit 101 (expect 101)
  - `test game::action::tests::chat_payload_is_golden_d_mode_then_utf8_text ... FAILED`
  - `assertion `left == right` failed: WO §8.3-D payload: mode 01, then 'h' 'i' (3 B)`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 319 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
- Restore: probe2 : porcelain CLEAN

## Probe 3 - make the 241 length gate fixed (SitDown -> 9 B)

- Target: `src/game/dispatcher.rs` (`parse_response_order`, stage-2 per-kind length)
- Named test: T8 `the_two_stage_length_gates_refuse_by_name`
- Result: exit 101 (expect 101)
  - `test game::dispatcher::tests::the_two_stage_length_gates_refuse_by_name ... FAILED`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 319 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
- Restore: probe3 : porcelain CLEAN

## Probe 4 - shift `OrderKind` discriminants (Move=1, Attack=2, SitDown=3)

- Target: `src/game/action.rs` (`OrderKind` enum)
- Named tests: T1 `constants_match_the_wire_truth` + T3 `attack_and_sitdown_are_goldens_b_and_c`
- Result: exit 101 / 101 (expect 101 / 101)
  - `test game::action::tests::constants_match_the_wire_truth ... FAILED`
  - `assertion `left == right` failed: §8.2/D3 — kind 0; CommonHeader.h:519 ORDER_MOVE(4) semantics inherited`
  - `test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 312 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
  - `test game::action::tests::attack_and_sitdown_are_goldens_b_and_c ... FAILED`
  - `assertion `left == right` failed: WO §8.3-B payload: kind 01, target 12345 = 0x00003039 LE (5 B)`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 319 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
- Restore: probe4 : porcelain CLEAN

## Probe 5 - resolve the local 241 echo target as `Unmatched`

- Target: `src/game/dispatcher.rs` (241 arm, local-target resolution)
- Named test: T10 `an_order_echo_resolves_the_target_triple`
- Result: exit 101 (expect 101)
  - `test game::dispatcher::tests::an_order_echo_resolves_the_target_triple ... FAILED`
  - `assertion `left == right` failed: the local unique resolves Local`
  - `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 319 filtered out; finished in 0.00s`
  - `error: test failed, to rerun pass `--lib``
- Restore: probe5 : porcelain CLEAN

## Post-probe pristine check

- `git status --porcelain` CLEAN - tree byte-identical to the gated `2436a54`.
- All five probes failed their named test; restores verified clean after each.
