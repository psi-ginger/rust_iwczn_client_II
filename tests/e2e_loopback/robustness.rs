// Step 6.3 — the robustness gate (WO-6.3 §6(b)): one falsifiable test per
// scenario, each asserting a bounded, stage-named outcome and zero
// orphaned tasks.
//
// R1 — a malformed frame mid-stream (`usSize` below the header minimum and
// an oversize declaration): the decoder's refusal is sticky
// (`net/codec.rs`) and must surface as `DriverError::Stage { stage:
// InWorld }` carrying the header guard's own variant, with the link
// dropped and the teardown observed.
//
// R2 — an integrity-tag flip mid-stream: only a tag-side byte trips the
// cipher's `Ok(false)` (`cipher.rs:279,295`), and that refusal must
// surface as `CodecError::Integrity` at `InWorld`.
//
// R3 — an abrupt disconnect mid-frame: a valid `usSize`, three wire bytes,
// then FIN. The **observed documented behavior** is pinned here — the
// partial frame is discarded, never delivered, never an error
// (`game/entry.rs:472-476`), and the session ends as a clean
// `Cause::ServerDisconnect` teardown.
//
// R4 — Ctrl+C mid-session: `run_with_watcher`'s factory seam is private
// (`driver.rs:469`), so the test spawns the *same* production primitive —
// `teardown::watch_signal` — over its own injected future and triggers it
// after the first byte-exact heartbeat proves the in-world loop is live
// (`evidence/live-fix/ctrl_c_investigation.md`: a real console event is
// undeliverable in this ConPTY environment, which is exactly why the seam
// exists).
//
// Zero-orphan evidence is behavioral on every path: the driver returning
// at all implies the watcher was aborted and awaited and the tracker
// drained (`driver.rs:485-494`); the double observing the client's EOF
// proves `terminate`'s close phase ran (`teardown.rs:366`); and the Err
// paths must never emit the drain-failed-after-failure warning
// (`driver.rs:492`).
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use goonzu_client::client::driver::{self, DriverError, Stage};
use goonzu_client::client::teardown::{watch_signal, Cause, Shutdown};
use goonzu_client::game::entry::GameError;
use goonzu_client::net::codec::CodecError;
use goonzu_client::protocol::header::HeaderError;

use super::frames::{CHAR_UNIQUE, MALFORMED_OVERSIZE_WIRE, MALFORMED_SHORT_WIRE, VILLAGE_REF};
use super::game_double::Tail;
use super::{
    bind, capture_subscriber, config, joined, spawn_front, spawn_game_with, Capture, Mutation,
    Transcript, DOUBLE_TIME_BUDGET,
};

/// R1/R2/R3 never wait out a heartbeat: the fault lands right after the
/// in-world set, so the whole session is milliseconds. This bound fails a
/// hang long before the driver's own 60 s budget would.
const FAST_SESSION_BOUND: Duration = Duration::from_secs(15);
/// R4 waits out the first heartbeat (the fixed 10 s ticker,
/// `heartbeat.rs:169`) before the seam fires — the bound budgets that plus
/// teardown plus CI slack.
const CTRL_C_BOUND: Duration = Duration::from_secs(30);

/// Runs one fault session to completion inside its bound and returns the
/// driver's result with the wall-clock duration it took. The doubles'
/// budgets (30 s) bound their own halves.
async fn fault_session(
    tail: Tail,
    bound: Duration,
) -> (
    Result<goonzu_client::client::driver::DriverOutcome, DriverError>,
    Duration,
    super::game_double::GameReport,
    super::front_double::FrontReport,
    String,
) {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        tail,
        DOUBLE_TIME_BUDGET,
    );

    let started = Instant::now();
    let outcome = tokio::time::timeout(
        bound,
        driver::run(&config(front_addr, game_addr), &Shutdown::new()),
    )
    .await
    .expect("the faulted session must terminate inside its bound");
    let elapsed = started.elapsed();

    let game_report = joined(game).await;
    let front_report = joined(front).await;
    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    (outcome, elapsed, game_report, front_report, logs)
}

/// Asserts the shared zero-orphan and bounded-behavior surface every fault
/// path must show: the front leg completed, the client's teardown close
/// was observed by the double, and the Err-path drain warning never fired.
fn assert_clean_aftermath(
    elapsed: Duration,
    bound: Duration,
    front: &super::front_double::FrontReport,
    game: &super::game_double::GameReport,
    logs: &str,
) {
    assert!(
        elapsed <= bound,
        "the session must terminate inside its {bound:?} bound; took {elapsed:?}"
    );
    assert!(
        front.closed_cleanly,
        "the front leg must have completed before the fault (driver.rs:574-576)"
    );
    assert!(
        !front.saw_corruption,
        "no front frame may arrive short or fail integrity"
    );
    assert!(
        game.client_closed,
        "the double must observe the client's own close — the teardown close \
         phase ran (teardown.rs:366) and the session leaves no open link"
    );
    assert!(
        !logs.contains("teardown failed after an earlier failure"),
        "a drain that fails after a refusal must warn (driver.rs:492); it \
         must not warn when the drain succeeds — zero orphaned tasks"
    );
}

/// R1: one falsifiable test, two wire variants — `usSize` 3 (the `2..=4`
/// band this port refuses, BUG-013) and `usSize` 64993 (one past the
/// 64992 receive limit). Both must refuse at `InWorld` with the header
/// guard's exact variant as the source, and terminate cleanly after.
#[tokio::test]
async fn r1_a_malformed_us_size_is_refused_at_in_world_with_the_header_guards_variant() {
    for (label, fault, expect) in [
        (
            "short",
            MALFORMED_SHORT_WIRE.to_vec(),
            "a declared usSize of 3 cannot hold a 5-byte header (BUG-013)",
        ),
        (
            "oversize",
            MALFORMED_OVERSIZE_WIRE.to_vec(),
            "a declared usSize of 64993 exceeds the 64992-byte receive limit",
        ),
    ] {
        let (outcome, elapsed, game, front, logs) =
            fault_session(Tail::RawFault(fault), FAST_SESSION_BOUND).await;

        let err = outcome.expect_err("a malformed mid-stream frame must refuse");
        match &err {
            DriverError::Stage { stage, source } => {
                assert_eq!(
                    *stage,
                    Stage::InWorld,
                    "the refusal must name the stage it stopped at ({label}); got {err}"
                );
                let game_error = source
                    .downcast_ref::<GameError>()
                    .unwrap_or_else(|| panic!("the source chain must carry GameError; got {err}"));
                let GameError::Codec(CodecError::Header(header)) = game_error else {
                    panic!("the source must be Codec(Header(..)); got {game_error:?}");
                };
                match (label, header) {
                    ("short", HeaderError::SizeBelowHeader { declared: 3 }) => {}
                    ("oversize", HeaderError::SizeTooLarge { declared: 64993 }) => {}
                    _ => panic!(
                        "{expect}; got {header:?} ({label} variant) — the \
                         guard's own table decides, protocol/header.rs:110-121"
                    ),
                }
            }
            other => panic!("expected Stage{{InWorld}}, got {other}"),
        }
        assert!(
            err.to_string().contains("InWorld"),
            "the error's display must name the stage ({label}); got {err}"
        );
        assert_clean_aftermath(elapsed, FAST_SESSION_BOUND, &front, &game, &logs);
    }
}

/// R2: one falsifiable test — a valid frame whose last wire byte (the
/// integrity tag's source, `cipher.rs:279,295`) is flipped. The refusal
/// must surface as `CodecError::Integrity` at `InWorld`.
#[tokio::test]
async fn r2_a_flipped_integrity_tag_is_refused_at_in_world() {
    let (outcome, elapsed, game, front, logs) =
        fault_session(Tail::CorruptFault, FAST_SESSION_BOUND).await;

    let err = outcome.expect_err("a tag-flipped frame must refuse");
    match &err {
        DriverError::Stage {
            stage: Stage::InWorld,
            source,
        } => {
            let game_error = source
                .downcast_ref::<GameError>()
                .expect("the source chain must carry GameError");
            assert!(
                matches!(game_error, GameError::Codec(CodecError::Integrity { .. })),
                "the refusal must be the cipher's integrity verdict \
                 (ITZEncrypt.cpp:111); got {game_error:?}"
            );
        }
        other => panic!("expected Stage{{InWorld}}, got {other}"),
    }
    assert!(
        err.to_string().contains("InWorld"),
        "the error's display must name the stage; got {err}"
    );
    assert_clean_aftermath(elapsed, FAST_SESSION_BOUND, &front, &game, &logs);
}

/// R3: one falsifiable test — three wire bytes of a valid frame, then FIN.
/// The **documented** behavior: the partial is discarded at EOF, never
/// delivered, never an error (`game/entry.rs:472-476`), and the session
/// ends as a clean `Cause::ServerDisconnect` teardown. Asserting *no
/// error* is the point: this pins the observed behavior against silent
/// change.
#[tokio::test]
async fn r3_a_partial_frame_then_fin_is_discarded_and_tears_down_cleanly() {
    let (outcome, elapsed, game, front, logs) =
        fault_session(Tail::PartialFault, FAST_SESSION_BOUND).await;

    let outcome = outcome.expect(
        "an abrupt mid-frame disconnect must NOT error — the partial frame is \
         discarded at EOF (game/entry.rs:472-476); got an Err",
    );
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the clean path must end at TornDown (driver.rs:498-503)"
    );
    assert_eq!(
        outcome.char_unique, CHAR_UNIQUE,
        "the outcome must still carry the handshake's char_unique"
    );
    assert_eq!(
        outcome.village,
        Some(VILLAGE_REF),
        "the outcome must still carry the requested village"
    );
    assert!(
        game.heartbeats == 0,
        "the partial frame arrived before any heartbeat — the in-world loop \
         must observe the close, not a heartbeat"
    );
    assert!(
        logs.contains("session ended") && logs.contains("stage=TornDown"),
        "the terminal INFO record must name TornDown; got:\n{logs}"
    );
    assert!(
        logs.contains("cause=Some(ServerDisconnect)"),
        "an unclaimed shutdown at teardown defaults to the server-side cause \
         (teardown.rs:358-362); got:\n{logs}"
    );
    assert_clean_aftermath(elapsed, FAST_SESSION_BOUND, &front, &game, &logs);
}

/// R4: one falsifiable test — Ctrl+C mid-session through the injected
/// watcher seam. The trigger fires only after the first byte-exact
/// heartbeat proves the in-world loop is running, so the claim lands
/// mid-session deterministically (`driver.rs:666` would otherwise race an
/// immediate trigger). The outcome must be `TornDown` with `Cause::CtrlC`
/// claimed by *this* watcher, observed by the double as a clean close.
#[tokio::test]
async fn r4_ctrl_c_mid_session_claims_the_cause_and_tears_down_cleanly() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(capture_subscriber(capture.clone()));
    let transcript = Arc::new(Transcript::new());

    let shutdown = Shutdown::new();
    // The seam (driver.rs:462-468): the same watch_signal the production
    // adapter spawns, over a future this test completes itself — a real
    // console event is undeliverable under ConPTY
    // (evidence/live-fix/ctrl_c_investigation.md).
    let seam = Arc::new(Notify::new());
    let watcher = tokio::spawn(watch_signal(shutdown.clone(), {
        let seam = Arc::clone(&seam);
        move || {
            let seam = Arc::clone(&seam);
            async move {
                seam.notified().await;
                Ok(())
            }
        }
    }));

    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        Tail::CtrlCSeam(Arc::clone(&seam)),
        DOUBLE_TIME_BUDGET,
    );

    let started = Instant::now();
    let outcome = tokio::time::timeout(
        CTRL_C_BOUND,
        driver::run(&config(front_addr, game_addr), &shutdown),
    )
    .await
    .expect("the Ctrl+C session must terminate inside its bound");
    let elapsed = started.elapsed();

    let outcome = outcome.expect("a Ctrl+C teardown must complete cleanly");
    assert_eq!(
        outcome.stage,
        Stage::TornDown,
        "the Ctrl+C path must end at TornDown (driver.rs:498-503)"
    );

    // The watcher claimed the flag: Some(Cause::CtrlC) is returned only
    // when *this* call won the claim (teardown.rs:302-306) — and the test
    // joined its own watcher, so it leaves no task behind either.
    let cause = watcher
        .await
        .expect("the injected watcher task must not panic");
    assert_eq!(
        cause,
        Some(Cause::CtrlC),
        "the injected watcher must have claimed the shutdown as Ctrl+C"
    );

    let game_report = joined(game).await;
    assert!(
        game_report.heartbeats >= 1,
        "the seam fired on the first byte-exact heartbeat, so the double \
         must have observed it"
    );
    assert!(
        game_report.client_closed,
        "the double must observe the client's own close after the Ctrl+C \
         teardown (teardown.rs:366)"
    );
    let front_report = joined(front).await;

    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    assert!(
        logs.contains("session ended")
            && logs.contains("stage=TornDown")
            && logs.contains("cause=Some(CtrlC)"),
        "the terminal INFO record must carry Cause::CtrlC from the injected \
         watcher; got:\n{logs}"
    );
    assert_clean_aftermath(elapsed, CTRL_C_BOUND, &front_report, &game_report, &logs);
}
