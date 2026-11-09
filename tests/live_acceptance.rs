// Step 8.4 — the live acceptance vehicle (WO-8.4 §7.4, D28).
//
// THE FOUR CONTRACT LINES (module doc, WO §7.4):
//
// 1. EVIDENCE ONLY, NEVER A GATE (CONTRIBUTING.md:28-46): this session
//    runs only when an operator asks for it; no verification-gate item
//    and no matrix cell depends on it, and stripping the `#[ignore]` to
//    "make it count" is a violation (WO §10).
// 2. `#[ignore]`d with the env recipe in its ignore message — the
//    published `GOONZU_*` contract (`DriverConfig::from_env`).
// 3. CREDENTIALS ARE NEVER RECORDED: no environment value is printed,
//    written, or asserted on — only latencies, dispatch counts, feed
//    results and the final stage leave this test (stdout is captured by
//    the operator into evidence/8.4/live/live-session.log per §11).
// 4. CROSS-PLATFORM, DRIVER-LEVEL: like `live_integration` — no Win32,
//    no `cfg(windows)`. The walk therefore feeds the decoded backbuffer
//    points directly (`wm_to_input` lives behind `cfg(windows)`, and §7.4
//    keeps this file free of that boundary), and the driver future is not
//    `Send` (`e2e_loopback.rs:21-25`), so it runs on a `LocalSet` task
//    beside this test's current-thread runtime — which is also what makes
//    the thread-scoped D30 capture see every driver record.
//
// The session *is* the evidence narrative (§7.4): one connection, world
// entry observed, then walk → chat → sit → cancel, then we end the
// session ourselves and assert the server kept it through every group.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use goonzu_client::client::commands::{self, ClientCommand};
use goonzu_client::client::driver::{self, DriverConfig, Stage};
use goonzu_client::client::teardown::{Cause, Shutdown};
use goonzu_client::game::action::{ChatMode, Order};
use goonzu_client::graphics::screen::ScreenMode;
use goonzu_client::input::{InputEvent, InputFeed, MapView};

/// One budget for the whole driver run (the session ends when this test
/// triggers it; the budget only catches a hang).
const SESSION_BUDGET: Duration = Duration::from_secs(120);
/// Budget for the driver to log world entry before we call it a failure.
const ENTRY_BUDGET: Duration = Duration::from_secs(60);
/// The §7.4 step-4 poll granularity over the capture (1 ms).
const POLL_GRANULARITY: Duration = Duration::from_millis(1);
/// How long to wait for the server's own-line 118 echo (evidence only —
/// §2.3-6: whether a live server echoes its recipient's own line is
/// server behavior, observed never assumed).
const ECHO_POLL_BUDGET: Duration = Duration::from_secs(5);
/// The frozen stage signal the capture must show before the feed is armed
/// (D24: `driver.rs:850`'s `outcome` field rendering `Entered`).
const ENTERED_NEEDLE: &str = "outcome=Entered";
/// The chat marker line: typed, flushed, then searched for in the capture
/// — only a 118 dispatch of our own line renders it (`?outcome`'s Debug),
/// so its appearance *is* the echo observation.
const CHAT_MARKER: &str = "8.4 accepted";
/// The cancel marker: typed and cancelled in step d — if it ever appears
/// in the capture, the cancelled line reached a dispatch (E45 violated).
const CANCEL_MARKER: &str = "cancel-probe";

// ---------------------------------------------------------------------------
// D30 (WO-8.4 §2.2-9): the local Capture copy — `e2e_loopback.rs`'s is
// frozen (D22), so each new test binary carries its own ~20 lines (the
// other copy lives in `input_loopback.rs`; both disclosed in §14).
// ---------------------------------------------------------------------------

/// A byte sink the scoped tracing subscriber writes into.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Capture;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// The D30 DEBUG, ANSI-free fmt subscriber over the capture sink.
fn debug_capture_subscriber(capture: Capture) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(capture)
        .finish()
}

/// The capture's bytes as lossy UTF-8 (the fmt writer is bytes-only).
fn snapshot(capture: &Capture) -> String {
    String::from_utf8_lossy(&capture.0.lock().expect("capture lock")).into_owned()
}

/// Non-overlapping occurrences of `needle` (dispatch counts, §7.4 step 7).
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// The frozen 8.1 stage `enter_world` arms — `wired_view()`'s constants
/// (`src/input/tests.rs:1327-1343`): 800x600 backbuffer rect
/// (0,0,959,679), center (100,100), map size (400,400). Disclosed (D27):
/// the live map's real dimensions never reach the client, so clicks stay
/// near this view's center and any server-side refusal is recorded.
fn wired_view() -> MapView {
    MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    }
}

/// One timed feed (§7.4 step 5): `Instant` → `feed.feed` → elapsed,
/// asserted **≤ 16 ms** in-process — P4-C3's input→enqueue leg is the
/// budget; network legs are recorded as evidence, never asserted (§10).
fn timed_feed(
    feed: &mut InputFeed,
    label: &str,
    event: InputEvent,
    latencies: &mut Vec<(String, Duration)>,
) -> Option<ClientCommand> {
    let started = Instant::now();
    let fed = feed
        .feed(event)
        .expect("the session input channel must stay alive through the script");
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_millis(16),
        "P4-C3: {label} must enqueue within 16 ms, took {elapsed:?}"
    );
    latencies.push((label.to_owned(), elapsed));
    fed
}

/// WO-8.4 §7.4 / D28 — one live session, one connection, evidence only.
#[ignore = "live acceptance: needs a reachable game service (GOONZU_GAME_ADDR) and credentials (GOONZU_USER_ID, GOONZU_USER_PW, GOONZU_MAC_ADDRESS, GOONZU_CHAR_SLOT)"]
#[tokio::test]
async fn live_session_walks_chats_sits_and_cancels_within_budget() {
    // 1. Operator environment — the ignore message is the recipe. A
    //    failure here is an operator-env error; `ConfigError` carries
    //    variable *names*, never the password value.
    let config = DriverConfig::from_env()
        .await
        .expect("the GOONZU_* environment must be set (see the #[ignore] recipe)");

    // 2. The D30 DEBUG capture, thread-scoped: the current-thread test
    //    runtime runs the driver on this very thread, so every driver
    //    record lands here. The process-global DEBUG-to-sink subscriber
    //    first is the same callsite-interest guard as T53's (§14): a
    //    concurrent `--ignored` sibling (`live_integration`) sharing this
    //    process would otherwise register the driver callsites against
    //    `NoSubscriber` and starve this capture of the `Entered` needle.
    let _global = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::sink)
            .finish(),
    );
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(debug_capture_subscriber(capture.clone()));

    // 3. The frozen channel + feed — deliberately NOT armed yet (the D4
    //    guard discards everything until world entry in step 4).
    let (sender, receiver) = commands::channel();
    let mut feed = InputFeed::new(sender);
    let shutdown = Shutdown::new();
    let shutdown_task = shutdown.clone();

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let session_started = Instant::now();
            // The driver future is not `Send` (`e2e_loopback.rs:21-25`),
            // so it rides a LocalSet task beside this future; the session
            // budget turns a hang into a named failure.
            let mut driver_task = local.spawn_local(async move {
                match tokio::time::timeout(
                    SESSION_BUDGET,
                    driver::run_with_commands(&config, &shutdown_task, receiver),
                )
                .await
                {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(_) => Err("the session budget elapsed".to_owned()),
                }
            });

            // 4. Poll the capture (1 ms) until the frozen `Entered` record
            //    (D24) — then arm with the frozen `wired_view()` constants.
            //    Disclosed (D27): the live map's real dimensions are
            //    unknown to the client; clicks stay near the center and
            //    any server-side refusal is recorded, never asserted away.
            let entry_deadline = Instant::now() + ENTRY_BUDGET;
            let entered_at = loop {
                if snapshot(&capture).contains(ENTERED_NEEDLE) {
                    break Instant::now();
                }
                if Instant::now() >= entry_deadline {
                    panic!(
                        "the driver never logged the in-world Entered record \
                         within {ENTRY_BUDGET:?}"
                    );
                }
                tokio::select! {
                    joined = &mut driver_task => match joined {
                        Ok(Ok(outcome)) => panic!(
                            "the driver ended before world entry (stage {})",
                            outcome.stage
                        ),
                        Ok(Err(error)) => panic!("the driver failed before world entry: {error}"),
                        Err(join_error) => panic!("the driver task panicked: {join_error}"),
                    },
                    () = tokio::time::sleep(POLL_GRANULARITY) => {}
                }
            };
            eprintln!(
                "[live] world entry observed at +{:?}",
                entered_at.duration_since(session_started)
            );
            feed.enter_world(wired_view());

            let mut latencies: Vec<(String, Duration)> = Vec::new();
            let mut results: Vec<String> = Vec::new();

            // 5a. WALK — the three T52 window points decoded by hand
            //     through the +80/+40 border (E30): this file carries no
            //     `cfg(windows)`, so `wm_to_input` is not called; the
            //     backbuffer points it would produce are fed directly.
            let walk: [(&str, InputEvent, ClientCommand); 3] = [
                (
                    "walk #1 → tile (100,100)",
                    InputEvent::LeftDown { x: 480, y: 340 },
                    ClientCommand::Order(Order::Move { x: 100, y: 100 }),
                ),
                (
                    "walk #2 → tile (101,100)",
                    InputEvent::LeftDown { x: 520, y: 360 },
                    ClientCommand::Order(Order::Move { x: 101, y: 100 }),
                ),
                (
                    "walk #3 → tile (100,101)",
                    InputEvent::LeftDown { x: 440, y: 360 },
                    ClientCommand::Order(Order::Move { x: 100, y: 101 }),
                ),
            ];
            for (label, event, expect) in walk {
                let fed = timed_feed(&mut feed, label, event, &mut latencies);
                assert_eq!(
                    fed,
                    Some(expect),
                    "{label}: the click must resolve to its §2.4 hand tile"
                );
                results.push(format!("{label}: {fed:?}"));
            }

            // 5b. CHAT — focus, type the marker line, flush. The flush
            //     starts the input→echo clock.
            let focus = timed_feed(
                &mut feed,
                "chat focus (Enter down)",
                InputEvent::KeyDown { vk: 13 },
                &mut latencies,
            );
            assert_eq!(focus, None, "chat focus enqueues nothing (E1)");
            for ch in CHAT_MARKER.chars() {
                let typed = timed_feed(
                    &mut feed,
                    "chat typed character",
                    InputEvent::CharInput { ch },
                    &mut latencies,
                );
                assert_eq!(
                    typed, None,
                    "typing enqueues nothing (D3: the cap is refused at flush)"
                );
            }
            let chat_flushed_at = Instant::now();
            let chat = timed_feed(
                &mut feed,
                "chat flush (Enter down)",
                InputEvent::KeyDown { vk: 13 },
                &mut latencies,
            );
            assert_eq!(
                chat,
                Some(ClientCommand::Chat {
                    mode: ChatMode::Normal,
                    text: CHAT_MARKER.to_owned(),
                }),
                "the marker line flushes as a Normal chat"
            );
            results.push(format!("chat flush: {chat:?}"));

            // Poll for the server's own-line 118 echo (§7.4 step 5b): if
            // observed, record input→echo; if the server does not echo
            // its recipient's own line, that is the session's observed
            // behavior (§2.3-6) — recorded, never asserted away.
            let echo_needle = format!("text: \"{CHAT_MARKER}\"");
            let echo_deadline = Instant::now() + ECHO_POLL_BUDGET;
            let echo_at = loop {
                if snapshot(&capture).contains(&echo_needle) {
                    break Some(Instant::now());
                }
                if Instant::now() >= echo_deadline {
                    break None;
                }
                tokio::select! {
                    joined = &mut driver_task => match joined {
                        Ok(Ok(outcome)) => panic!(
                            "the driver ended during the chat phase (stage {})",
                            outcome.stage
                        ),
                        Ok(Err(error)) => panic!("the driver failed during chat: {error}"),
                        Err(join_error) => panic!("the driver task panicked: {join_error}"),
                    },
                    () = tokio::time::sleep(POLL_GRANULARITY) => {}
                }
            };
            let input_to_echo = echo_at.map(|at| at.duration_since(chat_flushed_at));

            // 5c. SIT — two physical Insert presses (Down/Up/Down): two
            //     `Ok(Some(SitDown))`, the release only clears the D9.1
            //     guard (server owns the toggle — Q2).
            let sit1 = timed_feed(
                &mut feed,
                "sit press #1 (Insert down)",
                InputEvent::KeyDown { vk: 45 },
                &mut latencies,
            );
            assert_eq!(
                sit1,
                Some(ClientCommand::Order(Order::SitDown)),
                "the first Insert press enqueues the frozen sit toggle"
            );
            results.push(format!("sit press #1: {sit1:?}"));
            let sit_up = timed_feed(
                &mut feed,
                "sit release (Insert up)",
                InputEvent::KeyUp { vk: 45 },
                &mut latencies,
            );
            assert_eq!(
                sit_up, None,
                "the KeyUp enqueues nothing — it clears the D9.1 guard \
                 (keybinds.rs:141)"
            );
            let sit2 = timed_feed(
                &mut feed,
                "sit press #2 (Insert down)",
                InputEvent::KeyDown { vk: 45 },
                &mut latencies,
            );
            assert_eq!(
                sit2,
                Some(ClientCommand::Order(Order::SitDown)),
                "the second physical press fires again — one action per press"
            );
            results.push(format!("sit press #2: {sit2:?}"));

            // 5d. ESCAPE — focus, type the cancel marker, Escape (UI-only
            //     cancel), re-focus, flush the empty line: zero commands
            //     from this group. The releases after each focus clear the
            //     D9.1 guard so the flush reaches `validate` — E45's
            //     substance ("empty → refused"), exactly as §7.3's
            //     enumerated script does.
            let esc_focus = timed_feed(
                &mut feed,
                "escape focus (Enter down)",
                InputEvent::KeyDown { vk: 13 },
                &mut latencies,
            );
            assert_eq!(esc_focus, None, "re-focus enqueues nothing");
            let _ = timed_feed(
                &mut feed,
                "escape focus release (Enter up)",
                InputEvent::KeyUp { vk: 13 },
                &mut latencies,
            );
            for ch in CANCEL_MARKER.chars() {
                let typed = timed_feed(
                    &mut feed,
                    "cancel typed character",
                    InputEvent::CharInput { ch },
                    &mut latencies,
                );
                assert_eq!(typed, None, "typing enqueues nothing");
            }
            let cancel = timed_feed(
                &mut feed,
                "cancel (Escape down)",
                InputEvent::KeyDown { vk: 27 },
                &mut latencies,
            );
            assert_eq!(
                cancel, None,
                "Escape cancels the line UI-only — no intent leaves \
                 translate (E3, keybinds.rs:77-99)"
            );
            let esc_refocus = timed_feed(
                &mut feed,
                "escape re-focus (Enter down)",
                InputEvent::KeyDown { vk: 13 },
                &mut latencies,
            );
            assert_eq!(esc_refocus, None, "re-focus enqueues nothing");
            let _ = timed_feed(
                &mut feed,
                "escape re-focus release (Enter up)",
                InputEvent::KeyUp { vk: 13 },
                &mut latencies,
            );
            let empty_flush = timed_feed(
                &mut feed,
                "empty flush (Enter down)",
                InputEvent::KeyDown { vk: 13 },
                &mut latencies,
            );
            assert_eq!(
                empty_flush, None,
                "the flush of the empty line is refused by validate \
                 before any send (E45)"
            );
            results.push(format!(
                "escape group (focus/type/Escape/re-focus/flush): {esc_focus:?} \
                 … {cancel:?} … {empty_flush:?} — zero commands"
            ));

            // The cancel marker must never appear anywhere in the capture —
            // strongest form of "no 118 dispatch ever shows it" (step 5b):
            // a cancelled line is never logged, never dispatched.
            assert!(
                !snapshot(&capture).contains(CANCEL_MARKER),
                "the cancelled line must never reach a dispatch record (E45)"
            );

            // Dispatch counts for the printout (record, never assert).
            let after_script = snapshot(&capture);
            let chat_dispatches = count(&after_script, "opcode=118");
            let order_dispatches = count(&after_script, "opcode=241");

            // 6. End the session ourselves — the hard session-survival
            //    assertion: the server kept the session through
            //    walk/chat/sit/cancel, and the teardown is clean.
            let claimed = shutdown.trigger(Cause::CtrlC);
            let outcome = match driver_task.await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => panic!("the live session failed: {error}"),
                Err(join_error) => panic!("the driver task panicked: {join_error}"),
            };
            assert_eq!(
                outcome.stage,
                Stage::TornDown,
                "the server must keep the session through walk/chat/sit/\
                 cancel, then a clean teardown (WO §7.4 step 6)"
            );

            // 7. The evidence block, to stdout (§11: the operator captures
            //    it into evidence/8.4/live/live-session.log). Credentials
            //    are never printed: only latencies, counts, feed results
            //    and the stage leave this test.
            let total = latencies.len();
            let sum_ms: f64 = latencies.iter().map(|(_, d)| d.as_secs_f64() * 1e3).sum();
            let min_ms: f64 = latencies
                .iter()
                .map(|(_, d)| d.as_secs_f64() * 1e3)
                .fold(f64::INFINITY, f64::min);
            let max_ms: f64 = latencies
                .iter()
                .map(|(_, d)| d.as_secs_f64() * 1e3)
                .fold(f64::NEG_INFINITY, f64::max);
            let avg_ms = if total == 0 {
                0.0
            } else {
                sum_ms / total as f64
            };

            println!();
            println!("=== WO-8.4 live acceptance — evidence only, never a gate ===");
            println!(
                "world entry: +{:?} after session start",
                entered_at.duration_since(session_started)
            );
            println!(
                "input → enqueue ({total} events): min {min_ms:.3} ms / \
                 avg {avg_ms:.3} ms / max {max_ms:.3} ms \
                 (budget ≤ 16 ms per event, P4-C3)"
            );
            for (label, elapsed) in &latencies {
                println!("  {label:<34} {:>9.3} ms", elapsed.as_secs_f64() * 1e3);
            }
            println!("feed results:");
            for line in &results {
                println!("  {line}");
            }
            println!(
                "dispatch counts observed: opcode=118 → {chat_dispatches}, \
                 opcode=241 → {order_dispatches}"
            );
            match input_to_echo {
                Some(elapsed) => println!(
                    "input → own-line 118 echo: {:.3} ms \
                     (1 ms poll granularity — evidence, not a budget)",
                    elapsed.as_secs_f64() * 1e3
                ),
                None => println!(
                    "no own-line 118 echo observed within {ECHO_POLL_BUDGET:?} \
                     (§2.3-6 server behavior — recorded, not asserted)"
                ),
            }
            println!("cancel marker in capture: absent (E45) ✓");
            println!(
                "shutdown trigger claimed Cause::CtrlC: {claimed} \
                 (D26: the frozen enum's user-interrupt cause)"
            );
            println!("final stage: {}", outcome.stage);
            println!("=== end evidence ===");
        })
        .await;
}
