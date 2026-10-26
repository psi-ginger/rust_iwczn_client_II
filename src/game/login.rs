//! Step 4.3 - `GAMEMSG_REQUEST_LOGIN` (opcode 2): the game login submission
//! (`CONTRIBUTING.md:82-85`).
//!
//! Once the opcode 1 handshake (Step 4.2) has keyed the outbound session
//! cipher, the client's first outbound frame on the game link is the 48-byte
//! `cltLogInInfo` body, carrying the front server's `auth_key` (Step 3.3)
//! together with the account name and password. This module builds that
//! payload at its literal byte offsets and sends it through
//! [`GameLink::send`], which enciphers it with the negotiated session cipher.
//!
//! # The legacy send site
//!
//! `Client\LogIn\LogIn.cpp:1037-1043` - the plain-login branch (the HanAuth
//! and Taiwan variants for those regions sit at `:1013-1036`):
//!
//! ```cpp
//! cltLogInInfo cllogin( siAuthKey, szID, szPassword, uiGamemode );
//! cltMsg clMsg(GAMEMSG_REQUEST_LOGIN, sizeof(cltLogInInfo), (BYTE*)&cllogin);
//! SendMsgToServer((sPacketHeader*)&clMsg);
//! ```
//!
//! Three preconditions guard that branch (`LogIn.cpp:984-992`):
//! `GameMode == GAMEMODE_CLIENT`, `MyCharUnique != 0` (`:987`) and
//! `bGameServerSecretKeySwitch == true` (`:989`). The latter two are set by
//! the very handshake of Step 4.2 (`Client-SystemNPC.cpp:698,706`), so by the
//! time [`request_game_login`] can run, the legacy's gates are already open -
//! and [`GameLink::send`] enforces the same rule structurally as
//! [`GameError::NotHandshaken`]. The legacy then records
//! `USERSTATE_GAME_AUTHOK` (`:992`); that is client state-machine bookkeeping
//! and is not part of this frame's wire contract.
//!
//! # The 48-byte layout - `cltLogInInfo`
//!
//! | Offset | Field | Legacy declaration | Carries |
//! |---|---|---|---|
//! | 0..4 | [`game_login_payload`]'s `auth_key` | `SI32 siAuthKey` | the front login's token (`FrontAuth::auth_key`, Step 3.3) |
//! | 4..24 | account name | `TCHAR szID[MAX_PLAYER_NAME]` | C-string, zero-filled |
//! | 24..44 | password | `TCHAR szPassword[MAX_PLAYER_NAME]` | C-string, zero-filled |
//! | 44 | game mode | `UI08 uiGameMode` | [`GAME_MODE_WINDOWED`] |
//! | 45..48 | tail pad | (implicit) | MSVC `/Zp8` rounding |
//!
//! The struct is declared at `GoonZuWorld\common\CommonHeader.h:1442-1452`
//! with `MAX_PLAYER_NAME` = 20 (`CommonHeader.h:575`), and its constructor
//! (`Common.cpp:868-874`) copies the key, `MStrCpy`s the two strings and
//! stores the mode - no other field exists. The 48 is packing, not
//! coincidence: 4 + 20 + 20 + 1 = 45 meaningful bytes, rounded up to the
//! struct's 4-byte alignment (from `SI32`) because **only `cltMsg` is inside
//! `#pragma pack(1)`** (`MsgType.h:2938-2989` spans exactly the `cltMsg`
//! class) while `cltLogInInfo` is compiled under MSVC's default `/Zp8`.
//!
//! The server port derives the identical number from the same header:
//! `GAME_LOGIN_PAYLOAD_LEN` = 48, "including its 3 trailing pad bytes"
//! (`gz-proto/src/game.rs:18-20`, citing `CommonHeader.h:1442-1452`), and its
//! `GameLogin::decode` reads `i32` at 0, `fixed_str(20)` at 4 and 24, `u8`
//! at 44, refusing a body shorter than 45 - exactly this layout. The three
//! pad bytes travel as zeros here; the legacy sends whatever sat in that
//! stack struct's tail, and neither side ever reads it.
//!
//! # Frame: opcode 2, `usSize` 53
//!
//! `cltMsg`'s constructor sets `clHead.usSize = sizeof(sPacketHeader) +
//! datasize` (`MsgType.h:2957-2974`, `:2967`) - so 5 + 48 = **53** on the
//! wire, with `usCmd` = 2 (`GAMEMSG_REQUEST_LOGIN`, the enumerator after
//! `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE = 1`, `MsgType.h:22-23`; the server
//! port's generated `GameMsg::RequestLogin = 2`, `opcodes_game.rs`, from the
//! same header). 53 is far below the 200-byte LZF threshold, so this frame
//! goes out uncompressed (`Client-Winsock.cpp:178-186`); the compression flag
//! path itself already travels through the encoder untouched (`net/codec.rs`).
//!
//! # `uiGameMode` is fixed at [`GAME_MODE_WINDOWED`]
//!
//! The legacy derives it from its own UI (`LogIn.cpp:994-1004`): 0 for the
//! windowed interface, 1 for fullscreen, with `CommonHeader.h:1447-1448`
//! documenting the 0/1/2 meanings. This port has no window manager, so 0 -
//! the legacy's own default branch - is the only value it can report
//! honestly; the server uses the field for interface-mode counters
//! (`Server-Login.cpp:61-72`) and the port reads it into `game_mode`
//! (`game.rs`, `GameLogin`). It is a constant rather than an argument
//! because there is no fullscreen state here to vary it.
//!
//! # The password travels too
//!
//! The scope names the account and the `auth_key`, but the struct's third
//! field is the password and both server lines check it: the legacy forwards
//! `siAuthKey`/`szID`/`szPassword` to its auth server
//! (`Server-Login.cpp:34-87`, `:80-82`), and the server port's
//! `handle_game_login` (`services/game/src/auth.rs`) looks the account up by
//! `userid`, then `admit_game_login` requires `auth_key == account_id` **and**
//! a password that verifies against the stored hash. Omitting it would turn
//! every login into a password rejection, so it is carried exactly as the
//! legacy carries it.
//!
//! # Refuse, not truncate
//!
//! `MStrCpy` is `StringCchCopy` (`Lib\commonutil.cpp:6-16`): it copies at
//! most 19 bytes plus a terminator into a 20-byte field and the return value
//! is discarded at the call site (`Common.cpp:871-872`), so an over-long
//! credential is silently shortened. This port refuses instead - the Step 3.3
//! decision (`auth/login.rs`, [`GameLoginError::CredentialTooLong`]),
//! because a truncated password can only ever produce a server-side
//! rejection that no longer names the real cause.
//!
//! # What the server does with it
//!
//! The port's `handle_game_login` first resolves `userid` against the account
//! table; an unknown name is refused with
//! `game login for unknown account "..."` - the rejection the
//! `CONTRIBUTING.md` Step 4.3 acceptance criterion names - and a match that
//! passes both checks of `admit_game_login` logs
//! `game: login ok for ... (account ..., auth_key ...)` and serves the 15
//! char-list frames of Step 4.4. On the legacy server the handler casts
//! `pclMsg->cData` straight to `cltLogInInfo*` (`Server-Login.cpp:56`) and
//! forwards the three fields to the auth server, which is the same contract
//! this payload satisfies.
//!
//! # Test addresses are synthetic (F-01)
//!
//! As on every link so far (QC decisions F-01/F-02 on
//! CAR-2026-10-24-RETRO01): the suite runs against a loopback peer, proving
//! the 53-byte frame, its opcode, its bit-exact payload and its session-cipher
//! pairing - not reachability of a live Game Server.

use crate::auth::login::{AuthKey, FrontAuth};
use crate::game::entry::{GameError, GameLink};

/// `GAMEMSG_REQUEST_LOGIN` - the game login opcode
/// (`MsgType.h:23`; the server port's `GameMsg::RequestLogin`,
/// `opcodes_game.rs`, generated from the same header).
pub const REQUEST_LOGIN: u16 = 2;

/// `sizeof(cltLogInInfo)` under MSVC default packing: 45 meaningful bytes
/// plus 3 tail pad (`CommonHeader.h:1442-1452` + `MAX_PLAYER_NAME` 20 at
/// `:575`); the server port's `GAME_LOGIN_PAYLOAD_LEN`
/// (`gz-proto/src/game.rs:18-20`) is the same 48. The frame's `usSize` is
/// therefore 53 (`MsgType.h:2967`).
pub const GAME_LOGIN_PAYLOAD_LEN: usize = 48;

/// `sizeof(szID)` / `sizeof(szPassword)` = `MAX_PLAYER_NAME`
/// (`CommonHeader.h:575`); the server port's `PLAYER_NAME_LEN`
/// (`game.rs`, `PlayerName` width) is the same 20.
pub const PLAYER_NAME_LEN: usize = 20;

/// `uiGameMode` = 0: the windowed interface (`CommonHeader.h:1447-1448`;
/// the legacy's `!m_bFullScreenMode` branch, `LogIn.cpp:994-996`). This port
/// reports no fullscreen state, so 0 is the only honest value (module docs,
/// game mode).
pub const GAME_MODE_WINDOWED: u8 = 0;

/// Why a game-login operation failed.
#[derive(Debug, thiserror::Error)]
pub enum GameLoginError {
    /// The link layer failed - most commonly [`GameError::NotHandshaken`],
    /// the [`GameLink::send`] gate standing in for the legacy's
    /// `bGameServerSecretKeySwitch == false` check
    /// (`Client-Winsock.cpp:171`), then the send failure cases.
    #[error(transparent)]
    Game(#[from] GameError),
    /// `szID` or `szPassword` cannot fit 19 bytes plus its NUL terminator
    /// (`CommonHeader.h:575`). The legacy's `MStrCpy` would transmit the
    /// truncated remainder (`commonutil.cpp:14`, return value discarded at
    /// `Common.cpp:871-872`); this port refuses rather than silently
    /// shrinking a credential (module docs, refuse not truncate).
    #[error("{field} of {len} bytes does not fit its {PLAYER_NAME_LEN}-byte field with its NUL terminator (CommonHeader.h:575)")]
    CredentialTooLong {
        /// The legacy field name (`szID`, `szPassword`).
        field: &'static str,
        /// Credential length (up to its first NUL) that failed to fit.
        len: usize,
    },
}

/// Copies `value` into the `slot`-sized field with `StringCchCopy`
/// semantics: bytes up to the first NUL, then zero-fill (`MStrCpy`,
/// `Lib\commonutil.cpp:6-16`; the zero tail is what `StringCchCopy` leaves
/// behind and the server reads only up to the terminator).
///
/// # Errors
///
/// [`GameLoginError::CredentialTooLong`] when the value needs 20 or more
/// bytes and therefore cannot fit a terminator.
fn copy_credential(
    slot: &mut [u8],
    field: &'static str,
    value: &str,
) -> Result<(), GameLoginError> {
    debug_assert_eq!(slot.len(), PLAYER_NAME_LEN, "one 20-byte field per call");
    let raw = value.as_bytes();
    let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    if len >= PLAYER_NAME_LEN {
        return Err(GameLoginError::CredentialTooLong { field, len });
    }
    slot[..len].copy_from_slice(&raw[..len]);
    Ok(())
}

/// Builds the 48-byte payload of a `GAMEMSG_REQUEST_LOGIN`
/// (`cltLogInInfo`, `CommonHeader.h:1442-1452`): `siAuthKey` at 0..4
/// little-endian, `szID` at 4..24, `szPassword` at 24..44 (each C-string,
/// zero-filled), `uiGameMode` at 44 = [`GAME_MODE_WINDOWED`], three zero pad
/// bytes at 45..48 (module docs, the layout).
///
/// `auth_key` arrives as [`AuthKey`] so the value can only be one that the
/// front login captured (Step 3.3) - the same token the server compares
/// against the account's `idnum` before it will admit the session.
///
/// # Errors
///
/// [`GameLoginError::CredentialTooLong`] for an account or password that
/// cannot fit its 20-byte field with a terminator (nothing is built).
pub fn game_login_payload(
    auth_key: AuthKey,
    account: &str,
    password: &str,
) -> Result<[u8; GAME_LOGIN_PAYLOAD_LEN], GameLoginError> {
    let mut payload = [0u8; GAME_LOGIN_PAYLOAD_LEN];
    payload[0..4].copy_from_slice(&auth_key.get().to_le_bytes());
    copy_credential(&mut payload[4..24], "szID", account)?;
    copy_credential(&mut payload[24..44], "szPassword", password)?;
    payload[44] = GAME_MODE_WINDOWED;
    Ok(payload)
}

/// Sends `GAMEMSG_REQUEST_LOGIN` carrying the Step 3.3 handoff - the frame
/// is `usSize` 53 on the wire (`MsgType.h:2967`) and enciphered with the
/// session cipher the opcode 1 handshake negotiated (`GameLink::send`;
/// the legacy's `SendMsgToServer`, `LogIn.cpp:1042`).
///
/// The account name is [`FrontAuth::user_id`] - the canonical id the front
/// server echoed back at login (`Client-Winsock.cpp:377`, Step 3.3) rather
/// than whatever the player typed, which is what the server looks the
/// account up by.
///
/// # Errors
///
/// [`GameLoginError::CredentialTooLong`] first, when a credential does not
/// fit (nothing is written); then [`GameLoginError::Game`] -
/// [`GameError::NotHandshaken`] before the handshake (the legacy's
/// `bGameServerSecretKeySwitch` gate, `Client-Winsock.cpp:171`), then the
/// [`GameLink::send`] failure cases.
pub async fn request_game_login(
    link: &mut GameLink,
    auth: &FrontAuth,
    password: &str,
) -> Result<(), GameLoginError> {
    let payload = game_login_payload(auth.auth_key(), auth.user_id(), password)?;
    link.send(REQUEST_LOGIN, &payload).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
