//! Step 6.1 unit tests — the parts that need no peer.
//!
//! The full conversation is Step 6.2's harness (`tests/e2e_loopback.rs`); what
//! is provable here is configuration, redaction, stage reporting, and that a
//! rejected configuration never opens a socket. Keeping the split honest is the
//! point: a green run of this module must not be readable as "the session
//! works".

use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;
use tracing_subscriber::fmt::MakeWriter;

use super::{
    log_session_end, run, run_with_watcher, ConfigError, DriverConfig, DriverError, DriverOutcome,
    Stage, DEFAULT_DRAIN_BOUND, DEFAULT_FRONT_ADDR, DEFAULT_GAME_ADDR, ENV_CHAR_SLOT, ENV_DRAIN_MS,
    ENV_FRONT_ADDR, ENV_MAC_ADDRESS, ENV_USER_ID, ENV_USER_PW, ENV_VILLAGE_REF,
};
use crate::client::teardown::{watch_signal, Cause, Shutdown};

/// The credentials every configuration in these tests starts from.
fn base() -> Vec<(&'static str, String)> {
    vec![
        (ENV_USER_ID, "alice".to_string()),
        (ENV_USER_PW, "hunter2".to_string()),
        (ENV_MAC_ADDRESS, "00-11-22-33-44-55".to_string()),
        (ENV_CHAR_SLOT, "3".to_string()),
    ]
}

/// Replaces one key, keeping every other entry.
fn set(pairs: &mut Vec<(&'static str, String)>, key: &'static str, value: &str) {
    pairs.retain(|(name, _)| *name != key);
    pairs.push((key, value.to_string()));
}

/// A lookup over the given pairs — the seam that keeps tests off the process
/// environment.
fn env(pairs: Vec<(&'static str, String)>) -> impl Fn(&str) -> Option<String> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    }
}

/// A configuration with valid credentials, aimed at `front`.
fn valid_config(front: SocketAddr) -> DriverConfig {
    DriverConfig {
        front,
        game: DEFAULT_GAME_ADDR.parse().expect("a literal address parses"),
        user_id: "alice".to_string(),
        password: "hunter2".to_string(),
        mac_address: "00-11-22-33-44-55".to_string(),
        char_slot: 3,
        village_ref: None,
        drain_bound: DEFAULT_DRAIN_BOUND,
    }
}

/// `FRONTMSG_RESPONSE_SECRETKEY` as it sits on the wire, in the clear — the
/// same bytes `auth::front`'s tests replay (FrontMsg.h:114).
const SECRETKEY_FRAME: [u8; 10] = [0x0A, 0x00, 0x00, 0x30, 0x75, 0x00, 0x00, 0xA7, 0x00, 0x5C];

/// An in-memory sink behind `tracing_subscriber::fmt`, so a test can read back
/// the record the driver actually rendered instead of asserting about one it
/// never emitted.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    /// Everything the subscriber wrote, lossily decoded.
    fn text(&self) -> String {
        let bytes = self.0.lock().expect("the capture mutex is not poisoned");
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// The writer half of [`Capture`]: every formatted record lands in the buffer.
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("the capture mutex is not poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(Arc::clone(&self.0))
    }
}

#[tokio::test]
async fn defaults_apply_when_only_credentials_are_set() {
    let cfg = DriverConfig::from_lookup(env(base()))
        .await
        .expect("the base configuration is valid");

    assert_eq!(cfg.front, DEFAULT_FRONT_ADDR.parse().unwrap());
    assert_eq!(cfg.game, DEFAULT_GAME_ADDR.parse().unwrap());
    assert_eq!(cfg.char_slot, 3);
    assert_eq!(cfg.village_ref, None);
    assert_eq!(cfg.drain_bound, DEFAULT_DRAIN_BOUND);
}

#[tokio::test]
async fn optional_village_and_drain_bound_are_honoured() {
    let mut pairs = base();
    set(&mut pairs, ENV_VILLAGE_REF, "12");
    set(&mut pairs, ENV_DRAIN_MS, "250");

    let cfg = DriverConfig::from_lookup(env(pairs)).await.unwrap();

    assert_eq!(cfg.village_ref, Some(12));
    assert_eq!(cfg.drain_bound, Duration::from_millis(250));
}

#[tokio::test]
async fn a_missing_credential_is_named_but_never_echoed() {
    let mut pairs = base();
    pairs.retain(|(name, _)| *name != ENV_USER_PW);

    let error = DriverConfig::from_lookup(env(pairs))
        .await
        .expect_err("a missing password must be refused");

    assert_eq!(error, ConfigError::Missing { name: ENV_USER_PW });
    let text = error.to_string();
    assert!(text.contains(ENV_USER_PW), "the variable must be named");
    assert!(!text.contains("hunter2"), "no secret may appear: {text}");
}

#[tokio::test]
async fn debug_redacts_the_password() {
    let cfg = DriverConfig::from_lookup(env(base())).await.unwrap();

    let rendered = format!("{cfg:?}");

    assert!(
        rendered.contains("<redacted>"),
        "field must be marked: {rendered}"
    );
    assert!(
        !rendered.contains("hunter2"),
        "no secret may appear: {rendered}"
    );
}

#[tokio::test]
async fn a_slot_outside_the_roster_is_refused_with_the_bound() {
    let mut pairs = base();
    set(&mut pairs, ENV_CHAR_SLOT, "15");

    let error = DriverConfig::from_lookup(env(pairs))
        .await
        .expect_err("15 is outside 0..15");

    assert_eq!(
        error,
        ConfigError::SlotOutOfRange {
            name: ENV_CHAR_SLOT,
            value: 15,
            max: 14,
        }
    );
}

#[tokio::test]
async fn a_non_numeric_slot_is_refused() {
    let mut pairs = base();
    set(&mut pairs, ENV_CHAR_SLOT, "middle");

    let error = DriverConfig::from_lookup(env(pairs))
        .await
        .expect_err("a word is not a slot");

    assert!(matches!(error, ConfigError::NotANumber { .. }));
    assert!(error.to_string().contains(ENV_CHAR_SLOT));
}

#[tokio::test]
async fn an_address_without_a_port_is_refused() {
    let mut pairs = base();
    set(&mut pairs, ENV_FRONT_ADDR, "127.0.0.1");

    let error = DriverConfig::from_lookup(env(pairs))
        .await
        .expect_err("host without port cannot resolve");

    assert!(matches!(error, ConfigError::Address { .. }));
    assert!(error.to_string().contains(ENV_FRONT_ADDR));
}

#[test]
fn stage_names_are_stable() {
    assert_eq!(Stage::FrontConnected.as_str(), "FrontConnected");
    assert_eq!(Stage::GameLogin.to_string(), "GameLogin");
    assert_eq!(Stage::TornDown.to_string(), "TornDown");
}

#[test]
fn a_stage_error_keeps_the_stage_and_the_source() {
    let error = DriverError::at(Stage::CharList, std::io::Error::other("boom"));

    let text = error.to_string();

    assert!(text.contains("CharList"), "the stage must be named: {text}");
    assert!(text.contains("boom"), "the source must survive: {text}");
}

#[tokio::test]
async fn an_invalid_configuration_never_opens_a_socket() {
    // An address that would hang or fail slowly if it were ever dialled: the
    // test passes only because validation runs first.
    let mut cfg = valid_config("192.0.2.1:1".parse().unwrap());
    cfg.char_slot = 99;

    let error = tokio::time::timeout(Duration::from_secs(2), run(&cfg, &Shutdown::new()))
        .await
        .expect("validation must not wait on the network")
        .expect_err("99 is outside the roster");

    assert!(matches!(
        error,
        DriverError::Config(ConfigError::SlotOutOfRange { .. })
    ));
}

#[tokio::test]
async fn a_claimed_shutdown_stops_before_the_first_stage() {
    let cfg = valid_config("192.0.2.1:1".parse().unwrap());
    let shutdown = Shutdown::new();
    assert!(shutdown.trigger(Cause::CtrlC), "first trigger wins");

    let error = run(&cfg, &shutdown).await.expect_err("already stopped");

    assert!(
        matches!(
            error,
            DriverError::Cancelled {
                stage: Stage::FrontConnected
            }
        ),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn a_closed_front_port_is_reported_at_the_front_stage() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port is available");
    let addr = listener.local_addr().expect("bound address");
    drop(listener);

    let cfg = valid_config(addr);
    let error = tokio::time::timeout(Duration::from_secs(10), run(&cfg, &Shutdown::new()))
        .await
        .expect("a refused connect must not hang")
        .expect_err("nothing is listening");

    match error {
        DriverError::Stage { stage, .. } => assert_eq!(stage, Stage::FrontConnected),
        other => panic!("expected a stage error, got: {other}"),
    }
}

/// WO-6.1 §6.4, first half: the password must not appear in `DriverError`'s
/// `Display` — nor in its `Debug`, nor anywhere down the source chain.
///
/// The expectation is the work order's (§5: "never logged, never in
/// `Debug`/`Display` output"), never something read back from the code under
/// test: the password under assertion is the one these tests configure
/// *before* any of the rendering happens.
#[tokio::test]
async fn a_driver_error_never_renders_the_password() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port is available");
    let front = listener.local_addr().expect("bound address");
    drop(listener); // nothing listens: the dial fails

    // A real error from a real session whose configuration carried the password.
    let cfg = valid_config(front);
    let dialled = tokio::time::timeout(Duration::from_secs(10), run(&cfg, &Shutdown::new()))
        .await
        .expect("a refused dial must not hang")
        .expect_err("nothing is listening");

    let variants = [
        dialled,
        DriverError::Config(ConfigError::Missing { name: ENV_USER_PW }),
        DriverError::Cancelled {
            stage: Stage::GameLogin,
        },
        DriverError::Refused {
            stage: Stage::CharList,
            detail: "slot 3 holds no character".to_string(),
        },
        DriverError::Stage {
            stage: Stage::GameLogin,
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "the game service dropped the link",
            )),
        },
    ];

    for error in variants {
        let mut rendered = error.to_string();
        let mut cause = std::error::Error::source(&error);
        while let Some(source) = cause {
            rendered.push_str("\ncaused by: ");
            rendered.push_str(&source.to_string());
            cause = source.source();
        }
        assert!(
            !rendered.contains("hunter2"),
            "the password reached Display: {rendered}"
        );
        assert!(
            !format!("{error:?}").contains("hunter2"),
            "the password reached Debug: {error:?}"
        );
    }
}

/// WO-6.1 §6.4, second half: the password must not appear in a rendered log
/// record at `INFO`. Both records are produced for real — a session attempt
/// under the driver's own configuration, and the terminal record `run` emits —
/// and the buffer is read back after the subscriber is uninstalled, so the
/// assertion runs over rendered bytes rather than an intent.
#[tokio::test]
async fn a_rendered_info_record_never_carries_the_password() {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(capture.clone())
        .finish();
    // Scoped to this test's thread: a parallel test's records never land in this
    // buffer, and this test's never land in theirs.
    let guard = tracing::subscriber::set_default(subscriber);

    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port is available");
    let front = listener.local_addr().expect("bound address");
    drop(listener);

    let cfg = valid_config(front);
    let _ = tokio::time::timeout(Duration::from_secs(10), run(&cfg, &Shutdown::new())).await;

    log_session_end(
        &DriverOutcome {
            stage: Stage::TornDown,
            worlds: 1,
            char_unique: 42,
            char_slot: cfg.char_slot,
            village: cfg.village_ref,
        },
        Some(Cause::CtrlC),
    );
    drop(guard);

    let rendered = capture.text();
    assert!(
        rendered.contains("session ended"),
        "the terminal record must render: {rendered}"
    );
    assert!(
        rendered.contains("TornDown"),
        "the stage must be in the record: {rendered}"
    );
    assert!(
        !rendered.contains("hunter2"),
        "the password reached an INFO record: {rendered}"
    );
}

/// WO-6.1 §6.3's Ctrl+C path, driven hermetically through the driver: the
/// watcher `run` arms claims the console event and the flow stops at the next
/// stage gate instead of marching on.
///
/// A real console event is out of the question (it would signal cargo's console
/// group too), so the seam is 5.3's injected future — the exact one
/// `spawn_ctrl_c_watcher` wires for production. The flag reaching `terminate`
/// with a game link open needs the whole conversation; WO §2 hands that proof to
/// the 6.2 harness.
#[tokio::test(start_paused = true)]
async fn a_console_event_stops_the_flow_at_the_next_stage_gate() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is available");
    let front = listener.local_addr().expect("bound address");

    let signal = Arc::new(Notify::new());
    let front_service = tokio::spawn({
        let signal = Arc::clone(&signal);
        async move {
            let (mut socket, _) = listener.accept().await.expect("the driver dials");
            // The console event lands while the flow waits for SECRETKEY. The
            // clock is paused, so it only advances once every task is idle — the
            // watcher has claimed the flag before the frame is written.
            signal.notify_one();
            tokio::time::sleep(Duration::from_millis(50)).await;
            socket
                .write_all(&SECRETKEY_FRAME)
                .await
                .expect("the frame is written");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    let cfg = valid_config(front);
    let shutdown = Shutdown::new();
    let error = run_with_watcher(&cfg, &shutdown, {
        let signal = Arc::clone(&signal);
        move |shutdown, tracker| {
            let signal = Arc::clone(&signal);
            tracker.spawn(async move {
                watch_signal(shutdown, move || {
                    let signal = Arc::clone(&signal);
                    async move {
                        signal.notified().await;
                        Ok(())
                    }
                })
                .await
            })
        }
    })
    .await
    .expect_err("the console event must stop the session");

    assert!(
        matches!(
            error,
            DriverError::Cancelled {
                stage: Stage::SecretKey
            }
        ),
        "the flow must stop at the next gate: {error}"
    );
    assert_eq!(
        shutdown.cause(),
        Some(Cause::CtrlC),
        "the watcher run arms must claim the console event"
    );
    front_service
        .await
        .expect("the fake front service finishes");
}
