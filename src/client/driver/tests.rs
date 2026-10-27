//! Step 6.1 unit tests — the parts that need no peer.
//!
//! The full conversation is Step 6.2's harness (`tests/e2e_loopback.rs`); what
//! is provable here is configuration, redaction, stage reporting, and that a
//! rejected configuration never opens a socket. Keeping the split honest is the
//! point: a green run of this module must not be readable as "the session
//! works".

use std::net::{SocketAddr, TcpListener};
use std::time::Duration;

use super::{
    run, ConfigError, DriverConfig, DriverError, Stage, DEFAULT_DRAIN_BOUND, DEFAULT_FRONT_ADDR,
    DEFAULT_GAME_ADDR, ENV_CHAR_SLOT, ENV_DRAIN_MS, ENV_FRONT_ADDR, ENV_MAC_ADDRESS, ENV_USER_ID,
    ENV_USER_PW, ENV_VILLAGE_REF,
};
use crate::client::teardown::{Cause, Shutdown};

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
