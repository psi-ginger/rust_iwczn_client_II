// Step 8.0 tests — the input core's eleven invariants (WO-8.0 §9, T1–T11).
//
// Pattern authorities: colocated unit tests (`src/client/commands/tests.rs`)
// and pure-function tables (`src/game/action/tests.rs`). F-01/P4-C4
// headless: every value here is built by hand — no window, no Win32, no
// network; the only channel is the in-process 7.4 `commands::channel()`.
// F-07: every expectation is an §8 E-row (or the §7.3/§7.4 contract row it
// cites), never read back from `translate`/`validate` under test.
use super::event::InputEvent;
use super::intent::{Intent, UiState};
use super::keybinds::{translate, VK_ESCAPE, VK_INSERT, VK_RETURN};
use super::validate::validate;
use crate::client::commands::{channel, ClientCommand, CommandSender};
use crate::game::action::{ChatMode, Order, MAX_CHAT_BYTES};
use std::time::{Duration, Instant};

// T1 — E1/E2: Enter focuses the chat line, the typed characters accumulate,
// and the second Enter flushes the whole line and closes the buffer.
#[test]
fn enter_focuses_chat_then_flushes_the_typed_line() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };

    let first = translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui);
    assert_eq!(
        first,
        Some(Intent::ChatFocus),
        "E1: Enter with the chat closed must focus the line"
    );
    assert!(ui.chat_open, "E1: the chat line must now be open");
    assert!(ui.line.is_empty(), "focus starts from a clean line (§7.3)");

    assert_eq!(
        translate(InputEvent::CharInput { ch: 'h' }, &mut ui),
        None,
        "typing is a state change only — no intent, no command"
    );
    assert_eq!(
        translate(InputEvent::CharInput { ch: 'i' }, &mut ui),
        None,
        "typing is a state change only — no intent, no command"
    );
    assert_eq!(ui.line, "hi", "the two characters accumulate in order");

    let second = translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui);
    assert_eq!(
        second,
        Some(Intent::SendChat {
            line: "hi".to_owned()
        }),
        "E2: Enter with the chat open must flush the typed line"
    );
    assert!(!ui.chat_open, "E2: the flush closes the line");
    assert!(
        ui.line.is_empty(),
        "the flushed line is taken whole, not copied"
    );
}

// T2 — E3: Escape while typing cancels: no intent leaves translate, the
// line closes and the typed text is discarded (UI-only, D2).
#[test]
fn escape_while_typing_cancels_the_line_without_a_command() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        Some(Intent::ChatFocus),
        "the line opens first"
    );
    translate(InputEvent::CharInput { ch: 'h' }, &mut ui);
    translate(InputEvent::CharInput { ch: 'i' }, &mut ui);
    assert_eq!(ui.line, "hi", "the typed text is present before the cancel");

    let intent = translate(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui);
    assert_eq!(
        intent, None,
        "E3: the cancel is UI-only — no intent, therefore no command"
    );
    assert!(!ui.chat_open, "E3: cancel closes the chat line");
    assert!(ui.line.is_empty(), "E3: cancel discards the typed text");
}

// T3 — E4: Escape with the chat closed emits CloseDialog, and validate
// refuses it — UI-only intents never enqueue (D2).
#[test]
fn escape_with_chat_closed_emits_close_dialog() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let intent = translate(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui)
        .expect("E4: Escape with nothing open must close/cancel");
    assert_eq!(intent, Intent::CloseDialog, "E4: the close/cancel signal");
    assert_eq!(
        validate(&intent),
        None,
        "CloseDialog is UI-only — validate never enqueues it (D2)"
    );
    assert_eq!(
        ui,
        UiState {
            in_world: true,
            ..UiState::default()
        },
        "E4 row: Escape alone changes no state"
    );
}

// T4 — E5/Q2: Insert maps to the frozen sit-down order — 240 kind-2, no
// payload; the server owns the toggle logic (action.rs:99-100).
#[test]
fn insert_maps_to_the_frozen_sit_down_order() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let intent = translate(InputEvent::KeyDown { vk: VK_INSERT }, &mut ui)
        .expect("E5: Insert with the chat closed is the sit toggle");
    assert_eq!(intent, Intent::SitToggle, "E5: the toggle intent");
    let command = validate(&intent).expect("SitToggle must reach the frozen order");
    assert_eq!(
        command,
        ClientCommand::Order(Order::SitDown),
        "Q2 pin: the frozen 240 kind-2 order, no payload (action.rs:99-100)"
    );
}

// T5 — E6: unknown and unbound input is discarded without a panic and
// without touching UiState — there is no queue to hold it (D1/P4-C1).
#[test]
fn unknown_and_unbound_input_is_discarded_without_panic() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let before = ui.clone();
    let events = [
        InputEvent::KeyDown { vk: 0x70 },    // F1 — unbound
        InputEvent::KeyUp { vk: VK_RETURN }, // releases never fire (§2.2-4)
        InputEvent::LeftDown { x: 3, y: 4 }, // clicks are 8.1's (D8)
        InputEvent::LeftUp { x: 3, y: 4 },
        InputEvent::CharInput { ch: 'x' }, // chat closed: not a typing state
    ];
    for event in events {
        assert_eq!(
            translate(event, &mut ui),
            None,
            "E6: {event:?} must be discarded without panic"
        );
    }
    assert_eq!(ui, before, "E6: discarded input leaves UiState untouched");
}

// T6 — E7: before the in-world guard, every one of the six event kinds is
// discarded — including a valid Enter/Insert pair — and UiState stays
// byte-identical (the D4 boolean replaces GetFrame() > 40).
#[test]
fn events_before_the_in_world_guard_are_discarded() {
    let mut ui = UiState {
        in_world: false,
        ..UiState::default()
    };
    let before = ui.clone();
    let events = [
        InputEvent::Move { x: 10, y: 20 },
        InputEvent::LeftDown { x: 1, y: 2 },
        InputEvent::LeftUp { x: 1, y: 2 },
        InputEvent::KeyDown { vk: VK_RETURN }, // would focus if allowed
        InputEvent::KeyDown { vk: VK_INSERT }, // would toggle if allowed
        InputEvent::KeyDown { vk: VK_ESCAPE },
        InputEvent::KeyUp { vk: VK_RETURN },
        InputEvent::CharInput { ch: 'a' }, // would type if allowed
    ];
    for event in events {
        assert_eq!(
            translate(event, &mut ui),
            None,
            "E7: {event:?} before in_world must be discarded"
        );
    }
    assert_eq!(ui, before, "E7: the guard leaves UiState byte-identical");
}

// T7 — E8/D8: a hover stream yields zero intents, zero commands and zero
// sends. The tiles are deliberately in range: if the Move arm ever leaked,
// validate would accept them and the channel would see them — which is
// exactly what must never happen (P4-C1: with no intent, there is nothing
// downstream that could hold or shift one).
#[tokio::test]
async fn move_streams_never_enqueue_a_command() {
    let (sender, mut receiver) = channel();
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let before = ui.clone();
    let mut intents = 0usize;
    let mut sends = 0usize;
    for i in 0..1_000i32 {
        if let Some(intent) = translate(InputEvent::Move { x: i, y: 999 - i }, &mut ui) {
            intents += 1;
            if let Some(command) = validate(&intent) {
                sender.send(command).expect("receiver alive in T7");
                sends += 1;
            }
        }
    }
    assert_eq!(intents, 0, "E8: a Move stream yields zero intents (D8)");
    assert_eq!(sends, 0, "E8: a Move stream yields zero commands");
    assert_eq!(ui, before, "hover tracking must not mutate UiState either");
    drop(sender);
    assert!(
        receiver.recv().await.is_none(),
        "E8: the channel must have seen nothing at all"
    );
}

// T8 — E9: the frozen Order::Move takes u16 — negative and over-range
// tiles are refused whole by validate, in-range bounds pass exactly.
#[test]
fn move_to_refuses_tiles_outside_the_u16_range() {
    for (x, y) in [(-1, 0), (0, -5), (65_536, 0), (0, 65_536)] {
        assert_eq!(
            validate(&Intent::MoveTo { x, y }),
            None,
            "E9: ({x}, {y}) must be refused, not clamped or wrapped"
        );
    }
    assert_eq!(
        validate(&Intent::MoveTo { x: 0, y: 0 }),
        Some(ClientCommand::Order(Order::Move { x: 0, y: 0 })),
        "E9: (0, 0) passes as the u16 lower bound"
    );
    assert_eq!(
        validate(&Intent::MoveTo {
            x: 65_535,
            y: 65_535
        }),
        Some(ClientCommand::Order(Order::Move {
            x: 65_535,
            y: 65_535
        })),
        "E9: (65535, 65535) passes as the u16 ceiling"
    );
}

// T9 — E10: empty and over-cap chat lines are refused by name — the
// refusal never shortens the line, and a line exactly at MAX_CHAT_BYTES
// (255, action.rs:53) still passes whole.
#[test]
fn chat_flush_refuses_empty_and_over_cap_lines_without_clamping() {
    assert_eq!(
        validate(&Intent::SendChat {
            line: String::new()
        }),
        None,
        "E10: an empty line is refused (action.rs:202-204)"
    );

    let over = "x".repeat(MAX_CHAT_BYTES + 1);
    assert_eq!(over.len(), 256, "E10: the stimulus is one byte over cap");
    let over_intent = Intent::SendChat { line: over.clone() };
    assert_eq!(
        validate(&over_intent),
        None,
        "E10: a 256-byte line is refused, never truncated"
    );
    assert_eq!(
        over_intent,
        Intent::SendChat { line: over },
        "the refused line must be untouched — refusals are never truncation"
    );

    let at_cap = "x".repeat(MAX_CHAT_BYTES);
    assert_eq!(
        at_cap.len(),
        255,
        "E10: MAX_CHAT_BYTES is 255 (action.rs:53)"
    );
    let command = validate(&Intent::SendChat {
        line: at_cap.clone(),
    })
    .expect("a 255-byte line must pass");
    assert_eq!(
        command,
        ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: at_cap
        },
        "the passing line travels whole, byte for byte"
    );
}

/// T10 — E12/D7: the batch latency pin (P4-C3).
///
/// Margin rationale: the budget is ≤ 16 ms **per event** (one 60 FPS
/// frame). This drives 100 scripted events — far more than one frame ever
/// carries — through the entire `translate → validate → send` chain and
/// demands the whole batch inside that single frame's budget: each event
/// therefore gets ~1/100 of its budget. Debug-build enum dispatch plus
/// mpsc push cost is µs-scale, so the expected batch lands around a
/// millisecond — ≥10× headroom under the assertion. On any flake the
/// finding is disclosed and adjudicated
/// (`CONTRIBUTING_PHASE4.md` P4-C3) — never silently relaxed.
#[tokio::test]
async fn the_enqueue_path_stays_within_the_16ms_frame_budget() {
    // 16 full cycles + the four-event tail, one flush each (see the script).
    const EXPECTED_ENQUEUED: usize = 17;

    let (sender, mut receiver) = channel();
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };

    // The script: 16 × [open, 'h', 'i', flush, unknown F1, hover Move]
    // + [open, 'h', 'i', flush] = exactly 100 events, 17 flushes.
    let mut script = Vec::with_capacity(100);
    for _ in 0..16 {
        script.extend([
            InputEvent::KeyDown { vk: VK_RETURN },
            InputEvent::CharInput { ch: 'h' },
            InputEvent::CharInput { ch: 'i' },
            InputEvent::KeyDown { vk: VK_RETURN },
            InputEvent::KeyDown { vk: 0x70 }, // F1: unknown, discarded
            InputEvent::Move { x: 5, y: 7 },  // hover: never enqueues (D8)
        ]);
    }
    script.extend([
        InputEvent::KeyDown { vk: VK_RETURN },
        InputEvent::CharInput { ch: 'h' },
        InputEvent::CharInput { ch: 'i' },
        InputEvent::KeyDown { vk: VK_RETURN },
    ]);
    assert_eq!(
        script.len(),
        100,
        "the latency script is exactly 100 events"
    );

    let started = Instant::now();
    let mut enqueued = 0usize;
    for event in script {
        if feed(event, &mut ui, &sender).is_some() {
            enqueued += 1;
        }
    }
    let elapsed = started.elapsed();

    assert_eq!(
        enqueued, EXPECTED_ENQUEUED,
        "every flush must enqueue exactly one command"
    );
    assert!(
        elapsed < Duration::from_millis(16),
        "E12/P4-C3: 100 events must complete translate+validate+enqueue \
         in < 16 ms, took {elapsed:?}"
    );

    drop(sender);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(
        drained, EXPECTED_ENQUEUED,
        "every enqueued command must drain — none lost"
    );
}

// T11 — E11/D5: the flushed chat carries the declared default mode,
// exactly — ChatMode::Normal (value 1, action.rs:148) — with no coercion
// anywhere in the hop.
#[test]
fn chat_flush_uses_the_declared_default_mode() {
    let command = validate(&Intent::SendChat {
        line: "hi".to_owned(),
    })
    .expect("a valid line must produce a command");
    assert_eq!(
        command,
        ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: "hi".to_owned()
        },
        "E11/D5: the declared default travels uncoerced"
    );
    assert_eq!(
        ChatMode::Normal as u8,
        1,
        "E11: Normal is the frozen value 1 (action.rs:148)"
    );
}

/// One event through the whole P4-C1 chain: `translate` → `validate` →
/// `sender.send` — synchronous, with no intermediate storage of any kind,
/// exactly the shape a production caller will use (D1/D7). Returns the
/// command when it was enqueued; the receiver is alive for every call in
/// these tests.
fn feed(event: InputEvent, ui: &mut UiState, sender: &CommandSender) -> Option<ClientCommand> {
    let intent = translate(event, ui)?;
    let command = validate(&intent)?;
    sender
        .send(command.clone())
        .expect("receiver alive for the batch");
    Some(command)
}

// ---------------------------------------------------------------------------
// Step 8.2 — WO-8.2 §9, T12–T23. APPENDED ONLY: the T1–T11 block above is
// byte-identical to the Step 8.0 delivery (DoD 13.2; evidence
// `t1-t11-unchanged.txt`). F-07: every assertion cites a §8 E-row (E13–E19
// new; E1–E12 inherited). P4-C4: values built by hand — no window, no
// Win32, no network.
// ---------------------------------------------------------------------------
use super::keybinds::VK_BACK;

// T12 — E13/D9.1/D9.2: a held Insert fires once per physical press; the
// OS auto-repeat stream is discarded until the matching KeyUp, which
// re-arms the key for the next press (legacy got this from WM_KEYUP —
// `mainfunction.cpp:1229`; the unguarded KeyDown bind spammed
// Order::SitDown on every repeat).
#[test]
fn held_insert_fires_once_until_released() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };

    let first = translate(InputEvent::KeyDown { vk: VK_INSERT }, &mut ui);
    assert_eq!(
        first,
        Some(Intent::SitToggle),
        "E13: the first press fires the toggle"
    );
    assert_eq!(
        ui.repeat_vk,
        Some(VK_INSERT),
        "E13/D9.1: the press opens the repeat window"
    );
    let held = ui.clone();
    for repeat in 1..5 {
        assert_eq!(
            translate(InputEvent::KeyDown { vk: VK_INSERT }, &mut ui),
            None,
            "E13/D9.1: auto-repeat {repeat} must be discarded, not re-fired"
        );
        assert_eq!(ui, held, "E13: a discarded repeat leaves UiState untouched");
    }

    assert_eq!(
        translate(InputEvent::KeyUp { vk: VK_INSERT }, &mut ui),
        None,
        "E6: releases never fire (8.0 row)"
    );
    assert_eq!(
        ui.repeat_vk, None,
        "E13/D9.2: the matching KeyUp ends the window"
    );
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_INSERT }, &mut ui),
        Some(Intent::SitToggle),
        "E13: a fresh press after the release fires again"
    );
}

// T13 — E13/E15: a held Enter cannot oscillate the chat line. Repeats —
// whether delivered as consecutive KeyDowns or interleaved with filtered
// '\r' CharInput (8.3's decode policy must not matter, D11) — are
// discarded while the window is open; a release ends it, so the next
// deliberate press flushes.
#[test]
fn held_enter_does_not_oscillate_the_chat_line() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        Some(Intent::ChatFocus),
        "E1: the first press opens the line"
    );
    assert_eq!(
        ui.repeat_vk,
        Some(VK_RETURN),
        "E13: the press opens the repeat window"
    );

    for _ in 0..3 {
        assert_eq!(
            translate(InputEvent::CharInput { ch: '\r' }, &mut ui),
            None,
            "E15: '\\r' is a command, never text"
        );
        assert!(ui.line.is_empty(), "E15: the line stays empty");
        assert_eq!(
            ui.repeat_vk,
            Some(VK_RETURN),
            "E13/D9.3: a filtered character does not end the window"
        );
    }
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        None,
        "E13/D9.1: the held repeat must not flush (no open/close flicker)"
    );
    assert!(ui.chat_open, "E13: the chat stays open — no oscillation");

    assert_eq!(
        translate(InputEvent::KeyUp { vk: VK_RETURN }, &mut ui),
        None,
        "E6: releases never fire (8.0 row)"
    );
    assert_eq!(ui.repeat_vk, None, "E13/D9.2: the release ends the window");
    translate(InputEvent::CharInput { ch: 'h' }, &mut ui);
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        Some(Intent::SendChat {
            line: "h".to_owned()
        }),
        "E13/E2: the next deliberate press flushes"
    );
    assert!(!ui.chat_open, "E2: the flush closes the line");
}

// T14 — E13-divergence (a)/E4: Escape auto-repeat is DELIBERATELY
// unguarded — a lone Escape and its repeats leave UiState byte-identical
// (T3's identity, generalized), because Escape never enqueues and its
// signals are idempotent (D9.6; legacy idiom `NExitDlg.cpp:63`). This
// pins the asymmetry so a future "helpful" Escape guard must revise
// WO-8.2 first.
#[test]
fn escape_repeats_are_deliberately_unguarded() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let before = ui.clone();
    for press in 1..=3 {
        assert_eq!(
            translate(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui),
            Some(Intent::CloseDialog),
            "E4/E13(a): press {press} emits the close/cancel signal"
        );
        assert_eq!(
            ui, before,
            "E4: Escape alone changes no state (T3 identity)"
        );
    }
    assert_eq!(
        validate(&Intent::CloseDialog),
        None,
        "D2: the signal never enqueues"
    );
}

// T15 — E14/D11: Backspace edits the line — char-wise pop, safe on an
// empty buffer (legacy edit-control behavior, `Edit.cpp:1024`), and the
// edited line flushes whole.
#[test]
fn backspace_edits_the_chat_line() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui);
    translate(InputEvent::CharInput { ch: 'h' }, &mut ui);
    translate(InputEvent::CharInput { ch: 'i' }, &mut ui);
    assert_eq!(ui.line, "hi", "the stimulus");

    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_BACK }, &mut ui),
        None,
        "E14: editing is a state change only"
    );
    assert_eq!(ui.line, "h", "E14: one char popped");
    translate(InputEvent::KeyDown { vk: VK_BACK }, &mut ui);
    assert_eq!(ui.line, "", "E14: two pops empty the line");
    translate(InputEvent::KeyDown { vk: VK_BACK }, &mut ui);
    assert_eq!(
        ui.line, "",
        "E14: Backspace on an empty line is a no-op (no panic)"
    );

    translate(InputEvent::CharInput { ch: 'o' }, &mut ui);
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        Some(Intent::SendChat {
            line: "o".to_owned()
        }),
        "E14/E2: the edited line flushes whole"
    );
}

// T16 — E14/D9.7: editing keys are deliberately NOT latched — held
// Backspace must repeat (the asymmetry with T12 is the design, not an
// oversight).
#[test]
fn backspace_autorepeat_is_intentional() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui);
    for ch in ['a', 'b', 'c'] {
        translate(InputEvent::CharInput { ch }, &mut ui);
    }
    assert_eq!(ui.line, "abc", "the stimulus");

    for step in ["ab", "a", ""] {
        assert_eq!(
            translate(InputEvent::KeyDown { vk: VK_BACK }, &mut ui),
            None,
            "E14: editing never emits an intent"
        );
        assert_eq!(
            ui.line, step,
            "E14/D9.7: consecutive presses pop with no KeyUp between ({step:?})"
        );
    }
}

// T17 — E15/D11: control characters never enter the line; printable
// characters (ASCII and non-ASCII) do, byte-exact (the cap is byte-based,
// `action.rs:53`).
#[test]
fn control_characters_never_enter_the_chat_line() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui);

    for ch in ['\r', '\x08', '\t', '\u{1}'] {
        assert_eq!(
            translate(InputEvent::CharInput { ch }, &mut ui),
            None,
            "E15: {ch:?} is discarded, never pushed"
        );
        assert!(ui.line.is_empty(), "E15: the line stays empty after {ch:?}");
    }
    translate(InputEvent::CharInput { ch: 'é' }, &mut ui);
    translate(InputEvent::CharInput { ch: '世' }, &mut ui);
    assert_eq!(ui.line, "é世", "E15: printable non-ASCII enters whole");
    assert_eq!(
        ui.line.len(),
        5,
        "E15: UTF-8 byte count (2 + 3) — the cap counts bytes"
    );
}

// T18 — E16/D10: Escape closes the active dialog — the close direction of
// the legacy per-dialog contract (`ItemInventory.cpp:1479`); the signal
// stays UI-only (D2) and the Escape arm never touches the repeat guard
// (D9.6).
#[test]
fn escape_closes_the_active_dialog() {
    let mut ui = UiState {
        in_world: true,
        dialog_open: true,
        ..UiState::default()
    };
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui),
        Some(Intent::CloseDialog),
        "E16: the close transition emits the signal"
    );
    assert!(!ui.dialog_open, "E16/D10: the dialog is closed");
    assert_eq!(
        ui.repeat_vk, None,
        "E13(a): the Escape arm never touches the guard (D9.6)"
    );
    assert_eq!(
        validate(&Intent::CloseDialog),
        None,
        "D2: UI-only — validate refuses it"
    );
}

// T19 — E16: the focused input wins — Escape cancels the chat line first
// (`PartyDlg.cpp:411-413` clears the input) and leaves an open dialog
// alone.
#[test]
fn chat_cancel_takes_priority_over_the_dialog() {
    let mut ui = UiState {
        in_world: true,
        chat_open: true,
        dialog_open: true,
        ..UiState::default()
    };
    ui.line.push_str("hi");
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui),
        None,
        "E3: chat cancel is UI-only"
    );
    assert!(!ui.chat_open, "E3: the chat closed");
    assert!(ui.line.is_empty(), "E3: the line was discarded");
    assert!(
        ui.dialog_open,
        "E16: the dialog is untouched — focused input wins"
    );
}

// T20 — E17: dialog state is non-modal — game keys keep working (legacy
// popups are custom-drawn in the game window; `mainfunction.cpp`'s
// WM_KEYDOWN switch `:1282` runs regardless).
#[test]
fn dialog_state_does_not_swallow_game_keys() {
    let mut ui = UiState {
        in_world: true,
        dialog_open: true,
        ..UiState::default()
    };
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_INSERT }, &mut ui),
        Some(Intent::SitToggle),
        "E17: Insert still sits with a dialog open"
    );
    assert_eq!(
        validate(&Intent::SitToggle),
        Some(ClientCommand::Order(Order::SitDown)),
        "E5/Q2: the frozen order (action.rs:94)"
    );
    assert_eq!(
        translate(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui),
        Some(Intent::ChatFocus),
        "E17: Enter still focuses chat with a dialog open"
    );
    assert!(ui.chat_open, "the chat opened");
    assert!(
        ui.dialog_open,
        "E17: the dialog is still open — neither state gates the other"
    );
}

// T21 — E18: arrow keys remain unbound (charter non-goal; the legacy
// movement block is dead code, `mainfunction.cpp:1205-1217` /
// `MouseOperation.cpp:273-522`) — down and up, discarded, untouched.
#[test]
fn arrow_keys_remain_unbound() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let before = ui.clone();
    for vk in [0x25, 0x26, 0x27, 0x28] {
        assert_eq!(
            translate(InputEvent::KeyDown { vk }, &mut ui),
            None,
            "E18: arrow KeyDown {vk:#04x} must be discarded"
        );
        assert_eq!(
            translate(InputEvent::KeyUp { vk }, &mut ui),
            None,
            "E18: arrow KeyUp {vk:#04x} must be discarded"
        );
        assert_eq!(ui, before, "E18: discarded arrows leave UiState untouched");
    }
}

// T22 — E19: the legacy no-op keys (empty F8/F11/F12 arms,
// `mainfunction.cpp:1378/:1287/:1292`) and the Win keys (window
// minimize `:1344-1350` — not ported, window management is not the
// input layer) are discarded untouched.
#[test]
fn legacy_no_op_keys_are_discarded() {
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };
    let before = ui.clone();
    for vk in [0x77, 0x7A, 0x7B, 0x5B, 0x5C] {
        assert_eq!(
            translate(InputEvent::KeyDown { vk }, &mut ui),
            None,
            "E19: KeyDown {vk:#04x} is a legacy no-op"
        );
        assert_eq!(ui, before, "E19: discarded input leaves UiState untouched");
    }
}

// T23 — E14+E16 integration: an edited line flushes through the whole
// P4-C1 chain (translate → validate → send), and the dialog-close signal
// never reaches the channel (D2).
#[tokio::test]
async fn the_edited_line_flushes_through_the_full_chain() {
    let (sender, mut receiver) = channel();
    let mut ui = UiState {
        in_world: true,
        ..UiState::default()
    };

    assert_eq!(
        feed(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui, &sender),
        None,
        "E1: focus is UI-only — nothing enqueues"
    );
    for ch in ['c', 'a', 't'] {
        feed(InputEvent::CharInput { ch }, &mut ui, &sender);
    }
    assert_eq!(ui.line, "cat", "E14: the stimulus");
    assert_eq!(
        feed(InputEvent::KeyDown { vk: VK_BACK }, &mut ui, &sender),
        None,
        "E14: editing enqueues nothing"
    );
    assert_eq!(ui.line, "ca", "E14: one pop");

    let flushed = feed(InputEvent::KeyDown { vk: VK_RETURN }, &mut ui, &sender)
        .expect("E14/E2: the edited line must enqueue");
    assert_eq!(
        flushed,
        ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: "ca".to_owned()
        },
        "E14/E11: whole line, declared default mode"
    );

    ui.dialog_open = true;
    assert_eq!(
        feed(InputEvent::KeyDown { vk: VK_ESCAPE }, &mut ui, &sender),
        None,
        "E16/D2: the close signal is refused by validate — never enqueued"
    );
    assert!(!ui.dialog_open, "E16: the dialog closed");

    drop(sender);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(
        drained, 1,
        "exactly the one flush reached the channel — the signal did not"
    );
}
