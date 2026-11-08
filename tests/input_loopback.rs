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
