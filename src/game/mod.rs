//! Game Server flow (port 4010), micro-step by micro-step per
//! `CONTRIBUTING.md`.
//!
//! - Step 4.1: [`entry`], the Game Server link with the fixed inbound link
//!   cipher (`[0x12, 0x34]`) installed from byte 0.
//! - Step 4.2: [`entry`]'s `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` (opcode 1)
//!   handshake — strict 8-byte parse, negotiated state, the outbound
//!   session cipher it keys, and the send gate it opens.

pub mod entry;
