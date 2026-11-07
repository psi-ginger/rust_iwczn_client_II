//! Step 8.0 §7.1 — [`InputEvent`], the raw OS-shaped value type: one input
//! fact per window message, and equally a value a test builds by hand
//! (P4-C4 headless — no HWND, no FFI, no Win32 object in this module).

/// One raw input fact, decoded from a window message (8.3) or built by a test.
/// Pure data: no HWND, no FFI, `Copy` — the legacy `cltKey`'s payload without
/// the legacy queue (`WO-8.0` D1: the struct survives, the queue does not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// Pointer moved to screen `(x, y)` (`WM_MOUSEMOVE`, `mainfunction.cpp:208`).
    /// Hover/tracking only — never enqueues (D8).
    Move {
        /// Screen x.
        x: i32,
        /// Screen y.
        y: i32,
    },
    /// Left button pressed at `(x, y)` (`mainfunction.cpp:232`; 8.1 owns clicks).
    LeftDown {
        /// Screen x.
        x: i32,
        /// Screen y.
        y: i32,
    },
    /// Left button released (`mainfunction.cpp:251`; 8.1).
    LeftUp {
        /// Screen x.
        x: i32,
        /// Screen y.
        y: i32,
    },
    /// A virtual-key press (`WM_KEYDOWN`, `mainfunction.cpp:1282`).
    KeyDown {
        /// The virtual-key code (the `wparam` of the message, 8.3).
        vk: u32,
    },
    /// A virtual-key release (`WM_KEYUP`, `mainfunction.cpp:1202`) — every
    /// binding fires on `KeyDown` (§2.2-4); `KeyUp` is accepted and ignored.
    KeyUp {
        /// The virtual-key code (the `wparam` of the message, 8.3).
        vk: u32,
    },
    /// A character for the chat line (`WM_CHAR` → line buffer; §2.2-5).
    CharInput {
        /// The decoded character.
        ch: char,
    },
}
