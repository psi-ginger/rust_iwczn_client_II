//! Game Server flow (port 4010), micro-step by micro-step per
//! `CONTRIBUTING.md`.
//!
//! - Step 4.1: [`entry`], the Game Server link with the fixed inbound link
//!   cipher (`[0x12, 0x34]`) installed from byte 0.
//! - Step 4.2: [`entry`]'s `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` (opcode 1)
//!   handshake — strict 8-byte parse, negotiated state, the outbound
//!   session cipher it keys, and the send gate it opens.
//! - Step 4.3: [`login`], the `GAMEMSG_REQUEST_LOGIN` (opcode 2) submission
//!   carrying the Step 3.3 `auth_key`, enciphered with the negotiated
//!   session cipher.
//! - Step 4.4: [`char_info`], the opcode 3 character roster
//!   (`cltCharListInfo`, 984 bytes) and the 592-byte `cltMakeCharInfo`
//!   detail record of opcodes 5 and 6, each field at its literal offset.

pub mod char_info;
pub mod entry;
pub mod login;
