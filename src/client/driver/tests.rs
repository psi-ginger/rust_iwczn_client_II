//! Unit tests for the Step 6.1 driver's pure surfaces (config defaults, the
//! dispatchable-opcode set, worldlist-derived addressing, env-spec parsing).
//!
//! The orchestrated flow itself is proven end to end by the hermetic wire
//! doubles in `tests/e2e_loopback.rs` (Step 6.2), whose expectations are
//! hand-placed bytes — never this module's own output (F-07).

use super::{
    is_dispatchable, resolve_spec, world_addr, ConfigError, DriverConfig, DEFAULT_DRAIN_BOUND,
    DEFAULT_MAC_ADDRESS, DEFAULT_STEP_TIMEOUT,
};
use crate::auth::front;
use crate::auth::world_list::WorldInfo;
use crate::client::heartbeat::HEARTBEAT_INTERVAL;
use crate::game::entry;
use std::time::Duration;

/// The default configuration is the documented environment contract with no
/// variables set: the two service ports, the placeholder MAC, slot 0, no
/// world/village, and the production timing values (10 s heartbeat, 10 s step
/// bound, 5 s drain bound).
#[test]
fn default_config_is_the_documented_contract() {
    let config = DriverConfig::default();
    assert_eq!(config.front_addr.port(), front::DEFAULT_PORT);
    assert_eq!(config.front_addr.port(), 5700);
    assert_eq!(config.game_addr.port(), entry::DEFAULT_PORT);
    assert_eq!(config.game_addr.port(), 4010);
    assert!(config.front_addr.ip().is_loopback());
    assert!(config.game_addr.ip().is_loopback());
    assert_eq!(config.mac_address, DEFAULT_MAC_ADDRESS);
    assert_eq!(config.world_id, None);
    assert_eq!(config.char_slot, 0);
    assert_eq!(config.village_ref, None);
    assert_eq!(config.heartbeat_interval, HEARTBEAT_INTERVAL);
    assert_eq!(config.heartbeat_interval, Duration::from_secs(10));
    assert_eq!(config.step_timeout, DEFAULT_STEP_TIMEOUT);
    assert_eq!(config.drain_bound, DEFAULT_DRAIN_BOUND);
}

/// The entry waits dispatch exactly the five opcodes the Step 5.4 dispatcher
/// owns — and nothing else, so a foreign opcode during entry still fails the
/// phase loudly instead of being absorbed.
#[test]
fn is_dispatchable_covers_exactly_the_five_dispatcher_opcodes() {
    for cmd in [6u16, 11, 118, 200, 215] {
        assert!(is_dispatchable(cmd), "opcode {cmd} is dispatcher-owned");
    }
    for cmd in [1u16, 2, 3, 4, 5, 100, 106, 115, 248, 1101, 1102] {
        assert!(
            !is_dispatchable(cmd),
            "opcode {cmd} is not dispatcher-owned"
        );
    }
}

/// A worldlist entry's advertised ip:port is the dial address (the legacy's
/// "dial where the list says"); an unparseable ip string fails with the entry
/// named, never a silent fallback.
#[test]
fn world_addr_forms_the_advertised_dial_address() {
    let world = worldlist_entry("127.0.0.1", 4010);
    assert_eq!(
        world_addr(&world).expect("a parseable ip yields an address"),
        "127.0.0.1:4010"
            .parse::<std::net::SocketAddr>()
            .expect("literal")
    );

    let bad = worldlist_entry("not-an-ip", 4010);
    let error = world_addr(&bad).expect_err("an unparseable ip must fail");
    assert!(
        matches!(
            error,
            super::DriverError::WorldAddress {
                world_id: 7,
                port: 4010,
                ..
            }
        ),
        "unexpected error: {error}"
    );
    assert!(
        error.to_string().contains("not-an-ip"),
        "the failure must name the offending ip: {error}"
    );
}

/// An address spec without a port, and one with a non-u16 port, are refused
/// before any DNS work happens — no guessing, no default port grafted on.
#[tokio::test]
async fn resolve_spec_refuses_malformed_specs_before_resolving() {
    let no_port = resolve_spec("TEST_SPEC", "127.0.0.1".to_owned())
        .await
        .expect_err("a spec without a port must be refused");
    assert!(matches!(
        no_port,
        ConfigError::NotHostPort {
            name: "TEST_SPEC",
            ..
        }
    ));

    let bad_port = resolve_spec("TEST_SPEC", "127.0.0.1:http".to_owned())
        .await
        .expect_err("a non-u16 port must be refused");
    assert!(matches!(
        bad_port,
        ConfigError::Parse {
            name: "TEST_SPEC",
            kind: "u16 port",
            ..
        }
    ));
}

/// A `WorldInfo` with only the address fields meaningful — the remaining
/// fields are parser outputs this test does not exercise.
fn worldlist_entry(ip: &str, port: u16) -> WorldInfo {
    WorldInfo {
        world_id: 7,
        world_name: "unit-test".to_owned(),
        server_id: 0,
        game_server_ip: ip.to_owned(),
        game_server_port: port,
        open_date: String::new(),
        short_message: String::new(),
        refresh_world_id: 7,
        state: 0,
        current_connections: 0,
        goonzu_name: String::new(),
        goonzu_text: String::new(),
        population: 0,
        current_price: 0,
        npc_rate: 0,
        fixation_money: 0,
    }
}
