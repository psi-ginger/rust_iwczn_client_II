//! Shared little-endian byte readers, fixed-width C-string decoding, and the
//! credential copier.
//!
//! Introduced by Step 7.0's lean audit (recommendations C1–C2, applied under
//! the Custodian's Step 7.1 directive; C3 and C4 close the set under Step 6 —
//! `docs/agents/WO-step6-structure.md`): the readers below were previously
//! triplicated across `game::{char_info,dispatcher,world_entry}` and
//! duplicated across `auth::{login,world_list}`. They now live here once and
//! are reused by the `graphics` decoders, which length-gate every section
//! before reading. The credential copier (Step 6's C3) was previously
//! duplicated across `auth::login` and `game::login`.
//!
//! Every infallible reader **panics** if `at` runs past the end of the slice —
//! the exact contract the per-module copies had. Callers must gate the slice
//! length first (the repo's "length gate, then field read" convention); the
//! panic path is unreachable in gated code. The `try_`-prefixed checked readers
//! return [`None`] instead for callers that surface truncation as a typed
//! error.

/// Reads a little-endian `u16` at `at`. Panics if out of bounds — gate first.
#[inline]
pub fn u16_at(payload: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(
        payload[at..at + 2]
            .try_into()
            .expect("2 bytes checked by the length gate"),
    )
}

/// Reads a little-endian `i16` at `at`. Panics if out of bounds — gate first.
#[inline]
pub fn i16_at(payload: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(
        payload[at..at + 2]
            .try_into()
            .expect("2 bytes checked by the length gate"),
    )
}

/// Reads a little-endian `u32` at `at`. Panics if out of bounds — gate first.
#[inline]
pub fn u32_at(payload: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(
        payload[at..at + 4]
            .try_into()
            .expect("4 bytes checked by the length gate"),
    )
}

/// Reads a little-endian `i32` at `at`. Panics if out of bounds — gate first.
#[inline]
pub fn i32_at(payload: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(
        payload[at..at + 4]
            .try_into()
            .expect("4 bytes checked by the length gate"),
    )
}

/// Reads a little-endian `i64` at `at`. Panics if out of bounds — gate first.
#[inline]
pub fn i64_at(payload: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(
        payload[at..at + 8]
            .try_into()
            .expect("8 bytes checked by the length gate"),
    )
}

/// Checked little-endian `u16` at `at`: [`None`] if the slice is too short.
#[inline]
pub fn try_u16_at(payload: &[u8], at: usize) -> Option<u16> {
    payload
        .get(at..at + 2)?
        .try_into()
        .ok()
        .map(u16::from_le_bytes)
}

/// Checked little-endian `i16` at `at`: [`None`] if the slice is too short.
#[inline]
pub fn try_i16_at(payload: &[u8], at: usize) -> Option<i16> {
    payload
        .get(at..at + 2)?
        .try_into()
        .ok()
        .map(i16::from_le_bytes)
}

/// Checked little-endian `u32` at `at`: [`None`] if the slice is too short.
#[inline]
pub fn try_u32_at(payload: &[u8], at: usize) -> Option<u32> {
    payload
        .get(at..at + 4)?
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
}

/// Checked little-endian `i32` at `at`: [`None`] if the slice is too short.
#[inline]
pub fn try_i32_at(payload: &[u8], at: usize) -> Option<i32> {
    payload
        .get(at..at + 4)?
        .try_into()
        .ok()
        .map(i32::from_le_bytes)
}

/// Checked little-endian `i64` at `at`: [`None`] if the slice is too short.
#[inline]
pub fn try_i64_at(payload: &[u8], at: usize) -> Option<i64> {
    payload
        .get(at..at + 8)?
        .try_into()
        .ok()
        .map(i64::from_le_bytes)
}

/// Fixed-width C string: bytes up to the first NUL, lossily decoded — the
/// MBCS client stores names and text this way (`StringCchCopy`,
/// `Lib\commonutil.cpp:6-16`). This is the single reader behind the former
/// `char_info::fixed_str`, `dispatcher::cstring_lossy`, and the `cstr` bodies
/// in `auth::{login,world_list}`.
#[inline]
pub fn cstr_lossy(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// [`cstr_lossy`] over the `len` bytes starting at `off` — the offset+len form
/// the `auth` parsers used. Panics if the window is out of bounds — gate first.
#[inline]
pub fn cstr_at(payload: &[u8], off: usize, len: usize) -> String {
    cstr_lossy(&payload[off..off + len])
}

/// The fixed width of one credential field — `userid`/`userpw`/`macaddress`
/// on the front (`FrontMsg.h:344-347`) and `szID`/`szPassword` in the game
/// login (`CommonHeader.h:575`): 20 bytes each, terminator included.
const CREDENTIAL_FIELD_LEN: usize = 20;

/// A credential that cannot fit its fixed-width field with a NUL terminator,
/// reported by [`copy_credential`]. Each caller converts it explicitly
/// (`.map_err`) into its own error variant — the repo's conversion idiom
/// (`src/client/event_loop.rs:236-239`), not a blanket `From`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CredentialTooLong {
    /// The legacy field name (`userid`, `szID`, …).
    pub(crate) field: &'static str,
    /// Credential length (up to its first NUL) that failed to fit.
    pub(crate) len: usize,
}

/// Copies `value` into the fixed-width `slot` with `StringCchCopy`
/// semantics: bytes up to the first NUL, then zero-fill (`MStrCpy`,
/// `Lib\commonutil.cpp:6-16`; the legacy's remainder is uninitialized
/// stack — zero-filling is observationally identical, because the server
/// reads only up to the terminator, and the zero tail is exactly what
/// `StringCchCopy` leaves behind).
///
/// # Errors
///
/// [`CredentialTooLong`] when the value needs 20 or more bytes and therefore
/// cannot fit a terminator.
pub(crate) fn copy_credential(
    slot: &mut [u8],
    field: &'static str,
    value: &str,
) -> Result<(), CredentialTooLong> {
    debug_assert_eq!(
        slot.len(),
        CREDENTIAL_FIELD_LEN,
        "one 20-byte field per call"
    );
    let raw = value.as_bytes();
    let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    if len >= CREDENTIAL_FIELD_LEN {
        return Err(CredentialTooLong { field, len });
    }
    slot[..len].copy_from_slice(&raw[..len]);
    Ok(())
}

#[cfg(test)]
mod tests;
