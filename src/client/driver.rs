//! Step 6.1 — the end-to-end driver: one session, every delivered stage, one
//! process (`CONTRIBUTING_PHASE2.md`, "Step 6.1").
//!
//! Until this module existed the crate was a library of verified parts that no
//! binary ever assembled: [`crate::main`](crate) booted a runtime and logged a
//! version string. The driver is the missing spine. It owns **no wire parsing**
//! of its own — every frame is handed to the parser the corresponding step
//! already delivered, so a defect here can only be a sequencing or teardown
//! defect, never a silent re-interpretation of bytes.
//!
//! # The conversation it drives
//!
//! ```text
//! front service                                  game service
//!   connect            (3.1)                        connect            (4.1)
//!   RESONSE_SECRETKEY  (3.1)                        RESPONSE_CLIENTCHARUNIQUE (4.2)
//!   REQUEST_NOTICE     (3.2)                        REQUEST_LOGIN      (4.3)
//!   REQUEST_WORLDLIST  (3.2)                        N x RESPONSE_CHARLIST (4.4)
//!   REQUEST_LOGIN      (3.3) -> auth_key            REQUEST_SELECTCHAR (4.5)
//!                                                   RESPONSE_SERVERINFO (4.6)
//!                                                   RESPONSE_LOGIN      (4.6)
//!                                                   REQUEST_VILLAGESTRUCT (4.7, optional)
//!                                                   in-world: 5.1 loop + 5.2 heartbeat + 5.4 dispatcher
//! ```
//!
//! # Stage observability
//!
//! [`DriverOutcome`] names the stage the session reached and
//! [`DriverError`] carries the stage that failed, so "it did not work" always
//! becomes "it failed at `GameLogin`". A successful run ends at
//! [`Stage::TornDown`], and [`Stage::TornDown`] is written by [`run`] only
//! **after** the teardown call returns — the stage is a claim about work that
//! happened, never an intention.
//!
//! # What this step deliberately does not prove
//!
//! A hermetic end-to-end proof needs a byte-exact peer; that is Step 6.2's
//! harness (`tests/e2e_loopback.rs`). This module's own tests prove the parts
//! that need no server: configuration, redaction, stage reporting, and that
//! validation happens before any socket is touched.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::auth::front::FrontLink;
use crate::auth::login::{parse_login_response, request_login};
use crate::auth::world_list::{
    parse_notice, request_notice, request_worldlist, WorldList, RESPONSE_NOTICE, RESPONSE_WORLDLIST,
};
use crate::client::event_loop::{run_with_shutdown, Flow, LoopError, LoopEvent};
use crate::client::heartbeat::{heartbeat_ticker, is_heartbeat, send_heartbeat};
use crate::client::teardown::{terminate, Shutdown, TaskTracker};
use crate::game::char_info::{
    parse_char_list_entry, CharListEntry, CHAR_SLOTS, RESPONSE_CHARLIST, RESPONSE_LOGIN,
};
use crate::game::dispatcher::{DispatchError, DispatcherState};
use crate::game::entry::{GameError, GameLink};
use crate::game::login::request_game_login;
use crate::game::world_entry::{
    request_select_char, request_village_struct, WorldEntryState, RESPONSE_SERVERINFO,
    RESPONSE_VILLAGESTRUCT, SERVICE_AREA_ENGLISH,
};
use crate::net::codec::Frame;

/// Environment variable naming the front service address (`host:port`).
pub const ENV_FRONT_ADDR: &str = "GOONZU_FRONT_ADDR";
/// Environment variable naming the game service address (`host:port`).
pub const ENV_GAME_ADDR: &str = "GOONZU_GAME_ADDR";
/// Environment variable carrying the account id.
pub const ENV_USER_ID: &str = "GOONZU_USER_ID";
/// Environment variable carrying the account password. Never logged.
pub const ENV_USER_PW: &str = "GOONZU_USER_PW";
/// Environment variable carrying the MAC address the front login echoes.
pub const ENV_MAC_ADDRESS: &str = "GOONZU_MAC_ADDRESS";
/// Environment variable selecting the character slot (`0..CHAR_SLOTS`).
pub const ENV_CHAR_SLOT: &str = "GOONZU_CHAR_SLOT";
/// Optional environment variable naming a village blueprint to request.
pub const ENV_VILLAGE_REF: &str = "GOONZU_VILLAGE_REF";
/// Environment variable overriding the teardown drain bound, in milliseconds.
pub const ENV_DRAIN_MS: &str = "GOONZU_DRAIN_MS";

/// Default front service address (`CONTRIBUTING_PHASE2.md`, Local Mode).
pub const DEFAULT_FRONT_ADDR: &str = "127.0.0.1:5700";
/// Default game service address (`CONTRIBUTING_PHASE2.md`, Local Mode).
pub const DEFAULT_GAME_ADDR: &str = "127.0.0.1:4010";
/// Default teardown drain bound: the Step 5.3 bound, reused rather than tuned.
pub const DEFAULT_DRAIN_BOUND: Duration = Duration::from_millis(5_000);

/// The stage a session reached. Ordered as the conversation runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// The front link was dialled (3.1).
    FrontConnected,
    /// The front handshake delivered its secret key (3.1).
    SecretKey,
    /// Notice and world list were requested and parsed (3.2).
    WorldList,
    /// The front login returned an `auth_key` (3.3).
    FrontLogin,
    /// The game link was dialled (4.1).
    GameConnected,
    /// The game handshake delivered `ClientCharUnique` (4.2).
    LinkKey,
    /// The opcode-2 game login was acknowledged (4.3).
    GameLogin,
    /// The roster was read and the requested slot was found (4.4).
    CharList,
    /// `REQUEST_SELECTCHAR` was sent (4.5).
    CharSelected,
    /// `RESPONSE_SERVERINFO` was applied (4.6).
    ServerInfo,
    /// The opcode-5 record populated the local `CharClient` (4.6).
    CharClient,
    /// The in-world conditions hold and the session ran (4.7 / 5.1).
    InWorld,
    /// Teardown returned; written only after it does.
    TornDown,
}

impl Stage {
    /// The stage's stable name, used in logs, errors and this crate's docs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrontConnected => "FrontConnected",
            Self::SecretKey => "SecretKey",
            Self::WorldList => "WorldList",
            Self::FrontLogin => "FrontLogin",
            Self::GameConnected => "GameConnected",
            Self::LinkKey => "LinkKey",
            Self::GameLogin => "GameLogin",
            Self::CharList => "CharList",
            Self::CharSelected => "CharSelected",
            Self::ServerInfo => "ServerInfo",
            Self::CharClient => "CharClient",
            Self::InWorld => "InWorld",
            Self::TornDown => "TornDown",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Everything the driver needs to open one session.
///
/// `Debug` is hand-written: the password must never reach a log line, an error
/// report or a panic message, and a derived `Debug` is the usual way it leaks.
#[derive(Clone, PartialEq, Eq)]
pub struct DriverConfig {
    /// The front service (`127.0.0.1:5700` by default).
    pub front: SocketAddr,
    /// The game service (`127.0.0.1:4010` by default).
    pub game: SocketAddr,
    /// The account id sent to both services.
    pub user_id: String,
    /// The account password. Never rendered by `Debug` or by any error.
    pub password: String,
    /// The MAC address the front login payload carries (3.3).
    pub mac_address: String,
    /// The roster slot to select (`0..CHAR_SLOTS`).
    pub char_slot: i32,
    /// The village blueprint to request, when the map is a village (4.7).
    pub village_ref: Option<i32>,
    /// How long teardown may wait for tracked tasks to end (5.3).
    pub drain_bound: Duration,
}

impl fmt::Debug for DriverConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriverConfig")
            .field("front", &self.front)
            .field("game", &self.game)
            .field("user_id", &self.user_id)
            .field("password", &"<redacted>")
            .field("mac_address", &self.mac_address)
            .field("char_slot", &self.char_slot)
            .field("village_ref", &self.village_ref)
            .field("drain_bound", &self.drain_bound)
            .finish()
    }
}

impl DriverConfig {
    /// Reads the process environment (`GOONZU_*`), resolving addresses through
    /// [`tokio::net::lookup_host`] so a tunnel host name works as well as an IP.
    ///
    /// # Errors
    ///
    /// As [`DriverConfig::from_lookup`].
    pub async fn from_env() -> Result<Self, ConfigError> {
        let lookup = |name: &str| std::env::var(name).ok();
        Self::from_lookup(lookup).await
    }

    /// Builds a configuration from an arbitrary lookup function — the seam the
    /// tests use so no test mutates the process environment.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Missing`] for an absent credential, [`ConfigError::NotANumber`]
    /// for a non-numeric slot or drain bound, [`ConfigError::SlotOutOfRange`] for a
    /// slot outside the roster, and [`ConfigError::Address`] when an address cannot
    /// be resolved.
    pub async fn from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let front = resolve(&lookup, ENV_FRONT_ADDR, DEFAULT_FRONT_ADDR).await?;
        let game = resolve(&lookup, ENV_GAME_ADDR, DEFAULT_GAME_ADDR).await?;
        let user_id = required(&lookup, ENV_USER_ID)?;
        let password = required(&lookup, ENV_USER_PW)?;
        let mac_address = required(&lookup, ENV_MAC_ADDRESS)?;

        let slot_raw = required(&lookup, ENV_CHAR_SLOT)?;
        let char_slot = slot_raw
            .trim()
            .parse::<i32>()
            .map_err(|_| ConfigError::NotANumber {
                name: ENV_CHAR_SLOT,
                value: slot_raw.clone(),
            })?;

        let village_ref = match lookup(ENV_VILLAGE_REF).filter(|v| !v.trim().is_empty()) {
            Some(raw) => Some(
                raw.trim()
                    .parse::<i32>()
                    .map_err(|_| ConfigError::NotANumber {
                        name: ENV_VILLAGE_REF,
                        value: raw.clone(),
                    })?,
            ),
            None => None,
        };

        let drain_bound = match lookup(ENV_DRAIN_MS).filter(|v| !v.trim().is_empty()) {
            Some(raw) => Duration::from_millis(raw.trim().parse::<u64>().map_err(|_| {
                ConfigError::NotANumber {
                    name: ENV_DRAIN_MS,
                    value: raw.clone(),
                }
            })?),
            None => DEFAULT_DRAIN_BOUND,
        };

        let config = Self {
            front,
            game,
            user_id,
            password,
            mac_address,
            char_slot,
            village_ref,
            drain_bound,
        };
        config.validate()?;
        Ok(config)
    }

    /// Checks the invariants [`run`] relies on, so a hand-built configuration
    /// fails the same way one read from the environment does.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Missing`] for an empty credential and
    /// [`ConfigError::SlotOutOfRange`] for a slot outside `0..CHAR_SLOTS`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.user_id.is_empty() {
            return Err(ConfigError::Missing { name: ENV_USER_ID });
        }
        if self.password.is_empty() {
            return Err(ConfigError::Missing { name: ENV_USER_PW });
        }
        if self.mac_address.is_empty() {
            return Err(ConfigError::Missing {
                name: ENV_MAC_ADDRESS,
            });
        }
        let max = CHAR_SLOTS as i32 - 1;
        if !(0..=max).contains(&self.char_slot) {
            return Err(ConfigError::SlotOutOfRange {
                name: ENV_CHAR_SLOT,
                value: self.char_slot,
                max,
            });
        }
        Ok(())
    }
}

/// Why a configuration could not be built.
///
/// No variant ever carries the password: a rejected credential is named, never
/// echoed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A required variable was absent or empty.
    #[error("{name} is required")]
    Missing {
        /// The variable's name.
        name: &'static str,
    },
    /// A variable was present but not the expected number.
    #[error("{name} is not a number: {value:?}")]
    NotANumber {
        /// The variable's name.
        name: &'static str,
        /// The rejected text — only ever used for non-secret variables.
        value: String,
    },
    /// The requested slot is outside the roster.
    #[error("{name} must be between 0 and {max}, got {value}")]
    SlotOutOfRange {
        /// The variable's name.
        name: &'static str,
        /// The rejected slot.
        value: i32,
        /// The highest valid slot.
        max: i32,
    },
    /// An address could not be resolved.
    #[error("{name} cannot be resolved: {detail}")]
    Address {
        /// The variable's name.
        name: &'static str,
        /// The resolver's own message.
        detail: String,
    },
}

/// Credentials and endpoints, redacted for logs.
fn required<F>(lookup: &F, name: &'static str) -> Result<String, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(name) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(ConfigError::Missing { name }),
    }
}

/// Resolves `name` (or its default) to one socket address.
async fn resolve<F>(
    lookup: &F,
    name: &'static str,
    default: &str,
) -> Result<SocketAddr, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let spec = lookup(name)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string());
    let spec = spec.trim();
    let mut addrs = tokio::net::lookup_host(spec)
        .await
        .map_err(|error| ConfigError::Address {
            name,
            detail: error.to_string(),
        })?;
    addrs.next().ok_or_else(|| ConfigError::Address {
        name,
        detail: format!("{spec} resolved to no address"),
    })
}

/// Why a session stopped.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// The configuration was rejected before any socket was opened.
    #[error("configuration: {0}")]
    Config(#[from] ConfigError),
    /// The session was stopped before the named stage ran.
    #[error("shutdown requested before {stage}")]
    Cancelled {
        /// The stage that would have run next.
        stage: Stage,
    },
    /// A stage refused to continue: a protocol or state condition failed, with
    /// no underlying error object to carry.
    #[error("{stage} refused: {detail}")]
    Refused {
        /// The stage that refused.
        stage: Stage,
        /// What was wrong, in the parser's or the driver's own words.
        detail: String,
    },
    /// A stage failed with an error from the step that owns it.
    #[error("{stage} failed: {source}")]
    Stage {
        /// The stage that failed.
        stage: Stage,
        /// The owning step's error, kept as the source chain.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl DriverError {
    /// Wraps an owning step's error under the stage it happened in.
    fn at<E>(stage: Stage, error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Stage {
            stage,
            source: Box::new(error),
        }
    }

    /// Records a refusal with no underlying error.
    fn refused(stage: Stage, detail: impl Into<String>) -> Self {
        Self::Refused {
            stage,
            detail: detail.into(),
        }
    }
}

/// What a session achieved before it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverOutcome {
    /// The stage reached; [`Stage::TornDown`] after a clean teardown.
    pub stage: Stage,
    /// How many worlds the front service advertised.
    pub worlds: usize,
    /// This connection's `MyCharUnique` (4.2).
    pub char_unique: i32,
    /// The slot that was selected.
    pub char_slot: i32,
    /// The village blueprint that was requested, when one was.
    pub village: Option<i32>,
}

/// Runs one complete session: front handshake through in-world, then teardown.
///
/// Teardown is not conditional on success — whichever way the flow ends, an open
/// game link is closed and drained before this function returns or propagates.
///
/// # Errors
///
/// [`DriverError::Config`] before any socket is touched, [`DriverError::Cancelled`]
/// when the shutdown flag is already claimed, [`DriverError::Stage`] carrying the
/// failing stage and the owning step's error, and [`DriverError::Refused`] when a
/// protocol or state condition stops the flow without an underlying error.
pub async fn run(cfg: &DriverConfig, shutdown: &Shutdown) -> Result<DriverOutcome, DriverError> {
    cfg.validate()?;

    let mut game: Option<GameLink> = None;
    let outcome = flow(cfg, shutdown, &mut game).await;

    if let Some(mut link) = game.take() {
        let tracker = TaskTracker::new();
        match terminate(&mut link, shutdown, &tracker, cfg.drain_bound).await {
            Ok(()) => {}
            Err(error) if outcome.is_err() => {
                warn!(error = %error, "teardown failed after an earlier failure");
            }
            Err(error) => return Err(DriverError::at(Stage::TornDown, error)),
        }
    }

    outcome.map(|mut outcome| {
        outcome.stage = Stage::TornDown;
        info!(
            stage = %outcome.stage,
            worlds = outcome.worlds,
            char_unique = outcome.char_unique,
            char_slot = outcome.char_slot,
            village = ?outcome.village,
            cause = ?shutdown.cause(),
            "session ended"
        );
        outcome
    })
}

/// The stages themselves. `game` is filled in as soon as the game link exists so
/// [`run`] can tear it down on every error path below.
async fn flow(
    cfg: &DriverConfig,
    shutdown: &Shutdown,
    game: &mut Option<GameLink>,
) -> Result<DriverOutcome, DriverError> {
    check_shutdown(shutdown, Stage::FrontConnected)?;

    let mut front = FrontLink::connect(cfg.front)
        .await
        .map_err(|error| DriverError::at(Stage::FrontConnected, error))?;
    let _secret = front
        .handshake()
        .await
        .map_err(|error| DriverError::at(Stage::SecretKey, error))?;
    debug!(stage = %Stage::SecretKey, "front handshake complete");
    check_shutdown(shutdown, Stage::SecretKey)?;

    request_notice(&mut front)
        .await
        .map_err(|error| DriverError::at(Stage::WorldList, error))?;
    let notice_frame = read_front(&mut front, Stage::WorldList).await?;
    let notice = parse_notice(notice_frame.cmd(), notice_frame.payload())
        .map_err(|error| DriverError::at(Stage::WorldList, error))?;
    debug!(notice_len = notice.len(), "notice received");

    request_worldlist(&mut front, &cfg.user_id)
        .await
        .map_err(|error| DriverError::at(Stage::WorldList, error))?;
    let world_frame = read_front(&mut front, Stage::WorldList).await?;
    let world_list = WorldList::from_frame(&world_frame)
        .map_err(|error| DriverError::at(Stage::WorldList, error))?;
    if world_list.worlds.is_empty() {
        return Err(DriverError::refused(
            Stage::WorldList,
            "the world list is empty",
        ));
    }
    let worlds = world_list.worlds.len();
    debug!(worlds, "world list received");
    check_shutdown(shutdown, Stage::WorldList)?;

    request_login(&mut front, &cfg.user_id, &cfg.password, &cfg.mac_address)
        .await
        .map_err(|error| DriverError::at(Stage::FrontLogin, error))?;
    let login_frame = read_front(&mut front, Stage::FrontLogin).await?;
    let login = parse_login_response(login_frame.cmd(), login_frame.payload())
        .map_err(|error| DriverError::at(Stage::FrontLogin, error))?;
    let auth = login
        .capture()
        .map_err(|error| DriverError::at(Stage::FrontLogin, error))?;
    // The front session is finished for this connection; the legacy closes it
    // before dialling the game service (3.3), and dropping it does the same.
    drop(front);
    check_shutdown(shutdown, Stage::FrontLogin)?;

    let link = GameLink::connect(cfg.game)
        .await
        .map_err(|error| DriverError::at(Stage::GameConnected, error))?;
    *game = Some(link);

    let unique = game_link_mut(game, Stage::LinkKey)?
        .handshake()
        .await
        .map_err(|error| DriverError::at(Stage::LinkKey, error))?;
    debug!(char_unique = unique.char_unique, "game handshake complete");
    check_shutdown(shutdown, Stage::LinkKey)?;

    request_game_login(game_link_mut(game, Stage::GameLogin)?, &auth, &cfg.password)
        .await
        .map_err(|error| DriverError::at(Stage::GameLogin, error))?;
    debug!(stage = %Stage::GameLogin, "game login submitted");
    check_shutdown(shutdown, Stage::GameLogin)?;

    let roster = read_roster(game_link_mut(game, Stage::CharList)?).await?;
    let slot_taken = roster
        .iter()
        .any(|entry| i32::from(entry.slot) == cfg.char_slot && !entry.name.trim().is_empty());
    if !slot_taken {
        return Err(DriverError::refused(
            Stage::CharList,
            format!("slot {} holds no character", cfg.char_slot),
        ));
    }
    check_shutdown(shutdown, Stage::CharList)?;

    request_select_char(game_link_mut(game, Stage::CharSelected)?, cfg.char_slot)
        .await
        .map_err(|error| DriverError::at(Stage::CharSelected, error))?;
    debug!(stage = %Stage::CharSelected, slot = cfg.char_slot, "character selected");
    check_shutdown(shutdown, Stage::CharSelected)?;

    let mut state = WorldEntryState::new(unique.char_unique);

    let server_info_frame =
        read_game(game_link_mut(game, Stage::ServerInfo)?, Stage::ServerInfo).await?;
    let accepted = state
        .apply_server_info(
            server_info_frame.cmd(),
            server_info_frame.payload(),
            SERVICE_AREA_ENGLISH,
        )
        .map_err(|error| DriverError::at(Stage::ServerInfo, error))?;
    if !accepted {
        warn!(
            stage = %Stage::ServerInfo,
            "the service-area banner was not for this build; state left untouched"
        );
    }
    check_shutdown(shutdown, Stage::ServerInfo)?;

    let char_frame = read_game(game_link_mut(game, Stage::CharClient)?, Stage::CharClient).await?;
    if !state
        .apply_login(char_frame.cmd(), char_frame.payload())
        .map_err(|error| DriverError::at(Stage::CharClient, error))?
    {
        return Err(DriverError::refused(
            Stage::CharClient,
            "the opcode-5 record belongs to another character",
        ));
    }
    let char_client = state
        .char_client()
        .ok_or_else(|| DriverError::refused(Stage::CharClient, "no character was adopted"))?
        .clone();
    check_shutdown(shutdown, Stage::CharClient)?;

    if let Some(village_ref) = cfg.village_ref {
        request_village_struct(game_link_mut(game, Stage::InWorld)?, village_ref)
            .await
            .map_err(|error| DriverError::at(Stage::InWorld, error))?;
        let village_frame = read_game(game_link_mut(game, Stage::InWorld)?, Stage::InWorld).await?;
        state
            .apply_village_struct(village_frame.cmd(), village_frame.payload())
            .map_err(|error| DriverError::at(Stage::InWorld, error))?;
    }

    if !state.is_in_world() {
        return Err(DriverError::refused(
            Stage::InWorld,
            "world-entry conditions are not satisfied (the banner was not accepted?)",
        ));
    }
    check_shutdown(shutdown, Stage::InWorld)?;

    let mut dispatcher = DispatcherState::new(i32::from(char_client.char_unique));
    dispatcher.adopt_local(&char_client);

    let link = game_link_mut(game, Stage::InWorld)?;
    let loop_result = run_with_shutdown(
        link,
        &[heartbeat_ticker()],
        shutdown,
        async |link, event| -> Result<Flow, InWorldError> {
            if is_heartbeat(&event) {
                send_heartbeat(link).await?;
                debug!("heartbeat sent");
                return Ok(Flow::Continue);
            }
            if let LoopEvent::Frame(frame) = &event {
                let outcome = dispatcher.dispatch(frame.cmd(), frame.payload())?;
                debug!(opcode = frame.cmd(), ?outcome, "in-world frame dispatched");
            }
            Ok(Flow::Continue)
        },
    )
    .await;

    if let Err(error) = loop_result {
        return match error {
            LoopError::Link(error) => Err(DriverError::at(Stage::InWorld, error)),
            LoopError::Handler(error) => {
                Err(DriverError::refused(Stage::InWorld, error.to_string()))
            }
        };
    }
    info!(cause = ?shutdown.cause(), "in-world session ended");

    Ok(DriverOutcome {
        stage: Stage::InWorld,
        worlds,
        char_unique: unique.char_unique,
        char_slot: cfg.char_slot,
        village: cfg.village_ref,
    })
}

/// A handler failure inside the in-world loop: either the dispatcher or the
/// heartbeat, both of which must stop the session rather than be swallowed.
#[derive(Debug, thiserror::Error)]
enum InWorldError {
    /// A dispatcher parser refused a frame.
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    /// The heartbeat could not be written.
    #[error(transparent)]
    Heartbeat(#[from] GameError),
}

/// Borrows the game link, or refuses: every stage after 4.1 needs it open.
fn game_link_mut(game: &mut Option<GameLink>, stage: Stage) -> Result<&mut GameLink, DriverError> {
    game.as_mut()
        .ok_or_else(|| DriverError::refused(stage, "the game link is not open"))
}

/// Stops before a stage if the shutdown flag is already claimed.
fn check_shutdown(shutdown: &Shutdown, stage: Stage) -> Result<(), DriverError> {
    if shutdown.is_triggered() {
        return Err(DriverError::Cancelled { stage });
    }
    Ok(())
}

/// Reads exactly one frame from the front link, or reports a closed peer.
async fn read_front(front: &mut FrontLink, stage: Stage) -> Result<Frame, DriverError> {
    front
        .next_frame()
        .await
        .map_err(|error| DriverError::at(stage, error))?
        .ok_or_else(|| DriverError::refused(stage, "the front service closed before responding"))
}

/// Reads exactly one frame from the game link, or reports a closed peer.
async fn read_game(link: &mut GameLink, stage: Stage) -> Result<Frame, DriverError> {
    link.next_frame()
        .await
        .map_err(|error| DriverError::at(stage, error))?
        .ok_or_else(|| DriverError::refused(stage, "the game service closed before responding"))
}

/// Reads the full roster: the service answers a successful login with one
/// `cltCharListInfo` frame per slot, zeroed where a slot is empty (4.4).
async fn read_roster(link: &mut GameLink) -> Result<Vec<CharListEntry>, DriverError> {
    let mut roster = Vec::with_capacity(CHAR_SLOTS);
    for _ in 0..CHAR_SLOTS {
        let frame = read_game(link, Stage::CharList).await?;
        if frame.cmd() != RESPONSE_CHARLIST {
            return Err(DriverError::refused(
                Stage::CharList,
                format!("expected opcode {RESPONSE_CHARLIST}, got {}", frame.cmd()),
            ));
        }
        roster.push(
            parse_char_list_entry(frame.cmd(), frame.payload())
                .map_err(|error| DriverError::at(Stage::CharList, error))?,
        );
    }
    Ok(roster)
}

/// The opcodes this module names, kept together so the sequence above reads as
/// the conversation it is.
const _: () = {
    assert!(RESPONSE_NOTICE == 1001);
    assert!(RESPONSE_WORLDLIST == 1002);
    assert!(RESPONSE_SERVERINFO == 106);
    assert!(RESPONSE_LOGIN == 5);
    assert!(RESPONSE_VILLAGESTRUCT == 1102);
    assert!(RESPONSE_CHARLIST == 3);
};

#[cfg(test)]
mod tests;
