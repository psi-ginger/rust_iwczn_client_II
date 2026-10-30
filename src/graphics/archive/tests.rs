//! Unit tests for the `.sz`/`.mz` read seam and its loose fallback.
//!
//! **Synthetic fixtures** (R2: no real `.sz`/`.mz` exists in the tree).
//! Provenance for the control flow each test pins:
//!
//! | Behaviour | Built from |
//! |---|---|
//! | `unzOpen` then member scan | `XSPRObj.cpp:435-446` |
//! | `_tcsupr` on both names (case-insensitive) | `XSPRObj.cpp:448-451` |
//! | `bCompress` set **inside** the match branch | `XSPRObj.cpp:453` |
//! | short read -> `bCompressLoaded` stays false | `XSPRObj.cpp:459` |
//! | member found but unreadable -> error, **no** fallback | `XSPRObj.cpp:473-486` |
//! | archive/member absent -> loose-file path | `XSPRObj.cpp:487-503` |
//! | `.mz` member path for field objects | `FieldObject.cpp:342,345,368` |
//! | extracted bytes reach `LoadSprFromMemory` | `XSPRObj.cpp:477` |

use super::*;

use crate::graphics::spr::{TSPR_ENCODETYPE_SPR, TSPR_HEADER_LEN, TSPR_VERSION_NOPAL};

/// Writes one little-endian `i32`.
fn put_i32(bytes: &mut [u8], at: usize, value: i32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// A minimal but valid `.Spr` file: fixed header + one SPR stream row.
///
/// Provenance: `TSpr.h:32-47` (header) + `Graphics_PutSpr16T.cpp:41-44`
/// (one index per literal). Synthetic (R2).
fn sprite_bytes(width: i32, height: i32, image: &[u8]) -> Vec<u8> {
    use crate::graphics::spr::TSPR_HEADER_LEN;

    let mut file = vec![0u8; TSPR_HEADER_LEN];
    put_i32(&mut file, 0, TSPR_VERSION_NOPAL); // iVersion
    put_i32(&mut file, 4, TSPR_ENCODETYPE_SPR); // iEncodeType
    put_i32(&mut file, 8, width);
    put_i32(&mut file, 12, height);
    put_i32(&mut file, 16, width); // iHRes
    put_i32(&mut file, 20, height); // iVRes
    put_i32(&mut file, 24, image.len() as i32); // iImageDataSize
                                                // iImageNum (60) stays 0; ImageInfo[256] stays zeroed.
    file.extend_from_slice(image);
    file
}

/// One 3x1 sprite: three literal indices `0x11 0x22 0x33`.
fn small_sprite() -> Vec<u8> {
    sprite_bytes(3, 1, &[0x11, 0x22, 0x33])
}

// ---------------------------------------------------------------------------
// Member-first resolution
// ---------------------------------------------------------------------------

#[test]
fn member_present_is_served_by_the_archive() {
    let loose = small_sprite();
    let packed = small_sprite();
    let source = AssetSource::archive(vec![("HERO.SPR", MemberBytes::Data(&packed))], Some(&loose));

    let (bytes, resolved) = source.resolve("HERO.SPR").expect("member exists");
    assert_eq!(
        resolved,
        Resolved::FromArchive,
        "a matched member wins over the loose file (XSPRObj.cpp:453,477)"
    );
    assert_eq!(bytes, packed.as_slice());
    // ...and it is the archive bytes, not the loose bytes sitting right
    // beside it - the two differ only by identity here, so tag the loose
    // one and prove it was NOT chosen.
    let mut tagged_loose = small_sprite();
    tagged_loose[0] = 0xFF;
    let source = AssetSource::archive(
        vec![("HERO.SPR", MemberBytes::Data(&packed))],
        Some(&tagged_loose),
    );
    let (bytes, resolved) = source.resolve("HERO.SPR").unwrap();
    assert_eq!(resolved, Resolved::FromArchive);
    assert_eq!(
        bytes[0], TSPR_VERSION_NOPAL as u8,
        "archive bytes, not the tagged loose file"
    );
}

#[test]
fn member_lookup_is_case_insensitive_like_tcsupr() {
    // XSPRObj.cpp:448-451 upper-cases both the entry name and the sought
    // name before comparing, so case is not significant.
    let packed = small_sprite();
    let source = AssetSource::archive(vec![("HERO.SPR", MemberBytes::Data(&packed))], None);

    for query in ["HERO.SPR", "hero.spr", "HeRo.SpR"] {
        let (bytes, resolved) = source.resolve(query).expect("case must not matter");
        assert_eq!(resolved, Resolved::FromArchive, "{query}");
        assert_eq!(bytes, packed.as_slice(), "{query}");
    }

    // A different name still misses.
    assert!(matches!(
        source.resolve("VILLAIN.SPR"),
        Err(ArchiveError::MissingSource { .. })
    ));
}

// ---------------------------------------------------------------------------
// Loose fallback - the branch that decides everything (XSPRObj.cpp:473-503)
// ---------------------------------------------------------------------------

#[test]
fn absent_member_falls_back_to_the_loose_file() {
    let packed = small_sprite();
    let mut loose = small_sprite();
    loose[0] = 0xEE; // distinguishable from the packed bytes

    let source = AssetSource::archive(
        vec![("OTHER.SPR", MemberBytes::Data(&packed))],
        Some(&loose),
    );

    let (bytes, resolved) = source.resolve("HERO.SPR").expect("loose fallback");
    assert_eq!(
        resolved,
        Resolved::FromLoose,
        "member absent -> loose path (XSPRObj.cpp:487-503)"
    );
    assert_eq!(bytes[0], 0xEE, "the loose file's bytes");
}

#[test]
fn absent_archive_also_falls_back_to_the_loose_file() {
    // No members at all == unzOpen failed (XSPRObj.cpp:437 takes the
    // `if (uf)` branch only when the archive opened).
    let mut loose = small_sprite();
    loose[0] = 0xEE;
    let source = AssetSource::archive(vec![], Some(&loose));

    let (bytes, resolved) = source.resolve("HERO.SPR").expect("loose fallback");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(bytes[0], 0xEE);
}

#[test]
fn member_found_but_unreadable_is_an_error_with_no_fallback() {
    // XSPRObj.cpp:473-486: `bCompress` is TRUE (the name matched), so the
    // loose path at :487 is never reached - a short read is a hard error
    // even though a perfectly good loose file is sitting right there.
    let mut loose = small_sprite();
    loose[0] = 0xEE;
    let source = AssetSource::archive(vec![("HERO.SPR", MemberBytes::Unreadable)], Some(&loose));

    let err = source.resolve("HERO.SPR").expect_err("must not fall back");
    assert_eq!(
        err,
        ArchiveError::MemberReadFailed {
            member: "HERO.SPR".to_string()
        }
    );
    assert!(
        err.to_string().contains("HERO.SPR"),
        "the refusal names the member: {err}"
    );
    assert!(
        !err.to_string().contains("loose file to fall back"),
        "a read failure is not a missing-source error: {err}"
    );
}

#[test]
fn neither_member_nor_loose_is_a_named_error() {
    // The legacy falls back to the loose path and then fails the
    // LoadSprFromFile too (XSPRObj.cpp:491-503).
    let source = AssetSource::archive(vec![("OTHER.SPR", MemberBytes::Data(&[]))], None);
    let err = source.resolve("HERO.SPR").expect_err("nothing to serve");
    assert_eq!(
        err,
        ArchiveError::MissingSource {
            member: "HERO.SPR".to_string()
        }
    );
    assert!(err.to_string().contains("HERO.SPR"), "{err}");
}

#[test]
fn loose_only_source_resolves_the_loose_bytes() {
    let mut loose = small_sprite();
    loose[0] = 0xEE;
    let source = AssetSource::loose(&loose);

    let (bytes, resolved) = source.resolve("anything.spr").expect("loose serves");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(bytes[0], 0xEE);
}

// ---------------------------------------------------------------------------
// DoD 10.5 - extracted bytes decode through the §6.1 sprite loader
// ---------------------------------------------------------------------------

#[test]
fn archive_and_loose_bytes_both_decode_through_the_spr_loader() {
    let packed = small_sprite();
    let mut loose = small_sprite();
    // Tag the loose copy's first IMAGE byte — offset 0 is `iVersion`, not
    // pixel data; the image stream starts at TSPR_HEADER_LEN (3136).
    loose[TSPR_HEADER_LEN] = 0xEE;

    let from_archive =
        AssetSource::archive(vec![("HERO.SPR", MemberBytes::Data(&packed))], Some(&loose));
    let from_loose = AssetSource::archive(vec![], Some(&loose));

    let (sprite, resolved) = from_archive
        .load_sprite("HERO.SPR", RgbMode::Rgb565)
        .expect("archive member decodes");
    assert_eq!(resolved, Resolved::FromArchive);
    assert_eq!(sprite.header.i_x_size, 3);
    assert_eq!(sprite.header.i_y_size, 1);
    assert_eq!(
        sprite.image.pixels,
        vec![Some(0x11), Some(0x22), Some(0x33)],
        "the archive bytes reach load_sprite unchanged"
    );
    assert_eq!(sprite.image.bytes_consumed, 3);
    assert_eq!(sprite.embedded_palette, None, "v10 has no palette");

    let (loose_sprite, resolved) = from_loose
        .load_sprite("HERO.SPR", RgbMode::Rgb565)
        .expect("loose bytes decode");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(
        loose_sprite.image.pixels,
        vec![Some(0xEE), Some(0x22), Some(0x33)],
        "the fallback path decodes its own bytes, not the member's"
    );
}

#[test]
fn mz_members_follow_the_same_seam() {
    // FieldObject.cpp:342 builds `FieldObject\%s.mz` and reads the member
    // the same way the sprite path does.
    let packed = small_sprite();
    let source = AssetSource::archive(vec![("DUMMY.mz", MemberBytes::Data(&packed))], None);

    // Case-insensitive the same way (FieldObject.cpp uses unzOpen too).
    let (_, resolved) = source.resolve("DUMMY.MZ").expect("mz member");
    assert_eq!(resolved, Resolved::FromArchive);

    // An absent .mz member falls back to loose bytes as well.
    let mut loose = small_sprite();
    loose[0] = 0xEE;
    let source = AssetSource::archive(vec![("OTHER.mz", MemberBytes::Data(&packed))], Some(&loose));
    let (bytes, resolved) = source.resolve("DUMMY.mz").expect("mz fallback");
    assert_eq!(resolved, Resolved::FromLoose);
    assert_eq!(bytes[0], 0xEE);

    // A found-but-unreadable .mz member is an error, no fallback.
    let source = AssetSource::archive(vec![("DUMMY.mz", MemberBytes::Unreadable)], Some(&loose));
    assert!(matches!(
        source.resolve("DUMMY.mz"),
        Err(ArchiveError::MemberReadFailed { .. })
    ));
}

#[test]
fn corrupt_member_bytes_surface_the_spr_error_not_a_panic() {
    // The seam hands whatever it resolved to load_sprite, so a member that
    // is present but not a sprite is a typed SprError (P3-2).
    let truncated = vec![0u8; 100];
    let source = AssetSource::archive(vec![("BAD.SPR", MemberBytes::Data(&truncated))], None);
    let err = source
        .load_sprite("BAD.SPR", RgbMode::Rgb565)
        .expect_err("not a sprite");
    assert!(
        matches!(
            err,
            AssetError::Sprite(SprError::HeaderTruncated { got: 100, .. })
        ),
        "{err:?}"
    );

    // And an unreadable member reaches the caller as the archive error.
    let source = AssetSource::archive(vec![("BAD.SPR", MemberBytes::Unreadable)], None);
    assert!(matches!(
        source.load_sprite("BAD.SPR", RgbMode::Rgb565),
        Err(AssetError::Archive(ArchiveError::MemberReadFailed { .. }))
    ));
}
