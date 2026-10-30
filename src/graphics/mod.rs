//! GoonZu asset decoders — sprite (`.Spr`), palette (`.pal`), map (`.Map`)
//! and the `.sz`/`.mz` archive read seam.
//!
//! Step 7.1 (`docs/agents/WO-7.1.md`): a pure, headless decoder library.
//! **No window, no GPU, no present path, no network** — that is Step 7.2.
//!
//! Three standing rules govern every decoder here:
//!
//! - **P3-1 byte-exactness.** Every layout is coded to the canonical legacy
//!   source under `GoonZu_Source/` (the format of record), cited `file:line`
//!   in each module. Where the work order's prose and the cited source
//!   disagree, the source wins and the discrepancy is filed as a finding.
//! - **P3-2 typed refusal.** Malformed input returns a named, typed error
//!   instead of panicking or silently emitting a partial result. Readers are
//!   length-gated before every field read (the repo convention, shared
//!   readers in [`crate::protocol::bytes`]).
//! - **P3-3 headless tests.** Unit tests are deterministic and hermetic:
//!   real in-tree fixtures (`Mask.Spr`, `monarch.pal`) for sprite and
//!   palette, synthetic builders citing the legacy layout `file:line` for
//!   map and archive (`WO-7.1.md` R2 — no real `.Map`/`.Mpr`/`.sz`/`.mz`
//!   fixture exists in the tree).
//!
//! Module map:
//!
//! | Module | Format of record |
//! |---|---|
//! | [`spr`] | `TSpr.h:11-47`, `TSpr.cpp:54-127`, `Graphics_PutSpr16T.cpp:28-48`, `Graphics_PutMpr16.cpp:27-53` |
//! | [`palette`] | `TSpr.cpp:138-203` (`TPal::LoadPalFromFile`), `Graphics.h:15-29` |
//! | [`map`] | `map.h:33-38,176-194,258-259`, `MapManager.cpp:687-703`, `map-save.cpp:44-118` |
//! | [`archive`] | `XSPRObj.cpp:426-514`, `FieldObject.cpp:342-368` |

pub mod archive;
pub mod map;
pub mod palette;
pub mod spr;
