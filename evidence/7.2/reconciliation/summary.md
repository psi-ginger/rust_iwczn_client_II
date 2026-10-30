# evidence/7.2/reconciliation - merge-topology reconciliation (Custodian/Lead, under directive)

The Step 7.2 Verifier session was interrupted (quota exhaustion) after technical
verification and operator on-screen confirmation but before the merge-topology
reconciliation. This bundle records the reconciliation executed by the
Custodian/Lead under the directive of 2026-10-30 ("Pull Step 7.2, Reconcile
Topology & Merge to Main").

## Topology before reconciliation

Divergence from merge-base `2602e13` (two lineages, no shared tip):

- `origin/main` @ `60d38dd` - Step 7.1 S4 sign-off only (matrix row 7.1 PASSED,
  WO-7.1 Verifier verdict, `evidence/7.1/**`).
- `origin/step/7.2-windowing` @ `4005fcd` - the full 7.1 implementation lineage
  (`078ecd0` + `2c0964e` + `1601b26`) plus the 7.2 lineage (`b582896` WO-7.2,
  `cac0ffb` directive amendment, `2f36a0f` render loop, `d6c0600` delivery note,
  `4005fcd` operator-feedback fix). Never touched `TASK_MATRIX.md`.

## Reconciliation performed

- Merge (not rebase) of `origin/step/7.2-windowing` into `main`, `--no-ff`,
  merge commit `0e86e3a` (parents `60d38dd` + `4005fcd`). A merge preserves
  both lineages and every SHA cited by the sign-off record; a rebase would
  have orphaned the `2c0964e`/`4005fcd` citations in the matrix and WOs.
- Auto-merged without conflict (`ort` strategy). `docs/agents/WO-7.1.md` kept
  **both** section-9 return blocks (Implementer delivery note + Verifier
  verdict); `docs/agents/WO-7.2.md` and `evidence/7.2/mutation-probes.md`
  arrived intact; matrix row 7.1 (PASSED @ `2c0964e`) survived unmodified.
- **Code identity:** `git diff 4005fcd 0e86e3a -- src tests Cargo.toml Cargo.lock`
  is empty - the merge changed docs/evidence only; the reconciled tree runs the
  exact delivered 7.2 code.

## Five mandatory gates at merge commit `0e86e3a` (logs here)

| gate | exit | log |
| :--- | :--: | :--- |
| `cargo check --all-targets` | 0 | check.log |
| `cargo clippy --all-targets -- -D warnings` | 0 | clippy.log (0 warnings) |
| `cargo fmt --check` | 0 | fmt.log (0 diff lines, 0 bytes) |
| `cargo test` | 0 | test.log - **324/324** = 299 lib + 11 asset_fixtures + 14 e2e; 5 ignored (1 operator demo test + 4 live_integration) |
| `cargo doc --no-deps` | 0 | doc.log (0 warnings) |

Counts copied verbatim from the `test result:` lines of `test.log`.

## Adversary probes re-verified post-merge (Custodian/Lead)

The five WO-7.2 §8 mutation probes (originally run by the Implementer and
recorded in `evidence/7.2/mutation-probes.md` at the delivery SHA) were
re-executed independently at the merge commit, each: mutate -> named test fails
exit-101 -> restore -> tree byte-clean:

| probe | mutation | named failing test | exit |
| :--- | :--- | :--- | :--: |
| (a) | `blit.rs` pitch advance `dest_y * width` -> `dest_y` | `pitch_advance_lands_each_row_on_its_own_scanline` | 101 |
| (b) | `blit.rs` color key removed (`None` writes idx 254) | `transparent_cells_never_write` | 101 |
| (c) | `draw_map.rs` unique-90 layer arms swapped | `unique_90_swaps_layer_1_for_layer_0` | 101 |
| (d) | `present.rs` crop origin `+80` -> `+160` | `crop_is_the_one_tile_border_for_every_mode` | 101 |
| (e) | `tick.rs` drain bound `0..10` -> `0..11` | `at_most_ten_messages_drain_per_tick` | 101 |

After all five restores: `git status` clean, no `PROBE` markers in `src/`, and
the full suite re-ran 324/324 again (deterministic restoration).

## Operator confirmation

The operator (PsiGinger) visually confirmed Step 7.2 on screen at `4005fcd`
(the flat-ochre demo-scene defect found during that session was fixed in
`4005fcd` itself - see the WO-7.2 section-9 operator-feedback fix record;
the fixed scene renders a two-color diamond checkerboard with the real
`Mask.Spr` overlay at crop center, verified by headless frame dump + BMP).

## Non-claims

This is not a Verifier S4 clean-clone bundle (the Verifier session did not
complete; precedent `evidence/7.1/` covers the identical 7.1 code lineage and
`evidence/7.2/mutation-probes.md` covers the probes at the delivery SHA). It
is the Custodian/Lead reconciliation record: same-tree gate re-run, independent
post-merge probe re-verification, and the topology/merge audit above.
