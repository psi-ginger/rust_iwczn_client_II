//! `.sz` / `.mz` archive read seam with loose-file fallback.
//!
//! Format of record — the two call sites' control flow:
//!
//! - Sprite path: `GoonZuWorld/common/Char/CCD/XSPRObj.cpp:426-514`
//!   (builds `CCD\ImageData\%5s.sz`, `unzOpen`, scans members, falls back).
//! - Field-object path: `common/Map/FieldObject/FieldObject.cpp:342,345,368`
//!   (`FieldObject\%s.mz`, `unzOpen` → `unzOpenCurrentFile`).
//!
//! ## Fallback semantics (the load-bearing control flow)
//!
//! ```text
//! unzOpen(archive)                         XSPRObj.cpp:435
//!   if opened: scan members, _tcsupr both   :438-451  (case-insensitive)
//!     on match: bCompress = TRUE; read it   :453-461
//!       read complete  -> bCompressLoaded   :459
//!       read short     -> stays FALSE       :459
//!   if bCompress (member found)             :473
//!     loaded  -> use the archive bytes      :477
//!     NOT loaded -> error, bLoaded stays FALSE (NO fallback)  :479-486
//!   else (archive absent OR member absent)  :487
//!     -> loose-file path                    :489-503
//! ```
//!
//! The decisive detail: `bCompress` is set **inside** the member-match
//! branch (`:453`), so a **member that is absent falls through to the loose
//! file**, while a member that is **found but unreadable is an error with
//! no fallback** (`:473-486`). Both branches are reproduced exactly and
//! pinned separately by test.
//!
//! Per `WO-7.1.md` §6.4 this step models the seam as an in-memory archive
//! **of named members** plus a loose slice — not a zip central-directory
//! parser — so the semantics are testable headlessly.

use thiserror::Error;

use crate::graphics::palette::RgbMode;
use crate::graphics::spr::{load_sprite, LoadedSprite, SprError};

/// The bytes an archive holds for one member.
///
/// `Unreadable` models a member whose lookup succeeds but whose read comes
/// up short — `unzReadCurrentFile` returning less than
/// `file_info.uncompressed_size` (`XSPRObj.cpp:459`), where the legacy sets
/// `bCompressLoaded = true` only on a complete read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberBytes<'a> {
    /// A complete read (`XSPRObj.cpp:459`, true branch).
    Data(&'a [u8]),
    /// A short read (`XSPRObj.cpp:459`, false branch) — found, unusable.
    Unreadable,
}

/// Where an asset's bytes come from: a loose file, or an archive that may
/// fall back to a loose file (`XSPRObj.cpp:473-503`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetSource<'a> {
    /// Archive members keyed by name, matched case-insensitively the way
    /// `_tcsupr` on both sides makes it (`XSPRObj.cpp:448-451`).
    members: Vec<(&'a str, MemberBytes<'a>)>,
    /// The loose file the legacy reads when the archive or the member is
    /// absent (`XSPRObj.cpp:489-503`).
    loose: Option<&'a [u8]>,
}

/// Which side served the bytes — recorded so callers (and tests) can see
/// the fallback decision rather than guess it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Served by the archive member.
    FromArchive,
    /// Served by the loose file (archive absent, or member absent).
    FromLoose,
}

/// Why resolution failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArchiveError {
    /// The member was found but could not be read in full — **no** loose
    /// fallback here, matching `XSPRObj.cpp:473-486`.
    #[error("archive member `{member}` found but its read came up short (no loose fallback, XSPRObj.cpp:479-486)")]
    MemberReadFailed {
        /// The member name that was matched.
        member: String,
    },

    /// Neither the member nor a loose file could supply bytes — the legacy
    /// `FindFirstFile` miss then `LoadSprFromFile` failure
    /// (`XSPRObj.cpp:491-503`).
    #[error("no bytes for `{member}`: not in the archive and no loose file to fall back to")]
    MissingSource {
        /// The member name that was sought.
        member: String,
    },
}

impl<'a> AssetSource<'a> {
    /// A loose file only — the plain `LoadSprFromFile` path
    /// (`XSPRObj.cpp:503`).
    pub fn loose(bytes: &'a [u8]) -> Self {
        AssetSource {
            members: Vec::new(),
            loose: Some(bytes),
        }
    }

    /// An archive of members with a loose fallback beside it (the legacy
    /// `XSPRObj.cpp` arrangement).
    pub fn archive(members: Vec<(&'a str, MemberBytes<'a>)>, loose: Option<&'a [u8]>) -> Self {
        AssetSource { members, loose }
    }

    /// Resolves `member` member-first with loose fallback.
    ///
    /// Name matching is ASCII case-insensitive, reproducing `_tcsupr` on
    /// both the entry name and the sought name (`XSPRObj.cpp:448-451`).
    ///
    /// # Errors
    ///
    /// [`ArchiveError::MemberReadFailed`] when the member matches but its
    /// bytes are unusable (no fallback), [`ArchiveError::MissingSource`]
    /// when neither the member nor a loose file exists.
    pub fn resolve(&self, member: &str) -> Result<(&[u8], Resolved), ArchiveError> {
        for (name, bytes) in &self.members {
            if name.eq_ignore_ascii_case(member) {
                return match bytes {
                    MemberBytes::Data(data) => Ok((data, Resolved::FromArchive)),
                    // Found-but-unreadable stops here: the legacy does not
                    // fall through to the loose file (XSPRObj.cpp:473-486).
                    MemberBytes::Unreadable => Err(ArchiveError::MemberReadFailed {
                        member: member.to_string(),
                    }),
                };
            }
        }

        // Member (or whole archive) absent -> loose path (XSPRObj.cpp:487-503).
        match self.loose {
            Some(bytes) => Ok((bytes, Resolved::FromLoose)),
            None => Err(ArchiveError::MissingSource {
                member: member.to_string(),
            }),
        }
    }

    /// Resolves `member` and decodes the bytes through the §6.1 sprite
    /// loader — the seam's whole purpose: whichever side wins, the bytes
    /// reach the same `load_sprite`.
    ///
    /// # Errors
    ///
    /// Any [`ArchiveError`] from resolution, or any [`SprError`] from the
    /// decode.
    pub fn load_sprite(
        &self,
        member: &str,
        mode: RgbMode,
    ) -> Result<(LoadedSprite, Resolved), AssetError> {
        let (bytes, resolved) = self.resolve(member).map_err(AssetError::Archive)?;
        let sprite = load_sprite(bytes, mode).map_err(AssetError::Sprite)?;
        Ok((sprite, resolved))
    }
}

/// A failure from the seam: resolution or decode.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssetError {
    /// Resolution failed ([`ArchiveError`]).
    #[error(transparent)]
    Archive(#[from] ArchiveError),

    /// The resolved bytes did not decode ([`SprError`]).
    #[error(transparent)]
    Sprite(#[from] SprError),
}

#[cfg(test)]
mod tests;
