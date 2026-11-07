//! Step 8.0 — the input subsystem core (`WO-8.0` §7): two pure enum hops
//! and one synchronous feed into the already-shipped Step 7.4 channel.
//!
//! ```text
//! InputEvent ──translate(&mut UiState)──▶ Intent ──validate──▶ ClientCommand
//!   event.rs          keybinds.rs         intent.rs         validate.rs
//!                                                          │
//!                                           CommandSender::send (7.4, frozen)
//! ```
//!
//! One synchronous call chain per event — **no intermediate queue of any
//! kind**: the tokio unbounded mpsc behind
//! [`CommandSender`](crate::client::commands::CommandSender) is the only
//! buffer (D1, `CONTRIBUTING_PHASE4.md` P4-C1). [`translate`] mutates
//! only [`UiState`]; [`validate()`] refuses typed before enqueue and never
//! clamps (D3, WO-7.4 D4). The budget is structural — no `.await`
//! between an event and `send` (D7, P4-C3) — and every gate test builds
//! [`InputEvent`] values directly: no window, no Win32, no network
//! (P4-C4).
//!
//! Step 8.1 adds the mouse half: [`screen`] is the exact inverse of the
//! frozen `draw_center_xy` (round-trip-pinned), [`target`] carries
//! `FindLandChar`'s hit-test, and [`translate`]'s click rows map one left
//! press to `MoveTo`/`AttackTarget` through the same synchronous chain
//! (WO-8.1 §7).

pub mod event;
pub mod intent;
pub mod keybinds;
pub mod screen;
pub mod target;
pub mod validate;

pub use event::InputEvent;
pub use intent::{Intent, UiState};
pub use keybinds::{translate, VK_BACK, VK_ESCAPE, VK_INSERT, VK_RETURN};
pub use screen::{
    is_in_map_area, is_pt_in_map, real_to_tile, screen_to_real, screen_to_tile, MapView,
};
pub use target::{pick_target, CharTarget, RealRect, TargetClass};
pub use validate::validate;

#[cfg(test)]
mod tests;
