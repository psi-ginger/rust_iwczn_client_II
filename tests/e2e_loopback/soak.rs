// Step 6.3 — the scaled-cadence hermetic soak, as a gate (WO-6.3 §6(a)).
//
// Two full `driver::run` sessions run **concurrently** (one current-thread
// task, `tokio::join!` — the driver future is not `Send`, the production
// posture, `src/main.rs:30`), each held in-world while its game double
// streams a dispatcher-frame storm of a different size: 1000 vs 4000
// frames. The heartbeat ticker is fixed at 10 s (`heartbeat.rs:169`, a
// documented port decision) so the *frame stream* is the scaled dimension
// (WO-6.3 §3); storm-size independence is then a falsifiable claim — the
// two sessions' heartbeat cadences must stay within scheduling distance of
// each other and inside the fixed interval, whatever the stream size.
//
// The storm mix is the NPC-led opcode-6 batch (34 x 68-byte NPC records
// then the two PCs — LIVE-01's broadcast shape,
// `docs/rca/2026-10-28-live-opcode6-npc-stride.md`; a PC-only storm is the
// forbidden regression) plus every other opcode the dispatcher owns and one
// foreign opcode (`dispatcher.rs:888-918`: `Unhandled` is not an error).
//
// Zero-orphan evidence is behavioral, not introspective (`src/**` is
// read-only and exposes no task table): the driver returning `Ok` at all
// implies the watcher was aborted and awaited (`driver.rs:485-486`) and the
// tracker drained (an Ok-path drain failure surfaces as
// `Err(DriverError::at(Stage::TornDown, ..))`, `driver.rs:494`); the double
// observing the client's EOF proves `terminate`'s close phase ran
// (`teardown.rs:366`); and the capture must never carry the Err-path drain
// warning (`driver.rs:492`).
//
// The DEBUG capture counts the driver's per-dispatch records
// (`driver.rs:684`) so the assertions cover what the *client processed*,
// not what the doubles wrote. See the root's `debug_capture_subscriber`
// for why the callsite interest cache cannot hide those records here.
use std::sync::Arc;
use std::time::{Duration, Instant};

use goonzu_client::client::driver::{DriverError, DriverOutcome, Stage};

use super::frames::{CHAR_UNIQUE, VILLAGE_REF};
use super::game_double::Tail;
use super::{
    bind, config, debug_capture_subscriber, joined, run_driver, spawn_front, spawn_game_with,
    Capture, Mutation, Transcript,
};

/// The storm budget for either double: the storm is heartbeat-bound (three
/// 10 s ticker periods after entry, `heartbeat.rs:169`), so ~35 s covers
/// entry + storm + teardown with slack; the budget only has to outlive a
/// hang so a stuck double fails the test rather than the gate run.
const SOAK_DOUBLE_TIME_BUDGET: Duration = Duration::from_secs(90);
/// The small storm's frame target (WO-6.3 §6(a): >= 1000 dispatcher frames
/// streamed; this is the measured floor per session, the actual sent count
/// is the heartbeat-bound one and is asserted).
const SMALL_STORM_TARGET: usize = 1000;
/// The large storm's frame target — 4x the small one, the independence
/// lever.
const LARGE_STORM_TARGET: usize = 4000;
/// The observed heartbeat interval must sit inside this window: the ticker
/// fires at exactly 10 s; the loopback observation delay is sub-millisecond
/// and the window only budgets scheduler noise on a loaded gate machine.
const CADENCE_MIN: Duration = Duration::from_millis(9_500);
const CADENCE_MAX: Duration = Duration::from_millis(11_000);
/// Two concurrent sessions started within milliseconds of each other must
/// observe their k-th heartbeats this close together, storm size aside.
const PAIRWISE_MAX_DRIFT: Duration = Duration::from_secs(1);
/// FIN (double) → driver return: the in-world loop observes EOF on its next
/// read, teardown closes and drains. 500 ms budgets the drain poll
/// (`DRAIN_POLL_INTERVAL` 1 ms, an empty tracker) with CI slack.
const TEARDOWN_MAX_LATENCY: Duration = Duration::from_millis(500);

/// One soak session's measured results, for the cross-session assertions
/// and the printed evidence.
struct Session {
    /// The driver's result.
    outcome: Result<DriverOutcome, DriverError>,
    /// When `driver::run` returned.
    returned: Instant,
    /// What the game double observed.
    report: super::game_double::GameReport,
    /// What the front double observed.
    front: super::front_double::FrontReport,
}

/// One full session: binds, spawns both doubles with the storm tail, runs
/// the driver inline (production posture) and joins everything.
async fn storm_session(target: usize) -> Session {
    let transcript = Arc::new(Transcript::new());
    let (front_listener, front_addr) = bind().await;
    let (game_listener, game_addr) = bind().await;
    let front = spawn_front(front_listener, Mutation::None, Arc::clone(&transcript));
    let game = spawn_game_with(
        game_listener,
        Mutation::None,
        Arc::clone(&transcript),
        Tail::Storm {
            target_frames: target,
        },
        SOAK_DOUBLE_TIME_BUDGET,
    );

    // The driver's own DRIVER_TIME_BUDGET bounds this run; `returned` is
    // the timestamp every teardown-latency measurement below uses.
    let outcome = run_driver(config(front_addr, game_addr)).await;
    let returned = Instant::now();

    Session {
        outcome,
        returned,
        report: joined(game).await,
        front: joined(front).await,
    }
}

/// The soak gate: two concurrent in-world sessions, storm streams of
/// different sizes, fixed heartbeat cadence, clean teardown, zero orphans
/// (WO-6.3 §6(a), acceptance items 1-4).
#[tokio::test]
async fn two_concurrent_in_world_sessions_hold_their_cadence_through_scaled_storms() {
    let capture = Capture::default();
    let _guard = tracing::subscriber::set_default(debug_capture_subscriber(capture.clone()));

    // Both sessions run on this one task: the driver futures interleave at
    // every await, which is as concurrent as the production posture allows
    // and keeps the two heartbeat clocks trivially comparable.
    let (small, large) = tokio::join!(
        storm_session(SMALL_STORM_TARGET),
        storm_session(LARGE_STORM_TARGET)
    );

    // -- Per-session outcomes: both must complete cleanly at TornDown. ----
    for (label, session) in [("small", &small), ("large", &large)] {
        let outcome = session
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("the {label} storm session must complete: {error}"));
        assert_eq!(
            outcome.stage,
            Stage::TornDown,
            "the {label} session must end at TornDown (driver.rs:498-503)"
        );
        assert_eq!(
            outcome.worlds, 1,
            "the {label} world list carried one entry"
        );
        assert_eq!(
            outcome.char_unique, CHAR_UNIQUE,
            "the {label} outcome must still carry the handshake's char_unique \
             after the storm"
        );
        assert_eq!(
            outcome.village,
            Some(VILLAGE_REF),
            "the {label} outcome must still carry the requested village"
        );
    }

    // -- The doubles observed clean conversations. ------------------------
    for (label, session) in [("small", &small), ("large", &large)] {
        assert_eq!(
            session.front.frames_received, 3,
            "the {label} front leg must have seen exactly notice, world list \
             and login (driver.rs:541-568)"
        );
        assert!(
            session.front.closed_cleanly,
            "the {label} front link must close after the login response \
             (driver.rs:574-576)"
        );
        assert!(
            !session.front.saw_corruption,
            "no {label} front frame may arrive short or fail integrity"
        );
        assert!(
            !session.report.closed_early,
            "the {label} client must still be in-world when the storm runs"
        );
        assert!(
            !session.report.saw_corruption,
            "no {label} game frame may arrive short or fail integrity"
        );
        assert!(
            session.report.client_closed,
            "the {label} double must observe the client's own close after \
             its FIN — the teardown close phase ran (teardown.rs:366)"
        );
    }

    // -- The storms streamed their targets, and the client dispatched. ---
    assert!(
        small.report.storm_frames >= SMALL_STORM_TARGET,
        "the small storm must write at least its {}-frame target, wrote {}",
        SMALL_STORM_TARGET,
        small.report.storm_frames
    );
    assert!(
        large.report.storm_frames >= LARGE_STORM_TARGET,
        "the large storm must write at least its {}-frame target, wrote {}",
        LARGE_STORM_TARGET,
        large.report.storm_frames
    );

    let logs = {
        let guard = capture.0.lock().expect("capture lock");
        String::from_utf8_lossy(&guard).into_owned()
    };
    let dispatched = logs
        .lines()
        .filter(|line| line.contains("in-world frame dispatched"))
        .count();
    assert!(
        dispatched >= SMALL_STORM_TARGET + LARGE_STORM_TARGET,
        "the client must dispatch every streamed frame — the dispatcher \
         records at driver.rs:684 prove processing, not just writing; \
         expected >= {} dispatches, counted {dispatched} in:\n{logs}",
        SMALL_STORM_TARGET + LARGE_STORM_TARGET
    );
    let npc_led = logs
        .lines()
        .filter(|line| line.contains("in-world frame dispatched") && line.contains("npcs: 34"))
        .count();
    // Every 6-frame cycle carries exactly one NPC-led batch (34 x 68 NPC
    // records — LIVE-01's shape), so >= frames/6 - 2 batches must have been
    // dispatched: the PC-only-storm regression cannot pass this.
    let min_batches = (small.report.storm_frames + large.report.storm_frames) / 6 - 2;
    assert!(
        npc_led >= min_batches,
        "the NPC-led opcode-6 batches (34 x 68, LIVE-01) must be dispatched \
         at scale — expected >= {min_batches} batches, counted {npc_led} in:\n{logs}"
    );
    assert_eq!(
        logs.lines()
            .filter(|line| line.contains("driver: session ended"))
            .count(),
        2,
        "both sessions must emit their terminal INFO record \
         (driver.rs:510-520); the in-world record at driver.rs:699 is a \
         different line and must not be counted here"
    );
    assert_eq!(
        logs.lines()
            .filter(|line| line.contains("driver: in-world session ended"))
            .count(),
        2,
        "both in-world loops must end through the loop's own exit path \
         (driver.rs:699)"
    );
    assert!(
        !logs.contains("teardown failed after an earlier failure"),
        "no session may hit the Err-path drain warning (driver.rs:492) — a \
         drain that fails on the Ok path is an Err outcome, already asserted"
    );

    // -- Heartbeat cadence: >= 3 byte-exact observations per session. -----
    // Byte-exactness itself is asserted inside the double's reader
    // (`assert_heartbeat`); here the cadence and the independence.
    for (label, session) in [("small", &small), ("large", &large)] {
        assert!(
            session.report.heartbeats >= 3,
            "the {label} session must observe at least three heartbeats to \
             make cadence falsifiable, observed {}",
            session.report.heartbeats
        );
        for (index, delta) in session
            .report
            .heartbeat_times
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .enumerate()
        {
            assert!(
                delta >= CADENCE_MIN && delta <= CADENCE_MAX,
                "the {label} heartbeat interval #{index} must stay on the \
                 fixed 10 s ticker (heartbeat.rs:169); measured {delta:?}"
            );
        }
    }

    // -- Storm-size independence, measured. ------------------------------
    let paired = small
        .report
        .heartbeat_times
        .iter()
        .zip(large.report.heartbeat_times.iter())
        .enumerate()
        .map(|(index, (a, b))| {
            let drift = (*a).max(*b).duration_since((*a).min(*b));
            (index, drift)
        })
        .collect::<Vec<_>>();
    for (index, drift) in &paired {
        assert!(
            *drift <= PAIRWISE_MAX_DRIFT,
            "the {index}-th heartbeat must land within {PAIRWISE_MAX_DRIFT:?} \
             in both sessions regardless of storm size (1000 vs 4000 frames); \
             measured {drift:?}"
        );
    }
    for (label, session) in [("small", &small), ("large", &large)] {
        let latency = teardown_latency(
            session
                .report
                .fin_instant
                .expect("the storm double must record its FIN"),
            session.returned,
        );
        assert!(
            latency <= TEARDOWN_MAX_LATENCY,
            "the {label} session's FIN-to-return teardown must stay bounded; \
             measured {latency:?}"
        );
    }

    // -- The measured numbers, for the delivery note (WO-6.3 §9). --------
    let drift_max = paired.iter().map(|(_, drift)| *drift).max();
    println!(
        "soak measurements: small storm target={SMALL_STORM_TARGET} sent={} \
         heartbeats={} interval={:?}; large storm target={LARGE_STORM_TARGET} \
         sent={} heartbeats={} interval={:?}; dispatches={dispatched} \
         npc_led_batches={npc_led}; max cross-session heartbeat drift \
        ={drift_max:?}; teardown latency small={:?} large={:?}",
        small.report.storm_frames,
        small.report.heartbeats,
        interval_summary(&small.report.heartbeat_times),
        large.report.storm_frames,
        large.report.heartbeats,
        interval_summary(&large.report.heartbeat_times),
        teardown_latency(small.report.fin_instant.expect("small FIN"), small.returned),
        teardown_latency(large.report.fin_instant.expect("large FIN"), large.returned),
    );
}

/// The FIN-to-return teardown latency: the double's FIN timestamp to the
/// driver's return, saturating (the FIN always precedes the return).
fn teardown_latency(fin: Instant, returned: Instant) -> Duration {
    returned.saturating_duration_since(fin)
}

/// The min/max of one session's observed heartbeat intervals — printed
/// evidence, asserted above.
fn interval_summary(times: &[Instant]) -> (Duration, Duration) {
    let mut intervals = times
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    let min = *intervals.first().expect("at least two heartbeats");
    let max = *intervals.last().expect("at least two heartbeats");
    (min, max)
}
