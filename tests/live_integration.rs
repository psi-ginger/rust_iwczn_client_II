// Live integration against a reachable GoonZu server (QC 2026-10-26).
//
// WHY THIS EXISTS, AND WHY THE LOOPBACK SUITE IS NOT ENOUGH
//
// Every one of the 131 hermetic tests drives a *loopback* peer that encodes
// with this crate's own `Cipher` (F-01/F-02: the live server was unavailable).
// That is circular for exactly one assumption: the game link's inbound cipher.
// The server encodes its handshake with `Cipher::session_cipher(cfg.link_key.0,
// cfg.link_key.1)` — the **configured** link key (SEC-014, `main.rs:680`) —
// while this client hardcodes `LINK_KEY1/LINK_KEY2 = 0x12, 0x34`. A loopback
// peer using `Cipher::link_cipher()` cannot detect a mismatch, because both
// sides share the same constant. Only a handshake against the real service
// proves the deployment assumption.
//
// These tests are `#[ignore]`d: the hermetic gate (`cargo test`, 131/131) must
// never require a network, a running server, or an ngrok tunnel. Run them
// explicitly with:
//
//     cargo test --test live_integration -- --ignored --nocapture
//
// Endpoints come from the environment because tunnel addresses are per-session
// and must not be baked into the repo:
//
//     GOONZU_LIVE_FRONT=host:port   (default 127.0.0.1:5700)
//     GOONZU_LIVE_GAME=host:port    (default 127.0.0.1:4010)
//
// `host:port` is resolved with `lookup_host`, so a DNS name (ngrok) and a bare
// IP both work. No credentials are used: both services push their handshake
// frame unsolicited at accept (`front/src/main.rs:129`, `game/src/main.rs:687`),
// so connect + handshake is the complete authenticated-free surface.

use goonzu_client::auth::front::{FrontLink, SecretKey};
use goonzu_client::client::event_loop::{run, Flow, LoopEvent, Ticker};
use goonzu_client::game::entry::{ClientCharUnique, GameLink};
use std::time::Duration;
use tokio::net::lookup_host;

/// Resolves `host:port` from `var`, falling back to `default_host:default_port`.
///
/// `SocketAddr::from_str` cannot parse a DNS name, so the host is resolved here
/// — that is what makes an ngrok tunnel usable at all.
async fn resolve(var: &str, default_host: &str, default_port: u16) -> std::net::SocketAddr {
    let spec = std::env::var(var).unwrap_or_else(|_| format!("{default_host}:{default_port}"));
    let (host, port) = spec
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("{var}={spec:?} is not host:port"));
    let port: u16 = port
        .parse()
        .unwrap_or_else(|_| panic!("{var}={spec:?}: port {port:?} is not u16"));
    let mut addrs = lookup_host((host, port))
        .await
        .unwrap_or_else(|e| panic!("{var}={spec:?}: DNS resolve failed: {e}"))
        .collect::<Vec<_>>();
    addrs.sort();
    let addr = addrs
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{var}={spec:?}: resolved to zero addresses"));
    eprintln!("[live] {var}={spec} -> {addr}");
    addr
}

/// Step 3.1 live: front connect + `FRONTMSG_RESPONSE_SECRETKEY` (30000).
#[tokio::test]
#[ignore = "live integration: needs a reachable front service (GOONZU_LIVE_FRONT)"]
async fn live_front_handshake_extracts_secret_key() {
    let addr = resolve("GOONZU_LIVE_FRONT", "127.0.0.1", 5700).await;

    let mut front = FrontLink::connect(addr)
        .await
        .expect("front connect (TCP dial + accept)");
    let key: SecretKey = front
        .handshake()
        .await
        .expect("FRONTMSG_RESPONSE_SECRETKEY handshake");

    // The seeds are real, distinct bytes off the wire — not an all-zero default.
    assert_ne!(
        (key.key1(), key.key2()),
        (0, 0),
        "server sent an all-zero seed pair; the payload parse is suspect"
    );
    assert!(front.is_keyed(), "handshake must key the outbound channel");
    eprintln!(
        "[live] front OK: session_seed=({:#04x}, {:#04x})",
        key.key1(),
        key.key2()
    );
}

/// Step 4.2 live: game connect + `GAMEMSG_RESPONSE_CLIENTCHARUNIQUE` (opcode 1).
///
/// The load-bearing assertion in this file: the handshake decodes only if the
/// server's *configured* link key equals this client's hardcoded `0x12, 0x34`.
#[tokio::test]
#[ignore = "live integration: needs a reachable game service (GOONZU_LIVE_GAME)"]
async fn live_game_handshake_decodes_the_link_cipher() {
    let addr = resolve("GOONZU_LIVE_GAME", "127.0.0.1", 4010).await;

    let mut link = GameLink::connect(addr)
        .await
        .expect("game connect (TCP dial + accept)");
    let state: ClientCharUnique = link
        .handshake()
        .await
        .expect("GAMEMSG_RESPONSE_CLIENTCHARUNIQUE handshake");

    // A decoded opcode-1 frame with a sane char_unique is proof the inbound
    // link cipher matched: garbage keys yield a framing/integrity refusal.
    assert_ne!(state.char_unique, 0, "char_unique must be allocated");
    assert!(
        state.version >= 0,
        "server version {} looks corrupt; check the decode",
        state.version
    );
    assert!(
        link.negotiated().is_some(),
        "handshake must install the negotiated state"
    );
    // The P-08 correlation line, exactly as CONTRIBUTING documents it.
    eprintln!(
        "client handshake succeeded: session_seed = ({:#04x}, {:#04x}), char_unique = {} (server version {})",
        state.key1, state.key2, state.char_unique, state.version
    );
}

/// Step 5.2 live: the real 10 s heartbeat ticker against a live server.
///
/// The half of the 5.2 acceptance ("...over 60 seconds without dropping
/// connection") that loopback structurally cannot prove: whether a *real*
/// game service tolerates our opcode-100 frames for the full window, or
/// drops/refuses the connection. This drives the production path —
/// [`heartbeat_ticker`] at its real 10 s cadence, [`is_heartbeat`] routing,
/// and [`send_heartbeat`] awaited from the async handler — for `TICKS`
/// intervals (70 s, comfortably over the 60 s acceptance window).
///
/// The server **mutes** opcode 100's recv log line (`main.rs:1106-1112`), so
/// "server logs record it" cannot be asserted from here either; what is
/// asserted is the connection staying up and every send succeeding, which is
/// the observable that actually matters. Ignored by default and slow by
/// design — run it explicitly when a server is reachable.
#[tokio::test]
#[ignore = "live soak: 70s against a reachable game service (GOONZU_LIVE_GAME)"]
async fn live_heartbeat_keeps_the_connection_up_over_60s() {
    use goonzu_client::client::heartbeat::{heartbeat_ticker, is_heartbeat, send_heartbeat};

    let addr = resolve("GOONZU_LIVE_GAME", "127.0.0.1", 4010).await;

    let mut link = GameLink::connect(addr).await.expect("game connect");
    let state = link
        .handshake()
        .await
        .expect("handshake before the heartbeat soak");

    // Establish the session before the soak. The server arms a 10 s handshake
    // deadline at accept (`main.rs:671`; `config.rs:430` asserts
    // `Duration::from_millis(10_000)`) and holds it as the read bound until it
    // receives its first inbound frame. The opcode-1 handshake is
    // server→client only, so the server has heard *nothing* from us yet and
    // its state is still `Connected`, not `Established` (`session.rs:280`,
    // `main.rs:809`). That first inbound frame is what calls `establish()`
    // (`main.rs:1064`) and disarms the deadline.
    //
    // Our first *ticker* fire is also at 10 s — the same 10 s — and because
    // the ticker starts after connect+handshake it lands just *past* the
    // deadline, so the server reclaims the connection first (an earlier
    // version of this test lost exactly that race: 1 of 7 heartbeats, then a
    // clean EOF at ~10 s). A real client sends login (opcode 2) within
    // milliseconds of the handshake and never races; we hold no credentials,
    // so we establish the session with one immediate heartbeat instead — any
    // well-formed inbound frame establishes, and opcode 100's arm is a no-op
    // (`main.rs:1863-1868`).
    send_heartbeat(&mut link)
        .await
        .expect("establishing heartbeat before the soak");
    eprintln!("[live] session established via an early heartbeat; entering the soak");
    // Anchor the 60 s assertion on the *connection*, not the first tick: the
    // acceptance is that the link is never dropped across the window, and the
    // link came up before the ticker's first fire. (Anchoring on tick 1 would
    // measure 10s..70s = exactly 60 s with no margin.)
    let established = std::time::Instant::now();

    // The production ticker: 10 s, unshortened. 7 ticks = 70 s > the 60 s
    // acceptance window. A ticker interval of 0 is rejected by the loop, and
    // shortening it here would stop testing what the acceptance specifies.
    const TICKS: u32 = 7;
    let tickers = [heartbeat_ticker()];
    let mut sent = 0u32;
    let mut start = None;

    let result = run(
        &mut link,
        &tickers,
        async |link, ev| -> Result<Flow, std::convert::Infallible> {
            if is_heartbeat(&ev) {
                let now = tokio::time::Instant::now();
                start.get_or_insert(now);
                // The real 5.2 path: await the send from inside the handler.
                // A write failure surfaces here and stops the loop, which the
                // assertion below catches as a non-Ok result.
                if let Err(e) = send_heartbeat(link).await {
                    panic!("heartbeat send failed on tick {sent}: {e}");
                }
                sent += 1;
                eprintln!(
                    "[live] heartbeat {sent}/{TICKS} sent at +{:?}",
                    now.duration_since(start.unwrap())
                );
                if sent >= TICKS {
                    return Ok(Flow::Break);
                }
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    // A link failure (server refused the heartbeat, dropped us, or a cipher
    // desync) is the regression this soak exists to catch.
    result.unwrap_or_else(|e| panic!("connection did not survive the soak: {e}"));
    assert_eq!(
        sent, TICKS,
        "every scheduled heartbeat must fire and send over an open link"
    );
    let elapsed = established.elapsed();
    assert!(
        elapsed >= Duration::from_secs(60),
        "the connection must survive the 60 s acceptance window, dropped after only {elapsed:?}"
    );
    eprintln!(
        "[live] heartbeat soak OK: {TICKS} heartbeats over {elapsed:?} of open connection, \
         never dropped, char_unique={}",
        state.char_unique
    );
}

/// Step 5.1 live: the event loop over a real WAN path (through the tunnel).
///
/// Proves what loopback cannot: an *idle* remote socket does not block the
/// loop, tickers keep firing, and no spurious link error surfaces from real
/// network jitter — the acceptance for the non-blocking event loop.
#[tokio::test]
#[ignore = "live integration: needs a reachable game service (GOONZU_LIVE_GAME)"]
async fn live_event_loop_idles_cleanly_and_ticks() {
    let addr = resolve("GOONZU_LIVE_GAME", "127.0.0.1", 4010).await;

    let mut link = GameLink::connect(addr).await.expect("game connect");
    let state = link
        .handshake()
        .await
        .expect("handshake before entering the loop");

    // A slow ticker: with no inbound traffic after the handshake, every wake
    // must be a tick. If the loop blocked on the idle socket, this would hang.
    let tickers = [Ticker::new("live-hb", Duration::from_millis(500))];
    let mut ticks = 0u32;
    let mut frames = 0u32;

    let result = run(
        &mut link,
        &tickers,
        async |_link, ev| -> Result<Flow, std::convert::Infallible> {
            match ev {
                LoopEvent::Ticked { name, fired } => {
                    ticks += 1;
                    eprintln!("[live] tick name={name} fired={fired}");
                    if ticks >= 3 {
                        return Ok(Flow::Break);
                    }
                }
                LoopEvent::Frame(f) => {
                    frames += 1;
                    eprintln!("[live] unexpected inbound frame cmd={:#06x}", f.cmd_id());
                }
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    // `Ok(())` here means either our Break or a clean server close; a link
    // failure (cipher desync, framing refusal, socket error) is the regression.
    result.unwrap_or_else(|e| panic!("event loop failed against the live server: {e}"));
    assert!(
        ticks >= 3,
        "idle remote socket starved the tickers: only {ticks} tick(s) in the run"
    );
    eprintln!(
        "[live] event loop OK over WAN: {ticks} tick(s), {frames} frame(s), char_unique={}",
        state.char_unique
    );
}
