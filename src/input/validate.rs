//! Step 8.0 §7.4 — [`validate`]: the D3 gate between intent and wire.
//!
//! Every refusal below happens **before** `CommandSender::send`
//! (WO-7.4 D4): negative or over-`u16` tiles, empty chat and lines over
//! [`MAX_CHAT_BYTES`] are a typed `None` — never a clamp, never a panic,
//! in the `action.rs:198-214` refuse-never-truncate style. The frozen
//! execute-time refusals in `commands.rs`/`action.rs` stay the last line
//! of defense, not the user experience.

use super::intent::Intent;
use crate::client::commands::ClientCommand;
use crate::game::action::{ChatMode, Order, MAX_CHAT_BYTES};

/// Intent → wire-level command, or `None` = typed refusal before enqueue (D3).
/// Produces only frozen 7.3/7.4 types; never coerces, never clamps.
pub fn validate(intent: &Intent) -> Option<ClientCommand> {
    match intent {
        // E9: the frozen `Order::Move` carries `u16` fields
        // (`action.rs:87-94`) — negative or over-range math is refused
        // here, never wrapped and never clamped.
        Intent::MoveTo { x, y } => {
            let x = u16::try_from(*x).ok()?;
            let y = u16::try_from(*y).ok()?;
            Some(ClientCommand::Order(Order::Move { x, y }))
        }
        Intent::AttackTarget { target_char } => Some(ClientCommand::Order(Order::Attack {
            target_char: *target_char,
        })),
        // E5/Q2: the frozen kind-2 order, no payload — the server owns
        // the toggle logic (`action.rs:99-100`).
        Intent::SitToggle => Some(ClientCommand::Order(Order::SitDown)),
        // E10/E11/D5: empty and over-cap lines are refused whole, and the
        // declared default mode travels uncoerced (`action.rs:148`).
        Intent::SendChat { line } => {
            if line.is_empty() {
                return None;
            }
            if line.len() > MAX_CHAT_BYTES {
                return None;
            }
            Some(ClientCommand::Chat {
                mode: ChatMode::Normal,
                text: line.clone(),
            })
        }
        // §7.4 table: UI-only intents never enqueue (D2).
        Intent::ChatFocus | Intent::CloseDialog => None,
    }
}
