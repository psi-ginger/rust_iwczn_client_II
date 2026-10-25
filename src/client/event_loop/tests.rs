// Step 5.1 tests — falsifiable, byte-exact, loopback.
//
// F-01/F-02 (QC decisions on CAR-2026-10-24-RETRO01): the live `goonzu-rs`
// server is unavailable, so every peer here is a loopback socket that writes
// **real link-ciphered frames** with a fresh `Cipher::link_cipher()` — the
// same per-session pairing the game service uses (`main.rs:680`). Read
// windows are pinned small so the 100 ms behaviour is observable in
// milliseconds, not seconds. What these prove is the *loop's* contract: an
// idle socket does not block, tickers keep firing alongside frames, a frame
// split across budget windows reassembles (cancellation safety), a clean
// close ends the loop, and a decode refusal stops it.
//
// F-07: the loop is never its own expectation source — every frame the peer
// writes is a handwritten `(cmd, payload)`, the handler records events into a
// plain `Vec` compared against those literals, and the cancellation-safety
// test's expectation (one whole frame) is derived from the bytes the peer
// chose to send, split at a literal offset.
use super::*;
use crate::game::entry::GameError;
use crate::net::cipher::Cipher;
use std::convert::Infallible;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Binds a loopback listener on an ephemeral port.
async fn local_listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
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
         NetworkOperation() iteration (:1625-1630); this is that cadence as a \
         strict per-read budget"
    );
    assert!(
        READ_POLL_BUDGET < crate::net::session::DEFAULT_READ_TIMEOUT,
        "the 100 ms budget must dominate the session's 5 s read window so a \
         ReadTimeout can never surface from an idle poll (net/session.rs)"
    );
}

#[tokio::test]
async fn an_idle_socket_does_not_block_the_loop_and_tickers_keep_firing() {
    let (listener, addr) = local_listener().await;
    // The peer accepts and then stays silent for the whole test: the link is
    // open but idle, the exact case the legacy's blocking GetMessage stalls on.
    let _silent = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
    });

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut last_fired = 0u64;
    run(
        &mut link,
        &tickers,
        |_link, ev| -> Result<Flow, Infallible> {
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

#[tokio::test]
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

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers = [Ticker::new("hb", Duration::from_millis(25))];
    let mut frames = 0u32;
    let mut ticks = 0u32;
    run(
        &mut link,
        &tickers,
        |_link, ev| -> Result<Flow, Infallible> {
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

#[tokio::test]
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

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers: [Ticker; 0] = [];
    let started = std::time::Instant::now();
    let mut got: Option<Frame> = None;
    run(
        &mut link,
        &tickers,
        |_link, ev| -> Result<Flow, Infallible> {
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
    // The frame was written at ~150 ms; the strict budget means the loop
    // notices within one budget of arrival, well before a full extra stall.
    assert!(
        elapsed < Duration::from_millis(400),
        "frame delivery lagged behind the read budget: {elapsed:?}"
    );
}

#[tokio::test]
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

    let mut link = GameLink::connect(addr).await.expect("connect");
    let tickers: [Ticker; 0] = [];
    let mut got: Option<Frame> = None;
    run(
        &mut link,
        &tickers,
        |_link, ev| -> Result<Flow, Infallible> {
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
        |_link, ev| -> Result<Flow, Infallible> {
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
        |_link, _ev| -> Result<Flow, Infallible> { Ok(Flow::Continue) },
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
