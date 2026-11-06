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

/// The whole keybind table as code: event + state → optional intent (D2).
/// Pure; the only side effect is `UiState`. Never panics (P4-C4 invariant 3).
///
/// Every row of the §7.3 table is tested through T1–T7 (§9); the guard is
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
                if ui.chat_open {
                    // E3: cancel while typing — UI-only, no intent leaves
                    // the translator (D2).
                    ui.chat_open = false;
                    ui.line.clear();
                    None
                } else {
                    // E4: nothing to cancel — ask the UI to close (D2:
                    // `validate` refuses it, UI-only).
                    Some(Intent::CloseDialog)
                }
            }
            VK_INSERT => {
                if ui.chat_open {
                    // §7.3: the typing state swallows the key — the line
                    // buffer owns the keyboard until Enter or Escape.
                    None
                } else {
                    // E5: the frozen sit toggle (Q2 — the server toggles).
                    Some(Intent::SitToggle)
                }
            }
            // E6: unknown keys are discarded outright — nothing holds
            // them, because there is nothing to hold them (D1).
            _ => None,
        },
        // §2.2-4: every binding fires on `KeyDown`; releases are accepted
        // and ignored (E6).
        InputEvent::KeyUp { .. } => None,
        InputEvent::CharInput { ch } => {
            // §2.2-5: typing enters only while the chat line is open; the
            // buffer grows unbounded by design — the cap is refused at
            // flush by `validate`, never applied mid-word (D3, E10).
            if ui.chat_open {
                ui.line.push(ch);
            }
            None
        }
        // D8: `Move` is hover/tracking material for 8.1 and the clicks are
        // 8.1's — none of them ever reaches the enqueue path, which is
        // why no key-shifting machinery exists here.
        InputEvent::Move { .. } | InputEvent::LeftDown { .. } | InputEvent::LeftUp { .. } => None,
    }
}
