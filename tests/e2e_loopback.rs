// Step 6.2 — the hermetic E2E loopback harness (WO-6.2).
//
// An in-process fake front service and fake game service speak the full
// conversation byte-exactly over loopback and drive `driver::run` end to
// end: front handshake → notice → world list → front login → game handshake
// → game login → roster → select char → SERVERINFO/opcode-5 → village →
// in-world dispatcher frames → heartbeat → a server-side FIN that must
// surface as a clean teardown.
//
// F-07 (`CONTRIBUTING_PHASE2.md:64-83`): every expectation is hand-placed
// in `frames.rs` at its literal wire offset with the legacy/server authority
// cited inline — never re-derived from the client's parsers, and never as
// an expected ciphertext from the client's own encoder. The mutation tests
// prove the harness notices a single flipped byte at each stage.
//
// The suite runs on the real clock (WO-6.2 §5.3): the in-world phase waits
// out exactly one 10 s heartbeat tick, because `driver::run` dials through
// the production connect path whose bounded window races tokio's virtual
// auto-advance (`src/client/event_loop/tests.rs`, module docs).
//
// The doubles are spawned tasks; the driver runs inline on the test's
// current-thread runtime — exactly as `src/main.rs:30` runs it in
// production, and for the same reason: its future is not `Send` (it holds
// non-`Send` internals across await points), so it can be awaited but not
// spawned.
#[path = "e2e_loopback/frames.rs"]
mod frames;
#[path = "e2e_loopback/front_double.rs"]
mod front_double;
#[path = "e2e_loopback/game_double.rs"]
mod game_double;
#[path = "e2e_loopback/robustness.rs"]
mod robustness;
#[path = "e2e_loopback/soak.rs"]
mod soak;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use goonzu_client::auth::login::LoginError;
use goonzu_client::auth::world_list::WorldListError;
use goonzu_client::client::commands::{channel, ClientCommand};
use goonzu_client::client::driver::{self, DriverConfig, DriverError, DriverOutcome, Stage};
use goonzu_client::client::teardown::Shutdown;
use goonzu_client::game::action::{ChatMode, Order, REQUEST_CHAT};
use goonzu_client::game::entry::GameError;
use goonzu_client::game::world_entry::WorldEntryError;

use frames::{
    char_update_body, chat_body, frame, game_login_payload, handshake_payload, logout_payload,
    make_char_record, mutated_worldlist_response, negative_village_struct_payload, person_ip_body,
    roster_entry, select_char_payload, server_info_payload, village_request_payload,
    village_struct_payload, worldinfo_entry, Dir, Entry, Leg, Mutation, Transcript,
    CHAR_LIST_PAYLOAD_LEN, CHAR_UNIQUE, CHAR_UPDATE_PAYLOAD_LEN, CHAT_TOWN_WIRE, HEARTBEAT_WIRE,
    MAKE_CHAR_PAYLOAD_LEN, MOVE_COMMAND_WIRE, NEARBY_UNIQUE, OCCUPIED_SLOT, OP_CHARLIST,
    OP_CHARUPDATE, OP_CHAT, OP_CLIENTCHARUNIQUE, OP_GAME_LOGIN, OP_LOGINMANY, OP_LOGOUT,
    OP_MAKE_CHAR, OP_PERSON_IP, OP_REQUEST_LOGIN, OP_REQUEST_NOTICE, OP_REQUEST_WORLDLIST,
    OP_RESPONSE_LOGIN, OP_RESPONSE_NOTICE, OP_RESPONSE_WORLDLIST, OP_SELECTCHAR, OP_SERVERINFO,
    OP_VILLAGESTRUCT_REQ, OP_VILLAGESTRUCT_RES, ORDER_ECHO_241_WIRE, PERSON_IP_PAYLOAD_LEN,
    ROSTER_SLOTS, SECRETKEY_WIRE, VILLAGE_REF, WORLDINFO_LEN,
};
use front_double::{run_front_double, FrontReport};
use game_double::{run_game_double_with, GameReport};

/// The account id the test types into the front requests (`szAccountID[20]`,
/// `userid[20]`).
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

/// Binds a loopback listener on an ephemeral port (F-01: nothing but
/// `127.0.0.1` ever appears in this suite).
async fn bind() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    (listener, addr)
}

/// The session configuration every test runs: credentials that fit every
/// 20-byte C-string slot, the occupied roster slot, and the village
/// blueprint (inside `1..MAX_VILLAGE_NUMBER`).
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

/// Runs `driver::run` inline on the test's current-thread runtime — the
/// production posture (`src/main.rs:30`) — inside a budget so a hung session
/// fails the test rather than the gate run.
async fn run_driver(cfg: DriverConfig) -> Result<DriverOutcome, DriverError> {
    let shutdown = Shutdown::new();
    tokio::time::timeout(DRIVER_TIME_BUDGET, driver::run(&cfg, &shutdown))
        .await
        .expect("the driver run must finish inside its budget")
}

/// Step 7.4: [`run_driver`] through `driver::run_with_commands` — the
/// command channel wired into the in-world loop (WO-7.4 §7.3). Same budget,
/// same inline posture; the caller owns the sender half.
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

/// Spawns the game double: its own task, its own budget, its own report.
fn spawn_game(
    listener: TcpListener,
    mutation: Mutation,
    transcript: Arc<Transcript>,
) -> JoinHandle<GameReport> {
    spawn_game_with(
        listener,
        mutation,
        transcript,
        game_double::Tail::HeartbeatThenFin,
        DOUBLE_TIME_BUDGET,
    )
}

/// Step 6.3: [`spawn_game`] with the in-world tail and the budget chosen by
/// the caller — the storm needs a longer budget than the 6.2 conversation
/// (its third heartbeat lands ~30 s after entry, `heartbeat.rs:169`).
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
async fn joined<T>(handle: JoinHandle<T>) -> T {
    handle.await.expect("a double task must not panic")
}

/// Step 4 (G-7): the game double with Step 4's SERVERINFO hold wired in —
/// see [`game_double::ServerInfoGate`].
fn spawn_game_gated(
    listener: TcpListener,
    mutation: Mutation,
    transcript: Arc<Transcript>,
    gate: game_double::ServerInfoGate,
) -> JoinHandle<GameReport> {
    tokio::spawn(async move {
        tokio::time::timeout(
            DOUBLE_TIME_BUDGET,
            game_double::run_game_double_gated(
                listener,
                TEST_USER,
                TEST_PASSWORD,
                mutation,
                &transcript,
                game_double::Tail::HeartbeatThenFin,
                Some(gate),
            ),
        )
        .await
        .expect("the game double must finish inside its budget")
    })
}

/// A byte sink the scoped tracing subscriber writes into, so the terminal
/// INFO record the driver emits can be asserted on (WO-6.2 §5.4).
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

/// An INFO-level, ANSI-free fmt subscriber over the capture sink. Installed
/// with `tracing::subscriber::set_default`, which is thread-scoped — the
/// current-thread test runtime runs the driver on this very thread, so the
/// capture is hermetic to this test.
fn capture_subscriber(capture: Capture) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(capture)
        .finish()
}

/// Step 6.3: the same capture subscriber at `DEBUG` — the soak counts the
/// driver's per-dispatch records (`driver.rs:684`) to prove the client
/// *processed* every streamed frame, not merely that the doubles wrote
/// them (WO-6.3 §6(a)).
///
/// Safe by construction: `set_default` builds a `Dispatch`, which registers
/// itself and **rebuilds every callsite's interest**
/// (`tracing-core-0.1.36/src/dispatcher.rs:472-481`, `callsite.rs:484-488`);
/// a mixed-subscriber process collapses differing opinions to
/// `Interest::sometimes` (`tracing-core-0.1.36/src/subscriber.rs:658-664`)
/// and defers to the current thread's subscriber — this thread's — so a
/// concurrently running INFO-captured test can never cache this callsite
/// out from under the soak.
fn debug_capture_subscriber(capture: Capture) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(capture)
        .finish()
}

/// The full flow: every frame on both legs, byte-exact, then a clean
/// teardown after the server-side close (WO-6.2 §6, acceptance items 1-2).
#[tokio::test]
async fn a_full_session_conversation_is_byte_exact_and_tears_down_cleanly() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(game_listener, Mutation::None, Arc::clone(&transcript));

    let outcome = run_driver(config(front_addr, game_addr))
        .await
        .expect("the full session must complete cleanly");

    let front_report = joined(front).await;
    assert_eq!(
        front_report.frames_received, 3,
        "the client must send exactly notice, world list and login \
         (driver.rs:541-568)"
    );
    assert!(
        front_report.closed_cleanly,
        "the driver must drop the front link after the login response \
         (driver.rs:574-576)"
    );
    assert!(
        !front_report.saw_corruption,
        "no frame on the front leg may arrive short or fail its integrity check"
    );

    let game_report = joined(game).await;
    assert!(
        game_report.heartbeats >= 1,
        "the in-world phase must observe at least one client heartbeat \
         (acceptance item: heartbeat observation)"
    );
    assert!(
        !game_report.closed_early,
        "the client must still be in-world when the first heartbeat arrives"
    );
    assert!(
        !game_report.saw_corruption,
        "no frame on the game leg may arrive short or fail its integrity check"
    );

    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "a successful run ends at TornDown, written only after teardown \
         returns (driver.rs:498-503)"
    );
    assert_eq!(outcome.worlds, 1, "the world list carried one entry");
    assert_eq!(
        outcome.char_unique, CHAR_UNIQUE,
        "the outcome must carry the session's char_unique from the handshake"
    );
    assert_eq!(
        outcome.char_slot, OCCUPIED_SLOT,
        "the outcome must carry the selected slot"
    );
    assert_eq!(
        outcome.village,
        Some(VILLAGE_REF),
        "the outcome must carry the requested village"
    );

    // The byte-exact conversation: every frame, both directions, in order.
    assert!(
        !transcript.is_empty(),
        "the transcript must carry the frames both doubles observed"
    );
    let mut expected = frames::expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    for _ in 0..game_report.heartbeats {
        expected.push(Entry {
            leg: Leg::Game,
            dir: Dir::C2S,
            plain: HEARTBEAT_WIRE.to_vec(),
            wire_len: HEARTBEAT_WIRE.len(),
        });
    }
    assert_eq!(
        transcript.snapshot(),
        expected,
        "every server→client frame sent and every client→server frame \
         received must match the expected conversation byte-for-byte"
    );

    // The teardown cause is observable only in the emitted record
    // (WO-6.2 §5.4): a server-side close must surface as
    // Cause::ServerDisconnect at TornDown.
    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    assert!(
        logs.contains("session ended"),
        "the driver must emit its terminal INFO record (driver.rs:510-520)"
    );
    assert!(
        logs.contains("stage=TornDown"),
        "the terminal record must name the TornDown stage; got:\n{logs}"
    );
    assert!(
        logs.contains("cause=Some(ServerDisconnect)"),
        "the terminal record must carry Cause::ServerDisconnect from the \
         server-side FIN; got:\n{logs}"
    );
    assert!(
        logs.contains("worlds=1"),
        "the terminal record must carry the world count; got:\n{logs}"
    );
}

/// Every server→client wire length in the flow, from the independent size
/// constants — the arithmetic the legacy structs pin, asserted without any
/// socket (acceptance item 2's size half).
#[test]
fn the_wire_lengths_match_the_legacy_sizes() {
    assert_eq!(SECRETKEY_WIRE.len(), 10, "usSize 10 (FrontMsg.h:132-141)");
    assert_eq!(
        frame(OP_REQUEST_NOTICE, &[]).len(),
        5,
        "sRequest_Notice is header-only (FrontMsg.h:477-480)"
    );
    assert_eq!(
        frame(OP_RESPONSE_NOTICE, &frames::NOTICE_TEXT).len(),
        21,
        "usSize = 5 + 16 (FrontClientProtocol.cpp:760)"
    );
    assert_eq!(
        frame(OP_REQUEST_WORLDLIST, &frames::account_id_payload(TEST_USER)).len(),
        25,
        "usSize = 5 + 20 (FrontMsg.h:486-491)"
    );
    {
        let mut payload = vec![0x01, 0x00];
        payload.extend_from_slice(&worldinfo_entry());
        assert_eq!(
            frame(OP_RESPONSE_WORLDLIST, &payload).len(),
            5 + 2 + WORLDINFO_LEN,
            "usSize = 5 + 2 + 428 (WorldList.cpp:215-256)"
        );
    }
    assert_eq!(
        frame(
            OP_REQUEST_LOGIN,
            &frames::login_request_payload(TEST_USER, TEST_PASSWORD, TEST_MAC)
        )
        .len(),
        65,
        "usSize = 5 + 60 (FrontMsg.h:336-348)"
    );
    assert_eq!(
        frame(OP_RESPONSE_LOGIN, &frames::login_response_payload()).len(),
        113,
        "usSize = 5 + 108 (FrontMsg.h:419-442)"
    );
    assert_eq!(
        frame(OP_CLIENTCHARUNIQUE, &handshake_payload()).len(),
        13,
        "usSize = 5 + 8 (MsgType.h:22)"
    );
    assert_eq!(
        frame(OP_GAME_LOGIN, &game_login_payload(TEST_PASSWORD)).len(),
        53,
        "usSize 53 (MsgType.h:2967)"
    );
    assert_eq!(
        frame(OP_CHARLIST, &roster_entry(0, TEST_USER, None)).len(),
        5 + CHAR_LIST_PAYLOAD_LEN,
        "usSize 989 (Char-Common.h:453-471)"
    );
    assert_eq!(
        frame(OP_SELECTCHAR, &select_char_payload(OCCUPIED_SLOT)).len(),
        9,
        "usSize = 5 + 4 (NSelectChar.cpp:1353)"
    );
    assert_eq!(
        frame(OP_SERVERINFO, &server_info_payload()).len(),
        53,
        "usSize = 5 + 48 (Server-Login.cpp:184)"
    );
    assert_eq!(
        frame(OP_MAKE_CHAR, &make_char_record(CHAR_UNIQUE as u16, 12)).len(),
        5 + MAKE_CHAR_PAYLOAD_LEN,
        "usSize 597 (CommonHeader.h:2166)"
    );
    assert_eq!(
        frame(OP_VILLAGESTRUCT_REQ, &village_request_payload(VILLAGE_REF)).len(),
        9,
        "usSize = 5 + 4 (Client-Login.cpp:281)"
    );
    assert_eq!(
        frame(OP_VILLAGESTRUCT_RES, &village_struct_payload(VILLAGE_REF)).len(),
        5 + frames::VILLAGE_STRUCT_PAYLOAD_LEN,
        "usSize = 5 + 22 (MsgType.h:3085-3102)"
    );
    {
        let body = vec![0u8; 2 * MAKE_CHAR_PAYLOAD_LEN];
        assert_eq!(
            frame(OP_LOGINMANY, &body).len(),
            5 + 2 * MAKE_CHAR_PAYLOAD_LEN,
            "opcode 6 body = two 592-byte records (Client-Login.cpp:53-85)"
        );
    }
    assert_eq!(
        frame(OP_LOGOUT, &logout_payload(NEARBY_UNIQUE)).len(),
        9,
        "usSize = 5 + 4 (MsgType.h:2977-2983)"
    );
    assert_eq!(
        frame(
            OP_CHAT,
            &chat_body(NEARBY_UNIQUE as i16, "someone", "hi all")
        )
        .len(),
        5 + 20 + 14,
        "usSize = 5 + 20 fixed + name + text + NUL (MsgType-System.h:1158)"
    );
    assert_eq!(
        frame(OP_CHARUPDATE, &char_update_body(CHAR_UNIQUE as i16)).len(),
        5 + CHAR_UPDATE_PAYLOAD_LEN,
        "usSize = 5 + 24 (CommonLogic.h:383-410)"
    );
    assert_eq!(
        frame(OP_PERSON_IP, &person_ip_body(CHAR_UNIQUE as i16)).len(),
        5 + PERSON_IP_PAYLOAD_LEN,
        "usSize = 5 + 72 (game.rs:426)"
    );
    assert_eq!(
        HEARTBEAT_WIRE.len(),
        5,
        "the heartbeat is header-only (MsgType.h:2967)"
    );
    assert_eq!(
        HEARTBEAT_WIRE,
        [0x05, 0x00, 0x00, 0x64, 0x00],
        "usSize 5 LE, ucChk 0, usCmd 100 LE (heartbeat/tests.rs:74)"
    );
}

/// Mutation — front stage: one flipped byte in the world-list response's
/// `usCount` must make the client refuse at `WorldList`, and the harness
/// must have observed the corrupted frame (acceptance item 3).
#[tokio::test]
async fn a_flipped_byte_in_the_world_list_response_is_refused_at_the_world_list_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (_game_listener, game_addr) = bind().await; // never dialled
    let front = spawn_front(
        front_listener,
        Mutation::WorldListCount,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a world list that declares two entries but carries one must refuse");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::WorldList,
                ..
            }
        ),
        "expected a Stage{{WorldList}} refusal, got {err:?}"
    );

    let front_report = joined(front).await;
    assert_eq!(
        front_report.frames_sent, 3,
        "the double must have sent SECRETKEY, notice and the corrupted world list"
    );
    assert!(
        front_report.ended_by_peer,
        "the refusing client must drop the front connection"
    );
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Front
                && entry.dir == Dir::S2C
                && entry.plain == mutated_worldlist_response()
        }),
        "the harness must have observed the corrupted world-list frame it sent"
    );
    assert!(
        transcript
            .snapshot()
            .iter()
            .all(|entry| entry.leg == Leg::Front),
        "the session must stop before the game leg"
    );
}

/// Characterization (Step 3, T-4 / G-1) — a login response that parses but
/// carries `usResult` 2 must be refused at `FrontLogin` with the owning
/// step's `LoginError::Rejected` in the source chain, before anything moves
/// toward the game leg (`Client-Winsock.cpp:287-350` result switch,
/// `login.rs` capture gate).
#[tokio::test]
async fn a_rejected_login_result_is_refused_at_the_front_login_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (_game_listener, game_addr) = bind().await; // never dialled
    let front = spawn_front(
        front_listener,
        Mutation::LoginResult,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a login response with usResult 2 must never capture");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::FrontLogin,
                ..
            }
        ),
        "expected a Stage{{FrontLogin}} refusal, got {err:?}"
    );
    let source = std::error::Error::source(&err).expect("a Stage refusal carries its step's error");
    let login = source
        .downcast_ref::<LoginError>()
        .expect("the front login stage must surface a LoginError");
    assert!(
        matches!(login, LoginError::Rejected { result: 2 }),
        "expected LoginError::Rejected{{result: 2}}, got {login:?}"
    );

    let front_report = joined(front).await;
    assert_eq!(
        front_report.frames_received, 3,
        "the double must have seen the notice, world list and login requests"
    );
    assert_eq!(
        front_report.frames_sent, 4,
        "the double must have sent SECRETKEY, notice, world list and the rejected login"
    );
    assert!(
        front_report.closed_cleanly,
        "the refusing client must close the front link as it stops"
    );
    assert!(
        transcript
            .snapshot()
            .iter()
            .all(|entry| entry.leg == Leg::Front),
        "the session must stop before the game leg"
    );
}

/// Characterization (Step 3, T-4 / G-2) — a notice response of 15 bytes
/// instead of the exact 16 must be refused at `WorldList` with
/// `WorldListError::UnexpectedNoticeLength` in the source chain
/// (`FrontClientProtocol.cpp:760` exact 16, `world_list.rs` gate).
#[tokio::test]
async fn a_short_notice_response_is_refused_at_the_world_list_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (_game_listener, game_addr) = bind().await; // never dialled
    let front = spawn_front(
        front_listener,
        Mutation::ShortNotice,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a 15-byte notice must never parse");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::WorldList,
                ..
            }
        ),
        "expected a Stage{{WorldList}} refusal, got {err:?}"
    );
    let source = std::error::Error::source(&err).expect("a Stage refusal carries its step's error");
    let world = source
        .downcast_ref::<WorldListError>()
        .expect("the world-list stage must surface a WorldListError");
    assert!(
        matches!(world, WorldListError::UnexpectedNoticeLength { len: 15 }),
        "expected WorldListError::UnexpectedNoticeLength{{len: 15}}, got {world:?}"
    );

    let front_report = joined(front).await;
    assert_eq!(
        front_report.frames_sent, 2,
        "the double must have sent SECRETKEY and the short notice only"
    );
    assert!(
        front_report.ended_by_peer,
        "the refusing client must drop the front connection before the world-list request"
    );
}

/// Mutation — game-login stage: one flipped byte in the occupied roster
/// entry's name must make the client refuse at `CharList` (acceptance
/// item 3).
#[tokio::test]
async fn a_flipped_byte_in_the_roster_is_refused_at_the_char_list_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(game_listener, Mutation::RosterName, Arc::clone(&transcript));

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("an empty occupied slot must refuse the roster");
    assert!(
        matches!(
            err,
            DriverError::Refused {
                stage: Stage::CharList,
                ..
            }
        ),
        "expected a Refused{{CharList}} refusal, got {err:?}"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete before the game leg"
    );

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for SELECTCHAR"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the session must not reach the in-world phase"
    );

    let mut corrupted = roster_entry(OCCUPIED_SLOT as u8, TEST_USER, Some("Hero"));
    corrupted[80] = 0; // szName[0] -> NUL, the mutation the double applied
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Game
                && entry.dir == Dir::S2C
                && entry.plain == frame(OP_CHARLIST, &corrupted)
        }),
        "the harness must have observed the corrupted roster frame it sent"
    );
}

/// Mutation — world-entry stage: one flipped byte in the opcode-5 record's
/// `uiMakeCharUnique` must make the client refuse at `CharClient`
/// (acceptance item 3).
#[tokio::test]
async fn a_flipped_byte_in_the_opcode_5_record_is_refused_at_the_char_client_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(
        game_listener,
        Mutation::MakeCharUnique,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("an opcode-5 record for another character must refuse");
    assert!(
        matches!(
            err,
            DriverError::Refused {
                stage: Stage::CharClient,
                ..
            }
        ),
        "expected a Refused{{CharClient}} refusal, got {err:?}"
    );

    let front_report = joined(front).await;
    assert!(front_report.closed_cleanly);

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for the village request"
    );
    assert_eq!(game_report.heartbeats, 0);

    let mut record = make_char_record(CHAR_UNIQUE as u16, 12);
    record[4] ^= 0x01; // the mutation the double applied
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Game
                && entry.dir == Dir::S2C
                && entry.plain == frame(OP_MAKE_CHAR, &record)
        }),
        "the harness must have observed the corrupted opcode-5 frame it sent"
    );
}

/// Mutation — in-world stage: one flipped byte in the chat body's declared
/// `siDataSize` must make the dispatcher refuse and the client stop at
/// `InWorld` (acceptance item 3).
#[tokio::test]
async fn a_flipped_byte_in_a_chat_frame_is_refused_at_the_in_world_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(
        game_listener,
        Mutation::ChatDataSize,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a chat body whose declared size disagrees must refuse");
    assert!(
        matches!(
            err,
            DriverError::Refused {
                stage: Stage::InWorld,
                ..
            }
        ),
        "expected a Refused{{InWorld}} refusal, got {err:?}"
    );

    let front_report = joined(front).await;
    assert!(front_report.closed_cleanly);

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for a heartbeat"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the session must stop inside the dispatcher, before any heartbeat"
    );

    let mut chat = chat_body(NEARBY_UNIQUE as i16, "someone", "hi all");
    let declared = i16::from_le_bytes([chat[0], chat[1]]);
    chat[0..2].copy_from_slice(&(declared + 1).to_le_bytes()); // the mutation
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Game && entry.dir == Dir::S2C && entry.plain == frame(OP_CHAT, &chat)
        }),
        "the harness must have observed the corrupted chat frame it sent"
    );
    assert!(
        transcript.snapshot().len() >= ROSTER_SLOTS,
        "the harness must have observed the whole roster before the in-world set"
    );
}

// ---------------------------------------------------------------------------
// Step 4 characterization (G-1 … G-6) — one falsifiable test per seam of the
// game leg, each asserting a bounded, stage-named outcome. G-7 lives beside
// its Ctrl+C predecessor in `robustness.rs`.
// ---------------------------------------------------------------------------

/// Step 4 (G-1): the game double's **first** frame is a link-ciphered
/// opcode 3 in the unsolicited opcode-1 handshake's place. This port refuses
/// any first command it does not own (`GameError::UnexpectedOpcode`,
/// `entry.rs:236-241`) and negotiates nothing on a failure — the encoder and
/// the key slot are written only after a full parse succeeds
/// (`entry.rs:447-451`) — so the session must stop at `LinkKey` with the
/// owning step's `GameError` in the source chain and no game login ever sent.
#[tokio::test]
async fn a_foreign_first_game_frame_is_refused_at_the_link_key_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(
        game_listener,
        Mutation::GameHandshakeForeign,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a roster frame in the handshake's place must refuse");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::LinkKey,
                ..
            }
        ),
        "expected a Stage{{LinkKey}} refusal, got {err:?}"
    );
    let source = std::error::Error::source(&err).expect("a Stage refusal carries its step's error");
    let game_error = source
        .downcast_ref::<GameError>()
        .expect("the link-key stage must surface a GameError");
    assert!(
        matches!(game_error, GameError::UnexpectedOpcode { cmd: OP_CHARLIST }),
        "expected GameError::UnexpectedOpcode{{cmd: {OP_CHARLIST}}}, got {game_error:?}"
    );

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg must complete before the game leg is dialled"
    );

    let game_report = joined(game).await;
    assert_eq!(
        game_report.frames_received, 0,
        "a refused handshake must never reach the opcode-2 game login"
    );
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for the game login"
    );
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Game
                && entry.dir == Dir::S2C
                && entry.plain == frame(OP_CHARLIST, &roster_entry(0, TEST_USER, None))
        }),
        "the harness must have observed the foreign frame it put in the \
         handshake's place"
    );
}

/// Step 4 (G-2): nothing is listening on the game port. The front leg must
/// run all the way to its login response first — the legacy dials the game
/// service only after the front login captured (`front_phase`,
/// `driver.rs:536-591`) — and the dial failure must be named
/// `GameConnected`.
#[tokio::test]
async fn a_dead_game_port_is_reported_at_the_game_connected_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    drop(game_listener); // nothing listens: the dial must fail fast
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a refused game dial must refuse the session");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::GameConnected,
                ..
            }
        ),
        "expected a Stage{{GameConnected}} refusal, got {err:?}"
    );

    let front_report = joined(front).await;
    assert_eq!(
        front_report.frames_received, 3,
        "the front leg must have sent notice, world list and login before \
         the game service was dialled"
    );
    assert!(
        front_report.closed_cleanly,
        "the front link must be dropped where `front_phase` drops it \
         (driver.rs:587)"
    );
    assert!(
        transcript
            .snapshot()
            .iter()
            .all(|entry| entry.leg == Leg::Front),
        "the session must stop before any game frame is exchanged"
    );
}

/// Step 4 (G-3): the SERVERINFO banner names another region
/// (`ConstServiceArea_Korea` = 0x01). The legacy handler's silent `return`
/// (`Client-System.cpp:1682`) drops it without storing anything, so every
/// *other* world-entry condition holds and the session stops at `InWorld`
/// with the driver's own detail — after the warning that names the stage it
/// left untouched (`driver.rs:649-654`).
#[tokio::test]
async fn a_banner_for_another_region_is_dropped_and_refused_at_the_in_world_stage() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(
        game_listener,
        Mutation::ServerInfoRegion,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a banner for another region must never be stored");
    assert!(
        matches!(
            err,
            DriverError::Refused {
                stage: Stage::InWorld,
                ..
            }
        ),
        "expected a Refused{{InWorld}} refusal, got {err:?}"
    );
    let DriverError::Refused { detail, .. } = &err else {
        panic!("expected a Refused refusal, got {err:?}");
    };
    assert!(
        detail.contains("the banner was not accepted?"),
        "the refusal must name the banner as the unsatisfied condition; got: {detail}"
    );

    // The region gate's own report: the frame parsed, the banner did not
    // match, and the state was left untouched at `ServerInfo`.
    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    assert!(
        logs.contains("the service-area banner was not for this build; state left untouched"),
        "the dropped banner must be warned about, not silently ignored; got:\n{logs}"
    );
    assert!(
        logs.contains("stage=ServerInfo"),
        "the warning must name the stage it left untouched; got:\n{logs}"
    );

    let front_report = joined(front).await;
    assert!(front_report.closed_cleanly);

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for a heartbeat"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the session must stop before the in-world phase"
    );
}

/// Step 4 (G-4): the dispatcher's per-frame outcome is observable in the
/// driver's own `DEBUG` record, so the wiring between world entry and the
/// dispatcher is pinned from the outside: the opcode-200 delta naming the
/// session's `char_unique` must be reported as landing on the **local**
/// avatar, which is only true when `DispatcherState::new` was handed the
/// handshake's unique (`dispatcher.rs:921-948`). The expectation is derived
/// from the fixture's own `CHAR_UNIQUE` and `ApplyTarget`'s meaning, never
/// read back from the record under assertion.
#[tokio::test]
async fn the_dispatch_record_reports_the_local_target_for_the_sessions_own_unique() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(debug_capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(game_listener, Mutation::None, Arc::clone(&transcript));

    let outcome = run_driver(config(front_addr, game_addr))
        .await
        .expect("the full session must complete cleanly");

    let front_report = joined(front).await;
    let game_report = joined(game).await;
    assert_eq!(outcome.stage, Stage::TornDown);
    assert!(
        front_report.closed_cleanly && !game_report.saw_corruption,
        "both legs must complete cleanly for the dispatch record to exist"
    );

    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    let expected = format!(
        "opcode={OP_CHARUPDATE} outcome=Applied {{ char_unique: {CHAR_UNIQUE}, target: Local }}"
    );
    assert!(
        logs.contains(&expected),
        "the opcode-200 record must show the session's own unique landing on \
         the local avatar (dispatcher.rs:921-948); expected `{expected}` in:\n\
         {logs}"
    );
}

/// Step 4 (G-6): the village answer carries a negative `VillageUnique`. The
/// legacy's message-box branch (`Client-SystemNPC.cpp:880,898-900`) is
/// surfaced as `WorldEntryError::NegativeVillageUnique`, which the driver
/// maps to the stage it was reading at — `InWorld`.
#[tokio::test]
async fn a_negative_village_answer_is_refused_at_the_in_world_stage() {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game(
        game_listener,
        Mutation::VillageNegative,
        Arc::clone(&transcript),
    );

    let err = run_driver(config(front_addr, game_addr))
        .await
        .expect_err("a negative village answer must refuse the session");
    assert!(
        matches!(
            err,
            DriverError::Stage {
                stage: Stage::InWorld,
                ..
            }
        ),
        "expected a Stage{{InWorld}} refusal, got {err:?}"
    );
    let source = std::error::Error::source(&err).expect("a Stage refusal carries its step's error");
    let world = source
        .downcast_ref::<WorldEntryError>()
        .expect("the in-world stage must surface a WorldEntryError");
    assert!(
        matches!(
            world,
            WorldEntryError::NegativeVillageUnique { village_unique: -1 }
        ),
        "expected WorldEntryError::NegativeVillageUnique{{village_unique: \
         -1}}, got {world:?}"
    );

    let front_report = joined(front).await;
    assert!(front_report.closed_cleanly);

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for the in-world set"
    );
    assert_eq!(game_report.heartbeats, 0);
    assert!(
        transcript.snapshot().iter().any(|entry| {
            entry.leg == Leg::Game
                && entry.dir == Dir::S2C
                && entry.plain == frame(OP_VILLAGESTRUCT_RES, &negative_village_struct_payload())
        }),
        "the harness must have observed the negative village frame it sent"
    );
}

// ---------------------------------------------------------------------------
// Step 7.4 (WO-7.4 §9) — the command channel's three end-to-end invariants,
// T6-T8. Same harness posture as the 6.2 conversation above: both doubles
// spawned first, the driver run inline on the test's current-thread runtime,
// expectations from the §8 goldens and `expected_conversation` only (F-07).
// ---------------------------------------------------------------------------

/// T6 (WO-7.4 §9) — two commands queued **before** the session starts travel
/// the wire byte-exact: **G1** (Move, x=100 y=200) then **G2** (Town chat
/// `"hi"`), both `Dir::C2S`, in queue order, against the §8 goldens — and the
/// session then tears down cleanly (`Stage::TornDown`, no corruption).
///
/// The `Tail::Commands` tail replaces the default one because that tail
/// asserts the first in-world C2S frame is a heartbeat
/// (`game_double.rs:403-415`); here the two command frames arrive instead.
#[tokio::test]
async fn queued_commands_travel_the_wire_byte_exact() {
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

    // Both commands are queued before the run; the sender is dropped, so the
    // arm drains them and then disables itself (D3) — the session ends with
    // the channel already closed and unaffected by that.
    let (sender, receiver) = channel();
    sender
        .send(ClientCommand::Order(Order::Move { x: 100, y: 200 }))
        .expect("queue the move");
    sender
        .send(ClientCommand::Chat {
            mode: ChatMode::Town,
            text: "hi".to_owned(),
        })
        .expect("queue the town chat");
    drop(sender);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the command session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the commands must end at TornDown"
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
        "both commands fly within the first milliseconds — long before the \
         10 s heartbeat tick"
    );

    // The byte-exact conversation: everything a plain session exchanges, then
    // G1 and G2 as the two C2S frames, in queue order (WO-7.4 §8).
    let mut expected = frames::expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: MOVE_COMMAND_WIRE.to_vec(),
        wire_len: MOVE_COMMAND_WIRE.len(),
    });
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: CHAT_TOWN_WIRE.to_vec(),
        wire_len: CHAT_TOWN_WIRE.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the queued commands must reach the wire byte-exact, in order: G1 \
         (Move) then G2 (Town chat)"
    );
}

/// T7 (WO-7.4 §9, D4) — an empty chat command is refused by the frozen
/// `send_chat` **before a single byte is written**: the driver surfaces it as
/// `DriverError::Refused { stage: InWorld }` whose detail carries the frozen
/// `ActionError::EmptyChat` Display (`action.rs:175`), no opcode-117 frame
/// ever reaches the transcript, and the double observes the client's own
/// close with zero heartbeats — the session died long before the 10 s tick.
#[tokio::test]
async fn an_empty_chat_command_refuses_the_session_without_a_frame() {
    let transcript = Arc::new(Transcript::new());

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    // The double waits for the one command frame the client would write —
    // which never arrives; the refusing client's close ends that read as
    // EOF (`closed_early`).
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
    sender
        .send(ClientCommand::Chat {
            mode: ChatMode::Normal,
            text: String::new(),
        })
        .expect("queue the empty chat");
    drop(sender);

    let err = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect_err("an empty chat must refuse the session");
    match &err {
        DriverError::Refused { stage, detail } => {
            assert_eq!(
                *stage,
                Stage::InWorld,
                "the refusal must be attributed to the in-world stage"
            );
            assert!(
                detail.contains("chat text is empty"),
                "the detail must carry the frozen ActionError::EmptyChat \
                 Display (action.rs:175); got: {detail}"
            );
        }
        other => panic!(
            "expected DriverError::Refused {{ stage: Stage::InWorld, \
             detail }}, got {other:?}"
        ),
    }

    let front_report = joined(front).await;
    assert!(
        front_report.closed_cleanly,
        "the front leg completed before the refusal"
    );

    let game_report = joined(game).await;
    assert!(
        game_report.closed_early,
        "the refusing client must drop the game link while the double waits \
         for the command frame"
    );
    assert!(
        !game_report.saw_corruption,
        "the refusal must be a clean close, not a broken frame"
    );
    assert_eq!(
        game_report.heartbeats, 0,
        "the session died long before the 10 s heartbeat tick"
    );

    // No opcode-117 frame anywhere on the transcript (WO-7.4 §9/T7) — and
    // since the client sent nothing at all, the transcript is exactly the
    // conversation up to the in-world set.
    let snapshot = transcript.snapshot();
    let chat_requests = snapshot
        .iter()
        .filter(|entry| {
            entry.dir == Dir::C2S
                && entry.plain.len() >= 5
                && u16::from_le_bytes([entry.plain[3], entry.plain[4]]) == REQUEST_CHAT
        })
        .count();
    assert_eq!(
        chat_requests, 0,
        "an empty chat must never put a REQUEST_CHAT (117) frame on the wire"
    );
    assert_eq!(
        snapshot,
        frames::expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC),
        "the refusing client must have sent nothing beyond the normal \
         conversation"
    );
}

/// T8 (WO-7.4 §9, D5) — the 241 echo is a **report**, not a dispatch: the
/// driver logs the dedicated `in-world order echo reported` record naming the
/// session's own unique and the `Local` target, and the dispatcher stays
/// untouched — the second opcode-200 written *after* the echo still resolves
/// to `Local`. The transcript is byte-exact: the normal conversation, G1 out,
/// then G3 (echo) and G4 (delta) in; clean teardown.
#[tokio::test]
async fn an_order_echo_is_reported_and_the_next_delta_stays_local() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(debug_capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    // G4: the harness's own stimulus — byte-identical to the 200 the
    // in-world set already writes (`frames.rs:850`); the load-bearing claim
    // is the log record, not its bytes (WO-7.4 §8).
    let g4 = frame(OP_CHARUPDATE, &char_update_body(CHAR_UNIQUE as i16));

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    // The double reads the one Move command, answers with the 241 echo (G3)
    // followed by the second 200 (G4), then FINs — the clean server-side
    // close the happy paths end with.
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        game_double::Tail::Commands {
            expect: 1,
            reply: vec![ORDER_ECHO_241_WIRE.to_vec(), g4.clone()],
        },
        DOUBLE_TIME_BUDGET,
    );

    let (sender, receiver) = channel();
    sender
        .send(ClientCommand::Order(Order::Move { x: 100, y: 200 }))
        .expect("queue the move");
    drop(sender);

    let outcome = run_driver_with_commands(config(front_addr, game_addr), receiver)
        .await
        .expect("the echo session must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the server-side FIN after the echo must end at TornDown"
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
        "the echo exchange completes well before the 10 s heartbeat tick"
    );

    // The byte-exact conversation: the normal one, G1 out (C2S), then G3 and
    // G4 in (S2C) — the §8 goldens in travel order.
    let mut expected = frames::expected_conversation(TEST_PASSWORD, TEST_USER, TEST_MAC);
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::C2S,
        plain: MOVE_COMMAND_WIRE.to_vec(),
        wire_len: MOVE_COMMAND_WIRE.len(),
    });
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::S2C,
        plain: ORDER_ECHO_241_WIRE.to_vec(),
        wire_len: ORDER_ECHO_241_WIRE.len(),
    });
    expected.push(Entry {
        leg: Leg::Game,
        dir: Dir::S2C,
        plain: g4.clone(),
        wire_len: g4.len(),
    });
    assert_eq!(
        transcript.snapshot(),
        expected,
        "the echo session must travel byte-exact: G1 out, then G3 and G4 in"
    );

    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };

    // The dedicated echo record (D5): one line, naming the echoing unique
    // and the local target.
    let echo_line = logs
        .lines()
        .find(|line| line.contains("in-world order echo reported"))
        .expect(
            "the 241 echo must be reported by its dedicated record, not the \
             blanket dispatch record (D5)",
        );
    assert!(
        echo_line.contains(&format!("char_unique={CHAR_UNIQUE}")),
        "the echo record must name the echoing unique; got: {echo_line}"
    );
    assert!(
        echo_line.contains("target=Local"),
        "the echo must resolve to the local avatar; got: {echo_line}"
    );

    // The **last** Applied record — the G4 delta written after the echo —
    // must occur after the echo record: the Ordered consumption is
    // report-only and did not poison the dispatcher, so the next delta still
    // resolves Local.
    let applied = format!(
        "opcode={OP_CHARUPDATE} outcome=Applied {{ char_unique: {CHAR_UNIQUE}, \
         target: Local }}"
    );
    let last_applied_at = logs
        .rfind(&applied)
        .expect("the G4 delta must be reported through the blanket record");
    let echo_at = logs.find(echo_line).expect("the echo line is in the log");
    assert!(
        last_applied_at > echo_at,
        "the opcode-200 after the echo must still resolve Local — the echo \
         was report-only; log:\n{logs}"
    );
}
