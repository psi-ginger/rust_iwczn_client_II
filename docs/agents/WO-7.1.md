# Work Order - Step 7.1: GoonZu Asset File Formats (Sprite / Map / Palette / Archive decoders)

## 1. Provenance, lane hats, baseline

Filed 2026-10-29 by the Acting QC Lead / Verifier session (`psi-ginger`) in the **Scribe lane**
under the Custodian's Phase III kickoff directive ("author the first work order … specifying
the exact structs and asset loader requirements referenced from `GoonZu_Source/`"). No separate
Scribe session was active; the `CONTRIBUTING*` edit (`CONTRIBUTING_PHASE3.md`) is made under
directive with this disclosure, precedent `fc5ffb1` / `b6b48a7`. Scope and acceptance below are
governed by `CONTRIBUTING_PHASE3.md` (established in the same directive); this work order does
not amend it.

- **Baseline:** `main` @ `606da1a`, **225/225** (211 lib + 14 e2e; five gates re-run exit-0
  this session — `fmt` 0 diffs, `doc` 0 warnings, clean tree; the four `live_integration`
  tests remain ignored). This is the Phase II signed-off baseline
  (`docs/audit/2026-10-29-phase-2-final-sign-off-report.md`).
- **Branch:** `step/7.1-assets`, cut from `main` @ `606da1a`.
- **Lanes:** Implementer → Verifier + **Adversary (mandatory)** — every decoder row's
  load-bearing claim is "byte-exact" and "refuses malformed input" (`CONTRIBUTING_PHASE3.md`,
  execution-model section). The Adversary requirement may be discharged by a **verified
  mutation probe** per the 6.3 ratification precedent (`WO-6.3.md` §9): break the decoder,
  show the named test fails, record probe + output in `evidence/7.1/`.
- **Write guard:** the Implementer writes **only** under `src/graphics/**` (new) and the new
  test files named in §5. `src/**` outside `src/graphics/**` is READ-ONLY — a defect found
  there is a finding `F-7.1-NN`, never an edit (`CONTRIBUTING_PHASE3.md` P3-6).

## 2. Scope (verbatim authority: `CONTRIBUTING_PHASE3.md`)

Port the classic on-disk **client asset decoders** from the legacy C++ tree to Rust, byte-exact
against the legacy load path, headless and deterministic, refusing malformed input by name:

- **(a) Sprite (`.Spr`) decoder** — `TSprHeader` + per-image `TSprImageInfo` + the SPR and MPR
  RLE image streams, format of record `GoonZu_Source/NLib/TSpr.h` / `TSpr.cpp` (§6.1).
- **(b) Palette (`.pal`) decoder** — `TPal::LoadPalFromFile`: exact-768-byte rule and the
  565/555 RGB16 conversion, format of record `GoonZu_Source/NLib/TSpr.cpp:138-205` (§6.2).
- **(c) Map (`.Map`) decoder** — `cltMapHeader` + the layered body in the exact `fread` order
  the legacy `MapManager` uses, format of record `GoonZu_Source/GoonZuWorld/Common/Map/*` (§6.3).
- **(d) Archive (`.sz`/`.mz`) read seam** — the zip-backed sprite/field-object load path with
  its loose-file fallback, format of record `GoonZu_Source/GoonZuWorld/...` (§6.4).

Deliverable is a pure `src/graphics/**` decoder library plus its tests. **No window, no GPU,
no present path, no network** — that is Step 7.2.

## 3. Resolutions (legacy quirks and environment facts — state, do not "fix")

These are decided here so the Implementer codes to a settled spec; each is pinned by a test
(`CONTRIBUTING_PHASE3.md` P3-5) and each is a **quirk that must survive**, not be corrected:

- **R1 — `TSprImageInfo.iSize` is NOT a decode bound.** The legacy blitter never reads `iSize`.
  Real `Mask.Spr` declares `ImageInfo[0].iSize = 224` while its SPR stream is 1760 bytes, yet
  decodes to full coverage (§6.1). The port must decode the stream to canvas coverage and must
  **not** truncate to, or reject on, `iSize`. Expose `iSize` as data only.
- **R2 — No real `.Map`/`.Mpr`/`.sz`/`.mz` fixture exists in the tree** (only `Mask.Spr` + twin
  `monarch.pal`). Map and archive tests therefore use **synthetic fixtures built from the cited
  layouts**, labelled synthetic with a provenance note (`CONTRIBUTING_PHASE3.md` P3-4). Sprite
  and palette tests use the **real** fixtures (they exist).
- **R3 — `TCHAR` is 1 byte on disk here.** `BeTheRich.vcproj` sets `CharacterSet="2"` (MBCS),
  so `cltMapHeader.Text[128]` and `Reserved[19]` are 128 and 19 **bytes**, and the header's
  on-disk size is fixed by that MBCS layout. The port parses the header at the MBCS byte widths
  (no UTF-16 assumption).
- **R4 — The server repository is not present in this environment,** and asset formats are
  client-side with no server counterpart. Per `CONTRIBUTING_PHASE3.md` P3-7 the second source is
  a real fixture byte stream (sprite/palette) or an independently-derived layout reading (map),
  **not** the server. Each format below names its second source.
- **R5 — v400 palette skip.** For `iVersion >= TSPR_VERSION_16PAL (400)`, the loader reads the
  header then **skips 512 bytes** of embedded palette before the image data
  (`TSpr.cpp:54-87`). The port must consume those 512 bytes for v400 files and must not treat
  them as image stream.

## 4. Non-goals

- No window creation, message pump, backbuffer, `Blt`/`BitBlt`, or present path (Step 7.2).
- No GPU, Direct3D, or shader code; no `wgpu`/`glow` dependency.
- No live server or network I/O in any test (`CONTRIBUTING_PHASE3.md` P3-3).
- No change to `src/**` outside `src/graphics/**`; no change to Phase II modules.
- No editor/tooling port (`MapTileDiv`, `DLGEditorS`, `NConvert` are reference-only).
- Not decoding every legacy asset type — only the four in §2. `.tsx`/`.gr2`/`.mdl`/`.npk` are out of scope.

## 5. File plan (ownership: `CONTRIBUTING_PHASE3.md` P3-6)

| File | Role | Writer |
|---|---|---|
| `src/graphics/mod.rs` (new) | graphics module root | Implementer (single writer) |
| `src/graphics/spr.rs` (new) | `TSprHeader`, `TSprImageInfo`, SPR/MPR stream decode (§6.1) | Implementer |
| `src/graphics/palette.rs` (new) | `TPal` 768-rule + 565/555 conversion (§6.2) | Implementer |
| `src/graphics/map.rs` (new) | `cltMapHeader` + layered body decode (§6.3) | Implementer |
| `src/graphics/archive.rs` (new) | `.sz`/`.mz` read seam + loose fallback (§6.4) | Implementer |
| `src/graphics/**/tests.rs` | colocated decoder unit tests | Implementer |
| `tests/asset_fixtures.rs` (new) | synthetic builders + real-fixture harness | Implementer |
| `src/lib.rs` | additive `pub mod graphics;` line only | Implementer (additive) |
| `tests/e2e_loopback/**`, `tests/live_integration.rs` | untouched | nobody |
| `src/**` outside `src/graphics/**` | READ-ONLY | nobody |
| `evidence/7.1/` | clean-clone gate bundle + mutation probe | Verifier only |

## 6. Design (format of record — every field byte-verified against `GoonZu_Source/`)

### 6.1 Sprite (`.Spr`)

Header, format of record `GoonZu_Source/NLib/TSpr.h:32-47` (verbatim struct, `SI32` = 4-byte
signed, little-endian on disk):

```cpp
struct TSprHeader {
public:
    SI32 iVersion;      // TSPR_VERSION_NOPAL=10 | TSPR_VERSION_16PAL=400
    SI32 iEncodeType;   // TSPR_ENCODETYPE_SPR=0 | TSPR_ENCODETYPE_MPR=1
    SI32 iXSize, iYSize;
    SI32 iHRes, iVRes;
    SI32 iImageNum;     // count of TSprImageInfo entries
    SI32 reserved[7];   // 28 bytes, ignored
    // then: TSprImageInfo[iImageNum], then image byte-stream(s)
};
struct TSprImageInfo { SI32 iCollInfo; SI32 iStartPos; SI32 iSize; };  // TSpr.h:21-26
```

Load path (`TSpr.cpp:54-87`): `fread` the 28-byte-fixed header region; if `iVersion >= 400`
skip **512** palette bytes (R5); then the image data. Constants: `TSPR_ENCODETYPE_SPR = 0`,
`TSPR_ENCODETYPE_MPR = 1`, `TSPR_VERSION_NOPAL = 10`, `TSPR_VERSION_16PAL = 400`
(`TSpr.h:11-15`).

**SPR RLE** (opaque sprite), decoder at `GoonZuWorld/Lib/NGraphics/Graphics_PutSpr16T.cpp:28-48`,
encoder at `Lib/Graphic/Graphic_Capsule.cpp:1125` (`EncodeSpr`): walk each row left→right to
`width`; a literal byte is a palette index; the sentinel **254 (`COLOR_TRANS`)** is followed by
a **count byte = transparent run length** (skip that many pixels, no index bytes). Verified on
real `Mask.Spr`: consumed exactly 1760/1760 stream bytes, covered exactly 3200/3200 px
(1600 literal idx `0xA3` + 80 skip runs).

**MPR RLE** (tile sprite), decoder at `Graphics_PutMpr16.cpp:27-53`, encoder `EncodeMpr`
(`Graphic_Capsule.cpp`): per row; sentinel **254 + count** = transparent run; otherwise a
**count byte followed by `count` index bytes** = a solid run of that color. (SPR emits one index
per non-transparent pixel; MPR emits a run-length + the repeated indices — the two decoders are
**not** interchangeable; test each separately.)

Color/palette: sprites are 8-bit indexed; the index maps through the active 512-byte
(`256 × RGB16`) color table at draw time (`Graphics_PutSpr16T.cpp:22`, `pColorTable[*psrc]`).
The decode step produces **indices + transparent-run spans**, not RGB — RGB conversion is the
palette's job (§6.2) and final color is 7.2's concern.

Second source (P3-7): the real `Mask.Spr` byte stream (independent of the port) + the encoder
(`EncodeSpr`) cross-checked against the decoder.

### 6.2 Palette (`.pal`)

`TPal::LoadPalFromFile`, `GoonZu_Source/NLib/TSpr.cpp:138-205`: file size must be **exactly
768** (`256 × 3` bytes RGB) or it is refused (`TSpr.cpp:159-164`); then each of 256 entries is
converted to `RGB16`. 565 mode (`TSpr.cpp:183-187`): `r = RGB16_565_RMASK & ((p[i*3] >> 1) << 11)`
with the analogous g/b packing; 555 mode uses the 555 masks. Masks/consts: `GoonZuWorld/Lib/
NGraphics` `RGB16_565_RMASK` etc. The port exposes a `Rgb565`/`Rgb555` conversion and the
exact-768 refusal. Real fixture: `monarch.pal` (768 B; both copies SHA-256 prefix
`574E44B1A8AFC1AE`) — used to pin the 565 conversion byte-exactly. This palette is loaded in the
legacy client as the color table (`Monarch.pal`, `Graphic_Capsule.cpp`).

Second source: the real `monarch.pal` bytes.

### 6.3 Map (`.Map`)

Header, format of record `GoonZuWorld/Common/Map/map.h:176-194` (verbatim, MBCS widths per R3):

```cpp
class cltMapHeader {
public:
    DWORD Version;        // >=2 adds the GeoInfo tail on read
    DWORD Unique;
    TCHAR Text[128];      // 128 bytes (MBCS)
    DWORD Reserved0;
    SHORT MapXsize, MapYsize;
    SI32  MapType;
    SI32  TileSet1;
    bool  bWeatherSwitch;
    SI16  TileSet2;
    TCHAR Reserved[19];   // 19 bytes (MBCS)
};
```

Tile constants `map.h:33-38`: `MAP_TILE_XSIZE 80`, `MAP_TILE_YSIZE 40`, halves 40/20,
`MAX_MAP_LAYER 2`. Geo constants `map.h:258-259`: `MAX_GEO_NUMBER 256`, `GEO_TEXT_LENGTH 64`.

**Body read order** — the load-bearing sequence, from `MapManager.cpp:687-703` (the legacy
`fread` order; matches the write order in `map-save.cpp:44-55`):

1. `cltMapHeader`
2. ID map (`ReadIDMap`)
3. collision map (`ReadCollMap`)
4. layer 0 (`pclLayer[0]->Read`, `MapLayor.cpp`)
5. layer 1 (`pclLayer[1]->Read`)
6. field-object map (`ReadFieldObjectMap`)
7. field-object map para (`ReadFieldObjectMapPara`)
8. if `Version >= 2` **and** service area is not Korea → GeoInfo (`pclMapGeoInfo->Read`)

The port decodes header + the two fixed layers + geo tail in this exact order, refusing a
truncated stream at whichever section runs out (P3-2). Field-object/geo sub-record sizes are
read per the cited `Read`/`Write` methods; each section is length-validated before advancing.

Second source: the write order in `map-save.cpp` (independent of the read path) — the two must
agree, and they do.

### 6.4 Archive (`.sz` / `.mz`) read seam

Sprites load either from a loose file or from a zip archive: `LoadSprFromFile` / 
`LoadSprFromMemory` (`Lib/Graphic/Graphic_Capsule.h:148-150`, `LoadTileFromFile:150`,
`EncodeSpr/EncodeMpr` at `:153+`). The field-object path builds a `.mz` name and reads a member
from it: `StringCchPrintf(szZipName, ..., "FieldObject\\%s.mz", pszZipName)` then
`unzOpen(szZipName)` / `unzOpenCurrentFile(uf)` (`FieldObject.cpp:342,345,368`); the sprite
character path does the same for `.sz` (`XSPRObj.cpp:431-503`, `unzOpen` → read → 
`LoadSprFromMemory`, with a **loose-file fallback** when the archive/member is absent
(`XSPRObj.cpp:495-510`)).

The port models this as a `Source` seam: an in-memory **archive of named members** or a
**loose byte slice**, resolved member-first with loose fallback exactly as the legacy path. The
archive step's job is the **read seam + fallback semantics**, not a full zip central-directory
parser in this step (a minimal member-lookup is acceptable if a real `.sz` is ever added; for
7.1 the archive is an in-memory member map, so the seam is testable headlessly). The decoder
consumes the extracted bytes through the same `TSpr` decode as §6.1.

Second source: the two call-sites' control flow (loose fallback is visible in both `.sz` and
`.mz` paths).

## 7. Forbidden shortcuts

- Editing `src/**` outside `src/graphics/**` to make a test pass (finding first).
- Truncating a decode to `iSize` or any legacy field that is not actually a bound (R1) — or,
  conversely, adding a bound the legacy loader does not enforce.
- Deriving a fixture expectation from the Rust decoder under test (F-07) — real fixtures carry
  independently-computed expectations; synthetic fixtures cite the legacy layout `file:line`.
- Presenting a synthetic fixture as a real game asset, or omitting its provenance note (P3-4).
- Interchanging the SPR and MPR decoders, or treating the v400 512-byte palette as image data
  (R5).
- Any GPU/window/display/network dependency in a gate test (P3-3).
- Silently "correcting" a legacy quirk listed in §3 instead of pinning it.

## 8. Evidence required (Verifier, S4 clean clone at the delivery SHA)

- `evidence/7.1/`: `head.txt`, `check.log`, `clippy.log`, `fmt.log`, `test.log`, `doc.log`,
  `env.txt`, `SHA256SUMS` — the standing bundle; the matrix test number is copied from
  `test.log`.
- **Mutation probe (Adversary equivalent):** the probe description, the mutated diff, and the
  failing named test output (e.g. break the SPR sentinel handling ⇒ the `Mask.Spr`
  exact-consumption test fails; break the map body order ⇒ the synthetic-map order test fails).
- Provenance note for every synthetic fixture (legacy layout `file:line` it was built from).
- Confirmation that `src/**` outside `src/graphics/**` is untouched (`git diff -- src` empty
  outside the new module).

## 9. Return blocks (append; do not rewrite the sections above)

### Delivery note - Implementer

(to be appended on delivery: branch, delivery SHA, the five gates exit-0 with counts copied
from `test.log`, per-format test inventory, and the `git diff -- src` confirmation)

### Verdict - Verifier

(to be appended on QC: S4 clean-clone re-run, mutation-probe result, per §2(a)-(d) acceptance,
and the status decision — Verifier only, per S3)

## 10. Definition of done

A step-7.1 delivery is `PASSED` only when **all** hold, on a clean clone at the delivery SHA:

1. All five mandatory gates exit-0 (`CONTRIBUTING_PHASE3.md` inherited item 2).
2. **Sprite:** real `Mask.Spr` decodes byte-exactly — header fields (`iVersion=10`,
   `iEncodeType=0`, `iXSize=80`, `iYSize=40`, `iImageNum=1`), stream consumption exactly
   `1760/1760`, canvas coverage exactly `3200/3200` px; `iSize=224` is exposed but not used as
   a bound (R1). SPR and MPR decoders are separately tested.
3. **Palette:** `monarch.pal` accepted (exactly 768 B); a 767-B and a 769-B file are refused by
   name; the 565 conversion of a known index is byte-exact.
4. **Map:** a synthetic v2 map (built from §6.3 layout, provenance noted) decodes in the exact
   `MapManager` body order; a truncated stream is refused at the named section; MBCS widths
   used (R3).
5. **Archive:** the member-vs-loose fallback resolves exactly as the legacy `.sz`/`.mz` paths,
   and the extracted bytes decode through §6.1.
6. Every §3 quirk (R1-R5) is pinned by a failing-if-corrected test.
7. The Adversary/mutation probe is recorded in `evidence/7.1/` and shows a named test failing
   under a deliberate decoder break.
8. No GPU/window/display/network in any test; `src/**` outside `src/graphics/**` untouched.
