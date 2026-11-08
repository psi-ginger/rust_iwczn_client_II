//! Step 8.3 (WO-8.3 D19/D20) — [`InputFeed`]: the production shape of the
//! chain every 8.0 test drove through its local helper — `translate` →
//! `validate` → `CommandSender::send`, one synchronous call per event
//! (P4-C1: no intermediate storage; P4-C3: no `.await` between the event
//! and `send`). The window side (`graphics::window::wm_to_input`, Windows
//! only) decodes raw messages into `InputEvent`s and hands them here; this
//! module stays cross-platform and never references it (doc-gate
//! portability, WO-8.3 §2.2-9).

use super::event::InputEvent;
use super::intent::UiState;
use super::keybinds::translate;
use super::screen::MapView;
use super::validate::validate;
use crate::client::commands::{ClientCommand, CommandError, CommandSender};

/// One session's input half: translator state plus the channel it feeds
/// (D19). `Send` (T46) — it rides the window's pump thread; `Debug` only,
/// deliberately not `Clone` (it owns the session's sender).
#[derive(Debug)]
pub struct InputFeed {
    /// The translator-local state `translate`/`enter_world` mutate.
    ui: UiState,
    /// The frozen 7.4 sending half (`commands.rs:54-71`).
    sender: CommandSender,
}

impl InputFeed {
    /// A fresh feed over `sender`: pre-world idle state
    /// (`UiState::default()` — every event discarded until `enter_world`).
    #[must_use]
    pub fn new(sender: CommandSender) -> Self {
        Self {
            ui: UiState::default(),
            sender,
        }
    }

    /// One event through the whole chain, synchronously (D19):
    /// `Ok(Some(command))` ⇔ exactly one command was sent;
    /// `Ok(None)` ⇔ `translate` or `validate` refused and nothing was sent;
    /// `Err(CommandError::Closed)` ⇔ the session is over — surfaced by
    /// name, never a panic, never a retry.
    ///
    /// # Errors
    ///
    /// [`CommandError::Closed`] when every receiver is gone (`commands.rs:47-52`).
    pub fn feed(&mut self, event: InputEvent) -> Result<Option<ClientCommand>, CommandError> {
        let Some(intent) = translate(event, &mut self.ui) else {
            return Ok(None);
        };
        let Some(command) = validate(&intent) else {
            return Ok(None);
        };
        self.sender.send(command.clone())?;
        Ok(Some(command))
    }

    /// The in-world stage reached, in one call (D20): `in_world = true`
    /// (8.0's D4 guard replaces `GetFrame() > 40`,
    /// `mainfunction.cpp:221,236,…`) **and** `view = Some(view)` (8.1's D10
    /// click context). The live trigger is 8.4's composition (divergence
    /// (a), WO-8.3 §8); tests stand in for it here.
    pub fn enter_world(&mut self, view: MapView) {
        self.ui.in_world = true;
        self.ui.view = Some(view);
    }

    /// Read-only access to the translator state (T45's pin): `feed` and
    /// `enter_world` are the only mutators.
    #[must_use]
    pub fn ui(&self) -> &UiState {
        &self.ui
    }
}
