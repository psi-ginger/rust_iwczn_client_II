//! Step 8.0 §7.3 — the keybind table as code: the three bound `VK_*`
//! constants (D6: defined fresh here — the tree had no virtual-key codes
//! at baseline) and [`translate`], the pure event + state → optional
//! intent function (D2).
//!
//! Stated divergence (WO-8.0 §2.2-4, Haru/CAPA): the legacy fired
//! Return/Insert on `WM_KEYUP` (`mainfunction.cpp:1202/:1218/:1229`) and
//! Escape on `WM_KEYDOWN` (`:1304`); this port binds all of them
//! uniformly on `KeyDown` — one action per key press is the user-visible
//! contract either way.
//!
//! Step 8.2 (WO-8.2 D9) restores the one-action-per-physical-press half of
//! that contract under OS auto-repeat: a held `VK_RETURN`/`VK_INSERT`
//! fires once and its repeats are discarded until the matching `KeyUp`
//! (the legacy `WM_KEYUP` binding got this for free). `VK_ESCAPE` is
//! deliberately unguarded (D9.6 — its repeats emit only wire-refused,
//! idempotent signals) and `VK_BACK` deliberately repeats (D9.7 —
//! editing is the one place repetition is the UX).

use super::event::InputEvent;
use super::intent::{Intent, UiState};

/// Bound virtual keys — standard Win32 `VK_*` values (the legacy cases use the
/// SDK macros at these lines): RETURN `mainfunction.cpp:1218`,
/// INSERT `mainfunction.cpp:1229`, ESCAPE `mainfunction.cpp:1304`.
pub const VK_RETURN: u32 = 0x0D;
/// `VK_INSERT` — the sit/stand toggle key (`mainfunction.cpp:1229`).
pub const VK_INSERT: u32 = 0x2D;
/// `VK_ESCAPE` — the close/cancel key (`mainfunction.cpp:1304`).
pub const VK_ESCAPE: u32 = 0x1B;
/// `VK_BACK` — the chat-line edit key (8.2 D11; legacy edit-control
/// handling `InterfaceMgr/Interface/Edit.cpp:1024`). Auto-repeat pops
/// repeatedly by design (D9.7 — editing is the one place repetition is
/// the UX).
pub const VK_BACK: u32 = 0x08;

/// The whole keybind table as code: event + state → optional intent (D2).
/// Pure; the only side effect is `UiState`. Never panics (P4-C4 invariant 3).
///
/// Every row of the WO-8.0 §7.3 / WO-8.2 §7.2 tables is tested through
/// T1–T7 and T12–T23 (§9); the guard is
/// checked first, so no event of any kind is seen before the world is up
/// (D4, E7).
pub fn translate(event: InputEvent, ui: &mut UiState) -> Option<Intent> {
    // D4: the in-world guard replaces the legacy `GetFrame() > 40` warm-up
    // gate (`mainfunction.cpp:221,236,254,288,302,1367`) — one boolean,
    // checked first for every event (E7).
    if !ui.in_world {
        return None;
    }
    match event {
        InputEvent::KeyDown { vk } => match vk {
            VK_RETURN => {
                // D9.1: OS auto-repeat (repeated KeyDown, one KeyUp) must
                // not re-fire — the legacy WM_KEYUP binding gave one action
                // per physical press for free (E13).
                if ui.repeat_vk == Some(VK_RETURN) {
                    return None;
                }
                ui.repeat_vk = Some(VK_RETURN);
                if ui.chat_open {
                    // E2: flush — the line travels whole or not at all;
                    // `validate` refuses it, never shortens it (D3).
                    ui.chat_open = false;
                    Some(Intent::SendChat {
                        line: std::mem::take(&mut ui.line),
                    })
                } else {
                    // E1: focus the chat line, from a clean buffer (§7.3).
                    ui.chat_open = true;
                    ui.line.clear();
                    Some(Intent::ChatFocus)
                }
            }
            VK_ESCAPE => {
                // D9.6: deliberately no auto-repeat guard — Escape never
                // enqueues (`validate` refuses CloseDialog, D2) and its
                // transitions are idempotent (T14 pins this; legacy idiom
                // `NExitDlg.cpp:63` `IsCreate()`).
                if ui.chat_open {
                    // E3: cancel while typing — UI-only, no intent leaves
                    // the translator (D2). Focused input wins over an open
                    // dialog (E16, `PartyDlg.cpp:411-413`).
                    ui.chat_open = false;
                    ui.line.clear();
                    None
                } else if ui.dialog_open {
                    // E16/D10: close the active dialog
                    // (`ItemInventory.cpp:1479` `Hide()`) — still UI-only:
                    // `validate` refuses the signal (D2).
                    ui.dialog_open = false;
                    Some(Intent::CloseDialog)
                } else {
                    // E4: nothing to cancel — ask the UI to close (D2:
                    // `validate` refuses it, UI-only).
                    Some(Intent::CloseDialog)
                }
            }
            VK_INSERT => {
                // D9.1: a held Insert must enqueue one Order::SitDown per
                // physical press, not one per OS repeat — the unguarded
                // stream spams the wire (E13, P4-C3).
                if ui.repeat_vk == Some(VK_INSERT) {
                    return None;
                }
                ui.repeat_vk = Some(VK_INSERT);
                if ui.chat_open {
                    // §7.3: the typing state swallows the key — the line
                    // buffer owns the keyboard until Enter or Escape.
                    None
                } else {
                    // E5: the frozen sit toggle (Q2 — the server toggles).
                    Some(Intent::SitToggle)
                }
            }
            VK_BACK => {
                // E14/D11: Backspace edits the line — char-wise pop, safe
                // on an empty buffer (the legacy edit control's behavior
                // with the caret at end, `Edit.cpp:1024`). No guard: held
                // Backspace must repeat (D9.7, T16).
                if ui.chat_open {
                    ui.line.pop();
                }
                None
            }
            // E6: unknown keys are discarded outright — nothing holds
            // them, because there is nothing to hold them (D1). Pressing
            // a different key ends the auto-repeat window (D9.4 — T10's
            // `…Enter, F1, …` script depends on this clause).
            _ => {
                ui.repeat_vk = None;
                None
            }
        },
        // §2.2-4: every binding fires on `KeyDown`; releases are accepted
        // and ignored (E6) — except that the matching release ends the
        // auto-repeat window (D9.2, E13).
        InputEvent::KeyUp { vk } => {
            if ui.repeat_vk == Some(vk) {
                ui.repeat_vk = None;
            }
            None
        }
        InputEvent::CharInput { ch } => {
            // §2.2-5: typing enters only while the chat line is open; the
            // buffer grows unbounded by design — the cap is refused at
            // flush by `validate`, never applied mid-word (D3, E10).
            // E15/D11: control characters never enter the line — the
            // legacy EDIT class consumed them as commands (`'\r'` is the
            // flush key, `'\x08'` the edit key), never as text.
            if ui.chat_open && !ch.is_control() {
                ui.line.push(ch);
                // D9.3: a character the player actually typed ends the
                // auto-repeat window (E13; also what keeps T1/T10's
                // KeyUp-less sequences firing).
                ui.repeat_vk = None;
            }
            None
        }
        // D8: `Move` is hover/tracking material for 8.1 and the clicks are
        // 8.1's — none of them ever reaches the enqueue path, which is
        // why no key-shifting machinery exists here.
        InputEvent::Move { .. } | InputEvent::LeftDown { .. } | InputEvent::LeftUp { .. } => None,
    }
}
