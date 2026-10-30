# evidence/7.1 - Adversarial mutation probes (Verifier S4, clean clone @ 2c0964e)

Each probe mutates the decoder, runs the named test subset, records the loud
deterministic failure, and reverts (clone verified pristine after all probes).

## Probe 1 - break SPR sentinel (COLOR_TRANS 254 -> 255)
exit 101. Named tests FAILED:
  - graphics::spr::tests::mask_spr_stream_consumes_exactly_1760_and_covers_exactly_3200
  - graphics::spr::tests::constants_match_the_cited_header (panicked left:255 right:254)
  - graphics::spr::tests::mask_spr_transparent_runs_are_separate_from_literals
  - graphics::spr::tests::r1_isize_is_exposed_but_never_a_decode_bound
Restored clean.

## Probe 2 - swap map body order (IdMap <-> CollMap read sequence)
exit 101. Named test FAILED:
  - graphics::map::tests::truncated_stream_is_refused_at_the_named_section
    assertion 'cut at 190 bytes must refuse at ID map':
      left:  Truncated { section: CollMap, needed: 24, got: 14 }
      right: Truncated { section: IdMap,   needed: 24, got: 14 }
Restored clean.

## Probe 3 - break 565 green packing (<< 5 -> << 6 at palette.rs:122)
exit 101. Named tests FAILED:
  - graphics::palette::tests::pal_565_conversion_of_known_indices_is_byte_exact (left:64)
  - graphics::palette::tests::r_green_channel_is_not_right_shifted_in_565
  - graphics::palette::tests::every_entry_is_converted_not_just_the_first
  - graphics::palette::tests::mode_selects_the_packing_of_the_cited_lines (left:56795 right:57083)
Restored clean.

## Determinism
Full cargo test re-run after all reverts: 289/289 green, clone porcelain clean.
