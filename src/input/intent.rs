//! Step 8.0 §7.2 — [`Intent`], the middle hop of the two-enum pipeline
//! (D2), and [`UiState`], the translator-local state that replaces the
//! legacy dialog flags. Both surfaces are declared in full now so 8.1/8.2
//! extend behavior, not the enum (the F-7.4-01 lesson, §2.2-1).

/// What the player wants, after keybind translation — the middle hop (D2).
/// Full surface declared now so 8.1/8.2 extend behavior, not the enum (F-7.4-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Walk to map tile `(x, y)` — screen math may be negative; the `u16`
    /// range check happens in `validate` (D3), never in the math.
    MoveTo {
        /// Target tile x, signed until validation (8.1's screen math feeds this).
        x: i32,
        /// Target tile y, signed until validation.
        y: i32,
    },
    /// Attack a resolved target (produced by 8.1's hit-test, not by `translate`).
    AttackTarget {
        /// The target's unique id (the frozen `Order::Attack` payload).
        target_char: u32,
    },
    /// Open the chat line (Enter, chat closed).
    ChatFocus,
    /// Flush the typed line (Enter, chat open) — validated to `ClientCommand::Chat` (D5).
    SendChat {
        /// The whole typed line; refused empty/over-cap by `validate` (D3).
        line: String,
    },
    /// Insert: sit toggle → frozen `Order::SitDown` (Q2: the server toggles).
    SitToggle,
    /// Escape with no dialog state: close/cancel signal (UI-only in 8.0).
    CloseDialog,
}

/// Translator-local UI state — the legacy dialog flags' modern replacement,
/// owned by the input layer; nothing else mutates it in Phase IV.
///
/// `Default` is the pre-world idle state: `in_world: false`,
/// `chat_open: false`, `line: String::new()`, `dialog_open: false`,
/// `repeat_vk: None` (WO-8.0 §7.2, WO-8.2 §7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiState {
    /// In-world stage reached (8.3 sets it; replaces `GetFrame() > 40`, D4-guard).
    pub in_world: bool,
    /// Chat line open (Enter toggles).
    pub chat_open: bool,
    /// The typed line; cleared on flush and on cancel (Q3: line buffer only).
    /// Control characters never enter it (8.2 D11); Backspace pops it (D11).
    pub line: String,
    /// An active dialog is open (8.2 D10) — the close direction of the
    /// legacy `NEXIT_DLG`/per-dialog Escape contract (`ItemInventory.cpp:1479`).
    /// Written by tests today and by the future dialog layer (Q3); Escape clears it.
    pub dialog_open: bool,
    /// Auto-repeat guard (8.2 D9): the `VK_RETURN`/`VK_INSERT` press not yet
    /// released. A repeated `KeyDown` of the same key while this is `Some(vk)`
    /// is an OS auto-repeat and is discarded; cleared by the matching `KeyUp`,
    /// by a pushed typed character, or by a different key's `KeyDown`.
    /// `VK_ESCAPE`/`VK_BACK` deliberately never set it (D9.6/D9.7).
    pub repeat_vk: Option<u32>,
}
