// Step 8.3 — raw window messages through decode + feed to the wire
// (WO-8.3 §9, T47–T50), over the frozen Step 6.2/7.4 loopback harness.
//
// D22: `tests/e2e_loopback.rs` and `tests/e2e_loopback/**` are READ-ONLY,
// so this binary re-declares the three harness modules it needs at its own
// root (the same `#[path]` shape e2e_loopback.rs itself uses at `:26-35`,
// so the harnesses' `use crate::frames::{…}` resolves here) and mirrors
// exactly the root items it uses (§2.2-10: an unused mirror is a gate
// failure). `robustness.rs`/`soak.rs` are deliberately not re-declared
// (§2.2-7) — they mirror e2e_loopback's root helpers, not ours.
//
// F-07: every wire expectation is a §8.3 frozen literal (§2.4 goldens,
// `WO-step7-in-world-actions.md` §8.3-A/C/D), never regenerated from the
// client's encoder; the conversation prefix comes only from the harness's
// hand-placed `expected_conversation`. Each test stands in for 8.4's
// composition (divergence (a)): decode → feed → drop the feed → run — the
// live stage trigger itself stays out of Phase IV.
//
// Disclosed (delivery note): `frames`/`game_double` carry a module-scoped
// `#[allow(dead_code)]` here — items used only by the deliberately
// non-re-declared `robustness`/`soak` siblings (e.g. `MALFORMED_*_WIRE`,
// `Tail::RawFault`, `mutated_worldlist_response`) would otherwise fail
// `-D warnings` in this binary. The frozen files are untouched, and this
// file's own code stays fully linted.
#![cfg(windows)]

#[path = "e2e_loopback/frames.rs"]
#[allow(dead_code)]
mod frames;
#[path = "e2e_loopback/front_double.rs"]
mod front_double;
#[path = "e2e_loopback/game_double.rs"]
#[allow(dead_code)]
mod game_double;

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use goonzu_client::client::commands::{channel, ClientCommand};
use goonzu_client::client::driver::{self, DriverConfig, DriverError, DriverOutcome, Stage};
use goonzu_client::client::teardown::Shutdown;
use goonzu_client::game::action::{ChatMode, Order};
use goonzu_client::graphics::screen::ScreenMode;
use goonzu_client::graphics::window::{
    wm_to_input, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEMOVE,
};
use goonzu_client::input::{InputEvent, InputFeed, MapView, UiState};

use frames::{
    expected_conversation, Dir, Entry, Leg, Mutation, Transcript, OCCUPIED_SLOT, VILLAGE_REF,
};
use front_double::{run_front_double, FrontReport};
use game_double::{run_game_double_with, GameReport};

/// The account id the test types into the front requests (`szAccountID[20]`,
/// `userid[20]`) — mirrored from `e2e_loopback.rs:70`.
const TEST_USER: &str = "e2e_user";
/// The password both legs must carry (`userpw[20]`, `szPassword[20]`).
const TEST_PASSWORD: &str = "s3cret-pw";
/// The MAC address the front login echoes (`macaddress[20]`).
const TEST_MAC: &str = "00:11:22:33:44:55";
/// One budget for the whole driver run: the happy path waits out a single
/// 10 s heartbeat tick, everything else finishes in milliseconds.
const DRIVER_TIME_BUDGET: Duration = Duration::from_secs(60);
/// One budget for either double's conversation.
const DOUBLE_TIME_BUDGET: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// The frozen §8.3 goldens (WO-8.3 §2.4, literals only — F-07: never
// regenerated from the client's own encoder).
// ---------------------------------------------------------------------------

/// The frozen §8.3-**C** SitDown wire image (`WO-step7-in-world-actions.md`
/// §8.3): `06 00 | 00 | F0 00 | 02` — 6 bytes.
const SITDOWN_WIRE: [u8; 6] = [0x06, 0x00, 0x00, 0xF0, 0x00, 0x02];
/// The frozen §8.3-**D** Normal chat `"hi"` wire image: `08 00 | 00 | 75 00 |
/// 01 68 69` — 8 bytes (mode `01` = `ChatMode::Normal`).
const CHAT_NORMAL_HI_WIRE: [u8; 8] = [0x08, 0x00, 0x00, 0x75, 0x00, 0x01, 0x68, 0x69];
/// The frozen §8.3-**A** Move layout for G1's tile (103, 101) — 10 bytes
/// (`usSize 10`, `ucChk 0`, `cmd 240`, `kind 00`, LE x `0x0067`, LE y
/// `0x0065`).
const MOVE_103_101_WIRE: [u8; 10] = [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x67, 0x00, 0x65, 0x00];

/// Binds a loopback listener on an ephemeral port (F-01: nothing but
/// `127.0.0.1` ever appears in this suite). Mirrored from
/// `e2e_loopback.rs:83-87`.
async fn bind() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    (listener, addr)
}

/// The session configuration every test runs: credentials that fit every
/// 20-byte C-string slot, the occupied roster slot, and the village
/// blueprint (inside `1..MAX_VILLAGE_NUMBER`). Mirrored from
/// `e2e_loopback.rs:92-103`.
fn config(front: std::net::SocketAddr, game: std::net::SocketAddr) -> DriverConfig {
    DriverConfig {
        front,
        game,
        user_id: TEST_USER.to_owned(),
        password: TEST_PASSWORD.to_owned(),
        mac_address: TEST_MAC.to_owned(),
        char_slot: OCCUPIED_SLOT,
        village_ref: Some(VILLAGE_REF),
        drain_bound: Duration::from_millis(250),
    }
}

/// [`run_driver_with_commands`] through `driver::run_with_commands` — the
/// command channel wired into the in-world loop (WO-7.4 §7.3). Mirrored
/// from `e2e_loopback.rs:118-129`; the caller owns the sender half.
async fn run_driver_with_commands(
    cfg: DriverConfig,
    commands: goonzu_client::client::commands::CommandReceiver,
) -> Result<DriverOutcome, DriverError> {
    let shutdown = Shutdown::new();
    tokio::time::timeout(
        DRIVER_TIME_BUDGET,
        driver::run_with_commands(&cfg, &shutdown, commands),
    )
    .await
    .expect("the driver run must finish inside its budget")
}

/// Spawns the front double: its own task, its own budget, its own report.
/// Mirrored from `e2e_loopback.rs:132-152`.
fn spawn_front(
    listener: TcpListener,
    mutation: Mutation,
    transcript: Arc<Transcript>,
) -> JoinHandle<FrontReport> {
    tokio::spawn(async move {
        tokio::time::timeout(
            DOUBLE_TIME_BUDGET,
            run_front_double(
                listener,
                TEST_USER,
                TEST_PASSWORD,
                TEST_MAC,
                mutation,
                &transcript,
            ),
        )
        .await
        .expect("the front double must finish inside its budget")
    })
}

/// Spawns the game double with the in-world tail and the budget chosen by
/// the caller. Mirrored from `e2e_loopback.rs:172-194`.
fn spawn_game_with(
    listener: TcpListener,
    mutation: Mutation,
    transcript: Arc<Transcript>,
    tail: game_double::Tail,
    budget: Duration,
) -> JoinHandle<GameReport> {
    tokio::spawn(async move {
        tokio::time::timeout(
            budget,
            run_game_double_with(
                listener,
                TEST_USER,
                TEST_PASSWORD,
                mutation,
                &transcript,
                tail,
            ),
        )
        .await
        .expect("the game double must finish inside its budget")
    })
}

/// Awaits a double's task (the budget lives inside the task itself).
/// Mirrored from `e2e_loopback.rs:197-199`.
async fn joined<T>(handle: JoinHandle<T>) -> T {
    handle.await.expect("a double task must not panic")
}

/// The wired 8.1 stage `enter_world` arms — the `wired()` helper's view
/// (`src/input/tests.rs:1327-1343`): frozen 800x600 backbuffer rect
/// (960x680 → `(0,0,959,679)`, `Client.cpp:3206-3208`), center (100,100),
/// map size (400,400) — everything else `UiState::default()`.
fn wired_view() -> MapView {
    MapView {
        screen: ScreenMode::Mode800x600.backbuffer(false).screen_rect(),
        center: (100, 100),
        map_size: (400, 400),
    }
}

/// Pack a window client point `(x, y)` into the `lParam` a mouse message
/// carries (LOWORD = x, HIWORD = y — the layout `wm_to_input` decodes,
/// `mainfunction.cpp:213`).
fn pack(x: i32, y: i32) -> isize {
    (((y as u32) << 16) | (x as u32)) as isize
}

/// T47 (WO-8.3 §9, E40) — a raw `WM_KEYDOWN(0x2D)` decoded and fed reaches
/// the wire byte-exact as the frozen §8.3-C SitDown golden, as the
/// session's only C2S frame, then a clean `Stage::TornDown`.
#[tokio::test]
async fn raw_insert_key_down_reaches_the_wire_as_the_frozen_sitdown() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 1,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    // Decode + feed before the run (the test stands in for 8.4's
    // composition — divergence (a), WO-8.3 §8): exactly one command
    // queued, then the feed is dropped, mirroring T6's `drop(sender)` so
    // the arm drains it and disables itself (D3).
    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());
    let decoded = wm_to_input(WM_KEYDOWN, 0x2D, 0);
    assert_eq!(
        decoded,
        Some(InputEvent::KeyDown { vk: 0x2D }),
        "E40: WM_KEYDOWN carries wParam as the virtual key (E32)"
    );
    let fed = feed
        .feed(decoded.expect("WM_KEYDOWN always decodes"))
        .expect("the receiver is alive for the feed");
    assert_eq!(
        fed,
        Some(ClientCommand::Order(Order::SitDown)),
        "E40: INSERT through decode + feed enqueues the frozen sit toggle"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the command must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the command flies within the first milliseconds — long before the \
         10 s heartbeat tick"
    );

    // The byte-exact conversation: the hand-placed prefix, then SitDown as
    // the session's only C2S frame (WO-8.3 §8.3-C, frozen literal).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: SITDOWN_WIRE.to_vec(),
        wire_len: SITDOWN_WIRE.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the raw INSERT must reach the wire byte-exact as the frozen \
         SitDown golden (06 00 00 F0 00 02)"
    );
}

/// T48 (WO-8.3 §9, E40) — the typed-chat chain: `WM_KEYDOWN(0x0D)` opens
/// the line (no send), `WM_CHAR 'h'`/`'i'` enter the buffer (no send), the
/// second `WM_KEYDOWN(0x0D)` flushes `ChatMode::Normal "hi"` — and those
/// raw messages reach the wire byte-exact as the frozen §8.3-D golden.
#[tokio::test]
async fn typed_chat_reaches_the_wire_as_the_frozen_normal_chat() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 1,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    let open = wm_to_input(WM_KEYDOWN, 0x0D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(open).expect("the receiver is alive"),
        None,
        "the first RETURN opens the chat line — no send (E1, ChatFocus)"
    );
    let h = wm_to_input(WM_CHAR, 'h' as u32 as usize, 0).expect("WM_CHAR 'h' decodes");
    assert_eq!(
        feed.feed(h).expect("the receiver is alive"),
        None,
        "'h' enters the buffer — no send (D3: the cap is refused at flush)"
    );
    let i = wm_to_input(WM_CHAR, 'i' as u32 as usize, 0).expect("WM_CHAR 'i' decodes");
    assert_eq!(
        feed.feed(i).expect("the receiver is alive"),
        None,
        "'i' enters the buffer — no send"
    );
    let flush = wm_to_input(WM_KEYDOWN, 0x0D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(flush).expect("the receiver is alive"),
        Some(ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: "hi".to_owned(),
        }),
        "the second RETURN flushes the whole line as a Normal chat"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the chat must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the chat flies within the first milliseconds — long before the \
         10 s heartbeat tick"
    );

    // The byte-exact conversation, then the Normal chat "hi" as the
    // session's only C2S frame (WO-8.3 §8.3-D, frozen literal).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: CHAT_NORMAL_HI_WIRE.to_vec(),
        wire_len: CHAT_NORMAL_HI_WIRE.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the typed chat must reach the wire byte-exact as the frozen \
         golden (08 00 00 75 00 01 68 69)"
    );
}

/// T49 (WO-8.3 §9, E40 + §2.4 G1) — a raw `WM_LBUTTONDOWN` at window
/// client `(480,360)` decodes through the `+80/+40` border to backbuffer
/// `(560,400)`, resolves to tile `(103,101)`, and reaches the wire
/// byte-exact as the frozen §8.3-A Move layout.
#[tokio::test]
async fn a_left_click_reaches_the_wire_as_the_frozen_move() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 1,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    let decoded = wm_to_input(WM_LBUTTONDOWN, 0, pack(480, 360));
    assert_eq!(
        decoded,
        Some(InputEvent::LeftDown { x: 560, y: 400 }),
        "E30/§2.4 G1 step 1: the +80/+40 window→backbuffer border"
    );
    let fed = feed
        .feed(decoded.expect("WM_LBUTTONDOWN always decodes"))
        .expect("the receiver is alive for the feed");
    assert_eq!(
        fed,
        Some(ClientCommand::Order(Order::Move { x: 103, y: 101 })),
        "E40/§2.4 G1: the click resolves to tile (103,101) — frozen move"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the move must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the move flies within the first milliseconds — long before the \
         10 s heartbeat tick"
    );

    // The byte-exact conversation, then the Move as the session's only
    // C2S frame (WO-8.3 §8.3-A, frozen literal for G1's tile).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: MOVE_103_101_WIRE.to_vec(),
        wire_len: MOVE_103_101_WIRE.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the raw click must reach the wire byte-exact as the frozen Move \
         golden (0A 00 00 F0 00 00 67 00 65 00)"
    );
}

/// T50 (WO-8.3 §9, E41) — six raw messages **before** `enter_world` are
/// each discarded with `Ok(None)` and `ui()` still the default (the D4
/// guard), yet the session still completes byte-exact with zero extra
/// frames: `Tail::Commands { expect: 0 }` is the fast reply-less FIN
/// (D22), ending at `Stage::TornDown` with zero heartbeats.
#[tokio::test]
async fn pre_world_messages_enqueue_nothing_and_the_session_still_tears_down() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 0,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender); // no enter_world — pre-world

    let pre_world: [(u32, usize, isize); 6] = [
        (WM_MOUSEMOVE, 0, pack(5, 7)),
        (WM_LBUTTONDOWN, 0, pack(480, 360)),
        (WM_LBUTTONDBLCLK, 0, pack(480, 360)),
        (WM_KEYDOWN, 0x2D, 0),
        (WM_KEYUP, 0x2D, 0),
        (WM_CHAR, 'i' as u32 as usize, 0),
    ];
    for (msg, w_param, l_param) in pre_world {
        let event = wm_to_input(msg, w_param, l_param)
            .expect("all six armed message types decode — E35 is the else");
        assert_eq!(
            feed.feed(event).expect("the receiver is alive"),
            None,
            "E41: message {msg:#06x} before enter_world is discarded (D4)"
        );
    }
    assert_eq!(
        feed.ui(),
        &UiState::default(),
        "E41: the discarded messages leave ui() byte-identical to the default"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the session must complete cleanly with an empty channel");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "an empty input channel never disables a live session (event_loop.rs:547)"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "expect: 0 means the reply-less FIN lands long before the 10 s tick"
    );

    // Transcript == the hand-placed happy path exactly: nothing was pushed,
    // because nothing was ever enqueued (WO-8.3 §9 T50).
    assert_eq!(
        transcript.snapshot(),
        expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC),
        "pre-world messages enqueue nothing — the transcript stays the \
         hand-placed conversation, byte for byte"
    );
}

// ---------------------------------------------------------------------------
// Step 8.4 — the in-gate acceptance script (WO-8.4 §7.3, T52–T55), appended
// after T50 with zero deletions (T47–T50, the mirrors and every existing
// `use` line stay byte-identical; the new `use` statements below are new
// statements, not edits).
//
// F-07: every wire expectation is §2.4's hand arithmetic over the frozen
// §8.3 goldens (§8.3-A layout instances for the three-click walk), never
// regenerated from the client's encoder; the conversation prefix comes
// only from the harness's hand-placed `expected_conversation`.
// ---------------------------------------------------------------------------

use std::sync::Mutex;

use frames::{chat_body, frame, CHAR_UNIQUE, OP_CHAT};

// ---------------------------------------------------------------------------
// D30 (WO-8.4 §2.2-9): a local byte sink + DEBUG fmt subscriber, copied
// from the frozen `e2e_loopback.rs:227-282` pattern. That file is
// READ-ONLY (D22), so each new test binary carries its own small copy —
// both copies (this file and `live_acceptance.rs`) are disclosed in the
// delivery note. `set_default` is thread-scoped: the current-thread test
// runtime runs the driver on this very thread, so the capture is hermetic
// to the test that installs it.
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

// ---------------------------------------------------------------------------
// WO-8.4 §2.4-1: the three-click walk goldens — hand instances of the
// frozen §8.3-A Move layout (`usSize 0A 00` · `ucChk 00` · `cmd F0 00` ·
// `kind 00` · x LE16 · y LE16), derived from the (103,101) instance
// `MOVE_103_101_WIRE` by substituting the G1–G3 hand tiles — never read
// back from the encoder (F-07).
// ---------------------------------------------------------------------------

/// G1: window (400,300) → backbuffer (480,340) → tile (100,100).
const MOVE_100_100_WIRE: [u8; 10] = [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x64, 0x00, 0x64, 0x00];
/// G2: window (440,320) → backbuffer (520,360) → tile (101,100).
const MOVE_101_100_WIRE: [u8; 10] = [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x65, 0x00, 0x64, 0x00];
/// G3: window (360,320) → backbuffer (440,360) → tile (100,101).
const MOVE_100_101_WIRE: [u8; 10] = [0x0A, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x64, 0x00, 0x65, 0x00];

/// T52 (WO-8.4 §9, E42) — the three-click walk: three raw
/// `WM_LBUTTONDOWN` at the §2.4 window points decode through the +80/+40
/// border to their hand tiles, enqueue three `Order::Move`, and reach the
/// wire byte-exact **in script order** as the session's only C2S frames —
/// then a clean `Stage::TornDown`.
#[tokio::test]
async fn three_click_walk_reaches_the_wire_as_three_frozen_moves() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 3,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    // §2.4 G1–G3: packed window point → +80/+40 backbuffer → hand tile →
    // frozen §8.3-A golden instance (each row: decode, then feed).
    type WalkRow = (isize, (i32, i32), (u16, u16), [u8; 10]);
    let script: [WalkRow; 3] = [
        (pack(400, 300), (480, 340), (100, 100), MOVE_100_100_WIRE),
        (pack(440, 320), (520, 360), (101, 100), MOVE_101_100_WIRE),
        (pack(360, 320), (440, 360), (100, 101), MOVE_100_101_WIRE),
    ];
    let mut enqueued = 0usize;
    for (l_param, backbuffer, tile, _golden) in script {
        let decoded = wm_to_input(WM_LBUTTONDOWN, 0, l_param);
        assert_eq!(
            decoded,
            Some(InputEvent::LeftDown {
                x: backbuffer.0,
                y: backbuffer.1,
            }),
            "E30/§2.4: the packed window point decodes through the \
             +80/+40 border to {backbuffer:?}"
        );
        let fed = feed
            .feed(decoded.expect("WM_LBUTTONDOWN always decodes"))
            .expect("the receiver is alive for the feed");
        assert_eq!(
            fed,
            Some(ClientCommand::Order(Order::Move {
                x: tile.0,
                y: tile.1,
            })),
            "E42/§2.4 G-row: the click must resolve to tile {tile:?}"
        );
        enqueued += 1;
    }
    assert_eq!(
        enqueued, 3,
        "E42: all three clicks must enqueue (drained count 3)"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the walk must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the three moves fly within the first milliseconds — long before \
         the 10 s heartbeat tick"
    );

    // The byte-exact conversation: the hand-placed prefix, then the three
    // Moves byte-exact in script order (WO-8.4 §2.4 goldens, wire_len 10
    // each — the session's only C2S frames).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    for (_, _, _, golden) in script {
        expected.push(Entry {
            leg: Leg::Game,
            dir: Dir::C2S,
            plain: golden.to_vec(),
            wire_len: golden.len(),
        });
    }
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the three clicks must reach the wire byte-exact in script order \
         as the session's only C2S frames (E42)"
    );
}

/// T53 (WO-8.4 §9, E43) — the typed-chat chain with its echo: focus,
/// type `hi`, flush → the frozen §8.3-D C2S golden on the wire, and the
/// game double's 118 reply for the session's **own** character
/// (`CHAR_UNIQUE`, roster name "Hero") is dispatched as
/// `DispatchOutcome::Chat` — proven through the local D30 DEBUG capture
/// (`in-world frame dispatched` + `opcode=118` + `outcome=Chat`) — then a
/// clean `Stage::TornDown`.
#[tokio::test]
async fn typed_chat_flushes_and_its_echo_dispatches() {
    // Callsite-interest guard (disclosed in the WO-8.4 §14 delivery note):
    // tracing-core caches each callsite's interest from whichever thread
    // registers it first — a concurrent test thread with no dispatcher
    // registers against `NoSubscriber` (`tracing-core subscriber.rs:676`,
    // `Interest::never`) and poisons that callsite for the whole process,
    // which empties this capture (observed: parallel-gate runs failed with
    // `capture: ""`; isolated runs passed). A `set_default` alone repairs
    // only the callsites registered at that instant. So a process-global
    // DEBUG subscriber — same dependency set, `std::io::sink` writer — is
    // installed FIRST (every later registration answers `always`, so no
    // further poison), and the scoped capture's own `Dispatch::new`
    // rebuild runs LAST (repairing whatever was poisoned before). From
    // then on this thread's records go to the scoped capture and every
    // other thread's events are discarded into the sink.
    let _global = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::sink)
            .finish(),
    );
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(debug_capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    // The reply's char_unique is a roster id the in-world set upserted
    // (frames.rs:659 chat_body) — dispatch has no other gate on 118.
    let reply = frame(OP_CHAT, &chat_body(CHAR_UNIQUE as i16, "Hero", "hi"));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 1,
            reply: vec![reply.clone()],
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    let open = wm_to_input(WM_KEYDOWN, 0x0D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(open).expect("the receiver is alive"),
        None,
        "the first RETURN opens the chat line — no send (E1, ChatFocus)"
    );
    let h = wm_to_input(WM_CHAR, 'h' as u32 as usize, 0).expect("WM_CHAR 'h' decodes");
    assert_eq!(
        feed.feed(h).expect("the receiver is alive"),
        None,
        "'h' enters the buffer — no send (D3: the cap is refused at flush)"
    );
    let i = wm_to_input(WM_CHAR, 'i' as u32 as usize, 0).expect("WM_CHAR 'i' decodes");
    assert_eq!(
        feed.feed(i).expect("the receiver is alive"),
        None,
        "'i' enters the buffer — no send"
    );
    let flush = wm_to_input(WM_KEYDOWN, 0x0D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(flush).expect("the receiver is alive"),
        Some(ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: "hi".to_owned(),
        }),
        "the second RETURN flushes the whole line as a Normal chat"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the chat must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the chat flies within the first milliseconds — long before the \
         10 s heartbeat tick"
    );

    // The byte-exact conversation: prefix, the frozen §8.3-D C2S golden,
    // then the 118 reply byte-exact (S2C plaintext, wire_len = plain.len).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: CHAT_NORMAL_HI_WIRE.to_vec(),
        wire_len: CHAT_NORMAL_HI_WIRE.len(),
    });
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::S2C,
        plain: reply.clone(),
        wire_len: reply.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the typed chat and its echo must reach the wire byte-exact \
         (08 00 00 75 00 01 68 69 out, frame(OP_CHAT, …) back)"
    );

    // The dispatch side of the echo (E43): the capture holds the frozen
    // driver record with opcode 118 and a Chat outcome. The prefix
    // conversation already carries a "someone" 118, so the first two
    // contains-checks are multi-hit by construction (per WO §7.3); the
    // `player_name: "Hero"` pin singles out the session's own reply —
    // only that dispatch renders the roster name (frames.rs:793).
    let log = String::from_utf8_lossy(&capture.0.lock().expect("capture lock")).into_owned();
    assert!(
        log.contains("in-world frame dispatched"),
        "E43: the frozen driver dispatch record must appear (driver.rs:850); \
         got:\n{log}"
    );
    assert!(
        log.contains("opcode=118"),
        "E43: the echo dispatch must be recorded with opcode=118; got:\n{log}"
    );
    assert!(
        log.contains("outcome=Chat"),
        "E43: the echo dispatch must render outcome=Chat; got:\n{log}"
    );
    assert!(
        log.contains("player_name: \"Hero\""),
        "E43: the session's own reply (Hero), not just the prefix's chat, \
         must be dispatched as Chat; got:\n{log}"
    );
}

/// T54 (WO-8.4 §9, E44) — two physical Insert presses (Down/Up/Down)
/// enqueue **two** `Order::SitDown`: the KeyUp only clears the D9.1
/// repeat guard, so the second press fires again — two identical frames
/// by design (the server owns the toggle, Q2) — and both reach the wire
/// byte-exact in order, then a clean `Stage::TornDown`.
#[tokio::test]
async fn two_insert_presses_enqueue_two_sitdowns() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 2,
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    let down1 = wm_to_input(WM_KEYDOWN, 0x2D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(down1).expect("the receiver is alive"),
        Some(ClientCommand::Order(Order::SitDown)),
        "E44: the first INSERT press enqueues the frozen sit toggle"
    );
    let up = wm_to_input(WM_KEYUP, 0x2D, 0).expect("WM_KEYUP always decodes");
    assert_eq!(
        feed.feed(up).expect("the receiver is alive"),
        None,
        "E44: the KeyUp enqueues nothing — it only clears the D9.1 guard \
         (keybinds.rs:141)"
    );
    let down2 = wm_to_input(WM_KEYDOWN, 0x2D, 0).expect("WM_KEYDOWN always decodes");
    assert_eq!(
        feed.feed(down2).expect("the receiver is alive"),
        Some(ClientCommand::Order(Order::SitDown)),
        "E44: the second physical press fires again — one action per press \
         (Q2: the server toggles)"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the two presses must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the two presses fly within the first milliseconds — long before \
         the 10 s heartbeat tick"
    );

    // The byte-exact conversation: prefix, then two identical SitDown
    // goldens in order (WO-8.3 §8.3-C, wire_len 6 each).
    let mut expected = expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    for _ in 0..2 {
        expected.push(Entry {
            leg: Leg::Game,
            dir: Dir::C2S,
            plain: SITDOWN_WIRE.to_vec(),
            wire_len: SITDOWN_WIRE.len(),
        });
    }
    assert_eq!(
        transcript.snapshot(),
        expected,
        "both presses must reach the wire as two identical frozen SitDown \
         frames (06 00 00 F0 00 02), in order"
    );
}

/// T55 (WO-8.4 §9, E45) — Escape cancels the typed line **UI-only**: the
/// §7.3 script's every step enqueues nothing, the cancelled line is
/// cleared (`chat_open=false`, `line` empty), and the final flush of the
/// now-empty line is refused by `validate` before any send — zero C2S
/// frames, transcript byte-identical to `expected_conversation`, clean
/// `Stage::TornDown`.
///
/// Script note (disclosed in the delivery note): §9's prose says
/// "seven-message" while §7.3 enumerates eight (both `KeyUp Enter`
/// releases included) — this test implements §7.3's enumeration: the
/// KeyUps clear the D9.1 repeat guard so the final flush passes the guard
/// and actually reaches `validate` (E45's substance).
#[tokio::test]
async fn escape_cancels_the_line_and_nothing_reaches_the_wire() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 0, // the T50 fast path: reply-less FIN
            reply: Vec::new(),
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    let mut feed = InputFeed::new(sender);
    feed.enter_world(wired_view());

    // WO-8.4 §7.3's script, as enumerated there (eight raw messages).
    let script: [(u32, usize, isize, &str); 8] = [
        (
            WM_KEYDOWN,
            0x0D,
            0,
            "the first RETURN opens the chat line (E1)",
        ),
        (
            WM_KEYUP,
            0x0D,
            0,
            "the release clears the D9.1 repeat guard (keybinds.rs:141)",
        ),
        (
            WM_CHAR,
            'h' as u32 as usize,
            0,
            "'h' enters the line buffer",
        ),
        (
            WM_CHAR,
            'i' as u32 as usize,
            0,
            "'i' enters the line buffer",
        ),
        (
            WM_KEYDOWN,
            0x1B,
            0,
            "Escape cancels the line UI-only (E3 — no intent leaves translate)",
        ),
        (
            WM_KEYDOWN,
            0x0D,
            0,
            "the RETURN re-focuses the (now empty) line",
        ),
        (WM_KEYUP, 0x0D, 0, "the release clears the guard again"),
        (
            WM_KEYDOWN,
            0x0D,
            0,
            "the flush of the EMPTY line — validate refuses before any send (E45)",
        ),
    ];
    for (msg, w_param, l_param, why) in script {
        let event = wm_to_input(msg, w_param, l_param)
            .expect("every scripted message decodes — armed types only");
        assert_eq!(
            feed.feed(event).expect("the receiver is alive"),
            None,
            "E45: {why} — must enqueue nothing"
        );
    }
    // The cancel's substance (E3/E45): the line is gone, nothing lingers
    // for the flush to send. (`in_world`/`view` stay armed — that is
    // `enter_world`'s own state, untouched by the script.)
    assert!(
        !feed.ui().chat_open,
        "E3: Escape closed the chat line (chat_open=false)"
    );
    assert!(
        feed.ui().line.is_empty(),
        "E3/E45: the cancelled line is cleared — nothing survives to flush"
    );
    assert!(
        !feed.ui().dialog_open,
        "no InterfaceMgr dialog exists for Escape to open (validate.rs:46-47)"
    );
    drop(feed);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly with zero commands");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN must end at TornDown"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete as in every happy path"
    );

    let game_report = joined(game).await;
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "expect: 0 means the reply-less FIN lands long before the 10 s tick"
    );

    // Transcript == the hand-placed happy path byte-for-byte: zero pushed
    // entries — if the cancelled line had survived, the flush would have
    // pushed the D golden and this equality fails (E45).
    assert_eq!(
        transcript.snapshot(),
        expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC),
        "the cancelled line must push nothing — the transcript stays the \
         hand-placed conversation, byte for byte"
    );
}
