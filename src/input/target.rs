//! Step 8.1 §7.2 — click-target selection: [`pick_target`], the math core
//! of legacy `cltUnderMouseObject::FindLandChar`
//! (`UnderMouseObject.cpp:150-190`), over caller-supplied candidates.
//!
//! Machinery split (WO-8.1 D13, P4-C2): the input layer owns no world
//! state, so classification (`IsEnemy`/`IsNPC`,
//! `MouseOperation.cpp:3259-3261/:3267/:3275-3277/:3283`) and the world
//! filters (self/dead/horse, `UnderMouseObject.cpp:160-162`) arrive
//! pre-computed on the candidate from 8.3's snapshot; only the *math* —
//! the Chebyshev ≤ 3-tile filter, the strict `RealRect` containment and
//! the nearest-center selection — lives here.

/// How the caller's world classifies a candidate — legacy `GetMouseMode`'s
/// bits, computed from world state the input layer does not own (D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetClass {
    /// `IsEnemy` → `MOUSEMODE_ATTACK` (`MouseOperation.cpp:3259-3261`,
    /// `:3275-3277`) → `ORDER_ATTACK` (`:5544`, `:5607-5631`).
    Enemy,
    /// Non-enemy NPC → `MOUSEMODE_CHARNPC` (`:3267`) → legacy dialog
    /// (`:4138+`, Q3-excluded) — the port **discards** the click (D12).
    Npc,
    /// Non-enemy player → `MOUSEMODE_OTHERUSER` (`:3283`) → legacy's move
    /// branch (`:5409`); the port emits `MoveTo` (D12).
    OtherUser,
}

/// `RealRect` of a candidate sprite (`Char-Client.cpp:1015`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealRect {
    /// Strict left bound (`< x`, `:1015`).
    pub left: i32,
    /// Strict top bound (`< y`, `:1016`).
    pub top: i32,
    /// Strict right bound (`x <`, `:1015`).
    pub right: i32,
    /// Strict bottom bound (`y <`, `:1016`).
    pub bottom: i32,
}

/// One clickable character candidate — a world snapshot supplied by the
/// caller (D10/D13); `translate` never mutates the list it sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharTarget {
    /// Unique id → `Intent::AttackTarget.target_char` (frozen `Order::Attack`).
    pub id: u32,
    /// The candidate's map tile (`GetX/GetY`, `FindLandChar:156`).
    pub tile: (i32, i32),
    /// Real-space sprite rect — strict containment (`Char-Client.cpp:1015-1016`).
    pub rect: RealRect,
    /// Real-space center (`siCenterX/Y`, `Char-Client.cpp:1018`).
    pub center: (i32, i32),
    /// The caller's classification (D13).
    pub class: TargetClass,
}

/// `FindLandChar`'s math core (`UnderMouseObject.cpp:150-190`): the
/// Chebyshev ≤ 3-tile filter (`:156-159`), strict `RealRect` containment
/// (`Char-Client.cpp:1015-1016`), length = Chebyshev distance to the
/// candidate center (`:1018`); nearest wins, ties → first in slice order
/// (D13, the stated WO-8.1 §8 divergence from legacy's pass-1 first-hit:
/// `:168-175` first-arrived + `:212-215` strictly-nearer pass-2).
///
/// Lifetime: written with the elided single-input form — clippy's
/// `needless_lifetimes` (`-D warnings`) rejects the explicit `<'a>` the
/// WO's sketch carried; elision binds the output to `candidates`'s
/// lifetime either way (one input reference).
#[must_use]
pub fn pick_target(
    candidates: &[CharTarget],
    click_tile: (i32, i32),
    click_real: (i32, i32),
) -> Option<&CharTarget> {
    let mut best: Option<(&CharTarget, i32)> = None;
    for candidate in candidates {
        // :156-159 — Chebyshev tile distance; beyond 3 tiles: not a candidate.
        let tile_len = (candidate.tile.0 - click_tile.0)
            .abs()
            .max((candidate.tile.1 - click_tile.1).abs());
        if tile_len > 3 {
            continue;
        }
        // Char-Client.cpp:1015-1016 — strict open interval, both axes.
        let inside = candidate.rect.left < click_real.0
            && click_real.0 < candidate.rect.right
            && candidate.rect.top < click_real.1
            && click_real.1 < candidate.rect.bottom;
        if !inside {
            continue;
        }
        // Char-Client.cpp:1018 — length to the sprite center.
        let length = (candidate.center.0 - click_real.0)
            .abs()
            .max((candidate.center.1 - click_real.1).abs());
        // D13: strictly nearer replaces; a tie keeps the earlier candidate
        // (`:212-215` strict `<` + `:168-175` first-hit on ties).
        match best {
            Some((_, best_len)) if length >= best_len => {}
            _ => best = Some((candidate, length)),
        }
    }
    best.map(|(candidate, _)| candidate)
}
