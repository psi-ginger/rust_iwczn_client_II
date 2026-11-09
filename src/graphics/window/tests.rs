// Step 8.3 tests — `wm_to_input` and the composed decode+feed chain
// (WO-8.3 §9, T40–T42 + T51).
//
// F-07: every expectation is an §8 E-row (E30–E35, inherited E12) carrying
// its legacy `file:line` or the §2.4 hand arithmetic — never read back from
// the code under test; the wire-side goldens live as §8.3 literals (§2.4).
// P4-C4: every stimulus is a raw integer triple (`msg`, `wparam`, `lparam`)
// — no window, no DC, no message struct is ever constructed here.

use super::*;
use crate::client::commands::channel;
use crate::input::MapView;
use std::time::{Duration, Instant};

/// Pack a client point `(x, y)` into the `lParam` a mouse message carries
/// (LOWORD = x, HIWORD = y — the layout `wm_to_input` decodes,
/// `mainfunction.cpp:213`).
fn pack(x: i32, y: i32) -> isize {
    (((y as u32) << 16) | (x as u32)) as isize
}

// T40 — E30/E31: the three ordinary mouse messages decode into backbuffer
// space with the +80/+40 border, and `WM_LBUTTONDBLCLK` is one more
// `LeftDown` (D18 — one intent per physical press under `CS_DBLCLKS`).
#[test]
fn wm_to_input_decodes_the_mouse_messages() {
    // E30/§2.4 G1 step 1: window client (480, 360) → backbuffer (560, 400)
    // (`map.h:33,35` + `Client.cpp:1005`, the `+80/+40` border).
    assert_eq!(
        wm_to_input(WM_MOUSEMOVE, 0, pack(480, 360)),
        Some(InputEvent::Move { x: 560, y: 400 }),
        "E30: WM_MOUSEMOVE decodes to Move in backbuffer space"
    );
    assert_eq!(
        wm_to_input(WM_LBUTTONDOWN, 0, pack(480, 360)),
        Some(InputEvent::LeftDown { x: 560, y: 400 }),
        "E30: WM_LBUTTONDOWN decodes to LeftDown"
    );
    assert_eq!(
        wm_to_input(WM_LBUTTONUP, 0, pack(480, 360)),
        Some(InputEvent::LeftUp { x: 560, y: 400 }),
        "E30: WM_LBUTTONUP decodes to LeftUp"
    );
    // A second point pins the offset itself, not the pair: (10, 20) → (90, 60).
    assert_eq!(
        wm_to_input(WM_MOUSEMOVE, 0, pack(10, 20)),
        Some(InputEvent::Move { x: 90, y: 60 }),
        "E30: the +80/+40 border applies to every coordinate"
    );
    // E31/D18: under CS_DBLCLKS Windows withholds the second DOWN and sends
    // DBLCLK instead (`window.rs` class style ← `main.cpp:852-862`); legacy
    // pushes a left-press key too (`mainfunction.cpp:218-230`) — one
    // intent per physical press, no double-click state.
    assert_eq!(
        wm_to_input(WM_LBUTTONDBLCLK, 0, pack(480, 360)),
        Some(InputEvent::LeftDown { x: 560, y: 400 }),
        "E31/D18: WM_LBUTTONDBLCLK is one more LeftDown, same coordinates"
    );
}

// T41 — E32/E33: keyboard messages carry the wparam untouched, and a
// `WM_CHAR` whose wparam is not a scalar is refused by name (P4-C4:
// never panics).
#[test]
fn wm_to_input_decodes_the_keyboard_messages() {
    // E32: the wparam IS the virtual-key code
    // (`mainfunction.cpp:1282` WM_KEYDOWN, `:1202` WM_KEYUP).
    assert_eq!(
        wm_to_input(WM_KEYDOWN, 0x0D, 0),
        Some(InputEvent::KeyDown { vk: 13 }),
        "E32: RETURN (0x0D) round-trips as KeyDown {{ vk: 13 }}"
    );
    assert_eq!(
        wm_to_input(WM_KEYDOWN, 0x2D, 0),
        Some(InputEvent::KeyDown { vk: 45 }),
        "E32: INSERT (0x2D) round-trips as KeyDown {{ vk: 45 }}"
    );
    assert_eq!(
        wm_to_input(WM_KEYUP, 0x70, 0),
        Some(InputEvent::KeyUp { vk: 112 }),
        "E32: F1 (0x70) release round-trips as KeyUp {{ vk: 112 }}"
    );
    // E33: WM_CHAR decodes a scalar; 0xD800 is a surrogate half and
    // `char::from_u32` refuses it — a typed `None`, never a panic
    // (`mainfunction.cpp:1261-1280`, WO-8.0's never-panics invariant).
    assert_eq!(
        wm_to_input(WM_CHAR, 0x68, 0),
        Some(InputEvent::CharInput { ch: 'h' }),
        "E33: 'h' (0x68) decodes to CharInput"
    );
    assert_eq!(
        wm_to_input(WM_CHAR, 0xD800, 0),
        None,
        "E33: an invalid scalar (surrogate half) is refused, not panicked"
    );
}

// T42 — E34/E35: coordinates are the 16-bit zero-extension of `lParam`
// plus the border (§2.4 G2), and the decode is total — every unarmed
// message returns `None`.
#[test]
fn wm_to_input_zero_extends_and_stays_total() {
    // E34/§2.4 G2: lParam = -1 (64-bit all-ones) → LOWORD = HIWORD =
    // 0xFFFF = 65535 zero-extended (legacy-faithful, WO-8.1 §8 (a) — no
    // sign extension) → +80/+40 → (65615, 65575).
    assert_eq!(
        wm_to_input(WM_LBUTTONDOWN, 0, -1),
        Some(InputEvent::LeftDown { x: 65615, y: 65575 }),
        "E34: coordinates are the 16-bit zero-extension + the +80/+40 border"
    );
    // E35: the decode is total — WM_RBUTTONDOWN (0x0204, right-click is a
    // charter non-goal), WM_PAINT (0x000F), WM_DESTROY (0x0002) and
    // WM_QUIT (0x0012) are outside the table (`mainfunction.cpp:1695`
    // keeps them on the DefWindowProc fallthrough).
    for msg in [0x0204u32, 0x000F, 0x0002, 0x0012] {
        assert_eq!(
            wm_to_input(msg, 0x41, -1),
            None,
            "E35: message {msg:#06x} is outside the decode table"
        );
    }
    // E32/E35: keyboard messages read `wParam` only — a garbage `lParam`
    // cannot change the decoded key.
    assert_eq!(
        wm_to_input(WM_KEYDOWN, 0x2D, -1),
        Some(InputEvent::KeyDown { vk: 45 }),
        "E32: the key comes from wParam; lParam is ignored"
    );
}

/// T51 — E12 (inherited) + E36/D23: the **composed** decode+feed chain
/// stays inside one 16 ms frame budget (P4-C3).
///
/// Margin rationale (restated from `src/input/tests.rs:306-316`, never
/// relaxed): the budget is ≤ 16 ms per event (one 60 FPS frame); this
/// drives 100 raw messages — far more than one frame ever carries — through
/// `wm_to_input → InputFeed::feed` and demands the whole batch inside that
/// single frame's budget, so each message gets ~1/100 of its budget.
/// Debug-build enum dispatch plus mpsc push is µs-scale and the decode adds
/// one `match` plus integer arithmetic (also µs-scale), so the expected
/// batch lands around a millisecond — ≥10× headroom under the assertion.
/// On any flake the finding is disclosed and adjudicated
/// (`CONTRIBUTING_PHASE4.md` P4-C3) — never silently relaxed.
#[tokio::test]
async fn the_composed_decode_and_feed_chain_stays_within_the_16ms_frame_budget() {
    // 16 full cycles + the four-event tail, one flush each (the T10 script).
    const EXPECTED_ENQUEUED: usize = 17;

    let (sender, mut receiver) = channel();
    let mut feed = InputFeed::new(sender);
    // E37's precondition: the D4 guard must pass or nothing enqueues.
    feed.enter_world(MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    });

    // T10's exact script (`tests.rs:328-346`) re-encoded as raw messages:
    // 16 × [open, 'h', 'i', flush, unknown F1, hover] + [open, 'h', 'i',
    // flush] = exactly 100 messages, 17 flushes.
    let mut script: Vec<(u32, usize, isize)> = Vec::with_capacity(100);
    for _ in 0..16 {
        script.extend([
            (WM_KEYDOWN, 0x0D, 0),
            (WM_CHAR, 'h' as u32 as usize, 0),
            (WM_CHAR, 'i' as u32 as usize, 0),
            (WM_KEYDOWN, 0x0D, 0),
            (WM_KEYDOWN, 0x70, 0),            // F1: unknown, discarded
            (WM_MOUSEMOVE, 0, (7 << 16) | 5), // hover (5,7): never enqueues (D8)
        ]);
    }
    script.extend([
        (WM_KEYDOWN, 0x0D, 0),
        (WM_CHAR, 'h' as u32 as usize, 0),
        (WM_CHAR, 'i' as u32 as usize, 0),
        (WM_KEYDOWN, 0x0D, 0),
    ]);
    assert_eq!(
        script.len(),
        100,
        "the latency script is exactly 100 raw messages"
    );

    let started = Instant::now();
    let mut enqueued = 0usize;
    for (msg, w_param, l_param) in script {
        let event = wm_to_input(msg, w_param, l_param)
            .expect("every scripted message decodes — only unarmed types return None");
        if feed
            .feed(event)
            .expect("the receiver is alive for the batch")
            .is_some()
        {
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
        "E12/P4-C3: 100 raw messages through decode+feed must complete \
         in < 16 ms, took {elapsed:?}"
    );

    drop(feed);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(
        drained, EXPECTED_ENQUEUED,
        "every enqueued command must drain — none lost"
    );
}

// ---------------------------------------------------------------------------
// Step 8.4 tests — the composition seam's pure halves (WO-8.4 §9, T56 + T58),
// appended after T51 with zero deletions (T40–T42/T51 byte-identical).
//
// F-07 as above: expectations come from E46/E47 and §2.4's hand arithmetic
// only; P4-C4: raw integers and pure functions, no window, no DC, no
// message struct, no Win32 object.
// ---------------------------------------------------------------------------

/// T56 — E46 (WO-8.4 §9): the composed chain `wm_to_input → feed`
/// completes in **&lt; 16 ms per event** — P4-C3's literal unit — for every
/// one of T10's 100 raw messages, with T10/T51's counts (exactly 17
/// enqueued, all drained).
///
/// Margin rationale (restated from `src/input/tests.rs:306-316` and T51's,
/// never relaxed): each event is one `match` plus integer arithmetic, one
/// enum dispatch, and one unbounded-mpsc push — µs-scale even in a debug
/// build, so the per-event 16 ms assertion carries roughly 1000× headroom
/// and the whole 100-event batch still lands far inside one frame. On any
/// flake the finding is disclosed and adjudicated
/// (`CONTRIBUTING_PHASE4.md` P4-C3) — never silently relaxed.
#[tokio::test]
async fn every_composed_event_enqueues_within_the_16ms_frame_budget() {
    // T10/T51's counts: 16 full cycles + the four-event tail, one flush
    // each — 100 raw messages, exactly 17 enqueues.
    const EXPECTED_ENQUEUED: usize = 17;

    let (sender, mut receiver) = channel();
    let mut feed = InputFeed::new(sender);
    // E37's precondition: the D4 guard must pass or nothing enqueues.
    feed.enter_world(MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    });

    // T10's exact script re-encoded as raw messages (WO-8.3 T51's
    // encoding): 16 × [open, 'h', 'i', flush, unknown F1, hover] +
    // [open, 'h', 'i', flush] = exactly 100 messages, 17 flushes.
    let mut script: Vec<(u32, usize, isize)> = Vec::with_capacity(100);
    for _ in 0..16 {
        script.extend([
            (WM_KEYDOWN, 0x0D, 0),
            (WM_CHAR, 'h' as u32 as usize, 0),
            (WM_CHAR, 'i' as u32 as usize, 0),
            (WM_KEYDOWN, 0x0D, 0),
            (WM_KEYDOWN, 0x70, 0),            // F1: unknown, discarded
            (WM_MOUSEMOVE, 0, (7 << 16) | 5), // hover (5,7): never enqueues (D8)
        ]);
    }
    script.extend([
        (WM_KEYDOWN, 0x0D, 0),
        (WM_CHAR, 'h' as u32 as usize, 0),
        (WM_CHAR, 'i' as u32 as usize, 0),
        (WM_KEYDOWN, 0x0D, 0),
    ]);
    assert_eq!(
        script.len(),
        100,
        "the latency script is exactly 100 raw messages"
    );

    // E46: time the whole composed chain per event — decode + translate +
    // validate + send — and hold every one under the 16 ms frame budget.
    let mut enqueued = 0usize;
    for (msg, w_param, l_param) in script {
        let started = Instant::now();
        let event = wm_to_input(msg, w_param, l_param)
            .expect("every scripted message decodes — only unarmed types return None");
        let fed = feed
            .feed(event)
            .expect("the receiver is alive for the batch");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(16),
            "E46/P4-C3: message {msg:#06x} through decode+feed must take \
             < 16 ms, took {elapsed:?}"
        );
        if fed.is_some() {
            enqueued += 1;
        }
    }

    assert_eq!(
        enqueued, EXPECTED_ENQUEUED,
        "every flush must enqueue exactly one command"
    );

    drop(feed);
    let mut drained = 0usize;
    while receiver.recv().await.is_some() {
        drained += 1;
    }
    assert_eq!(
        drained, EXPECTED_ENQUEUED,
        "every enqueued command must drain — none lost"
    );
}

/// T58 — E47 (WO-8.4 §9): `enter_world_view` pins the view to what the
/// render draws — the 800x600 backbuffer rect `(0,0,959,679)` (hand from
/// `Client.cpp:3206-3208`, `screen.rs:129-138`), the center `(32, 32)`
/// (the `draw_map(…, 32, 32, …)` literal, `window.rs:440-445`), and the
/// demo map's header `(64, 64)` (`build_scene`, `window.rs:624-629`) —
/// plus purity: a constructor with no state and no Win32, so degenerate
/// inputs pass through verbatim with only the center fixed.
#[test]
fn enter_world_view_pins_the_render_center_and_map_size() {
    let screen = ScreenMode::Mode800x600.backbuffer(false).screen_rect();
    assert_eq!(
        screen,
        ScreenRect {
            left: 0,
            top: 0,
            right: 959,
            bottom: 679,
        },
        "E47/§2.4-3: the hand rect for the 960x680 backbuffer (right = xsize-1)"
    );

    assert_eq!(
        enter_world_view(screen, (64, 64)),
        MapView {
            screen: ScreenRect {
                left: 0,
                top: 0,
                right: 959,
                bottom: 679,
            },
            center: (32, 32),
            map_size: (64, 64),
        },
        "E47: the view is the render's rect + draw_map's center + the demo \
         map's header — clicks decode against exactly what is on screen"
    );

    // Purity: same inputs, same view — no state, no side effect.
    assert_eq!(
        enter_world_view(screen, (64, 64)),
        enter_world_view(screen, (64, 64)),
        "E47: enter_world_view is pure — repeated calls are identical"
    );

    // Degenerate inputs pass through verbatim; only the center is fixed
    // (it is the render's own literal, D27).
    assert_eq!(
        enter_world_view(
            ScreenRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            (0, 0),
        ),
        MapView {
            screen: ScreenRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            center: (32, 32),
            map_size: (0, 0),
        },
        "E47: screen/map_size pass through verbatim — center (32, 32) is \
         the render's literal, not derived from the inputs"
    );
}
