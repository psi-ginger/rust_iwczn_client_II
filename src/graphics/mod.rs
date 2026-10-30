//! GoonZu graphics — asset decoders (sprite `.Spr`, palette `.pal`, map
//! `.Map`, archives) plus the Step 7.2 windowing & 2D render loop.
//!
//! Step 7.1 (`docs/agents/WO-7.1.md`): a pure, headless decoder library.
//! Step 7.2 (`docs/agents/WO-7.2.md`, amended 2026-10-29): the RGB565
//! backbuffer, blit kernels, screen geometry, tile-map draw, tick loop,
//! and a **direct Win32 present path** — the backbuffer is wired straight
//! into a real on-screen game window (no presenter trait, no headless
//! recorder). **No network** anywhere in this module tree.
//!
//! Standing rules govern every module here:
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
//!   fixture exists in the tree). Step 7.2 keeps gate tests on the pure
//!   math/framebuffer/tick logic; the windowed code compiles under
//!   `check`/`clippy` but is exercised only by the operator on screen via
//!   the `#[ignore]`-d demo test (`WO-7.2.md` §6.5).
//!
//! Module map:
//!
//! | Module | Format of record |
//! |---|---|
//! | [`spr`] | `TSpr.h:11-47`, `TSpr.cpp:54-127`, `Graphics_PutSpr16T.cpp:28-48`, `Graphics_PutMpr16.cpp:27-53` |
//! | [`palette`] | `TSpr.cpp:138-203` (`TPal::LoadPalFromFile`), `Graphics.h:15-29` |
//! | [`map`] | `map.h:33-38,176-194,258-259`, `MapManager.cpp:687-703`, `map-save.cpp:44-118` |
//! | [`archive`] | `XSPRObj.cpp:426-514`, `FieldObject.cpp:342-368` |
//! | [`framebuffer`] | `Graphics_BasicDraw.cpp:5-63`, `Graphic.h:157-165` |
//! | [`blit`] | `Graphic_Capsule.cpp:99-133,548-563,901-948`, `Graphics_PutSpr16T.cpp:280-394` |
//! | [`screen`] | `Client.cpp:3197-3251,5713-5718`, `mapdraw.cpp:36-52`, `map.h:33-36` |
//! | [`draw_map`] | `mapdraw.cpp:167-271,881-936`, `Map-Layor.cpp:79-151` |
//! | [`present`] | `Draw.cpp:1089-1154`, `Graphic.cpp:266-310` |
//! | [`tick`] | `main.cpp:1544-1636`, `Client-WinMain.cpp:206-219` |
//! | `window` (Windows) | `main.cpp:849-1078`, `mainfunction.cpp:116-1695` |

pub mod archive;
pub mod blit;
pub mod draw_map;
pub mod framebuffer;
pub mod map;
pub mod palette;
pub mod present;
pub mod screen;
pub mod spr;
pub mod tick;
#[cfg(windows)]
pub mod window;
