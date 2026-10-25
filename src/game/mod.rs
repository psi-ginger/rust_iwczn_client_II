//! Game Server flow (port 4010), micro-step by micro-step per
//! `CONTRIBUTING.md`.
//!
//! - Step 4.1: [`entry`], the Game Server link with the fixed inbound link
//!   cipher (`[0x12, 0x34]`) installed from byte 0.

pub mod entry;
