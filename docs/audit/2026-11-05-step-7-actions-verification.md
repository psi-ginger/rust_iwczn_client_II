# Step 7 In-World Actions & Protocol Spec (Phases 1 & 2) — Verifier Sign-Off

**Date:** 2026-11-05
**Verifier (`psi-ginger`), QC/Verifier lane** — S1 satisfied: spec author and implementer are `goonzu-rs`, signer is `psi-ginger`.
**Target:** `origin/step/7.3-in-world-actions` @ **`50b0854`** (`50b08544b6199b06e642262443b8b6ba192dbcdb`)
**Base:** `main` @ `81427d2` (`81427d26d7746ed208ade3e1f9d9f90d5000966a`) — baseline 337 → **new baseline 352**
**Work Order:** `docs/agents/WO-step7-in-world-actions.md` **Rev 2.1** (§8 protocol, §8.3 goldens frozen, §15 advisories)
**Delivery commits under audit:** `9020be9` (Phase 1, +9 tests 337→346) · `50b0854` (Phase 2, +6 tests 346→352), both authored `goonzu-rs`

**Verdict: PASSED — FAST-FORWARD MERGE AUTHORIZED.** All five gates exit-0 on a clean checkout at `50b0854` with **352/352** green (5 ignored preserved); Goldens **A–I verified byte-exact** under independent recomputation; the two-stage gate and every typed refusal behave by name; opcode 241 is provably **report-only** (zero state mutation); the write guard holds exactly to WO §7; all **five §12 mutation probes** broke their named test with pristine restores. One non-blocking doc observation (O-1, §8).

## 1. Scope & method

Four checklist items: (1) clean-clone gate verification at `50b0854`; (2) wire/golden audit of Phase 1 encoders (A–G) and the Phase 2 parser (H/I) plus the implementer's disclosure; (3) invariant (T11) and write-guard check; (4) signed verdict report + FF confirmation. Method per standing rules: S4 clean clone, S5 prose is not evidence (every claim re-derived from the checkout, never from the WO's or the implementer's prose), F-07 (test expectations trace to §8.3 literals, never to the code under test), F-01 (loopback only).

**Clean checkout:** S4 clone, `git switch --detach 50b0854`, porcelain **clean**, `git diff HEAD` empty, HEAD == `origin/step/7.3-in-world-actions`, 7178 tracked files. Branch stack: `81427d2` → `e32f481` (WO Rev 2) → `c38895e` (pre-impl QA, APPROVED) → `fb507a6` (WO Rev 2.1) → `9020be9` (Phase 1) → `50b0854` (Phase 2).

## 2. The five mandatory gates (clean clone, this session)

| # | Gate | Command | Exit | Result |
|---|---|---|---:|---|
| 1 | check | `cargo check --all-targets` (`RUSTFLAGS="-D warnings"`) | **0** | 0 warnings |
| 2 | clippy | `cargo clippy --all-targets -- -D warnings` | **0** | 0 warnings |
| 3 | fmt | `cargo fmt --check` | **0** | 0 diffs |
| 4 | test | `cargo test` | **0** | **352 passed, 0 failed, 5 ignored** |
| 5 | doc | `cargo doc --no-deps` | **0** | 0 warnings |

**Test composition (from the run, not from prose):**

| Suite | Count | Baseline `main` | Delta |
|---|---:|---:|---:|
| lib (`src/lib.rs`) | **319** passed (+1 ignored) | 304 | +9 (Phase 1) +6 (Phase 2) |
| `asset_fixtures` | **11** | 11 | 0 |
| `e2e_loopback` | **22** | 22 | 0 |
| `live_integration` | 0 (4 ignored) | 4 ignored | 0 |
| **Total passed** | **352/352** | 337 | **+15** |
| Ignored preserved | **5** (1 operator demo + 4 live) | 5 | 0 |

Dispatcher suite: **26 → 32** `#[test]` functions (+6, all pre-existing unchanged — WO §7's "additive only" clause). **Determinism:** a second full `cargo test` re-run reproduced 319/11/22 = 352 with 0 failures.

## 3. Wire protocol & golden frames A–I — 10/10 byte-exact

Each golden was **rebuilt from first principles** in an independent script (header = `usSize LE | 0x00 | usCmd LE`, payload assembled field-wise from the §8.3 inputs) and compared to the literal asserted in the tests. Neither the WO's table, the encoder, nor the parser was used to derive an expectation (F-07).

| Golden | Test that pins it | Independent build | Literal match | `usSize`==len | cmd |
|---|---|---|:---:|:---:|---:|
| A Move | `move_order_is_golden_a_byte_for_byte` (T2) | `0A 00 00 F0 00 00 64 00 C8 00` | ✅ | 10==10 | 240 |
| B Attack | `attack_and_sitdown_are_goldens_b_and_c` (T3) | `0A 00 00 F0 00 01 39 30 00 00` | ✅ | 10==10 | 240 |
| C SitDown | same (T3) | `06 00 00 F0 00 02` | ✅ | 6==6 | 240 |
| D Chat | `chat_payload_is_golden_d_mode_then_utf8_text` (T5) | `08 00 00 75 00 01 68 69` | ✅ | 8==8 | 117 |
| E Quit | `quit_front_warp_and_npcpos_match_goldens_e_f_g` (T7) | `05 00 00 6B 00` | ✅ | 5==5 | 107 |
| E Front | same (T7) | `05 00 00 6D 00` | ✅ | 5==5 | 109 |
| F Warp | same (T7) | `07 00 00 92 00 03 00` | ✅ | 7==7 | 146 |
| G NpcPos | same (T7) | `07 00 00 56 14 05 00` | ✅ | 7==7 | 5206 |
| H 241 Move | `golden_h_move_and_i_sitdown_parse_from_the_frozen_frames` (T8) | `0E 00 00 F1 00 39 30 00 00 00 64 00 C8 00` | ✅ | 14==14 | 241 |
| I 241 SitDown | same (T8) | `0A 00 00 F1 00 39 30 00 00 02` | ✅ | 10==10 | 241 |

**10/10 byte-identical; every `usSize` equals the actual frame length; every `ucChk` is 0.** Frame construction rides the shared `frame` builder (`src/game/test_support.rs:28-36`), which encodes the header contract (`usSize = 5 + payload.len()` LE, `ucChk = 0`, `usCmd` LE) — it is the *header* authority, not the encoder under test, and the payload half of every assertion is a hand-placed §8.3 literal. Phase 1 additionally proves the goldens **through the session cipher** (T14: encode → wire → decode recovers golden A and D bit-exact, keys ticking per frame) and the **pre-handshake gate** (T13: `NotHandshaken`, zero bytes on the socket, outbound cipher never keyed).

**Payload-size/`usSize` arithmetic** re-derived: 5+5=10, 5+1=6, 6+2=8, 5+0=5, 5+2=7, 5+9=14, 5+5=10 — consistent for all rows, and `usSize == 6 + text.len()` is asserted for chat (D2: no `siDataSize`).

## 4. Phase 2 parser: two-stage gate, refusals, disclosures

`parse_response_order` (`dispatcher.rs:686-726`) — verified structurally and by test:

- **Stage 1:** opcode gate first (`UnexpectedOpcode`), then the 5-byte prefix (`Truncated { expected: 5, got: 4 }`).
- **Kind before length (D8):** a reserved kind returns `UnknownOrderKind { kind: 200 }` **before any length pass** — proven at *both* lengths a known kind would gate (5 B → kind error, not `Truncated{9}`; 10 B → kind error, not `TrailingBytes{9}`), through `parse_response_order` *and* through `dispatch`, with state bit-identical after the refusal (T9).
- **Stage 2:** exact per-kind length through the shared `gate_frame` (`gate.rs:51-78`: opcode → `Truncated` → `TrailingBytes`): Move 9 B (`Truncated{9,8}`, `TrailingBytes{9,10}`), SitDown 5 B (`TrailingBytes{5,6}`) — all asserted **by name** (T8).
- **Outbound typed refusals (D6/D7):** `EmptyChat`, `ChatTooLong { len: 256, max: 255 }`, the inclusive 255-byte boundary accepted, and a byte-based case (128 × `é` = 256 UTF-8 bytes → refused whole, never truncated) — T6.

**Disclosure review (Golden H/I + auxiliary Attack-echo): ACCEPTED.** The implementer disclosed — in the test's own comment (`dispatcher/tests.rs:1065-1068`) and in the `50b0854` commit subject — that the task descriptor's Attack-echo case (char 1001, target 12345) is **not** Golden I: frozen §8.3-I is the SitDown echo, and §8.4.3/§15 require a WO revision to edit a golden. They therefore did *not* bend the golden; they added a separate test asserting the Attack echo against the §8.2 layout, hand-built byte by byte (`E9 03 00 00 | 01 | 39 30 00 00` = char 1001, kind 1, target 12345, 9 B — independently decoded and confirmed present). Golden I itself still parses `39 30 00 00 02` → `Order::SitDown`. The WO's §8.3 is byte-identical to `e32f481` (the only WO deltas on this branch are the Rev 2.1 header + §15, `fb507a6`). Correct handling of the frozen-golden rule; no finding.

## 5. Invariants: zero mutation on 241 (T11) and position authority

- **Code:** the `RESPONSE_ORDER` arm (`dispatcher.rs:1012-1029`) performs only immutable reads — `echo.char_unique == self.my_char_unique` and `self.nearby.contains_key(...)` — then constructs `DispatchOutcome::Ordered`. No `get_mut`, no `insert`, no field write exists on that path (contrast the 200/215/6/11 arms, which do mutate).
- **Test (T11, `an_order_echo_never_writes_state`):** the whole `DispatcherState` (local avatar, `LocalDelta`, `WorldChar` table) is `clone()`-compared **bit-identical** before/after three echo shapes — Golden H's absent-character Move, a Move naming the *local* avatar, and a SitDown naming a nearby entry — and the local avatar's coordinates remain `(10, 20)` after the Move echo. Position authority therefore stays **opcode 200** (WO §3.2).
- **Companion:** a refused echo (unknown kind) also leaves the state identical (T9). No outbound 200/position-upload exists anywhere in `action.rs` (grep: none), and opcode 267 is unimplemented (only two hex-fixture substrings match "267" in `net/cipher/fixtures.rs`).

## 6. Write guard (WO §7)

Per-commit scope, then a path sweep against `main`:

| Commit | Files | §7 row |
|---|---|---|
| `9020be9` Phase 1 | `src/game/action.rs` (A), `src/game/action/tests.rs` (A), `src/game/mod.rs` (M) | ✅ all three |
| `50b0854` Phase 2 | `src/game/dispatcher.rs` (M), `src/game/dispatcher/tests.rs` (M) | ✅ both, "additive only" |

- **`git diff origin/main...HEAD` on every READ-ONLY/untouched path is empty:** `src/lib.rs`, `src/{client,net,protocol,auth,graphics}/**`, `src/game/{entry,login,char_info,world_entry,gate,test_support}.rs`, `tests/**`, `Cargo.{toml,lock}`, `README.md`, `TASK_MATRIX.md`, `CONTRIBUTING*.md`, `GoonZu_Source/**`, `evidence/**`.
- **Additive-only proof for `dispatcher.rs`:** the only modified existing line is the `use` import, *extended* (`…i64_at` → `…i64_at, u16_at, u32_at`) with the original items intact; all 15 removed lines are module-doc truth updates ("five" → "six opcodes in scope, five of which write", the 241 row added to the opcode table). Existing arms, `ApplyTarget`, and every pre-existing assertion are untouched; 26 → 32 tests, +6 only.
- **`mod.rs`:** +4/0 — `pub mod action;` plus one doc bullet, exactly the §7 row. No driver/net/matrix leakage; `Cargo.toml` untouched (no new dependency).

## 7. WO DoD 1–8

| # | DoD item | Result |
|---|---|---|
| 1 | Five gates exit-0 at 337+N, fmt 0, doc 0, clean tree | ✅ N=15 → **352**, all exits 0, tree clean |
| 2 | §8.3 goldens A–I byte-exact; T1–T14 present and green | ✅ 10/10 (§3); inventory complete (§7.1) |
| 3 | D1–D9 each pinned by a fail-if-corrected test | ✅ mapping in §7.2 |
| 4 | Five §12 mutation probes break their named test, pristine restores | ✅ 5/5 (§9) |
| 5 | Write guard holds | ✅ §6 |
| 6 | F-07: expectations trace to §8.3 / legacy semantics, never the code under test | ✅ literals hand-placed; `frame` = header authority; refusals asserted by variant name |
| 7 | §8.4 mirror checklist recorded (or gap filed) | ✅ §8.4 present in the WO; advisories Q-1..Q-4 recorded as §15 (Rev 2.1) at `fb507a6`; Q-1 in force at mirror sign-off |
| 8 | No external network, no position upload, no 267, no simulation | ✅ new I/O tests bind `local_listener` (127.0.0.1:0) only; no 200 send; no 267; live tests remain ignored/evidence-only |

### 7.1 Test inventory (T1–T14)

- **Phase 1 (9):** T1 constants · T2 golden A · T3 goldens B/C · T4 LE round trip · T5 golden D · T6 typed chat refusals · T7 goldens E/F/G · T13 pre-handshake send gate · T14 cipher round trip.
- **Phase 2 (6):** T8 golden H/I parse · *auxiliary* Attack-echo (disclosed, §4) · T8 two-stage refusals by name · T9 unknown-kind-before-length · T10 target triple · T11 report-only.
- **T12** is the suite-level regression property (all 337 pre-existing tests unchanged and green; `other => Unhandled` at `dispatcher.rs:1030` still holds — `a_foreign_opcode_is_unhandled_not_an_error` green), not a dedicated function. §10's single T8 invariant is deliberately split across the parse-half and refusal-half functions (both labelled T8). Full coverage of T1–T14: **yes**.

### 7.2 D1–D9 → pinning tests

D1 opcodes → T1 · D2 header-only/no `siDataSize` → T5 (+T8 frame) · D3 kind tag/reserved refused → T3+T9 · D4 no padding/transmute, `System=8` folded → T4+T1 · D5 clean LE word pair → T4 · D6 refusals never clamp → T6 · D7 strict UTF-8 → T6 (`é` case) · D8 two-stage + report-only → T8/T9/T10/T11 · D9 second sources → §8.3 literals in tests (F-07).

## 8. Observations (non-blocking)

- **O-1 (doc-only).** `src/game/mod.rs:20-21` still calls the dispatcher "the inbound router for the **five** in-world opcodes (6, 11, 118, 200, 215)" while the dispatcher now routes six (241 added; its own module docs were correctly updated to "six opcodes in scope, five of which write", and `README.md:104-108` — Custodian-owned, untouched — carries the same pre-Step-7 list). The WO §7 row constrained `mod.rs` to `pub mod action;` + one doc bullet *additive only*, so the implementer could not touch that line without leaving the permitted file plan. Zero gate/test impact; recommend the Custodian fold a one-line correction into the next WO touch or the matrix-row append. **Not a blocker.**

No byte, size, opcode, golden, or behaviour discrepancy was found anywhere in Phases 1–2.

## 9. Mutation probes (WO §12, Adversary equivalent) — 5/5

Each probe applied a single semantic break, ran the named test, then restored via `git checkout --` with a clean-tree check after every step:

| # | Mutation | Named test(s) | Outcome |
|---|---|---|---|
| 1 | Swap Move's x/y words in `encode_order` | T2 `move_order_is_golden_a_byte_for_byte` + T4 `move_coordinates_survive_a_little_endian_round_trip` | **both FAILED** (exit 101) — golden A payload and the D5 offset assertion fired |
| 2 | Re-introduce a `siDataSize` prefix on chat | T5 `chat_payload_is_golden_d_mode_then_utf8_text` | **FAILED** — §8.3-D payload assertion |
| 3 | Make the 241 gate fixed-length (SitDown → 9 B) | T8 `the_two_stage_length_gates_refuse_by_name` | **FAILED** — `TrailingBytes{5,6}` case (`tests.rs:1153`) |
| 4 | Shift `OrderKind` (`Move=1, Attack=2, SitDown=3`) | T1 `constants_match_the_wire_truth` + T3 `attack_and_sitdown_are_goldens_b_and_c` | **both FAILED** — kind-0 and golden-B assertions |
| 5 | Resolve the local 241 target as `Unmatched` | T10 `an_order_echo_resolves_the_target_triple` | **FAILED** — "the local unique resolves Local" |

Restores: **0 dirty files after every probe; final porcelain clean and `git diff HEAD` empty** (tree byte-identical to `50b0854`).

## 10. Verdict & merge recommendation

**PASSED — FAST-FORWARD MERGE AUTHORIZED.**

- **Commit SHA under sign-off: `50b08544b6199b06e642262443b8b6ba192dbcdb`** (branch tip; local == origin).
- **FF confirmed:** `merge-base(branch, main) == main` tip `81427d2`; `git rev-list HEAD..origin/main` = **0** commits (branch is 5 commits ahead: `e32f481`, `c38895e`, `fb507a6`, `9020be9`, `50b0854`); `git merge-tree` writes a tree with no conflicts.
- **Recommendation to the Custodian:** fast-forward `main` from `81427d2` to `50b0854`, then re-run the post-merge gates on the merged tree (`cargo fmt --check` exit 0 and a full `cargo test` at **352/352**, per the Step 6 post-merge pattern). The §12 `evidence/7.3/` standing bundle (head/check/clippy/fmt/test/doc logs + `SHA256SUMS`) can be cut from this session's gate transcripts on request; this report reproduces every gate result and count needed to audit them.
- **Mirror note:** Phase 3 (driver wiring) remains a separate WO per §10; the `goonzu-rs` mirror checklist (§8.4) must not be signed off without **Q-1** in force (§15, DoD 7).

**Sign-off:** `psi-ginger` · Verifier lane · 2026-11-05 · evidence: this report's commands and file/line reads, all reproducible from `50b0854` in a clean clone.
