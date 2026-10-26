// Step 5.3 tests — falsifiable, loopback, virtual-time, no wall-clock flake.
//
// F-01: a real `Ctrl+C` is not hermetically deliverable on Windows
// (`GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` would signal cargo and the
// harness itself), so the signal path is driven through `watch_signal`'s
// injected-future seam; `spawn_ctrl_c_watcher` — the one-line
// `tokio::signal::ctrl_c` adapter — is exercised for its task accounting and
// documented as manually verified in the module docs. Everything else runs
// on loopback sockets this file's own peers open and close, and on tokio's
// virtual clock (`start_paused`), so a regression fails an assertion instead
// of hanging: every await that could block is wrapped in
// `tokio::time::timeout`.
//
// F-07: expectations never come from the unit under test — shutdown causes
// are checked against literals this file chose, stop latency is measured
// with `Instant::now()` around the call (not reported by the loop), and the
// peer's EOF is read by the test itself off the socket it accepted.
//
// Paused clock vs. kernel delivery (observed under parallel load): tokio's
// auto-advance fires the earliest timer the moment the runtime parks, which
// beats the kernel's own latency for a loopback handshake — surfacing as a
// spurious `ConnectTimeout`, or (via `join!` with an accept that never gets
// a connection) a permanent park. The two places where real OS delivery is
// load-bearing therefore differ from a naive paused test: dials run with the
// clock resumed (`dial`, `held_link`), and the peer's EOF is read as a
// bounded chain of idle-cycle polls (`read_eof`) exactly as the event loop
// tolerates an idle read window — both documented at their definitions.
use super::*;
use crate::client::event_loop::{
    run_with_shutdown, Flow, LoopError, LoopEvent, Ticker, READ_POLL_BUDGET,
};
use crate::game::entry::{GameError, GameLink};
use std::convert::Infallible;
use std::sync::atomic::AtomicBool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// Dials `addr` and returns the link — over the **real** clock, then back to
/// virtual (see [`held_link`] for why the dial cannot run paused).
///
/// # Panics
///
/// On a failed connect, or if the caller's runtime clock is not paused.
async fn dial(addr: std::net::SocketAddr) -> GameLink {
    tokio::time::resume();
    let link = GameLink::connect(addr).await;
    tokio::time::pause();
    link.expect("connect")
}

/// An open `GameLink` plus the peer socket on the other end, so a test can
/// both drive the link and observe what the "server" sees.
///
/// The handshake runs with the clock **resumed**: under
/// `#[tokio::test(start_paused = true)]` tokio auto-advances to the earliest
/// timer the moment the runtime parks, so a loopback dial that has not yet
/// been completed by the kernel loses that race — the 5 s connect window
/// fires (`ConnectTimeout`), and in a `join!` with `accept()` the accept
/// branch then waits forever with no timer left, i.e. a permanent park. This
/// was observed under parallel load, which widens the kernel-to-runtime
/// window. Established peers in `event_loop/tests.rs` avoid the race by
/// writing behind virtual sleeps; a dial has no such lever — the kernel is
/// not on tokio's clock — so the dial runs real and everything under test
/// runs virtual.
async fn held_link() -> (GameLink, TcpStream) {
    let (listener, addr) = local_listener().await;
    tokio::time::resume();
    let (link, peer) = tokio::join!(GameLink::connect(addr), listener.accept());
    tokio::time::pause();
    (link.expect("connect"), peer.expect("accept").0)
}

/// Reads from `peer` until EOF, tolerating idle cycles the way the event
/// loop tolerates them: each 100 ms window that answers nothing is polled
/// again rather than failing the test.
///
/// One timer-wrapped `read` would race tokio's paused-clock auto-advance
/// against kernel EOF delivery (same class as the dial race above) and turn
/// a correct close into a spurious `Elapsed`. A bounded number of cycles
/// keeps the failure falsifiable: a close that never happens panics after 32
/// instant virtual polls instead of hanging.
///
/// Returns the byte count read before EOF — a clean shutdown must be **zero**
/// (bytes would mean the peer saw payload, not a bare FIN).
async fn read_eof(mut peer: TcpStream) -> usize {
    let mut total = 0usize;
    let mut buf = [0u8; 64];
    for _ in 0..32 {
        match tokio::time::timeout(Duration::from_millis(100), peer.read(&mut buf)).await {
            Ok(Ok(0)) => return total,
            Ok(Ok(n)) => total += n,
            Ok(Err(err)) => panic!("the peer read must be a clean EOF, not an I/O error: {err}"),
            // An idle cycle: the EOF event had not reached the driver yet.
            Err(_elapsed) => {}
        }
    }
    panic!("the peer never observed EOF within 32 read windows (saw {total} byte(s))");
}

/// A complete plaintext frame: `[usSize LE][ucChk][usCmd LE][payload]` — the
/// pre-cipher image a misbehaving peer would write, which the link cipher
/// must refuse.
fn plaintext_frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

// ---------------------------------------------------------------------------
// The flag: one claim, first cause wins.
// ---------------------------------------------------------------------------

#[test]
fn first_trigger_wins_and_the_later_cause_is_dropped() {
    let shutdown = Shutdown::new();
    assert!(!shutdown.is_triggered(), "a fresh shutdown is untriggered");
    assert_eq!(
        shutdown.cause(),
        None,
        "no cause is recorded before a trigger"
    );

    assert!(
        shutdown.trigger(Cause::CtrlC),
        "the first claim must win the compare-exchange"
    );
    assert!(
        !shutdown.trigger(Cause::ServerDisconnect),
        "a second claim must lose — one shutdown, one cause"
    );
    assert_eq!(
        shutdown.cause(),
        Some(Cause::CtrlC),
        "the losing trigger must not overwrite the winner"
    );

    // The mirror order: the server cause survives a late Ctrl+C.
    let other = Shutdown::new();
    assert!(other.trigger(Cause::ServerDisconnect), "first claim wins");
    assert!(!other.trigger(Cause::CtrlC), "late Ctrl+C loses");
    assert_eq!(other.cause(), Some(Cause::ServerDisconnect));
}

// ---------------------------------------------------------------------------
// The signal seam: injected futures, honest failures.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn watch_signal_claims_ctrl_c_through_the_injected_seam() {
    let shutdown = Shutdown::new();
    let cause = watch_signal(shutdown.clone(), || async { Ok::<(), _>(()) }).await;

    assert_eq!(cause, Some(Cause::CtrlC));
    assert!(shutdown.is_triggered());
    assert_eq!(shutdown.cause(), Some(Cause::CtrlC));
}

#[tokio::test]
async fn a_failed_signal_registration_never_falsely_claims_a_cause() {
    let shutdown = Shutdown::new();
    let cause = watch_signal(shutdown.clone(), || async {
        Err::<(), _>(std::io::Error::other("no console attached"))
    })
    .await;

    assert_eq!(
        cause, None,
        "a registration failure must not masquerade as a delivered Ctrl+C"
    );
    assert!(
        !shutdown.is_triggered(),
        "nothing may be claimed without a signal"
    );
    assert_eq!(shutdown.cause(), None);
}

#[tokio::test]
async fn watch_signal_yields_to_a_cause_already_claimed() {
    let shutdown = Shutdown::new();
    assert!(
        shutdown.trigger(Cause::ServerDisconnect),
        "server dropped first"
    );

    let cause = watch_signal(shutdown.clone(), || async { Ok::<(), _>(()) }).await;

    assert_eq!(
        cause, None,
        "a Ctrl+C arriving after the shutdown is already claimed must not win"
    );
    assert_eq!(shutdown.cause(), Some(Cause::ServerDisconnect));
}

#[tokio::test]
async fn spawn_ctrl_c_watcher_is_accounted_and_abortable() {
    let tracker = TaskTracker::new();
    let watcher = spawn_ctrl_c_watcher(Shutdown::new(), &tracker);

    assert_eq!(
        tracker.live(),
        1,
        "the Ctrl+C watcher must be counted — an uncounted task is exactly \
         the orphan this step forbids"
    );

    watcher.abort();
    tokio::time::timeout(
        Duration::from_secs(1),
        tracker.drain(Duration::from_millis(500)),
    )
    .await
    .expect("drain must poll, not hang")
    .expect("an aborted watcher must be released by its guard, not orphaned");
    assert_eq!(tracker.live(), 0, "abort must return the count to zero");
}

// ---------------------------------------------------------------------------
// The loop honours the flag, and records why it stopped.
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn triggered_shutdown_stops_the_loop_within_one_read_budget() {
    let (mut link, _peer) = held_link().await;
    let shutdown = Shutdown::new();

    // A Ctrl+C that lands 10 ms in, i.e. while the loop is already waiting
    // on the socket. The spawned task runs the moment the loop yields and
    // stamps the virtual instant it claims the flag: latency is then
    // measured from *that* stamp to the loop's return. Anchoring on the
    // loop's own start instant instead would shave tokio's ~1 ms of virtual
    // clock granularity off a budget the loop never observed (F-07: the
    // clock is read outside the unit under test either way).
    let triggered_at = Arc::new(std::sync::Mutex::new(None));
    let late = shutdown.clone();
    let stamp = Arc::clone(&triggered_at);
    let _trigger = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(
            late.trigger(Cause::CtrlC),
            "the trigger must win uncontested"
        );
        *stamp.lock().expect("stamp lock") = Some(Instant::now());
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_with_shutdown(
            &mut link,
            &[],
            &shutdown,
            async |_link, _ev| -> Result<Flow, Infallible> { Ok(Flow::Continue) },
        ),
    )
    .await;
    let returned_at = Instant::now();

    assert!(
        outcome.is_ok(),
        "the loop must stop on the flag, not run to the wrapper's bound"
    );
    outcome
        .unwrap()
        .expect("a shutdown stop is Ok, like a clean EOF");
    assert_eq!(shutdown.cause(), Some(Cause::CtrlC));

    let triggered_at = triggered_at
        .lock()
        .expect("stamp lock")
        .expect("the loop must not return before the trigger it waited for");
    let latency = returned_at - triggered_at;
    assert!(
        latency <= READ_POLL_BUDGET,
        "stop latency from the request to loop exit must stay within one \
         read budget ({READ_POLL_BUDGET:?}), took {latency:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn server_eof_marks_the_server_disconnect_cause_and_stops_the_loop() {
    let (listener, addr) = local_listener().await;
    // The peer accepts and immediately drops: FIN, no bytes — the server
    // side hanging up mid-session.
    let _peer = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
    });

    let shutdown = Shutdown::new();
    let mut link = dial(addr).await;
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_with_shutdown(
            &mut link,
            &[],
            &shutdown,
            async |_link, _ev| -> Result<Flow, Infallible> { Ok(Flow::Continue) },
        ),
    )
    .await
    .expect("the loop must end within the wrapper bound");

    outcome.expect("a clean EOF ends the loop Ok");
    assert_eq!(
        shutdown.cause(),
        Some(Cause::ServerDisconnect),
        "a detected disconnect must be recorded for teardown reporting — \
         the legacy notices the dead session at Client-Winsock.cpp:834-844"
    );
}

#[tokio::test(start_paused = true)]
async fn a_failing_link_claims_server_disconnect_before_returning_the_error() {
    let (listener, addr) = local_listener().await;
    let _peer = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Valid framing, wrong crypto: the inbound link cipher must refuse
        // it (integrity failure) — the loop's error path, not EOF.
        stream
            .write_all(&plaintext_frame(1, &[0x2A, 0x00, 0x00, 0x00]))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let shutdown = Shutdown::new();
    let mut link = dial(addr).await;
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_with_shutdown(
            &mut link,
            &[],
            &shutdown,
            async |_link, _ev| -> Result<Flow, Infallible> { Ok(Flow::Continue) },
        ),
    )
    .await
    .expect("the loop must end within the wrapper bound");

    match outcome {
        Err(LoopError::Link(GameError::Codec(_))) => {}
        other => panic!("expected a decode refusal as LoopError::Link, got {other:?}"),
    }
    assert_eq!(
        shutdown.cause(),
        Some(Cause::ServerDisconnect),
        "a failed link must record the disconnect cause before propagating"
    );
}

#[tokio::test(start_paused = true)]
async fn a_handler_break_leaves_the_cause_unclaimed() {
    let (listener, addr) = local_listener().await;
    let _holder = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let shutdown = Shutdown::new();
    let mut link = dial(addr).await;
    let tickers = [Ticker::new("quick", Duration::from_millis(5))];
    let mut fired = 0u64;
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_with_shutdown(
            &mut link,
            &tickers,
            &shutdown,
            async |_link, ev| -> Result<Flow, Infallible> {
                if let LoopEvent::Ticked { fired: n, .. } = ev {
                    fired = n;
                    return Ok(Flow::Break);
                }
                Ok(Flow::Continue)
            },
        ),
    )
    .await
    .expect("the loop must end within the wrapper bound");

    outcome.expect("Flow::Break ends the loop Ok");
    assert_eq!(fired, 1, "the first ticker fire is what breaks the loop");
    assert_eq!(
        shutdown.cause(),
        None,
        "Flow::Break is routine control flow, not a disconnect: only EOF, a \
         failed link, or terminate() itself may claim a cause"
    );
}

// ---------------------------------------------------------------------------
// Socket close: the peer must see a clean EOF.
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn close_delivers_a_clean_eof_to_the_peer() {
    let (mut link, peer) = held_link().await;

    link.close().await.expect("close must succeed");

    let read = read_eof(peer).await;
    assert_eq!(
        read, 0,
        "the server must observe EOF (FIN) from the write-side shutdown, \
         not residual bytes and not a reset"
    );
}

// ---------------------------------------------------------------------------
// Drain: orphans are reported, never absorbed.
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn drain_reports_orphans_that_never_finish() {
    let tracker = TaskTracker::new();
    let stray = tracker.spawn(async { std::future::pending::<()>().await });
    assert_eq!(tracker.live(), 1, "the stray is counted as live");

    let err = tokio::time::timeout(
        Duration::from_secs(5),
        tracker.drain(Duration::from_millis(10)),
    )
    .await
    .expect("drain must answer within its own bound, never hang")
    .expect_err("a task that never finishes must NOT be reported as drained");
    assert!(
        matches!(
            err,
            DrainError::Timeout { live: 1, bound } if bound == Duration::from_millis(10)
        ),
        "the error must carry the live count and the caller's bound, got {err:?}"
    );

    // The stray is a fixture: release it and prove the count returns to zero.
    stray.abort();
    tokio::time::timeout(
        Duration::from_secs(5),
        tracker.drain(Duration::from_secs(1)),
    )
    .await
    .expect("drain must poll, not hang")
    .expect("an aborted task's guard must run");
    assert_eq!(tracker.live(), 0);
}

// ---------------------------------------------------------------------------
// Headline: the full terminate() — close, drain, record.
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn terminate_closes_the_socket_and_drains_every_tracked_task() {
    let (mut link, peer) = held_link().await;
    let shutdown = Shutdown::new();
    let tracker = TaskTracker::new();

    // Two background tasks of the shape later steps will spawn: they run
    // until the shared flag says stop, then stamp their own completion —
    // so the drain assertion proves the *stop path ran*, not just that the
    // counters hit zero.
    let done_a = Arc::new(AtomicBool::new(false));
    let stop_a = shutdown.clone();
    let stamp_a = Arc::clone(&done_a);
    let _task_a = tracker.spawn(async move {
        while !stop_a.is_triggered() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        stamp_a.store(true, Ordering::SeqCst);
    });

    let done_b = Arc::new(AtomicBool::new(false));
    let stop_b = shutdown.clone();
    let stamp_b = Arc::clone(&done_b);
    let _task_b = tracker.spawn(async move {
        while !stop_b.is_triggered() {
            tokio::time::sleep(Duration::from_millis(7)).await;
        }
        stamp_b.store(true, Ordering::SeqCst);
    });
    assert_eq!(tracker.live(), 2, "both tasks are tracked");

    // Ctrl+C observed before teardown: its claim must survive terminate().
    assert!(shutdown.trigger(Cause::CtrlC), "Ctrl+C claims first");

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        terminate(&mut link, &shutdown, &tracker, Duration::from_secs(1)),
    )
    .await
    .expect("terminate must finish within its wrapper bound");

    outcome.expect("clean close plus a full drain is Ok");
    assert_eq!(
        shutdown.cause(),
        Some(Cause::CtrlC),
        "terminate must never overwrite an already-claimed cause"
    );
    assert_eq!(
        tracker.live(),
        0,
        "zero orphaned background tasks — the step's acceptance criterion"
    );
    assert!(
        done_a.load(Ordering::SeqCst) && done_b.load(Ordering::SeqCst),
        "every task must have executed its stop path before drain returned"
    );

    let eof = read_eof(peer).await;
    assert_eq!(
        eof, 0,
        "the server must see the FIN from Session::close during teardown"
    );
}

#[tokio::test(start_paused = true)]
async fn terminate_defaults_an_unclaimed_shutdown_to_server_disconnect() {
    let (mut link, peer) = held_link().await;
    let shutdown = Shutdown::new();
    let tracker = TaskTracker::new();

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        terminate(&mut link, &shutdown, &tracker, Duration::from_millis(500)),
    )
    .await
    .expect("terminate must finish within its wrapper bound");

    outcome.expect("clean close plus an empty drain is Ok");
    assert_eq!(
        shutdown.cause(),
        Some(Cause::ServerDisconnect),
        "teardown with no cause in flight means the link is gone — the \
         default must be ServerDisconnect"
    );
    assert_eq!(tracker.live(), 0);

    let eof = read_eof(peer).await;
    assert_eq!(eof, 0, "the socket is closed even on the defaulted path");
}
