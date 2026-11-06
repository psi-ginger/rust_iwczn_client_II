//! Step 7.4: the in-world command channel — how an input layer (later step)
//! asks the loop to send on the player's behalf (WO-7.4 §7).
//!
//! The channel is **unbounded** (D7): `CommandSender::send` is synchronous
//! and fails only when the session's receiver is gone, so a future input
//! callback can enqueue without awaiting. Commands are consumed *on the loop
//! task* (D1) — the event loop's `select!` arm yields a
//! [`LoopEvent::Command`](crate::client::event_loop::LoopEvent) and the
//! driver's handler awaits [`ClientCommand::execute`] on the very
//! `&mut GameLink` the loop owns, so no second task ever writes the link.

use crate::game::action::{send_chat, send_order, ActionError, ChatMode, Order};
use crate::game::entry::GameLink;

/// One action the in-world loop may send (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCommand {
    /// A 240 order — the frozen 7.3 `Order` rides unchanged.
    Order(Order),
    /// A 117 chat: the mode travels exactly as given (no coercion, D4).
    Chat {
        /// The chat channel (`CHATMODE_*`, `action.rs` `ChatMode`).
        mode: ChatMode,
        /// The UTF-8 text; empty or over-cap is refused by `send_chat`.
        text: String,
    },
}

impl ClientCommand {
    /// Encodes and sends this command on `link`.
    ///
    /// # Errors
    ///
    /// `ActionError::EmptyChat` / `ActionError::ChatTooLong` (the frozen 7.3
    /// chat refusals, never a clamp) and `ActionError::Send` — the send gate
    /// or the socket, normalized from `GameError` by `#[from]` (T5).
    pub async fn execute(&self, link: &mut GameLink) -> Result<(), ActionError> {
        match self {
            ClientCommand::Order(order) => send_order(link, order).await?,
            ClientCommand::Chat { mode, text } => send_chat(link, *mode, text).await?,
        }
        Ok(())
    }
}

/// Why a command could not be queued (the receiver is gone).
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// The session dropped its receiver (ended, or never wired).
    #[error("the command channel is closed (the session is over)")]
    Closed,
}

/// The sending half — clone it into whatever produces input (D7).
#[derive(Debug, Clone)]
pub struct CommandSender {
    /// The tokio unbounded sender behind the newtype.
    inner: tokio::sync::mpsc::UnboundedSender<ClientCommand>,
}

impl CommandSender {
    /// Queues `command`; `Err(CommandError::Closed)` once the session is over.
    ///
    /// # Errors
    ///
    /// [`CommandError::Closed`] when every receiver is gone — the session
    /// ended, or was never wired.
    pub fn send(&self, command: ClientCommand) -> Result<(), CommandError> {
        self.inner.send(command).map_err(|_| CommandError::Closed)
    }
}

/// The receiving half — moved into the session as `Option<CommandReceiver>`.
#[derive(Debug)]
pub struct CommandReceiver {
    /// The tokio unbounded receiver behind the newtype.
    inner: tokio::sync::mpsc::UnboundedReceiver<ClientCommand>,
}

impl CommandReceiver {
    /// `Some(command)`, or `None` once every sender is gone (D3).
    pub(crate) async fn recv(&mut self) -> Option<ClientCommand> {
        self.inner.recv().await
    }
}

/// A fresh unbounded channel (D7).
///
/// The sender is cloned into the input layer; the receiver is handed to
/// [`crate::client::driver::run_with_commands`] (or left `None` by `run`,
/// D2).
#[must_use]
pub fn channel() -> (CommandSender, CommandReceiver) {
    let (inner, receiver) = tokio::sync::mpsc::unbounded_channel();
    (CommandSender { inner }, CommandReceiver { inner: receiver })
}

#[cfg(test)]
mod tests;
