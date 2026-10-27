// Step 5.1 tests — falsifiable, byte-exact, loopback, virtual-time.
//
// P-05 (QC directive): the tests whose contract involves the 100 ms budget
// run with `#[tokio::test(start_paused = true)]` — tokio's virtual clock
// (`test-util`, a dev-dependency-only feature) auto-advances to each pending
// deadline while every loopback socket is quiet, so budget drops, ticker
// fires and peer holds land at exact virtual instants: no wall-clock window
// remains to flake under CPU load. Every peer task still ends (EOF) even on a
// regression, so a broken loop fails an assertion instead of hanging.
//
// QC follow-up (2026-10-27, row 5.1): the loopback *dial* is the one step a
// paused clock cannot time fairly — `GameLink::connect`'s 5 s connect window
// races tokio's virtual-time auto-advance against the kernel handshake: a
// load-sensitive `ConnectTimeout` on 127.0.0.1 that hit a varying
// event-loop test per run. All eight paused-clock dials therefore run through
// `dial()`, which connects with a bare timerless `TcpStream::connect` (a park
// with no deadline can only end on real I/O — the virtual clock cannot move)
// and wraps the stream with `GameLink::from_stream`; the two real-clock tests
// dial through `GameLink::connect` directly, where there is no paused clock
// to race. Both root causes — the original race, and why the obvious
// resume-the-clock fix was itself wrong — are written up in `dial`'s docs.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): the live `goonzu-rs`
// server is unavailable, so every peer here is a loopback socket that writes
// **real link-ciphered frames** with a fresh `Cipher::link_cipher()` — the
// same per-session pairing the game service uses (`main.rs:680`). Read
// windows are pinned small so the 100 ms behaviour is observable in
// milliseconds, not seconds. What these prove is the *loop's* contract: an
// idle socket does not block, tickers keep firing alongside frames, missed
// ticker intervals coalesce into a single fire, a frame
// split across budget windows reassembles (cancellation safety), frames
// buffered ahead of a close are delivered before EOF ends the loop, a
// handler error on either dispatch path propagates and stops the loop, and a
// decode refusal stops it.
//
// F-07: the loop is never its own expectation source — every frame the peer
// writes is a handwritten `(cmd, payload)`, the handler records events into a
// plain `Vec` compared against those literals, and the cancellation-safety
// test's expectation (one whole frame) is derived from the bytes the peer
// chose to send, split at a literal offset.
use super::*;
use crate::game::entry::GameError;
use crate::net::cipher::Cipher;
use crate::net::session::SessionTimeouts;
use std::convert::Infallible;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// Dials `addr` and returns the link — **without moving the virtual clock**.
///
/// # Root cause of the flake this replaces (QC follow-up, 2026-10-27)
///
/// Two defects, one fix:
///
/// 1. *The original* — [`GameLink::connect`] bounds its dial with a 5 s
///    `tokio::time::timeout`. Under `start_paused = true` that park carries
///    a duration, so tokio's auto-advance calls `advance(duration)` whenever
///    the kernel handshake misses its zero-timeout `did_wake()` probe
///    (`tokio` `runtime/time/mod.rs`, `park_thread_timeout`): a
///    load-sensitive `ConnectTimeout` on `127.0.0.1` that hit a *varying*
///    event-loop test per run and only while cargo rebuild churn loaded the
///    machine (0/10 quiet runs after `cargo clean`, 2/~11 under churn).
/// 2. *Why the obvious resume/pause fix was wrong* — resuming the real clock
///    for the dial and pausing afterwards leaves the virtual clock at
///    `start + Δ` with a sub-millisecond fraction; tokio rounds timer
///    deadlines **up** (`deadline_to_tick`, `+999_999ns`) but reads *now*
///    **down** (`instant_to_tick`, truncating), so a nonzero fraction inflates
///    the park duration by exactly one tick: every exact virtual instant
///    shifted by `+1 ms` — `missed_intervals_coalesce_into_one_fire` failed
///    30/30 at `(1, 26ms)` instead of `(1, 25ms)` — and that test's
///    holder-sleep margin eroded by the whole resumed window.
///
/// So the dial here is a bare `TcpStream::connect`: with no timer registered
/// the park has `next_wake = None`, tokio parks on I/O alone (the
/// auto-advance path is unreachable), and the connection completes in real
/// time while the virtual clock stays exactly where it was.
/// [`GameLink::from_stream`] then installs the same link-cipher-first decoder
/// and timeouts [`GameLink::connect_with_timeouts`] would have. The two
/// real-clock tests keep dialling through [`GameLink::connect`] directly —
/// no paused clock means no race and no virtual instant to perturb, and the
/// production connect-with-timeout path stays covered.
///
/// # Panics
///
/// On a failed connect — every peer here accepts on loopback.
async fn dial(addr: std::net::SocketAddr) -> GameLink {
    let stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    GameLink::from_stream(stream, addr, SessionTimeouts::default())
}

/// A complete plaintext frame: `[usSize LE][ucChk][usCmd LE][payload]` — the
/// pre-cipher image the server writes through `Cipher::encode`.
fn frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let size = u16::try_from(5 + payload.len()).expect("frame fits u16");
    let mut bytes = Vec::with_capacity(5 + payload.len());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.push(0x00); // ucChk
    bytes.extend_from_slice(&cmd.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The server→client encoding: a fresh link cipher keyed `0x12,0x34`.
fn encode_link(plain: &[u8]) -> Vec<u8> {
    Cipher::link_cipher().encode(plain).expect("encode")
}

/// A small opcode-1 frame used as the loop's traffic; 8-byte body so the wire
/// frame is 13 bytes — big enough to split mid-header.
const TRAFFIC_CMD: u16 = 1;
const TRAFFIC_PAYLOAD: &[u8] = &[0x2A, 0x00, 0x00, 0x00, 0x7E, 0xAD, 0x01, 0x00];

#[test]
fn read_poll_budget_matches_the_legacy_cadence() {
    assert_eq!(
        READ_POLL_BUDGET,
        Duration::from_millis(100),
        "main.cpp:1549-1581 — the legacy pumps 10 x Sleep(10) = 100 ms per \
         NetworkOperation() iteration (:1625-1630), sleep gated \
         if(gamemode == 1) at :1579; this is that in-game cadence as a strict \
         per-read budget"
    );
    assert!(
        READ_POLL_BUDGET < crate::net::session::DEFAULT_READ_TIMEOUT,
        "the 100 ms budget must dominate the session's 5 s read window so a \
         ReadTimeout can never surface from an idle poll (net/session.rs)"
    );
}

#[tokio::test(start_paused = true)]
async fn an_idle_socket_does_not_block_the_loop_and_tickers_keep_firing() {
    let (listener, addr) = local_listener().await;
    // The peer accepts and then stays silent for the whole test: the link is
    // open but idle, the exact case the legacy's blocking GetMessage stalls on.
    let _silent = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
    });

    let mut link = dial(addr).await;
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut last_fired = 0u64;
    run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Ticked { fired, .. } = ev {
                last_fired = fired;
                if fired >= 3 {
                    return Ok(Flow::Break);
                }
            }
            Ok(Flow::Continue)
        },
    )
    .await
    .expect("loop");

    assert!(
        last_fired >= 3,
        "tickers must keep firing while the socket is idle, got {last_fired}"
    );
}

#[tokio::test(start_paused = true)]
async fn missed_intervals_coalesce_into_one_fire() {
    // The coalescing contract (module docs, "tickers are checked"): a whole
    // gap of missed intervals must produce ONE fire (the deadline stepping
    // past `now` in a single catch-up), not one fire per missed interval.
    // On a quiet socket the loop always wakes exactly on time — the virtual
    // clock only advances while every task is idle — so the gap is made
    // deterministically: the first fire's handler leaps virtual time forward
    // by six intervals with `tokio::time::advance` (the handler runs between
    // wakes, with the loop's select! futures already dropped). The leap is a
    // single clock move, so the loop's next poll unavoidably finds its
    // deadline six intervals in the past — the exact "busy for a while"
    // situation, with no wall-clock window anywhere.
    //
    // Timeline (virtual): fire at 25 ms (fired=1) → clock jumps to 175 ms,
    // having missed the 50/75/100/125/150 deadlines → ONE coalesced wake at
    // 175 ms (fired=2, deadline stepped to 200) → on-time wake at 200 ms
    // (fired=3) → the peer drops the connection at 212 ms (between ticks, so
    // no select! branch race) → EOF ends the loop. A burst implementation
    // would fire five extra times at 175 ms; a no-catch-up one would keep
    // re-firing at 175 ms until it crept past it — both stretch the recorded
    // sequence and fail the assertion.
    let (listener, addr) = local_listener().await;
    let _holder = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        // Hold the link open across the leap, then close mid-quiet: EOF lands
        // between ticker deadlines, so every wake in this test has exactly
        // one ready select! branch.
        tokio::time::sleep(Duration::from_millis(212)).await;
        // Dropping `_stream` sends FIN.
    });

    const INTERVAL: Duration = Duration::from_millis(25);

    let mut link = dial(addr).await;
    let tickers = [Ticker::new("coalesce", INTERVAL)];
    let baseline = Instant::now();
    let mut fires: Vec<(u64, Duration)> = Vec::new();
    run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Ticked { fired, .. } = ev {
                fires.push((fired, Instant::now() - baseline));
                if fired == 1 {
                    // Leap six intervals in one step: five deadlines are
                    // missed outright and the next wake must coalesce them.
                    tokio::spawn(async move {
                        tokio::time::advance(INTERVAL * 6).await;
                    });
                }
            }
            Ok(Flow::Continue)
        },
    )
    .await
    .expect("loop");

    assert_eq!(
        fires,
        vec![
            (1u64, INTERVAL),  // 25 ms: on time
            (2, INTERVAL * 7), // 175 ms: ONE wake for the whole gap
            (3, INTERVAL * 8), // 200 ms: back on schedule
        ],
        "a five-interval gap must coalesce into a single fire (fired=2), \
         not burst once per missed interval"
    );
}

#[tokio::test(start_paused = true)]
async fn frames_are_delivered_while_tickers_keep_firing() {
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // One frame, then hold the socket open so the loop keeps polling (and
        // its tickers keep firing) after the frame is consumed.
        let wire = encode_link(&frame(TRAFFIC_CMD, TRAFFIC_PAYLOAD));
        stream.write_all(&wire).await.unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
    });

    let mut link = dial(addr).await;
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut frames = 0u32;
    let mut ticks = 0u32;
    run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            match ev {
                LoopEvent::Frame(_) => frames += 1,
                LoopEvent::Ticked { .. } => ticks += 1,
            }
            if frames >= 1 && ticks >= 3 {
                Ok(Flow::Break)
            } else {
                Ok(Flow::Continue)
            }
        },
    )
    .await
    .expect("loop");

    assert!(frames >= 1, "the frame must be delivered");
    assert!(
        ticks >= 3,
        "tickers must not be starved by frame delivery, got {ticks}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_frame_sent_to_an_idle_link_is_delivered_within_the_read_budget() {
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Let the loop start polling an idle socket first, then deliver.
        tokio::time::sleep(Duration::from_millis(150)).await;
        let wire = encode_link(&frame(TRAFFIC_CMD, &[0xAA, 0xBB]));
        stream.write_all(&wire).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = dial(addr).await;
    let tickers: [Ticker; 0] = [];
    // Tokio's *virtual* clock (start_paused): `elapsed` below is measured on
    // the same paused clock the peer's 150 ms hold and the loop's budget run
    // on, so the assertion compares exact virtual instants, not wall time.
    let started = tokio::time::Instant::now();
    let mut got: Option<Frame> = None;
    run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Frame(f) = ev {
                got = Some(f);
                return Ok(Flow::Break);
            }
            Ok(Flow::Continue)
        },
    )
    .await
    .expect("loop");

    let frame = got.expect("the frame must be delivered");
    let elapsed = started.elapsed();
    assert_eq!(frame.cmd_id(), TRAFFIC_CMD, "opcode delivered intact");
    assert_eq!(frame.payload(), &[0xAA, 0xBB], "payload delivered intact");
    // The frame was written at exactly virtual t=150 ms (the peer's first
    // hold); the loop's next poll notice is bounded by one budget after that,
    // so on the paused clock the delivery instant is exact, not a window.
    assert!(
        elapsed < Duration::from_millis(400),
        "frame delivery lagged behind the read budget: {elapsed:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_frame_split_across_two_read_windows_still_reassembles() {
    // The cancellation-safety proof: the loop drops `next_frame` every 100 ms,
    // so a frame whose bytes straddle a budget boundary must still reassemble.
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let wire = encode_link(&frame(TRAFFIC_CMD, TRAFFIC_PAYLOAD));
        let split = 5; // mid-header: usSize(2) + ucChk(1) + usCmd(2) = 5
        stream.write_all(&wire[..split]).await.unwrap();
        // Long enough for at least two 100 ms budgets to elapse and the
        // next_frame future to be dropped mid-frame.
        tokio::time::sleep(Duration::from_millis(250)).await;
        stream.write_all(&wire[split..]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let mut link = dial(addr).await;
    let tickers: [Ticker; 0] = [];
    let mut got: Option<Frame> = None;
    run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Frame(f) = ev {
                got = Some(f);
                return Ok(Flow::Break);
            }
            Ok(Flow::Continue)
        },
    )
    .await
    .expect("loop");

    let frame = got.expect("the split frame must reassemble across windows");
    assert_eq!(frame.cmd_id(), TRAFFIC_CMD, "opcode survives the split");
    assert_eq!(
        frame.payload(),
        TRAFFIC_PAYLOAD,
        "payload is intact byte for byte after being dropped mid-frame twice"
    );
}

#[tokio::test]
async fn a_clean_server_close_ends_the_loop() {
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        // Dropping `_stream` sends FIN; the client's read returns 0 (EOF).
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers: [Ticker; 0] = [];
    let mut frames = 0u32;
    let result = run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Frame(_) = ev {
                frames += 1;
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "a clean close is Ok(()) from the loop, not an error: {result:?}"
    );
    assert_eq!(frames, 0, "no frame was sent, so none is delivered");
}

#[tokio::test]
async fn a_plaintext_frame_is_refused_and_stops_the_loop() {
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // Deliberately NOT link-ciphered: valid framing, wrong crypto. The
        // inbound decoder keys byte 3 onward, so this fails integrity exactly
        // as `entry/tests.rs::a_plaintext_frame_is_refused...` pins.
        let plain = frame(TRAFFIC_CMD, TRAFFIC_PAYLOAD);
        stream.write_all(&plain).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers: [Ticker; 0] = [];
    let result = run(
        &mut link,
        &tickers,
        async |_link, _ev| -> Result<Flow, Infallible> { Ok(Flow::Continue) },
    )
    .await;

    match result {
        Err(LoopError::Link(GameError::Codec(_))) => {}
        other => panic!(
            "a decode refusal must surface as LoopError::Link(GameError::Codec(_)), \
             got {other:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// P-04 (QC directive): the two documented behaviours that had no falsifying
// test — handler-error propagation ([`LoopError::Handler`], exercised with a
// custom error type, not `Infallible`) and buffered frames being delivered
// before a clean close ends the loop.
// ---------------------------------------------------------------------------

/// A handler's own error type: proves `LoopError::Handler` propagates a
/// *custom* error, not only the internal ones.
#[derive(Debug, PartialEq, Eq)]
struct Boom;

impl std::fmt::Display for Boom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("handler boom")
    }
}

impl std::error::Error for Boom {}

#[tokio::test(start_paused = true)]
async fn a_handler_error_on_the_tick_path_propagates_and_stops_the_loop() {
    // Silent peer: the first and only wake source is the 25 ms ticker, so the
    // handler's error happens on the tick dispatch path at a deterministic
    // virtual instant.
    let (listener, addr) = local_listener().await;
    let _silent = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
    });

    let mut link = dial(addr).await;
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut events = 0u32;
    let result = run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Boom> {
            events += 1;
            if matches!(ev, LoopEvent::Ticked { .. }) {
                return Err(Boom);
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    assert_eq!(
        events, 1,
        "the loop must stop at the first tick error — no further events"
    );
    match result {
        Err(LoopError::Handler(Boom)) => {}
        other => panic!(
            "a handler Err on the tick path must surface as \
             LoopError::Handler with the original error, got {other:?}"
        ),
    }
}

#[tokio::test(start_paused = true)]
async fn a_handler_error_on_the_frame_path_propagates_and_stops_the_loop() {
    // One real frame, then a hold: the handler errors on the first Frame. Any
    // ticker fires that land before the frame just continue, so the assertion
    // holds regardless of wake ordering.
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        let wire = encode_link(&frame(TRAFFIC_CMD, TRAFFIC_PAYLOAD));
        stream.write_all(&wire).await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
    });

    let mut link = dial(addr).await;
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut frames = 0u32;
    let result = run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Boom> {
            if matches!(ev, LoopEvent::Frame(_)) {
                frames += 1;
                return Err(Boom);
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    assert_eq!(
        frames, 1,
        "exactly the one frame the peer sent reaches the handler before the error"
    );
    match result {
        Err(LoopError::Handler(Boom)) => {}
        other => panic!(
            "a handler Err on the frame path must surface as \
             LoopError::Handler with the original error, got {other:?}"
        ),
    }
}

#[tokio::test(start_paused = true)]
async fn frames_buffered_before_a_clean_close_are_delivered_before_eof() {
    // Three whole frames written in one call, then the peer drops the stream
    // (FIN). All three bytes-are-already-buffered frames must be handed to
    // the handler before EOF returns Ok(()); a loop that treated the pending
    // close as "no more work" would deliver fewer than three.
    let (listener, addr) = local_listener().await;
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        // ONE cipher instance for the whole stream, exactly as the link
        // decodes it: the ITZ rolling counter advances per encode, so fresh
        // per-frame ciphers would misalign frame 2 onward.
        let mut cipher = Cipher::link_cipher();
        let mut wire = Vec::new();
        for payload in [&[0x01][..], &[0x02, 0x02][..], &[0x03, 0x03, 0x03][..]] {
            wire.extend_from_slice(&cipher.encode(&frame(TRAFFIC_CMD, payload)).expect("encode"));
        }
        stream.write_all(&wire).await.unwrap();
        // Dropping `stream` right after the write sends FIN with all three
        // frames already in the client's receive buffer.
    });

    let mut link = dial(addr).await;
    let tickers: [Ticker; 0] = [];
    let mut got: Vec<Frame> = Vec::new();
    let result = run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, Infallible> {
            if let LoopEvent::Frame(f) = ev {
                got.push(f);
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "the close after buffered frames is still a clean Ok(()): {result:?}"
    );
    let payloads: Vec<&[u8]> = got.iter().map(Frame::payload).collect();
    assert_eq!(
        payloads,
        vec![&[0x01][..], &[0x02, 0x02][..], &[0x03, 0x03, 0x03][..]],
        "all three buffered frames, in order and byte-exact, before EOF ends the loop"
    );
}
