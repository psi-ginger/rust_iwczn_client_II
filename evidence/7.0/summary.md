# evidence/7.0 - Verifier independent reproduction (Step 7.0 sign-off)

- **Baseline verified:** main @ 592befc (fast-forward from ee44877), tree clean, in sync with origin/main.
- **Audited-code provenance:** git diff 606da1a..592befc -- src tests Cargo.toml Cargo.lock is empty (production tree identical to the 225/225 Phase II record); committed src/ at HEAD is byte-identical to the audited ee44877 baseline (git diff ee44877..592befc -- src empty). Only 3 docs files differ in the range.

## Five mandatory gates (re-run by Verifier at 592befc; logs here)

| gate | exit | log |
| :--- | :--: | :--- |
| RUSTFLAGS=-D warnings cargo check --all-targets | 0 | check.log |
| cargo clippy --all-targets -- -D warnings | 0 | clippy.log |
| cargo fmt --check | 0 | mt.log (0 diff lines) |
| cargo doc --no-deps | 0 | doc.log (0 warnings) |
| cargo test | 0 | 	est.log - **225/225** (211 lib + 14 e2e; 4 live_integration ignored) |

## Independently reproduced audit numbers (S5)

- Inventory: **41 files / 18,229 lines** = 8,524 production (23) + 9,705 test-support (18). Exact match to report.
- Files >700 prod lines: dispatcher 1076, world_entry 832, char_info 798, driver 785, codec 737. Exact match.
- #[cfg(test)] census: 18 pointer-style (mod tests;/mod fixtures;) + **1 inline** (src/lib.rs L40, single 10-line test). Exact match; no hidden-test bloat.
- No 	odo!/unimplemented!/#[allow] in any production file (no-stub invariant holds).

## Independently corroborated findings

- **D1 dispatch_frame (dispatcher.rs:1068) genuinely dead** - symbol occurs exactly once in all .rs (its own decl); body is a 2-line delegator to state.dispatch (real entry dispatcher.rs:897). Zero callers. Confirmed.
- **C1 i16_at/i32_at/i64_at triplicated** at exactly char_info.rs:246/255/264, dispatcher.rs:424/429/439, world_entry.rs:379/388/397. Confirmed.
- **C2 le_u16/le_i16/le_i32/le_i64/cstr duplicated** across auth/login.rs:400-422 and auth/world_list.rs:351-363. Confirmed.
- **C3 copy_credential duplicated** auth/login.rs:251 + game/login.rs:181. Confirmed.
- **C4 gate_frame duplicated** dispatcher.rs:462 + world_entry.rs:407 (distinct error enums - C4's "share core, not blind dedup" caveat is correct). Confirmed.

## Notes
- pub-surface count: report states 264 (fn/struct/enum/const/type); a looser scan including use/mod re-exports yields 289. Classification-methodology difference on the **optional** S1 finding; not load-bearing and not a contradiction.
