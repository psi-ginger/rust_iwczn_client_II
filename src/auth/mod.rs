//! Client-side server-flow modules.
//!
//! Introduced micro-step by micro-step per `CONTRIBUTING.md`:
//! - Step 3.1: [`front`], the Front Server connection and `SECRETKEY`
//!   handshake.
//! - Step 3.2: [`world_list`], the Notice/WorldList requests and the strict
//!   `WORLDINFO` response parser.
//! - Step 3.3: [`login`], the front authentication request, the strict
//!   `sResponse_LogIn` parser, and the `auth_key` capture for MOD-04.

pub mod front;
pub mod login;
pub mod world_list;
