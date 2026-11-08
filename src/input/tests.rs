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

// Step 8.1 additions (WO-8.1 §9) — imports only; every T1-T11 body above
// stays byte-identical (DoD 2: the tests.rs diff is additions only).
use super::screen::{is_in_map_area, is_pt_in_map, real_to_tile, screen_to_tile, MapView};
use super::target::{CharTarget, RealRect, TargetClass};
use crate::game::action::encode_order;
use crate::graphics::screen::{draw_center_xy, ScreenMode};

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
// Step 8.1 tests — the mouse half (WO-8.1 §9, T12–T27).
//
// F-07: every expectation is an §8 E-row (E13–E29) carrying its legacy
// `file:line` or the §2.2 hand arithmetic — never read back from the code
// under test. P4-C4 headless: `MapView`/`CharTarget` values are built by
// hand (helpers at the bottom); no window, no Win32, no network. DoD 2:
// nothing above this line changes — pure additions.
// ---------------------------------------------------------------------------

/// T12 — E13: charter gate 1's round trip — `screen_to_tile` inverts the
/// frozen `draw_center_xy` over 3 view centers × 2 backbuffer rects ×
/// 11 tiles = 66 pairs (WO-8.1 §9).
///
/// Identity (§2.2-1): for any center the forward transform yields
/// `realx = 40(tx - ty)` and `realy = 20(tx + ty) + 20`, so
/// `tx' = (40tx + 20)/40 = tx` and `ty' = (40ty + 20)/40 = ty` — exact
/// for every tile ≥ 0 (remainder 20). Non-negative tiles only: past zero
/// the `+20` crosses the truncation boundary and legacy returns e.g.
/// `(-40 + 20)/40 = 0 ≠ -1` (C truncates toward zero, as Rust's `/`);
/// such tiles are refused by E16/E17/E19 before any order carries them.
#[test]
fn screen_to_tile_round_trips_draw_center_xy() {
    // The frozen backbuffer table (Client.cpp:3206-3208 / :3220-3222).
    let screens = [
        ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        ScreenMode::Mode1024x768.backbuffer(true).screen_rect(),
    ];
    assert_eq!(
        (screens[0].right + 1, screens[0].bottom + 1),
        (960, 680),
        "stimulus: the frozen 800x600 backbuffer"
    );
    assert_eq!(
        (screens[1].right + 1, screens[1].bottom + 1),
        (1200, 880),
        "stimulus: the frozen 1024x768-fullscreen backbuffer"
    );
    let centers = [(0, 0), (100, 100), (37, 91)];
    let tiles = [
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (100, 100),
        (101, 100),
        (100, 101),
        (399, 399),
        (799, 0),
        (0, 799),
        (250, 75),
    ];
    let mut checked = 0usize;
    for screen in &screens {
        let w = screen.right + 1;
        let h = screen.bottom + 1;
        for center in centers {
            for tile in tiles {
                let (px, py) = draw_center_xy(center.0, center.1, tile.0, tile.1, screen);
                assert_eq!(
                    screen_to_tile(px, py, center, screen),
                    tile,
                    "E13: tile {tile:?} at center {center:?} on the {w}x{h} rect must round-trip"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 66, "E13: 3 centers x 2 screens x 11 tiles");
}

/// T13 — E14: hand-computed goldens G1–G3 (WO-8.1 §2.2-2) — arithmetic
/// done by hand from `Map-Client.cpp:185-187` + `MapCommon.cpp:965-966`
/// over the frozen 960×680 rect (mid (480,340)), never recomputed here:
///
/// * G1: (480,340) → real (0,4020) → (0+4020)/40 = 100 r20 → (100,100)
/// * G2: (520,360) → real (40,4040) → (20+4040)/40 = 101, (-20+4040)/40 = 100
/// * G3: (440,360) → real (-40,4040) → (-20+4040)/40 = 100, (20+4040)/40 = 101
#[test]
fn screen_to_tile_matches_hand_computed_goldens() {
    let screen = ScreenMode::Mode800x600.backbuffer(false).screen_rect();
    assert_eq!(
        (screen.right, screen.bottom),
        (959, 679),
        "stimulus: the frozen 960x680 rect (Client.cpp:3206-3208)"
    );
    assert_eq!(
        screen_to_tile(480, 340, (100, 100), &screen),
        (100, 100),
        "E14: G1 — the rect center lands on the view center"
    );
    assert_eq!(
        screen_to_tile(520, 360, (100, 100), &screen),
        (101, 100),
        "E14: G2 — one tile east"
    );
    assert_eq!(
        screen_to_tile(440, 360, (100, 100), &screen),
        (100, 101),
        "E14: G3 — one tile south"
    );
}

/// T14 — E15: C-truncation goldens G4–G5 (WO-8.1 §2.2-2).
///
/// * G4: click (10,10) at center (0,0) → real (-470,-310);
///   (-235 + -310)/40 = -13.625 → **-13**, (235 + -310)/40 = -1.875 → **-1**
///   (floors would give (-14,-2)).
/// * G5: `real_to_tile(-41,-19)` → -41/2 = **-20** (floor: -21),
///   (-20-19)/40 = -0.975 → **0**, (20-19)/40 → 0 (floor paths: (-1,0)).
#[test]
fn screen_to_tile_truncates_toward_zero_like_legacy_c() {
    let screen = ScreenMode::Mode800x600.backbuffer(false).screen_rect();
    assert_eq!(
        screen_to_tile(10, 10, (0, 0), &screen),
        (-13, -1),
        "E15: G4 truncates toward zero like C99 (MapCommon.cpp:965-966); floor gives (-14,-2)"
    );
    assert_eq!(
        real_to_tile(-41, -19),
        (0, 0),
        "E15: G5 — -41/2 = -20 (not -21) keeps both quotients at zero; floors give (-1,0)"
    );
}

/// T15 — E16: `IsPtInMap`'s inclusive `ScreenRect` gate
/// (`Map-Client.cpp:199-200`, fired at `MouseOperation.cpp:534`).
#[test]
fn is_pt_in_map_gates_inclusive_screen_bounds() {
    let screen = ScreenMode::Mode800x600.backbuffer(false).screen_rect();
    assert_eq!(
        (screen.right, screen.bottom),
        (959, 679),
        "stimulus: the frozen 960x680 rect"
    );
    assert!(
        is_pt_in_map(0, 0, &screen),
        "E16: top-left corner is inside"
    );
    assert!(
        is_pt_in_map(959, 679, &screen),
        "E16: bottom-right corner is inside (bounds inclusive)"
    );
    for p in [(-1, 0), (960, 679), (0, 680), (959, 680)] {
        assert!(
            !is_pt_in_map(p.0, p.1, &screen),
            "E16: {p:?} is one pixel outside the inclusive rect"
        );
    }
}

/// T16 — E17: `IsInMapArea`'s half-open tile bounds (`map.cpp:226`:
/// `0 ≤ x < MapXsize`) — the predicate behind D14's named refusal.
#[test]
fn is_in_map_area_is_half_open() {
    assert!(
        is_in_map_area(0, 0, 100, 100),
        "E17: the origin is in the map"
    );
    assert!(
        is_in_map_area(99, 99, 100, 100),
        "E17: the last tile row/column is in the map"
    );
    for t in [(100, 99), (99, 100), (-1, 0), (0, -1)] {
        assert!(
            !is_in_map_area(t.0, t.1, 100, 100),
            "E17: {t:?} is outside the half-open 100x100 map"
        );
    }
}

/// T17 — E18: off-screen clicks hit the `IsPtInMap` gate
/// (`MouseOperation.cpp:534`) and are discarded with `UiState` untouched.
///
/// Hand arithmetic (§2.2-2): every stimulus computes to a tile that IS
/// inside the wired 400×400 map — (1000,340) → (107,94),
/// (960,0) → (98,86), (-1,-1) → (85,97), (0,680) → (103,115) — so only
/// the screen gate can be discarding them (probe 3 deletes exactly it).
#[test]
fn clicks_outside_the_screen_rect_are_discarded() {
    let mut ui = wired(vec![]);
    let before = ui.clone();
    for p in [(1000, 340), (960, 0), (-1, -1), (0, 680)] {
        assert_eq!(
            translate(InputEvent::LeftDown { x: p.0, y: p.1 }, &mut ui),
            None,
            "E18: off-screen click {p:?} is discarded (IsPtInMap, Map-Client.cpp:196-206)"
        );
        assert_eq!(
            ui, before,
            "E18: no mouse row ever mutates UiState (WO-8.1 §7.4)"
        );
    }
}

/// T18 — E19: an in-screen click whose tile leaves the map is refused by
/// name before enqueue (WO-8.1 D14; verification gate 2).
///
/// Hand arithmetic (§2.2-2): click (900,600) at center (100,100) →
/// real (420,4280) → (210+4280)/40 = 112 r10, (-210+4280)/40 = 101 r30
/// → tile (112,101), outside the half-open 100×100 map (`map.cpp:226`).
/// Legacy's gate sits at `UnderMouseObject.cpp:335` yet still encodes
/// ORDER_MOVE (`:5532-5537`) for the server to drop — the port refuses
/// (§8 divergence d).
#[test]
fn clicks_on_tiles_outside_the_map_are_refused_by_name() {
    let mut ui = wired(vec![]);
    let screen = ScreenMode::Mode800x600.backbuffer(false).screen_rect();
    assert!(
        is_pt_in_map(900, 600, &screen),
        "stimulus: the click is inside the screen — only the map gate can refuse it"
    );
    ui.view = Some(MapView {
        screen,
        center: (100, 100),
        map_size: (100, 100),
    });
    let before = ui.clone();
    assert_eq!(
        translate(InputEvent::LeftDown { x: 900, y: 600 }, &mut ui),
        None,
        "E19: tile (112,101) leaves the 100x100 map — typed refusal, never a clamp (D14)"
    );
    assert_eq!(ui, before, "E19: the refusal leaves UiState untouched");
}

/// T19 — E20: charter gate 1's click→move bytes — a click on empty land
/// walks the whole synchronous chain (`translate` → `validate`) into the
/// frozen `Order::Move` and its frozen encoder (`action.rs:113-125`).
#[test]
fn click_on_empty_land_encodes_the_frozen_move_bytes() {
    let mut ui = wired(vec![]); // empty snapshot: no candidate under the cursor
    let intent = translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui)
        .expect("E20: a click on empty land yields the move intent");
    assert_eq!(
        intent,
        Intent::MoveTo { x: 100, y: 100 },
        "E20: G1's hand-computed tile (§2.2-2)"
    );
    let command = validate(&intent).expect("E20: the move intent validates before enqueue (D3)");
    assert_eq!(
        command,
        ClientCommand::Order(Order::Move { x: 100, y: 100 }),
        "E20: the frozen Order::Move carries the tile uncoerced"
    );
    let ClientCommand::Order(order) = command else {
        panic!("E20: the command is an order");
    };
    assert_eq!(
        encode_order(&order),
        vec![0, 100, 0, 100, 0],
        "E20: kind 0 (OrderKind::Move, action.rs:65) + LE x,y — charter gate-1 click-to-move bytes"
    );
}

/// T20 — E21: an enemy under the cursor → `MOUSEMODE_ATTACK`
/// (`MouseOperation.cpp:3259-3261/:3275-3277` → `:5544`, `:5607-5631`)
/// → the frozen attack bytes.
#[test]
fn click_on_an_enemy_produces_the_frozen_attack_order() {
    let mut ui = wired(vec![candidate_at_g1(7, TargetClass::Enemy)]);
    let intent = translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui)
        .expect("E21: an enemy under the cursor yields the attack intent");
    assert_eq!(
        intent,
        Intent::AttackTarget { target_char: 7 },
        "E21: Enemy class → the frozen AttackTarget (D12)"
    );
    let command = validate(&intent).expect("E21: the attack intent validates");
    assert_eq!(
        command,
        ClientCommand::Order(Order::Attack { target_char: 7 }),
        "E21: the frozen Order::Attack"
    );
    let ClientCommand::Order(order) = command else {
        panic!("E21: the command is an order");
    };
    assert_eq!(
        encode_order(&order),
        vec![1, 7, 0, 0, 0],
        "E21: kind 1 (OrderKind::Attack, action.rs:67) + LE id (action.rs:121)"
    );
}

/// T21 — E22: a friendly NPC discards the click — legacy's dialog arm
/// (`MouseOperation.cpp:4138+`) is Q3-excluded, and there is no
/// fall-through move (D12; legacy's `bdoneswitch` stops there too).
#[test]
fn click_on_a_friendly_npc_is_discarded_not_moved() {
    let mut ui = wired(vec![candidate_at_g1(7, TargetClass::Npc)]);
    let before = ui.clone();
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui),
        None,
        "E22: Npc class → typed None, never a MoveTo (probe 4 breaks exactly this)"
    );
    assert_eq!(
        ui, before,
        "E22: the discarded click leaves UiState untouched"
    );
}

/// T22 — E23: a friendly player falls through to the move branch
/// (`MOUSEMODE_OTHERUSER`, `MouseOperation.cpp:3283` → `:5409`).
#[test]
fn click_on_a_friendly_player_falls_through_to_move() {
    let mut ui = wired(vec![candidate_at_g1(7, TargetClass::OtherUser)]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui),
        Some(Intent::MoveTo { x: 100, y: 100 }),
        "E23: OtherUser class → the move intent (D12, same as empty land)"
    );
}

/// T23 — E24+E25: the two hit-test filters of `FindLandChar`'s math core.
///
/// Click (520,340) hand-computes (§2.2-2) to real (40,4020), tile
/// (101,100): x == `rect.right` exactly — strict containment
/// (`Char-Client.cpp:1015-1016`) misses; `right + 1` contains the same
/// point (E24). A candidate 4 tiles away (Chebyshev) is skipped even
/// with a containing rect (`UnderMouseObject.cpp:156-159`, E25).
#[test]
fn target_hit_test_uses_strict_rect_containment_and_the_three_tile_filter() {
    let mut on_edge = wired(vec![CharTarget {
        id: 7,
        tile: (100, 100),
        rect: RealRect {
            left: -40,
            top: 4000,
            right: 40,
            bottom: 4040,
        },
        center: (0, 4020),
        class: TargetClass::Enemy,
    }]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 520, y: 340 }, &mut on_edge),
        Some(Intent::MoveTo { x: 101, y: 100 }),
        "E24: x == rect.right misses (strict < both axes) → the move branch"
    );

    let mut widened = wired(vec![CharTarget {
        id: 7,
        tile: (100, 100),
        rect: RealRect {
            left: -40,
            top: 4000,
            right: 41,
            bottom: 4040,
        },
        center: (0, 4020),
        class: TargetClass::Enemy,
    }]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 520, y: 340 }, &mut widened),
        Some(Intent::AttackTarget { target_char: 7 }),
        "E24: right+1 contains the same point → the enemy is hit"
    );

    let mut four_away = wired(vec![CharTarget {
        id: 7,
        tile: (105, 100),
        rect: RealRect {
            left: -40,
            top: 4000,
            right: 41,
            bottom: 4040,
        },
        center: (0, 4020),
        class: TargetClass::Enemy,
    }]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 520, y: 340 }, &mut four_away),
        Some(Intent::MoveTo { x: 101, y: 100 }),
        "E25: |105-101| = 4 tiles > 3 skips the candidate despite the containing rect"
    );
}

/// T24 — E26: `pick_target` selection — nearest center wins even when the
/// nearer candidate comes later (WO-8.1 D13, the stated §8 divergence from
/// legacy's pass-1 first-arrived effect), and equal lengths keep the first
/// candidate in slice order (`UnderMouseObject.cpp:168-175` first-hit +
/// `:212-215` strict `<`).
#[test]
fn nearest_center_wins_and_first_candidate_wins_ties() {
    let rect = RealRect {
        left: -1000,
        top: 3000,
        right: 1000,
        bottom: 5000,
    };
    let target = |id: u32, center: (i32, i32)| CharTarget {
        id,
        tile: (100, 100),
        rect,
        center,
        class: TargetClass::Enemy,
    };

    // Both rects contain G1's real dot (0,4020); the lengths are 100 then 0.
    let mut nearest = wired(vec![target(1, (0, 4120)), target(2, (0, 4020))]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut nearest),
        Some(Intent::AttackTarget { target_char: 2 }),
        "E26: the later, nearer candidate replaces the earlier farther one (D13)"
    );

    // Equal lengths (20 and 20): the first in slice order wins.
    let mut tie = wired(vec![target(3, (0, 4040)), target(4, (20, 4020))]);
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut tie),
        Some(Intent::AttackTarget { target_char: 3 }),
        "E26: a length tie keeps the first candidate (FindLandChar:168-175/:212-215)"
    );
}

/// T25 — E27: with the view and an enemy fully wired, release and hover
/// still never produce an intent — the legacy release arm pushes a no-flag
/// key (`mainfunction.cpp:251-258`) and hover is position tracking only
/// (`:208`; 8.0 D8 + WO-8.1 D11).
#[test]
fn release_and_hover_never_produce_intents_even_with_the_view_wired() {
    let mut ui = wired(vec![candidate_at_g1(7, TargetClass::Enemy)]);
    let before = ui.clone();
    assert_eq!(
        translate(InputEvent::LeftUp { x: 480, y: 340 }, &mut ui),
        None,
        "E27: LeftUp is inert (mainfunction.cpp:251-258)"
    );
    assert_eq!(ui, before, "E27: LeftUp leaves UiState untouched");
    assert_eq!(
        translate(InputEvent::Move { x: 480, y: 340 }, &mut ui),
        None,
        "E27: hover/Move stays 8.0 D8 — position tracking only"
    );
    assert_eq!(ui, before, "E27: Move leaves UiState untouched");
}

/// T26 — E28: the `!in_world` guard (8.0 D4, legacy `GetFrame() > 40` at
/// `mainfunction.cpp:236`) runs before any 8.1 click logic — proven by a
/// fully wired view + enemy that would otherwise produce an attack.
#[test]
fn clicks_before_the_in_world_guard_are_discarded() {
    let mut ui = wired(vec![candidate_at_g1(7, TargetClass::Enemy)]);
    ui.in_world = false;
    let before = ui.clone();
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui),
        None,
        "E28: pre-world clicks are discarded before the view/target logic runs"
    );
    assert_eq!(ui, before, "E28: UiState is byte-identical");
}

/// T27 — E29: the D15 split — keyboard input is focus-driven (8.0 rows),
/// the pointer is position-driven: the legacy `WM_LBUTTONDOWN` arm has no
/// focus check (`mainfunction.cpp:232-249`), so clicks act while the chat
/// line is open, leaving chat state untouched.
#[test]
fn clicks_act_while_the_chat_line_is_open() {
    let mut ui = wired(vec![]);
    ui.chat_open = true;
    ui.line = "hi".to_owned();
    let before = ui.clone();
    assert_eq!(
        translate(InputEvent::LeftDown { x: 480, y: 340 }, &mut ui),
        Some(Intent::MoveTo { x: 100, y: 100 }),
        "E29: an open chat line never swallows a click (D15)"
    );
    assert_eq!(
        ui, before,
        "E29: chat_open and the typed line travel untouched"
    );
}

// ---------------------------------------------------------------------------
// Step 8.2 — WO-8.2 §9, T28–T39. APPENDED ONLY: the T1–T11 block above is
// byte-identical to the Step 8.0 delivery (DoD 13.2; evidence
// `t1-t11-unchanged.txt`). F-07: every assertion cites a §8 E-row (E13–E19
// new; E1–E12 inherited). P4-C4: values built by hand — no window, no
// Win32, no network.
// ---------------------------------------------------------------------------
use super::keybinds::VK_BACK;

// T28 — E13/D9.1/D9.2: a held Insert fires once per physical press; the
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

// T29 — E13/E15: a held Enter cannot oscillate the chat line. Repeats —
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

// T30 — E13-divergence (a)/E4: Escape auto-repeat is DELIBERATELY
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

// T31 — E14/D11: Backspace edits the line — char-wise pop, safe on an
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

// T32 — E14/D9.7: editing keys are deliberately NOT latched — held
// Backspace must repeat (the asymmetry with T28 is the design, not an
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

// T33 — E15/D11: control characters never enter the line; printable
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

// T34 — E16/D10: Escape closes the active dialog — the close direction of
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

// T35 — E16: the focused input wins — Escape cancels the chat line first
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

// T36 — E17: dialog state is non-modal — game keys keep working (legacy
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

// T37 — E18: arrow keys remain unbound (charter non-goal; the legacy
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

// T38 — E19: the legacy no-op keys (empty F8/F11/F12 arms,
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

// T39 — E14+E16 integration: an edited line flushes through the whole
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

/// WO-8.1 §9 — the wired `UiState` every 8.1 click test starts from:
/// `in_world` true (8.0's D4 guard passes), the frozen 800×600 backbuffer
/// rect (960×680 → `(0,0,959,679)`, `Client.cpp:3206-3208`), view center
/// (100,100), map size (400,400), plus the given candidate snapshot —
/// everything else is `UiState::default()` (WO-8.1 D10).
fn wired(candidates: Vec<CharTarget>) -> UiState {
    UiState {
        in_world: true,
        view: Some(MapView {
            screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
            center: (100, 100),
            map_size: (400, 400),
        }),
        targets: candidates,
        ..UiState::default()
    }
}

/// The candidate whose class T20–T22 swap: at the clicked tile (100,100),
/// a sprite whose strict rect contains G1's real dot (0,4020)
/// (WO-8.1 §2.2-2) and whose center is exactly that point — length 0,
/// the nearest possible (E26).
fn candidate_at_g1(id: u32, class: TargetClass) -> CharTarget {
    CharTarget {
        id,
        tile: (100, 100),
        rect: RealRect {
            left: -40,
            top: 4000,
            right: 40,
            bottom: 4040,
        },
        center: (0, 4020),
        class,
    }
}

// ---------------------------------------------------------------------------
// Step 8.3 tests — the production feed (WO-8.3 §9, T43–T46). APPENDED
// ONLY: T1–T39 above stay byte-identical (DoD 13.2; evidence
// `t1-t39-unchanged.txt`). F-07: every assertion cites §8's E36–E39 rows
// with their legacy/frozen provenance — never read back from the code
// under test. P4-C4: values built by hand — no window, no Win32, no
// network; the only channel is the in-process 7.4 `commands::channel()`.
// ---------------------------------------------------------------------------
use super::feed::InputFeed;
use crate::client::commands::CommandError;

/// T43 — E36: `InputFeed::feed` is the whole P4-C1 chain in one synchronous
/// call — exactly one command drained for the valid intent, zero for every
/// typed refusal (the UI-only `CloseDialog` at `validate.rs:47` and the
/// empty-line `SendChat` at `validate.rs:35-37`), no intermediate storage
/// of any kind between the event and `send`.
#[tokio::test]
async fn input_feed_enqueues_validated_commands_synchronously() {
    let (sender, mut receiver) = channel();
    let mut feed = InputFeed::new(sender);
    // The D4 guard must pass or nothing enqueues (E7, keybinds.rs:50-52).
    feed.enter_world(MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    });

    // INSERT → the frozen sit toggle (E5) — the one enqueue of this test.
    assert_eq!(
        feed.feed(InputEvent::KeyDown { vk: VK_INSERT })
            .expect("the receiver is alive for the whole test"),
        Some(ClientCommand::Order(Order::SitDown)),
        "E36: the valid intent enqueues exactly one command (Ok(Some))"
    );

    // ESCAPE with nothing open → `CloseDialog`; validate refuses it (D2).
    assert_eq!(
        feed.feed(InputEvent::KeyDown { vk: VK_ESCAPE })
            .expect("the receiver is alive for the whole test"),
        None,
        "E36: the UI-only close signal returns Ok(None), zero sent"
    );

    // RETURN opens the chat line (`ChatFocus`, UI-only → no send) …
    assert_eq!(
        feed.feed(InputEvent::KeyDown { vk: VK_RETURN })
            .expect("the receiver is alive for the whole test"),
        None,
        "E36: focusing the chat line enqueues nothing"
    );
    // … KeyUp RETURN clears the D9 auto-repeat guard (keybinds.rs:140-145) …
    assert_eq!(
        feed.feed(InputEvent::KeyUp { vk: VK_RETURN })
            .expect("the receiver is alive for the whole test"),
        None,
        "the release is accepted and ignored — it only re-arms the key"
    );
    // … and the next RETURN flushes the still-empty line: `validate`
    // refuses it before any send (E10, action.rs:202-204).
    assert_eq!(
        feed.feed(InputEvent::KeyDown { vk: VK_RETURN })
            .expect("the receiver is alive for the whole test"),
        None,
        "E36/E10: the empty flush returns Ok(None), zero sent"
    );

    // Exactly one command ever reached the channel: `Ok(Some)` ⇔ exactly
    // one `send` happened (P4-C1 — the send is the only observable).
    drop(feed);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(
        drained, 1,
        "E36: exactly the INSERT's SitDown drained — the refusals sent nothing"
    );
}

/// T44 — E37: `enter_world` arms the feed in one call (D20) — a fresh feed
/// discards a wired-space click under the D4 guard with `ui()` still the
/// default, then `in_world` **and** `view` land together and the very same
/// click enqueues the frozen move (§2.4 G1: window (480,360) → backbuffer
/// (560,400) → real (80,4080) → tile (103,101)).
#[tokio::test]
async fn enter_world_arms_the_feed_in_one_call() {
    let (sender, mut receiver) = channel();
    let mut feed = InputFeed::new(sender);

    assert_eq!(
        feed.feed(InputEvent::LeftDown { x: 560, y: 400 })
            .expect("the receiver is alive for the whole test"),
        None,
        "E37: before enter_world the click is discarded (D4 guard)"
    );
    assert_eq!(
        feed.ui(),
        &UiState::default(),
        "E37: a discarded event leaves ui() byte-identical to UiState::default()"
    );

    let view = MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    };
    feed.enter_world(view);
    assert!(feed.ui().in_world, "E37: enter_world raises the stage flag");
    assert_eq!(
        feed.ui().view,
        Some(view),
        "E37: enter_world wires the click context in the same call (D20)"
    );

    assert_eq!(
        feed.feed(InputEvent::LeftDown { x: 560, y: 400 })
            .expect("the receiver is alive for the whole test"),
        Some(ClientCommand::Order(Order::Move { x: 103, y: 101 })),
        "E37/§2.4 G1: the same click now enqueues Order::Move {{ 103, 101 }}"
    );

    drop(feed);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(drained, 1, "exactly the one move reached the channel");
}

/// T45 — E38: a dead receiver surfaces `Err(CommandError::Closed)` **by
/// name** — no panic, no `Ok`, no retry — and `ui()` is unchanged across
/// the failure; `ui()` returns `&UiState`, so `feed`/`enter_world` are the
/// only mutators (read-only by type).
///
/// Precision (disclosed in the delivery note): the WO row's parenthetical
/// "(still default)" pins the *fresh* feed — but reaching `send` at all
/// requires `enter_world`, because the D4 guard blocks every event while
/// the state is default. So the pins are: fresh `ui() == default` before
/// arming, then "unchanged across the failure" against a post-arm
/// snapshot.
#[test]
fn feed_surfaces_the_closed_channel_by_name() {
    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);

    assert_eq!(
        feed.ui(),
        &UiState::default(),
        "E38: a fresh feed starts at UiState::default()"
    );

    feed.enter_world(MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    });
    let armed = feed.ui().clone();
    drop(receiver); // every receiver is gone → send must fail Closed

    assert!(
        matches!(
            feed.feed(InputEvent::LeftDown { x: 560, y: 400 }),
            Err(CommandError::Closed)
        ),
        "E38: the closed channel surfaces by name — no panic, no Ok"
    );
    assert_eq!(
        feed.ui(),
        &armed,
        "E38: ui() is unchanged across the failure"
    );
    let _: &UiState = feed.ui(); // read-only by type (E38)
}

/// T46 — E39: `InputFeed: Send` at compile time (it rides the window's
/// pump thread, D19) and `Debug`-formattable over a fresh feed without
/// panicking. The zero-queue claim itself is §11's grep over `feed.rs`
/// (P4-C1 — no `VecDeque`/ring/cap anywhere in `src/input/**`).
#[test]
fn input_feed_is_send_and_carries_no_queue() {
    fn assert_send<T: Send>() {}
    assert_send::<InputFeed>();

    let (sender, _receiver) = channel();
    let feed = InputFeed::new(sender);
    let rendered = format!("{feed:?}");
    assert!(
        !rendered.is_empty(),
        "E39: Debug renders a fresh feed without panicking"
    );
}
