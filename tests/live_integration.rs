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
        |_link, ev| -> Result<Flow, std::convert::Infallible> {
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
