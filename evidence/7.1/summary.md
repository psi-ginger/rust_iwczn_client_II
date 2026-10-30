# evidence/7.1 - Verifier independent reproduction (Step 7.1 sign-off)

- **Baseline verified:** clean clone from origin at delivery commit 2c0964e
  (branch step/7.1-assets; branch tip 1601b26 is the docs-only delivery note;
  git diff 2c0964e 1601b26 -- src tests Cargo.toml empty -> identical code).
  Merge-base with main = 2602e13 (exactly main head). Tree clean at checkout.

## Five mandatory gates (S4 clean clone, independent of Implementer env)

| gate | exit | log |
| :--- | :--: | :--- |
| RUSTFLAGS=-D warnings cargo check --all-targets | 0 | check.log |
| cargo clippy --all-targets -- -D warnings | 0 | clippy.log |
| cargo fmt --check | 0 | mt.log (0 diff lines) |
| cargo doc --no-deps | 0 | doc.log (0 warnings) |
| cargo test | 0 | 	est.log - **289/289** = 264 lib + 11 asset_fixtures + 14 e2e; 4 live_integration ignored; +60 over the 229 baseline |

Deterministic re-run after all mutation probes reverted: 289/289 again.

## Adversarial mutation probes (Adversary lane equivalent) - see mutation-probes.md

Three decoder-breaking mutations, each fails a NAMED test loudly and exit-101,
tree restored clean each time:
1. SPR sentinel 254->255 => mask_spr_stream_consumes_exactly_1760_and_covers_exactly_3200 (+3) FAIL.
2. Map body-order swap   => 	runcated_stream_is_refused_at_the_named_section FAIL (CollMap vs IdMap).
3. 565 green shift       => pal_565_conversion_of_known_indices_is_byte_exact (+3) FAIL.

## DoD acceptance (WO-7.1 section 10)

- **10.2 Sprite (real Mask.Spr):** byte-exact - header ver=10 enc=0 80x40 iImageNum=1;
  stream 1760/1760 consumed; canvas 3200/3200; iSize=224 exposed but never a bound (R1);
  SPR/MPR separately tested. Verifier's own independent byte-walk (Phase III setup, pre-code)
  reproduces 1760/3200/1600-literal/80-run exactly -> F-07 (no circularity) satisfied.
- **10.3 Palette (real monarch.pal):** exactly 768 B accepted; 767/769 refused by name
  (WrongSize{..}); 565 known-index byte-exact; green asymmetry pinned (no >>1 on green).
- **10.4 Map (synthetic, provenance file:line):** decodes in MapManager.cpp:687-703 order;
  13 truncation cuts refused at the NAMED section; R3 MBCS 176-B header.
- **10.5 Archive (synthetic):** member-first/loose-fallback matches the .sz/.mz legacy seam;
  extracted bytes decode through load_sprite.
- **Quirk ledger R1-R5:** each pinned by a failing-if-corrected test (probes 1-3 prove the
  mechanism; R5 pinned by 5_guard_is_eq_400_not_ge_400).
- **Headless:** no GPU/window/network; decode is pure byte-in/value-out.
- **Write guard:** outside src/graphics/**, the delivery touches only src/lib.rs (+1 line
  pub mod graphics;), src/protocol/{bytes.rs,mod.rs} (the D1+C1/C2 refactor the Custodian
  approved at 7.0 sign-off), and 2 doc-link comment retargets in dispatcher.rs needed to keep
  cargo doc warning-free after D1 removed the Frame import. net/client/e2e/Cargo untouched.

## WO-7.1 prose defects adjudicated (Implementer's honesty confirmed)

Two defects the Implementer flagged in the WO I authored are REAL and were correctly
implemented-to-source, not to my defective prose:
- 6.1 header: real eserved[8] + ImageInfo[256] = 3136 B fixed (TSpr.h:32-47); my WO said
  'reserved[7] ... 28-byte-fixed' - wrong. Source read + fixture math confirm 3136.
- R5: real guard is == 400 (TSpr.cpp:74) with the palette read AFTER the image (L66 then
  L74/L81), layout header|image|pal; my WO said >=400 skip-before-image - wrong.
Both are pinned by failing-if-corrected tests (quirk ledger). The WO prose should be corrected
in a future docs commit (Scribe lane); it does not block the code, which matches legacy truth.