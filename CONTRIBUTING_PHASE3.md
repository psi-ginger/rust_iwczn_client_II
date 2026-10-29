# GoonZu Client Rust — Phase 3 Governance (Step 7 onward)

> **Scope of this document.** `CONTRIBUTING.md` governs Steps 1.1–5.4 (the Phase II
> micro-step catalog — a closed, audited record of delivered work). `CONTRIBUTING_PHASE2.md`
> governs Step 6 (end-to-end orchestration — also closed and audited). **This document governs
> Step 7.1 onward — Phase III: Asset Loading & Client Rendering**, which ports the classic
> on-disk asset decoders (sprite / map / palette / archive) and the windowed 2D render loop
> from the legacy C++ tree (`GoonZu_Source/`) into Rust. It was established 2026-10-29 by the
> Custodian's Phase III kickoff directive, on the signed-off Phase II baseline
> (`origin/main` @ `606da1a`, all 27 matrix rows `PASSED`, zero open findings —
> `docs/audit/2026-10-29-phase-2-final-sign-off-report.md`).

---

## Inherited verbatim from `CONTRIBUTING.md` / `CONTRIBUTING_PHASE2.md`

The following apply **unchanged** to every Phase III step. The two earlier documents are
canonical; this list is the pointer, not a fork:

1. **Core philosophy** — zero-evasion, precision micro-steps; zero tolerance for stubbing,
   fake mocks, or unverified loops (`CONTRIBUTING.md`, philosophy section).
2. **The five mandatory gates** — `RUSTFLAGS=-D warnings cargo check --all-targets`,
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (stock rustfmt defaults,
   no `rustfmt.toml` in this repo), `cargo test`, `cargo doc --no-deps` (0 warnings). A gate
   that is not exit-0 blocks the row (`CONTRIBUTING.md`; gate commands recorded at
   `CONTRIBUTING_PHASE2.md:238-239`).
3. **The three execution invariants** (`CONTRIBUTING.md`, "Execution Invariants for AI
   Agents") — never jump steps; never hide failures; never stub (in Phase III the third
   invariant reads: never stub a decoder — a parser that cannot fully decode its fixture must
   **refuse**, never truncate).
4. **The audit discipline** — dual-source citations with accurate `file:line`; Haru/CAPA
   discipline (a spec-vs-code divergence is a finding, not a silent fix); S5 (prose is not
   evidence); F-07 (an expectation is never derived from the code under test); status is the
   Verifier's to set, in a `docs(qc)` commit only, and new rows start `PENDING`
   (`CONTRIBUTING_PHASE2.md`, QC-discipline sections).
5. **The multi-agent execution model and separation of duty** — Implementer authors code but
   never writes the matrix or its own `PASSED`; Verifier is the only status-writer; the Scribe
   owns `CONTRIBUTING*` and edits them only under directive with disclosure (precedents
   `fc5ffb1`, `b6b48a7`); the Adversary lane is mandatory whenever a row's load-bearing claim
   is a negative ("no leak", "byte-exact", "refuses malformed input")
   (`CONTRIBUTING_PHASE2.md`, "Multi-agent execution model").
6. **Definition-before-implementation** — a step's scope and acceptance are written into
   `CONTRIBUTING_PHASE3.md` (or a work order citing it) **before** any code exists
   (`CONTRIBUTING_PHASE2.md`).
7. **S4 clean-clone evidence; S7 no force-push of cited SHAs; repo language English only;
   credentials never committed** (standing governance).

---

## Phase III additions

These are **new** for Phase III (asset + graphics work); where a Phase II rule already
covered the ground, the inherited list above governs and this section does not restate it.

### P3-1. Byte-exactness is the acceptance, not a nicety

An asset decoder's contract is the **on-disk format**, not an abstraction of it. A decoder is
accepted only when it consumes its input **exactly**: no trailing bytes left unconsumed for a
fully-valid file, no over-read, no "close enough". The format of record is the legacy C++
code under `GoonZu_Source/` (cited `file:line` in the step's work order); where a real
fixture byte stream exists in-tree, the fixture is the final arbiter of the format.

### P3-2. Refuse, never truncate

A malformed, truncated, or over-long input must produce a **typed, loud refusal** (an
`Err` naming the failing field/offset/expectation), never a partially-populated result and
never a silent truncation to whatever decoded. This is the graphics-domain instance of the
third execution invariant. Every decoder must have at least one test that asserts the
**refusal path** by name (short file, bad version, wrong encode type, palette not 768 bytes,
RLE stream that under/over-covers the declared canvas).

### P3-3. Determinism and headless operation

No Phase III unit or gate test may require a GPU, a windowing system, a display, or a
network. Decoding is pure byte-in / value-out. Any blit or present operation under test runs
against an in-memory CPU buffer. The Windows present path (HWND, DirectDraw-era `Blt`/
`BitBlt`, window messages) is wired in Step 7.2 and is exercised only through seams that do
not open a real window in a gate test.

### P3-4. Synthetic-fixture provenance (this phase's F-07 discipline)

The tree carries exactly three real asset fixtures — `GoonZu_Source/MapTileDiv/MapTileDiv/
Mask.Spr` (4896 B), and the twin `GoonZu_Source/CharactorCombination/{Pal,Data}/monarch.pal`
(768 B each). No real `.Map`, `.Mpr`, `.sz`, or `.mz` file exists anywhere in the repository.
Therefore:

- A step that needs a fixture the tree does not carry must construct a **synthetic fixture
  from the cited struct layout** (a test builder that assembles the exact header + body bytes
  the legacy code would `fread`), commit it (or its builder) as source, and **label it
  synthetic** in its provenance note. A synthetic fixture is never presented as a real game
  asset.
- The synthetic expectation must be derived from the **legacy layout citations**, never from
  the Rust decoder being tested (F-07). For a real in-tree fixture (`Mask.Spr`, `monarch.pal`)
  the expectation is independent of the port entirely — prefer the real fixture whenever the
  format is covered by one.
- The two `monarch.pal` copies are byte-identical (SHA-256 prefix `574E44B1A8AFC1AE`,
  re-verified 2026-10-29); either may be cited, and they must stay identical.

### P3-5. Refusal to over-constrain (legacy quirks are stated, not "fixed")

The legacy code contains genuine quirks. The port's job is to **state** them and match
observable behavior, not to silently "correct" them into what the format "should" be. A
quirk found in the legacy source is a documented resolution in the work order — never an
unannounced deviation, and never a reason to invent a stricter rule the legacy loader does not
enforce. (Worked example: `TSprImageInfo.iSize` is **not** a decode bound — the legacy
`PutSpr` blitter never reads it, and `Mask.Spr` declares `iSize=224` against a 1760-byte image
stream; the port must not treat `iSize` as a bound.)

### P3-6. File boundaries and lane ownership for new paths

Phase III introduces new module paths. Until a step's work order names a different single
writer, ownership defaults to:

| Path | Role | Default writer |
|---|---|---|
| `src/graphics/**` (new) | asset decoders: sprite (`.Spr`/SPR+MPR RLE), map (`.Map`), palette (`.pal`), archive (`.sz`/`.mz`) | Implementer (single writer per step) |
| `src/graphics/**/tests.rs` | unit tests colocated with the decoder | Implementer |
| `tests/asset_fixtures.rs` + builders (new) | synthetic-fixture builders + real-fixture harness (S5-independent expectations) | Implementer |
| `tests/e2e_loopback/**`, `tests/live_integration.rs` | unchanged from Phase II | per `CONTRIBUTING_PHASE2.md` |
| `src/**` outside `src/graphics/**` | READ-ONLY in asset steps (a defect here is a finding, never an edit) | nobody |
| `evidence/7.x/` | the Verifier's clean-clone bundle | Verifier only |
| `TASK_MATRIX.md` status cells | Verifier only, in a `docs(qc)` commit | Verifier only |

The step's work order overrides this table where it names an explicit owner. As in Phase II,
one step = one Implementer = one file-ownership set; two steps never write the same file.

### P3-7. Dual-source adaptation

Phase II's dual-source rule (client port cross-checked against the Rust server stack) does
not transfer literally: the asset file formats are **client-side** and have no counterpart in
the server, and the server repository is not present in this environment. For Phase III the
format-of-record is `GoonZu_Source/` (legacy client), and the **second source** is a real
fixture byte stream or an independently-derived layout reading — not the server. Each work
order states, per format, what the second source is. A row that cannot name a second source
for a load-bearing format claim is a finding, not a pass.

---

## Verification gates for a Phase III step

A Phase III row may be marked `PASSED` only when, in addition to the five mandatory gates
(all exit-0, evidence in `evidence/7.x/` on a clean clone at the delivery SHA):

1. **Byte-exact fixture decode (P3-1/P3-4):** every real in-tree fixture in scope decodes
   exactly (header fields, body consumption = declared stream length, pixel/canvas coverage),
   asserted against an independent expectation.
2. **Named refusal paths (P3-2):** each decoder has ≥1 test asserting a typed refusal for
   each malformed class listed in its work order, by name.
3. **Determinism/headlessness (P3-3):** the gate suite runs with no GPU/window/display/network
   dependency; the load-bearing assertions are on CPU-buffer output.
4. **Synthetic-fixture provenance (P3-4):** every synthetic fixture in the evidence carries a
   provenance note citing the legacy layout `file:line` it was built from and labelled
   synthetic; no expectation is derived from the decoder under test (F-07).
5. **Quirk ledger (P3-5):** every legacy quirk relied upon is listed in the work order's
   resolutions with its citation, and each is pinned by a test that would fail if the port
   "corrected" it.

---

## Multi-agent execution model (Phase III)

Lane hats, separation of duty, the Verifier-only status rule, S1/S3/S4/S5/S7, Haru/CAPA and
the Adversary-mandatory trigger are inherited unchanged (item 5 above). Phase III-specific
clarifications:

- **Lane sequence per asset step:** Implementer → Verifier. The Adversary lane is mandatory
  for any step whose row claim includes "byte-exact" or "refuses malformed input" — i.e. every
  decoder step — discharged either by a real Adversary session or by a **verified mutation
  probe** (deliberately break the decoder, show the named test fails; the probe and its output
  go in the evidence), per the 6.3 ratification precedent.
- **`src/**` write-guard:** an asset step writes only under `src/graphics/**` (new) and its
  own new test files; a defect anywhere else is a finding, never an edit.
- **Merge:** fast-forward only, on Custodian approval, per `CONTRIBUTING_PHASE2.md:256`.

---

## Steps in this phase

| Step | Title | Work order | Status |
|:---:|:---|:---|:---|
| 7.1 | GoonZu Asset File Formats — sprite (SPR/MPR) + map + palette + archive decoders | `docs/agents/WO-7.1.md` | `PENDING` |
| 7.2 | Windowing & 2D render loop — window creation, backbuffer, present path | `docs/agents/WO-7.2.md` (to be authored) | `PENDING` |

Status for these rows is tracked in `TASK_MATRIX.md` (Phase III tracker) and is set by the
Verifier only, per S3. Additional steps (7.3+) are appended here by the Custodian/Scribe when
the phase is scoped further.
