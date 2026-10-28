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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use goonzu_client::client::driver::{self, DriverConfig, DriverError, DriverOutcome, Stage};
use goonzu_client::client::teardown::Shutdown;

use frames::{
    char_update_body, chat_body, frame, game_login_payload, handshake_payload, logout_payload,
    make_char_record, mutated_worldlist_response, person_ip_body, roster_entry,
    select_char_payload, server_info_payload, village_request_payload, village_struct_payload,
    worldinfo_entry, Dir, Entry, Leg, Mutation, Transcript, CHAR_LIST_PAYLOAD_LEN, CHAR_UNIQUE,
    CHAR_UPDATE_PAYLOAD_LEN, HEARTBEAT_WIRE, MAKE_CHAR_PAYLOAD_LEN, NEARBY_UNIQUE, OCCUPIED_SLOT,
    OP_CHARLIST, OP_CHARUPDATE, OP_CHAT, OP_CLIENTCHARUNIQUE, OP_GAME_LOGIN, OP_LOGINMANY,
    OP_LOGOUT, OP_MAKE_CHAR, OP_PERSON_IP, OP_REQUEST_LOGIN, OP_REQUEST_NOTICE,
    OP_REQUEST_WORLDLIST, OP_RESPONSE_LOGIN, OP_RESPONSE_NOTICE, OP_RESPONSE_WORLDLIST,
    OP_SELECTCHAR, OP_SERVERINFO, OP_VILLAGESTRUCT_REQ, OP_VILLAGESTRUCT_RES,
    PERSON_IP_PAYLOAD_LEN, ROSTER_SLOTS, SECRETKEY_WIRE, VILLAGE_REF, WORLDINFO_LEN,
};
use front_double::{run_front_double, FrontReport};
use game_double::{run_game_double, GameReport};

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
    tokio::spawn(async move {
        tokio::time::timeout(
            DOUBLE_TIME_BUDGET,
            run_game_double(listener, TEST_USER, TEST_PASSWORD, mutation, &transcript),
        )
        .await
        .expect("the game double must finish inside its budget")
    })
}

/// Awaits a double's task (the budget lives inside the task itself).
async fn joined<T>(handle: JoinHandle<T>) -> T {
    handle.await.expect("a double task must not panic")
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
