//! Step 3.3 — Front login, response parsing, and the `auth_key` handoff
//! token (`CONTRIBUTING.md:67-70`).
//!
//! # The legacy flow this step ports
//!
//! The login dialog's default (non-region) build sends
//! `FRONTMSG_REQUEST_LOGIN` from `LogIn.cpp:730-747`: `usSize = sizeof(
//! sRequest_LogIn)` (`:738`), `usCmd = FRONTMSG_REQUEST_LOGIN` (`:739`),
//! `MStrCpy` into `userid`/`userpw` (`:741-742`; `MStrCpy` is
//! `StringCchCopy` — `Lib/commonutil.cpp:6-16`), and the MAC address from
//! `GetMacAddress` (`:744`). The legacy front server refuses any frame whose
//! `usSize` is not exactly `sizeof(sRequest_LogIn)` and closes the socket
//! (`FrontClientProtocol.cpp:352-360`), then forwards the three 20-byte
//! fields verbatim (`:371-381`); the server port reads the same first 40
//! bytes (`services/front/src/main.rs:218-222`).
//!
//! The answer comes back as `FRONTMSG_RESPONSE_LOGIN`
//! (`Client-Winsock.cpp:283`, handler below at `:350-397`):
//!
//! 1. `siAccountStatus == -1` aborts with a generic error **before** the
//!    result switch is even reached (`:287-298`);
//! 2. `usResult == 0` flips `clUserState` to `USERSTATE_FRONT_AUTHOK`
//!    (`:354`), captures **`siAuthKey` into the client's `siAuthKey`
//!    global** (`:356` — the token `LogIn.cpp:1022/1039` later hands the
//!    Game Server in MOD-04), the service age from `m_uiRealAge` (`:359`),
//!    the canonical id echo from `userid[20]` (`:377`), and reads
//!    `siGameMasterLevel` / `szServerName` (`:384-396`);
//! 3. every other result is a refusal with a dialog message (`:400` onward).
//!
//! This module owns the request builder, the byte-exact response parser, and
//! the capture step that produces [`FrontAuth`] — the state holder. An
//! [`AuthKey`] can only enter client state through
//! [`LoginResponse::capture`] *after* the frame has passed every validation
//! below, so no token can be captured from a truncated, foreign, or rejected
//! response (dispatch scope item 4: captured safely, no premature mutation).
//!
//! # Opcodes — triple-confirmed
//!
//! | Opcode | Name | Legacy header | Server port |
//! |---|---|---|---|
//! | 1 | `FRONTMSG_REQUEST_LOGIN` | `FrontMsg.h:44` | `opcodes_front.rs:15` (`RequestLogin`) |
//! | 1000 | `FRONTMSG_RESPONSE_LOGIN` | `FrontMsg.h:93` | `opcodes_front.rs:63` (`ResponseLogin`) |
//!
//! The region-specific variants (Netmarble/HanAuth/Porte/English/…, opcodes
//! 9–24) exist for other builds; the vanilla build this port mirrors sends
//! **1** (`LogIn.cpp:727-747`, the `#else` branch of the region chain), and
//! the server port accepts 1 and 21 as the same credential prefix
//! (`main.rs:168-173`).
//!
//! # Request frame (client → server, always ciphered)
//!
//! `sRequest_LogIn` (`FrontMsg.h:336-348`) sits under the file's
//! `#pragma pack(1)` (`FrontMsg.h:117`, popped at `:655`), so it is byte
//! packed: 5-byte header + 60-byte payload = **`usSize` 65** on the wire.
//!
//! | Payload bytes | Field | Cite |
//! |---|---|---|
//! | 0..20 | `userid[20]` | `FrontMsg.h:344` |
//! | 20..40 | `userpw[20]` | `FrontMsg.h:345` |
//! | 40..60 | `macaddress[20]` | `FrontMsg.h:347` |
//!
//! All three are C strings: bytes up to the first NUL, zero-filled
//! afterwards — `StringCchCopy` semantics (`commonutil.cpp:14`). The legacy
//! formats the MAC as six uppercase hex pairs (`%02X` × 6, at most 12 chars)
//! or leaves the buffer all-zero when no adapter matches
//! (`Client.cpp:7253-7303`); this port takes it as a parameter rather than
//! inventing platform data (F-01/F-02: live values pending real-Front
//! ingestion). A field that cannot fit its 20 bytes with a terminator is
//! refused ([`LoginError::CredentialTooLong`]) — the legacy transmitted the
//! truncated copy `StringCchCopy` leaves behind; this port does not send
//! silently shrunken credentials.
//!
//! The send goes through [`FrontLink::send`], enciphered with the session
//! cipher like every other front frame — QC decision F-04: 100 % cipher
//! integrity, no plaintext send path.
//!
//! # Response frame (server → client, plaintext)
//!
//! `sResponse_LogIn` (`FrontMsg.h:419-442`) is also under `pack(1)`:
//! 5 + 108 = **`usSize` 113**, payload layout at the literal offsets —
//!
//! | Payload bytes | Field | Cite |
//! |---|---|---|
//! | 0..2 | `usResult` (u16) | `FrontMsg.h:423` |
//! | **2..6** | **`siAuthKey` (i32 LE)** | `FrontMsg.h:424` |
//! | 6..26 | `userid[20]` | `FrontMsg.h:427` |
//! | 26..28 | `siGameMasterLevel` (i16) | `FrontMsg.h:429` |
//! | 28..32 | `siBlockMode` (i32) | `FrontMsg.h:431` |
//! | 32..52 | `siBlockYear/Month/Day/Hour/Minute` (5 × i32) | `FrontMsg.h:432` |
//! | 52..82 | `szBlockReason[30]` | `FrontMsg.h:433` |
//! | 82..83 | `bAlreadyMakeCharSwitch` (bool, 1 byte) | `FrontMsg.h:435` |
//! | 83..103 | `szServerName[20]` | `FrontMsg.h:437` |
//! | 103..107 | `siAccountStatus` (i32) | `FrontMsg.h:438` |
//! | 107..108 | `m_uiRealAge` (u8) | `FrontMsg.h:440` |
//!
//! — 2+4+20+2+4+20+30+1+20+4+1 = **108 payload bytes**, independently pinned
//! by the server port as `LOGIN_RESPONSE_PAYLOAD_LEN` (`gz-proto/src/front.rs:24`)
//! with `auth_key` at payload offset 2 (`gz-proto/src/front.rs:288`). The
//! legacy front builds it with `usSize = sizeof(sendMsg)` and `usCmd =
//! FRONTMSG_RESPONSE_LOGIN` (`AuthProcess.cpp:86-92`) and writes it
//! plaintext (server→client is always clear — `front.rs` module docs,
//! asymmetry table). `siAuthKey` is the account's `idnum`
//! (`gz-proto/src/front.rs:97`, citing `AccountDBProcess.cpp:397`).
//!
//! # Result codes and the capture gate
//!
//! `usResult` (`FrontMsg.h:423`, corroborated by `gz-proto/src/front.rs:88-92`
//! and emitted by `main.rs:228-270`): `0` success, `1` unknown id, `2` wrong
//! password, `3` not an adult, `4` blocked, `5` database error. Kept **raw**
//! in [`LoginResponse::result`] — no enum is invented (Step 3.2 precedent:
//! only values with sources are named; escalation rule: do not guess).
//!
//! [`LoginResponse::capture`] reproduces the legacy's consumption order
//! exactly: the `siAccountStatus == -1` gate first (`Client-Winsock.cpp:287`),
//! then `usResult != 0` as a refusal (`:350`) — only `Ok` yields a
//! [`FrontAuth`] holding the [`AuthKey`]. Parsing stays pure: a rejected
//! frame still parses so callers can inspect the raw result and the server's
//! echoed fields; it just cannot move a token into state.
//!
//! # Parsing rules — strict on purpose
//!
//! 1. The opcode must be 1000 ([`LoginError::UnexpectedOpcode`]).
//! 2. The payload must be exactly 108 bytes: short is
//!    [`LoginError::Truncated`], surplus is [`LoginError::TrailingBytes`]
//!    (surfaced, never dropped). The legacy handler casts blindly and reads
//!    `siAccountStatus` at offset 103 from whatever arrived
//!    (`Client-Winsock.cpp:285-287`); this port refuses instead of reading
//!    out of bounds.
//! 3. Validation runs before any slicing, so no input can panic.
//! 4. Strings are C strings: cut at the first NUL, invalid UTF-8 replaced —
//!    byte-identical to the server port's reader (`gz-proto/src/wire.rs:239-243`).
//!
//! The legacy front closes sockets on credentials outside `[0-9a-Za-z_]`
//! (`FrontClientProtocol.cpp:283-322`); that policy stays server-side — a
//! refusal surfaces here as EOF/refusal from the link, not as a duplicated
//! client-side charset table.
//!
//! # Test addresses are synthetic (F-01)
//!
//! The suite runs on loopback with synthetic addresses — pending live
//! ingestion against the real Front (5700) and Game (4010) servers (QC
//! decision F-01 on CAR-2026-10-24-RETRO01; F-02: 5 s read timeout until
//! then). What the tests prove is byte arithmetic and cipher integrity, not
//! reachability; the live acceptance signal for this step is the server
//! port's `front: … login result=0 auth_key=…` line
//! (`main.rs:181-185`).

use crate::auth::front::{FrontError, FrontLink};
use crate::net::codec::Frame;

/// `FRONTMSG_REQUEST_LOGIN` (`FrontMsg.h:44`): the front login request,
/// opcode **1** — the vanilla build's login (`LogIn.cpp:739`), accepted by
/// the server port as `FrontMsg::RequestLogin` (`opcodes_front.rs:15`,
/// matched at `main.rs:168`).
pub const REQUEST_LOGIN: u16 = 1;

/// `FRONTMSG_RESPONSE_LOGIN` (`FrontMsg.h:93`): the front login response,
/// opcode 1000 — built at `AuthProcess.cpp:89`, consumed at
/// `Client-Winsock.cpp:283` (`opcodes_front.rs:63`).
pub const RESPONSE_LOGIN: u16 = 1000;

/// `sizeof(userid/userpw/macaddress)` (`FrontMsg.h:344-347`): every request
/// field is this many bytes, C-string semantics with zero-fill.
pub const CREDENTIAL_LEN: usize = 20;

/// `sizeof(sRequest_LogIn) - sizeof(sPacketHeader)` =
/// `20 + 20 + 20` under `pack(1)` (`FrontMsg.h:336-348`); the frame's
/// `usSize` is 65 (the legacy sends `sizeof(sendmsg)`, `LogIn.cpp:738`, and
/// the legacy front closes sockets on any other value,
/// `FrontClientProtocol.cpp:352-360`).
pub const LOGIN_REQUEST_PAYLOAD_LEN: usize = 60;

/// `sizeof(sResponse_LogIn) - sizeof(sPacketHeader)` = 108 under `pack(1)`
/// (`FrontMsg.h:419-442`); the frame's `usSize` is 113 (module docs,
/// response frame; `gz-proto/src/front.rs:24`).
pub const LOGIN_RESPONSE_PAYLOAD_LEN: usize = 108;

/// Why a login operation failed.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    /// The link layer failed — the send hit [`FrontError`] (e.g.
    /// [`FrontError::NotKeyed`] before the handshake, preserving the legacy's
    /// `bFrontServerSecretKeySwitch` gate), or the receive side surfaced a
    /// session/codec failure.
    #[error(transparent)]
    Front(#[from] FrontError),
    /// The frame's opcode is not the login response — dispatch drift or a
    /// frame this port's contract does not cover (`FrontMsg.h:93`).
    #[error("front opcode {cmd} is not FRONTMSG_RESPONSE_LOGIN (1000), FrontMsg.h:93")]
    UnexpectedOpcode {
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// A credential does not fit its 20-byte field with its NUL terminator
    /// (`FrontMsg.h:344-347`). `MStrCpy`/`StringCchCopy` would transmit the
    /// truncated remainder (`commonutil.cpp:14`, return value ignored by
    /// `LogIn.cpp:741-742`); this port refuses instead of silently
    /// shrinking a credential (module docs, request frame).
    #[error("{field} of {len} bytes does not fit its {CREDENTIAL_LEN}-byte field with its NUL terminator (FrontMsg.h:344-347)")]
    CredentialTooLong {
        /// The legacy field name (`userid`, `userpw`, `macaddress`).
        field: &'static str,
        /// Credential length (up to its first NUL) that failed to fit.
        len: usize,
    },
    /// The frame carries fewer than the 108 bytes `sResponse_LogIn` needs.
    /// The legacy would read `siAccountStatus` at offset 103 from a short
    /// buffer (`Client-Winsock.cpp:285-287`); this port refuses.
    #[error("login response payload of {got} bytes is short of the {expected} that sResponse_LogIn explains (FrontMsg.h:419-442)")]
    Truncated {
        /// `sizeof(sResponse_LogIn) - 5` = 108 bytes the struct explains.
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// The frame carries bytes past the 108-byte struct. Refused rather
    /// than consumed in silence: no trailing byte is ever dropped (Step 3.2
    /// acceptance criterion, carried into this step).
    #[error("login response payload of {got} bytes exceeds the {expected} that sResponse_LogIn explains — trailing bytes must not be silently dropped")]
    TrailingBytes {
        /// `sizeof(sResponse_LogIn) - 5` = 108 bytes the struct explains.
        expected: usize,
        /// Payload length actually received.
        got: usize,
    },
    /// `siAccountStatus == -1`: the legacy's generic pre-result rejection
    /// gate (`Client-Winsock.cpp:287-298`), checked **before** `usResult`
    /// just as the legacy does.
    #[error(
        "siAccountStatus is -1 — the legacy's generic rejection gate (Client-Winsock.cpp:287-298)"
    )]
    AccountStatusRejected,
    /// `usResult != 0`: the login failed (`FrontMsg.h:423` result codes,
    /// module docs, result codes). No [`AuthKey`] is captured.
    #[error("front login rejected with result {result} (FrontMsg.h:423)")]
    Rejected {
        /// The raw `usResult` the server sent.
        result: u16,
    },
}

/// Copies `value` into the 20-byte `slot` with `StringCchCopy` semantics:
/// bytes up to the first NUL, then zero-fill (the legacy's remainder is
/// uninitialized stack; zero-filling is observationally identical — the
/// server reads only up to the terminator).
///
/// # Errors
///
/// [`LoginError::CredentialTooLong`] when the value needs 20 or more bytes
/// and therefore cannot fit a terminator.
fn copy_credential(slot: &mut [u8], field: &'static str, value: &str) -> Result<(), LoginError> {
    debug_assert_eq!(slot.len(), CREDENTIAL_LEN, "one 20-byte field per call");
    let raw = value.as_bytes();
    let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    if len >= CREDENTIAL_LEN {
        return Err(LoginError::CredentialTooLong { field, len });
    }
    slot[..len].copy_from_slice(&raw[..len]);
    Ok(())
}

/// Builds the 60-byte payload of a `FRONTMSG_REQUEST_LOGIN`
/// (`sRequest_LogIn`, `FrontMsg.h:336-348`): `userid` at 0..20, `userpw` at
/// 20..40, `macaddress` at 40..60, each C-string + zero-fill.
///
/// `mac_address` takes the legacy's formatted value — six uppercase hex
/// pairs from `GetMacAddress`, or empty when no adapter matched
/// (`Client.cpp:7286-7288`, `:7265-7266`); this port does not synthesize
/// platform data (module docs, request frame).
///
/// # Errors
///
/// [`LoginError::CredentialTooLong`] for any field that cannot fit its 20
/// bytes with a terminator (nothing is built).
pub fn login_request_payload(
    user_id: &str,
    password: &str,
    mac_address: &str,
) -> Result<[u8; LOGIN_REQUEST_PAYLOAD_LEN], LoginError> {
    let mut payload = [0u8; LOGIN_REQUEST_PAYLOAD_LEN];
    copy_credential(&mut payload[0..20], "userid", user_id)?;
    copy_credential(&mut payload[20..40], "userpw", password)?;
    copy_credential(&mut payload[40..60], "macaddress", mac_address)?;
    Ok(payload)
}

/// Sends `FRONTMSG_REQUEST_LOGIN` carrying the credentials — `usSize` 65 on
/// the wire. The legacy send is `LogIn.cpp:730-747`.
///
/// Enciphered like every other front send (module docs, request frames) —
/// QC decision F-04: no plaintext exception.
///
/// # Errors
///
/// [`LoginError::CredentialTooLong`] when a credential does not fit
/// (nothing is written); then [`LoginError::Front`] —
/// [`FrontError::NotKeyed`] before the `SECRETKEY` handshake, then the
/// [`FrontLink::send`] failure cases.
pub async fn request_login(
    link: &mut FrontLink,
    user_id: &str,
    password: &str,
    mac_address: &str,
) -> Result<(), LoginError> {
    let payload = login_request_payload(user_id, password, mac_address)?;
    link.send(REQUEST_LOGIN, &payload).await?;
    Ok(())
}

/// The 4-byte front handoff token (`siAuthKey`, `FrontMsg.h:424`) — the
/// account's `idnum` (`gz-proto/src/front.rs:97`), carried into the Game
/// Server login as the legacy's `siAuthKey` global
/// (`Client-Winsock.cpp:356` → `LogIn.cpp:1039`, MOD-04 / Step 4.3).
///
/// Copyable and printable — it is a token, not a secret that guards the
/// process: the wire carries it in the clear server→client by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthKey(i32);

impl AuthKey {
    /// Wraps the raw `siAuthKey` bytes. Used by the capture path and by
    /// tests that stage a known token; state construction still goes
    /// through [`LoginResponse::capture`].
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    /// The raw 4-byte token, as `siAuthKey` / the server port's `auth_key: i32`.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

/// A successfully captured front login: the [`AuthKey`] plus the canonical
/// id the server echoed (`userid[20]`, `FrontMsg.h:427` — the legacy copies
/// it back over its input id at `Client-Winsock.cpp:377`).
///
/// Only constructible through [`LoginResponse::capture`], which runs after
/// the full frame validation and the legacy's rejection gates — the
/// dispatch's "captured safely, no premature mutation" invariant, enforced
/// by the type rather than by convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontAuth {
    auth_key: AuthKey,
    user_id: String,
}

impl FrontAuth {
    /// The handoff token for MOD-04 (Step 4.3's game login).
    #[must_use]
    pub const fn auth_key(&self) -> AuthKey {
        self.auth_key
    }

    /// The server's canonical account id echo.
    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

/// A fully parsed `FRONTMSG_RESPONSE_LOGIN` payload: all 108 bytes read at
/// their literal offsets (module docs, response frame), nothing left over,
/// nothing read out of bounds. Pure data — see [`LoginResponse::capture`]
/// for the state gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginResponse {
    /// `usResult` at payload 0..2 — raw, per Step 3.2's no-invented-enum
    /// precedent (module docs, result codes).
    pub result: u16,
    /// `siAuthKey` at payload 2..6 — the 4-byte handoff token.
    pub auth_key: i32,
    /// `userid[20]` at payload 6..26 — C string.
    pub user_id: String,
    /// `siGameMasterLevel` at payload 26..28.
    pub gm_level: i16,
    /// `siBlockMode` at payload 28..32.
    pub block_mode: i32,
    /// `siBlockYear/Month/Day/Hour/Minute` at payload 32..52, wire order.
    pub block_time: [i32; 5],
    /// `szBlockReason[30]` at payload 52..82 — C string.
    pub block_reason: String,
    /// `bAlreadyMakeCharSwitch` at payload 82..83 (`bool`, one byte; the
    /// server port encodes it as `u8`, `gz-proto/src/front.rs:133`) — any
    /// non-zero byte is true, matching C `bool` storage.
    pub already_made_char: bool,
    /// `szServerName[20]` at payload 83..103 — C string.
    pub server_name: String,
    /// `siAccountStatus` at payload 103..107 — `-1` is the legacy's
    /// generic-rejection sentinel (`Client-Winsock.cpp:287`).
    pub account_status: i32,
    /// `m_uiRealAge` at payload 107..108 (the legacy's `siServiceAge`,
    /// `Client-Winsock.cpp:359`).
    pub real_age: u8,
}

/// Little-endian `u16` at `off`.
fn le_u16(payload: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([payload[off], payload[off + 1]])
}

/// Little-endian `i16` at `off`.
fn le_i16(payload: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([payload[off], payload[off + 1]])
}

/// Little-endian `i32` at `off`.
fn le_i32(payload: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([
        payload[off],
        payload[off + 1],
        payload[off + 2],
        payload[off + 3],
    ])
}

/// Fixed-width C string: `len` bytes from `off`, cut at the first NUL,
/// invalid UTF-8 replaced — byte-identical to the server port's reader
/// (`gz-proto/src/wire.rs:239-243`).
fn cstr(payload: &[u8], off: usize, len: usize) -> String {
    let field = &payload[off..off + len];
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

impl LoginResponse {
    /// Parses a received [`Frame`] — opcode and payload checked together.
    ///
    /// # Errors
    ///
    /// As [`parse_login_response`].
    pub fn from_frame(frame: &Frame) -> Result<Self, LoginError> {
        parse_login_response(frame.cmd(), frame.payload())
    }

    /// The legacy's consumption step (`Client-Winsock.cpp:287-356`), in its
    /// order: the `siAccountStatus == -1` gate first, then `usResult == 0`
    /// — only then does [`FrontAuth`] (and with it the [`AuthKey`]) come
    /// into existence.
    ///
    /// Parsing stays pure: a rejected response still exposes its raw
    /// fields; this is the single door into client state.
    ///
    /// # Errors
    ///
    /// [`LoginError::AccountStatusRejected`] for `siAccountStatus == -1`;
    /// [`LoginError::Rejected`] for any `usResult != 0`.
    pub fn capture(&self) -> Result<FrontAuth, LoginError> {
        if self.account_status == -1 {
            return Err(LoginError::AccountStatusRejected);
        }
        if self.result != 0 {
            return Err(LoginError::Rejected {
                result: self.result,
            });
        }
        Ok(FrontAuth {
            auth_key: AuthKey::new(self.auth_key),
            user_id: self.user_id.clone(),
        })
    }
}

/// Parses a `FRONTMSG_RESPONSE_LOGIN` **payload** under the module's strict
/// rules: opcode first, then exactly [`LOGIN_RESPONSE_PAYLOAD_LEN`] bytes —
/// short refused as [`LoginError::Truncated`], surplus surfaced as
/// [`LoginError::TrailingBytes`] — then every field at its literal offset
/// (module docs, response frame).
///
/// # Errors
///
/// [`LoginError::UnexpectedOpcode`] unless `cmd` is [`RESPONSE_LOGIN`];
/// [`LoginError::Truncated`] / [`LoginError::TrailingBytes`] when
/// `payload.len() != 108`. All validation runs before any slicing, so no
/// input can panic or read out of bounds.
pub fn parse_login_response(cmd: u16, payload: &[u8]) -> Result<LoginResponse, LoginError> {
    if cmd != RESPONSE_LOGIN {
        return Err(LoginError::UnexpectedOpcode { cmd });
    }
    let got = payload.len();
    if got < LOGIN_RESPONSE_PAYLOAD_LEN {
        return Err(LoginError::Truncated {
            expected: LOGIN_RESPONSE_PAYLOAD_LEN,
            got,
        });
    }
    if got > LOGIN_RESPONSE_PAYLOAD_LEN {
        return Err(LoginError::TrailingBytes {
            expected: LOGIN_RESPONSE_PAYLOAD_LEN,
            got,
        });
    }
    // The exact 108 bytes are proven above; every slice below is in bounds.
    Ok(LoginResponse {
        result: le_u16(payload, 0),
        auth_key: le_i32(payload, 2),
        user_id: cstr(payload, 6, 20),
        gm_level: le_i16(payload, 26),
        block_mode: le_i32(payload, 28),
        block_time: [
            le_i32(payload, 32),
            le_i32(payload, 36),
            le_i32(payload, 40),
            le_i32(payload, 44),
            le_i32(payload, 48),
        ],
        block_reason: cstr(payload, 52, 30),
        already_made_char: payload[82] != 0,
        server_name: cstr(payload, 83, 20),
        account_status: le_i32(payload, 103),
        real_age: payload[107],
    })
}

#[cfg(test)]
mod tests;
