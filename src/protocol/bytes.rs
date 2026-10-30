//! Shared little-endian byte readers and fixed-width C-string decoding.
//!
//! Introduced by Step 7.0's lean audit (recommendations C1–C4, applied under
//! the Custodian's Step 7.1 directive): the readers below were previously
//! triplicated across `game::{char_info,dispatcher,world_entry}` and
//! duplicated across `auth::{login,world_list}`. They now live here once and
//! are reused by the `graphics` decoders, which length-gate every section
//! before reading.
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

#[cfg(test)]
mod tests;
