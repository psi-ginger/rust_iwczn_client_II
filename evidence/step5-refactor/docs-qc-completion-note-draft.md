# DRAFT — `docs(qc)` completion note for `main` (pending Verifier sign-off)

**Status: draft only.** To be committed as the short `docs(qc)`
completion note on `main` *immediately after* the Custodian's advance,
per `CONTRIBUTING_PHASE2.md:256-259`, and only once the Verifier's
signed verdict exists at
`docs/audit/2026-11-04-step-5-refactor-verification.md`. The Verifier's
merge-skew `fmt` + `test` re-run on `main` follows. Nothing here has been
committed; the Implementer does not file `docs(qc)` verdicts.

Proposed commit message:

```
docs(qc): Step 5 structural refactoring - evidence evidence/step5-refactor/

Custodian fast-forward of main to the branch tip on
refactor/step5-structure (delivery SHA preserved as parent; no merge
commit, no squash - full lane history kept). Verifier verdict: see
docs/audit/2026-11-04-step-5-refactor-verification.md.

Implementer record (re-derived by the Verifier at sign-off): all 5
gates exit 0 at every boundary (32 gate .exit files, all EXIT=0); test
count trajectory 334 -> 336 (Phase 1 added the adopt_local wiring test
+ its behavioral counterpart) and held 336/336 through Phase 5's three
determinism runs (5 ignored preserved: 1 lib + 4 live_integration).
Phases: 0 baseline 943679d, 1 characterization 0f52ca2, 2 T-2e survey
N/A 5ef7dc4, 3 T-1e in_world_phase byte-identical extraction 7251cbc
(40-line body SHA256 equal both sides), 4 T-3e client test_support
consolidation d06bf63 (+ evidence prose fix df316a3), 5 evidence
bundle (this note's source). Phase 5: 4 adversary probes - adopt_local
wiring, stage label of the moved in_world_phase body, dispatcher
routing and fall-through boundaries - all 4 fail their named test at
EXIT=101 under mutation and restore the tree byte-identically; blob
hash table 10/10; citations across the 10 changed files 48 -> 49
(+1, no file lost); F-07 intact (local_listener/HANDSHAKE_PAYLOAD/
SECRETKEY_FRAME each defined once tree-wide, removed literals
byte-identical to their donors); Cargo.toml/Cargo.lock untouched.

Documentation correction carried from Step 4 verdict section 8: Step 4's
summary.md prose says "13 of 14 probe runs failed their named test at
exit 101" - the correct count is 11 of 14 (P-01..P-04, P-06..P-11
incl. P-08a/P-08b; the P-05 trio exited 0 as disclosed). The Step 4
probe table itself was accurate; only that sentence miscounts, and no
technical claim depends on it. Step 4 section 8's stale "not pushed"
prose is likewise noted (refactor/step4-structure is on origin).

Evidence: evidence/step5-refactor/ (summary.md, baseline/, phase1-5/,
probes P-01..P-04 with mutation diffs + restore proofs, drafts).

Step: 5 refactor
Lane: custodian
```
