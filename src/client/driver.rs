//! Step 6.1 — the end-to-end client driver (`CONTRIBUTING_PHASE2.md`, "Step 6.1").
//!
//! Steps 1.1–5.4 delivered every leg of the session as isolated, QC'd modules;
//! [`run`] is the first code that assembles them into one continuous
//! conversation — the port's counterpart of the legacy's boot path
//! (`main.cpp` → `LogIn.cpp` → the message pump). `src/main.rs` is this
//! module's thin environment-configured shell: config + logging + runtime +
//! exit codes, nothing else.
//!
//! # The session this driver runs, in wire order
//!
//! ```text
//! front service                                game service
//!   |  SECRETKEY handshake (30000, 5 B)          |
//!   |<-------------------------------------------|
//!   |  REQUEST_NOTICE (2) / REQUEST_WORLDLIST (3)|  session-ciphered
//!   |------------------------------------------>|
//!   |  RESPONSE_NOTICE (1001) / WORLDLIST (1002) |  plaintext, n x 428 B
//!   |<-------------------------------------------|
//!   |  REQUEST_LOGIN (1, 60 B)                   |  session-ciphered
//!   |------------------------------------------>|
//!   |  RESPONSE_LOGIN (1000, 108 B) -> auth_key  |  plaintext
//!   |<-------------------------------------------|
//!   --- front session closed (Client-Winsock.cpp:788) ---
//!   |                    RESPONSE_CLIENTCHARUNIQUE (1, 8 B, link cipher) |
//!   |                   <---------------------------------------------|
//!   |  REQUEST_LOGIN (2, 48 B, session cipher)   |
//!   |------------------------------------------>|
//!   |  RESPONSE_CHARLIST (3) x CHAR_SLOTS, 984 B |
//!   |<-------------------------------------------|
//!   |  REQUEST_SELECTCHAR (4, slot SI32)         |  MsgType.h:2981 (9 B frame)
//!   |------------------------------------------>|
//!   |  RESPONSE_SERVERINFO (106, 48 B) then RESPONSE_LOGIN (5, 592 B) |
//!   |<-------------------------------------------|
//!   |  REQUEST_VILLAGESTRUCT (1101, ref SI32)    |  only when a ref is supplied
//!   |------------------------------------------>|
//!   |  RESPONSE_VILLAGESTRUCT (1102, 22 B)       |
//!   |<-------------------------------------------|
//!   |  ... in-world: dispatcher frames (6/11/118/200/215) + heartbeat (100)
//! ```
//!
//! Every constant above is the one its owning module already QC'd (front flow
//! 3.1–3.3, game flow 4.1–4.7, dispatcher 5.4); this driver **consumes** those
//! modules' public APIs and adds no parsing of its own — a wire fact is never
//! re-implemented here, only routed.
//!
//! # Configuration (the binary's environment contract)
//!
//! | Variable | Meaning | Default |
//! |---|---|---|
//! | `GOONZU_FRONT` | front service `host:port` (DNS names allowed — ngrok) | `127.0.0.1:5700` |
//! | `GOONZU_GAME` | game service `host:port` (used when `GOONZU_WORLD_ID` is unset) | `127.0.0.1:4010` |
//! | `GOONZU_USER` | account name (required) | — |
//! | `GOONZU_PASS` | password (required) | — |
//! | `GOONZU_WORLD_ID` | pick this world from the worldlist and dial the address the worldlist advertises | unset: dial `GOONZU_GAME` |
//! | `GOONZU_CHAR_SLOT` | character slot to select (`0..` the dialog's own range, `NSelectChar.cpp:1292`) | `0` |
//! | `GOONZU_VILLAGE_REF` | current map's `siVillageUnique` for the blueprint exchange (`Client-Login.cpp:281`); `0` or unset = not a village, request nothing (`Client-Login.cpp:279`) | unset |
//!
//! The timing knobs ([`DriverConfig::heartbeat_interval`],
//! [`DriverConfig::step_timeout`], [`DriverConfig::drain_bound`]) default to
//! production values and exist as fields — not environment variables — so the
//! hermetic harness can run the same `run` at scaled cadence. The MAC-address
//! field of the 60-byte front login body (`FrontMsg.h:344-347`) is sent as a
//! fixed default: reading a real MAC portably is out of scope, and the field's
//! server-side semantics are not verifiable from the sources this port dual
//! sources from — it is carried for layout parity, nothing more.
//!
//! # The two caller-supplied boundaries (stated, not stubbed)
//!
//! Two values the legacy reads from data this port does not yet hold remain
//! **caller inputs**, exactly as Steps 4.5–4.7 declared them:
//!
//! - the **game address**: the legacy dials the ip:port the worldlist entry
//!   advertises (`WorldInfo::game_server_ip`/`game_server_port`, "where the
//!   client dials next", `src/auth/world_list.rs`); setting `GOONZU_WORLD_ID`
//!   exercises that path, otherwise the operator supplies `GOONZU_GAME`.
//! - the **village ref**: map data (`MapData.siVillageUnique`) is a separate
//!   module with its own wire truth (`src/game/world_entry.rs`, "the village
//!   ref comes from map data"); this driver takes it from configuration and
//!   performs the same strict-range request the module builds.
//!
//! # What this driver deliberately does not do
//!
//! - **No wire parsing.** Frames are parsed only by the modules that own them;
//!   the driver routes by opcode and hands payloads to their parser. A parser
//!   bug can therefore never hide inside the driver, and the driver adds no
//!   surface for one.
//! - **No new mocks, no new expectations.** The hermetic proof of this
//!   orchestration is Step 6.2's `tests/e2e_loopback.rs`, whose wire doubles
//!   hand-place every frame from the byte fixtures the per-module suites
//!   already cite (F-07: an expectation never comes from the code under test).
//! - **No fabricated map-data or MAC lookups** (above).
//!
//! # Failure semantics
//!
//! Every wait is bounded by [`DriverConfig::step_timeout`] and tagged with the
//! [`Phase`] it stalled in; an EOF, a foreign opcode, or a service-area
//! mismatch surfaces as a [`DriverError`] rather than a hang or a silent
//! skip — invariant 2 ("never hide failures"). Two deliberate deviations from
//! the legacy's *silence*, both documented at their site:
//!
//! - The legacy's SERVERINFO handler drops a wrong-region banner with a bare
//!   `return` (`Client-System.cpp:1682`); this driver reports it as
//!   [`DriverError::ServiceAreaMismatch`] because a driver whose job is a
//!   complete session cannot treat "cannot enter this world" as a non-event.
//! - During the entry waits, the five dispatcher opcodes (6/11/118/200/215 —
//!   the `ClientMessageMap` rows `dispatcher.rs` documents) are dispatched as
//!   they arrive instead of being refused: the legacy routes them at any time
//!   after login, so a server legally interleaves them with the entry burst.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use thiserror::Error;
use tokio::net::lookup_host;

use crate::auth::front::{self, FrontError, FrontLink};
use crate::auth::login::{self, request_login, LoginError, LoginResponse};
use crate::auth::world_list::{
    parse_notice, request_notice, request_worldlist, WorldInfo, WorldList, WorldListError,
    RESPONSE_NOTICE, RESPONSE_WORLDLIST,
};
use crate::client::event_loop::{run_with_shutdown, Flow, LoopError, LoopEvent, Ticker};
use crate::client::heartbeat::{send_heartbeat, HEARTBEAT_INTERVAL, HEARTBEAT_TICKER_NAME};
use crate::client::teardown::{
    spawn_ctrl_c_watcher, terminate, Cause, Shutdown, TaskTracker, TeardownError,
};
use crate::game::char_info::{self, CharInfoError, CharListEntry, CHAR_SLOTS, RESPONSE_CHARLIST};
use crate::game::dispatcher::{self, dispatch_frame, DispatchError, DispatcherState};
use crate::game::entry::{self, GameError, GameLink};
use crate::game::login::{request_game_login, GameLoginError};
use crate::game::world_entry::{
    parse_server_info, request_select_char, request_village_struct, WorldEntryError,
    WorldEntryState, RESPONSE_SERVERINFO, RESPONSE_VILLAGESTRUCT, SERVICE_AREA_ENGLISH,
};

/// Default bound on any single conversation wait (a request-to-response gap).
///
/// Matches the scale of the game service's own handshake deadline
/// (`services/game/src/config.rs`, 10 s — `tests/live_integration.rs:155-172`);
/// a slower deployment fails loudly instead of hanging the process.
pub const DEFAULT_STEP_TIMEOUT: Duration = Duration::from_secs(10);

/// Default bound on the task drain at teardown (Step 5.3's acceptance window).
pub const DEFAULT_DRAIN_BOUND: Duration = Duration::from_secs(5);

/// The MAC-address slot of the 60-byte front login body (`FrontMsg.h:344-347`).
///
/// A fixed placeholder: see the module docs ("configuration"). The legacy
/// transmits the machine's real MAC; this port neither reads one (no portable,
/// dependency-free source) nor verifies any server-side use of it.
const DEFAULT_MAC_ADDRESS: &str = "000000000000";

/// Which leg of the session a wait (or a failure) belongs to.
///
/// Carried in every contextual [`DriverError`] so a stalled session names the
/// exact conversation that stopped, not just "somewhere after connect".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// `FRONTMSG_RESPONSE_SECRETKEY` handshake (30000).
    FrontHandshake,
    /// Notice/worldlist exchange (2/3 -> 1001/1002).
    WorldList,
    /// Front login and the `auth_key` response (1 -> 1000).
    FrontLogin,
    /// Game connect and the opcode-1 `ClientCharUnique` handshake.
    GameHandshake,
    /// Opcode-2 game login submission.
    GameLogin,
    /// The `CHAR_SLOTS` roster frames (opcode 3).
    CharList,
    /// The opcode-4 select-character request.
    SelectChar,
    /// The entry burst: SERVERINFO (106) then the opcode-5 record.
    WorldEntry,
    /// The optional village blueprint exchange (1101/1102).
    VillageBlueprint,
    /// The in-world event loop (dispatcher + heartbeat).
    InWorld,
}

/// A failure of the orchestrated session, tagged with the phase it occurred in.
#[derive(Debug, Error)]
pub enum DriverError {
    /// The front link itself failed (connect, handshake, send, decode).
    #[error(transparent)]
    Front(#[from] FrontError),
    /// The worldlist/notice conversation failed.
    #[error(transparent)]
    WorldList(#[from] WorldListError),
    /// The front login conversation failed.
    #[error(transparent)]
    Login(#[from] LoginError),
    /// The game link itself failed (connect, handshake, send, decode, socket).
    #[error(transparent)]
    Game(#[from] GameError),
    /// The opcode-2 game login submission failed.
    #[error(transparent)]
    GameLogin(#[from] GameLoginError),
    /// A roster or entry record failed its owning parser's strict gates.
    #[error(transparent)]
    CharInfo(#[from] CharInfoError),
    /// A world-entry request or record failed.
    #[error(transparent)]
    WorldEntry(#[from] WorldEntryError),
    /// An in-world frame failed the dispatcher's gates.
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    /// The Step 5.3 teardown failed (close or drain).
    #[error(transparent)]
    Teardown(#[from] TeardownError),
    /// The peer closed before the awaited frame arrived.
    #[error("phase {phase:?}: the server closed the connection before the expected frame arrived")]
    ServerClosed {
        /// The conversation that was still awaiting a frame.
        phase: Phase,
    },
    /// No matching frame within the configured budget.
    #[error("phase {phase:?}: no matching frame within {budget:?}")]
    StepTimeout {
        /// The conversation that stalled.
        phase: Phase,
        /// The budget that elapsed.
        budget: Duration,
    },
    /// A frame arrived whose opcode the phase does not cover — dispatch drift,
    /// or a server speaking a contract this port does not model.
    #[error("phase {phase:?}: unexpected opcode {cmd}")]
    UnexpectedOpcode {
        /// The conversation that was in progress.
        phase: Phase,
        /// The opcode that arrived instead.
        cmd: u16,
    },
    /// The SERVERINFO banner was dropped by the legacy region gate
    /// (`Client-System.cpp:1682`): its `siServiceArea` is not this build's
    /// [`SERVICE_AREA_ENGLISH`]. The legacy returns silently; a driver that
    /// owes a complete session reports the impossibility instead.
    #[error(
        "phase {phase:?}: SERVERINFO dropped by the region gate (this build siServiceArea \
         {client:#04x}, banner {banner:#04x})"
    )]
    ServiceAreaMismatch {
        /// This build's service area.
        client: i16,
        /// The banner's service area.
        banner: i16,
        /// The conversation that was in progress.
        phase: Phase,
    },
    /// The configured slot holds no character in the received roster.
    #[error("roster offers no character in slot {slot} (received slots: {offered:?})")]
    SlotEmpty {
        /// The configured slot.
        slot: i32,
        /// Every slot the roster frames carried.
        offered: Vec<u8>,
    },
    /// `GOONZU_WORLD_ID` names a world the worldlist did not carry.
    #[error("world {world_id} is not in the worldlist ({seen} world(s) received)")]
    WorldNotFound {
        /// The configured world id.
        world_id: u16,
        /// How many worlds the worldlist carried.
        seen: usize,
    },
    /// The selected world's advertised ip:port is not dialable (not an
    /// `ip:port` this resolver can form — the legacy would hand the string to
    /// `connect` and fail there instead).
    #[error("world {world_id} advertises an unusable address {ip}:{port}")]
    WorldAddress {
        /// The world whose entry carried the bad address.
        world_id: u16,
        /// The advertised ip string.
        ip: String,
        /// The advertised port.
        port: u16,
    },
    /// The entry conversation finished without satisfying the derived
    /// in-world condition (banner + local character + no outstanding
    /// blueprint — `src/game/world_entry.rs`).
    #[error("the entry conversation completed without reaching the in-world condition")]
    NotInWorld,
}

/// A failure to build a [`DriverConfig`] from the environment.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required variable is absent.
    #[error("required environment variable {name} is not set")]
    Missing {
        /// The variable's name.
        name: &'static str,
    },
    /// A variable's value is not valid Unicode.
    #[error("environment variable {name} is not valid Unicode")]
    NotUnicode {
        /// The variable's name.
        name: &'static str,
    },
    /// An integer variable's value does not parse.
    #[error("{name}={value:?} is not a valid {kind}: {source}")]
    Parse {
        /// The variable's name.
        name: &'static str,
        /// The literal value.
        value: String,
        /// What the value should have been.
        kind: &'static str,
        /// The parse failure.
        source: std::num::ParseIntError,
    },
    /// An address variable is not `host:port`.
    #[error("{name}={spec:?} is not host:port")]
    NotHostPort {
        /// The variable's name.
        name: &'static str,
        /// The literal value.
        spec: String,
    },
    /// `host:port` did not resolve (DNS failure or zero addresses).
    #[error("{name}={spec:?} did not resolve to an address: {source}")]
    Resolve {
        /// The variable's name.
        name: &'static str,
        /// The literal value.
        spec: String,
        /// The resolver failure.
        source: std::io::Error,
    },
}

/// Everything the driver needs to run one session (module docs,
/// "configuration").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverConfig {
    /// The front service address.
    pub front_addr: SocketAddr,
    /// The game service address, used when [`Self::world_id`] is `None`
    /// (otherwise the selected world's advertised address is dialled).
    pub game_addr: SocketAddr,
    /// The account name — carried into the worldlist request and both logins.
    pub user: String,
    /// The password — front login body and the opcode-2 game login.
    pub password: String,
    /// The 60-byte front login body's third credential slot (the module's
    /// `DEFAULT_MAC_ADDRESS`, layout parity only).
    pub mac_address: String,
    /// Select this world from the worldlist and dial its advertised address.
    pub world_id: Option<u16>,
    /// The character slot to select (`0..` the dialog's range).
    pub char_slot: i32,
    /// The current map's `siVillageUnique`, or `None`/`0` for "not a
    /// village" — the legacy sentinel that requests nothing
    /// (`Client-Login.cpp:279`).
    pub village_ref: Option<i32>,
    /// The in-world heartbeat cadence. Production value:
    /// [`HEARTBEAT_INTERVAL`] (10 s, Step 5.2); the field exists so the
    /// hermetic harness can soak at scaled cadence (Step 6.3).
    pub heartbeat_interval: Duration,
    /// The bound on each conversation wait (module docs, failure semantics).
    pub step_timeout: Duration,
    /// The bound on the teardown task drain (Step 5.3's acceptance window).
    pub drain_bound: Duration,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            front_addr: SocketAddr::from(([127, 0, 0, 1], front::DEFAULT_PORT)),
            game_addr: SocketAddr::from(([127, 0, 0, 1], entry::DEFAULT_PORT)),
            user: String::new(),
            password: String::new(),
            mac_address: DEFAULT_MAC_ADDRESS.to_owned(),
            world_id: None,
            char_slot: 0,
            village_ref: None,
            heartbeat_interval: HEARTBEAT_INTERVAL,
            step_timeout: DEFAULT_STEP_TIMEOUT,
            drain_bound: DEFAULT_DRAIN_BOUND,
        }
    }
}

impl DriverConfig {
    /// Builds the configuration from the environment (module docs,
    /// "configuration"). DNS names resolve, so a per-session ngrok tunnel
    /// works exactly as it does for `tests/live_integration.rs`.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] for a missing required variable, a non-`host:port`
    /// address, an unresolvable host, or an integer that does not parse —
    /// nothing is guessed and no default stands in for a *required* value.
    pub async fn from_env() -> Result<Self, ConfigError> {
        let world_id = match optional_env("GOONZU_WORLD_ID")? {
            Some(raw) => Some(parse_env_int("GOONZU_WORLD_ID", &raw)?),
            None => None,
        };
        let char_slot = match optional_env("GOONZU_CHAR_SLOT")? {
            Some(raw) => parse_env_int("GOONZU_CHAR_SLOT", &raw)?,
            None => 0,
        };
        let village_ref = match optional_env("GOONZU_VILLAGE_REF")? {
            Some(raw) => Some(parse_env_int("GOONZU_VILLAGE_REF", &raw)?),
            None => None,
        };
        Ok(Self {
            front_addr: resolve_env("GOONZU_FRONT", format!("127.0.0.1:{}", front::DEFAULT_PORT))
                .await?,
            game_addr: resolve_env("GOONZU_GAME", format!("127.0.0.1:{}", entry::DEFAULT_PORT))
                .await?,
            user: required_env("GOONZU_USER")?,
            password: required_env("GOONZU_PASS")?,
            world_id,
            char_slot,
            village_ref,
            ..Self::default()
        })
    }
}

/// What one completed session produced — every load-bearing observable of the
/// run, for logging and for the Step 6.2 harness to assert against.
#[derive(Debug)]
pub struct SessionReport {
    /// The front's `SECRETKEY` seeds (payload offsets 2/4).
    pub front_seed: (u8, u8),
    /// The connection's allocated `char_unique` (opcode 1).
    pub char_unique: i32,
    /// The negotiated session-cipher seeds (opcode 1, `key1`/`key2`).
    pub session_seed: (u8, u8),
    /// The server's version field from the handshake.
    pub server_version: i16,
    /// The account the front login confirmed
    /// ([`crate::auth::login::FrontAuth::user_id`]).
    pub user_id: String,
    /// The selected world's worldlist entry, when `world_id` was configured.
    pub world: Option<WorldInfo>,
    /// How many worlds the worldlist carried.
    pub worlds_seen: usize,
    /// The full parsed roster (`CHAR_SLOTS` entries, empty slots included).
    pub roster: Vec<CharListEntry>,
    /// The slot that was selected.
    pub selected_slot: i32,
    /// The selected character's name, as the roster advertised it.
    pub selected_name: String,
    /// The Step 4.5–4.7 entry state after the session (banner, local
    /// [`char_info::MakeCharInfo`]-derived client, blueprint wait).
    pub entry: WorldEntryState,
    /// The Step 5.4 dispatcher state after the session (local delta + nearby
    /// table as the in-world frames left it).
    pub dispatcher: DispatcherState,
    /// In-world frames handed to the dispatcher.
    pub frames_dispatched: u32,
    /// Heartbeat ticks actually sent.
    pub heartbeats_sent: u64,
    /// Why the session ended (Step 5.3's first-wins cause).
    pub cause: Cause,
}

/// Runs one complete session: front flow (3.1–3.3) -> game flow (4.1–4.4) ->
/// world entry (4.5–4.7) -> in-world loop (5.1 + 5.2 + 5.4) -> teardown (5.3).
///
/// The orchestration lives here (library, not binary) precisely so Step 6.2's
/// hermetic harness can drive this exact code path end to end.
///
/// # Errors
///
/// [`DriverError`] for every failure mode; each contextual variant names the
/// [`Phase`] it occurred in. A session that reaches in-world and then loses
/// the link surfaces as [`DriverError::Game`] (the loop records
/// [`Cause::ServerDisconnect`] on the shutdown flag before propagating).
pub async fn run(cfg: &DriverConfig) -> Result<SessionReport, DriverError> {
    // ---------------- front service: handshake, worldlist, login ----------------
    let mut front = FrontLink::connect(cfg.front_addr).await?;
    let secret = front.handshake().await?;
    let front_seed = (secret.key1(), secret.key2());
    tracing::info!(
        addr = %cfg.front_addr,
        key1 = front_seed.0,
        key2 = front_seed.1,
        "front handshake complete"
    );

    let worldlist = within(cfg.step_timeout, Phase::WorldList, async {
        // The notice is the splash the legacy shows while the list loads; the
        // responses may arrive in either order, so each is consumed by opcode
        // and the wait ends at the worldlist (README Step 3.2).
        request_notice(&mut front).await?;
        request_worldlist(&mut front, &cfg.user).await?;
        loop {
            let Some(frame) = front.next_frame().await? else {
                return Err(DriverError::ServerClosed {
                    phase: Phase::WorldList,
                });
            };
            match frame.cmd_id() {
                RESPONSE_NOTICE => {
                    let notice = parse_notice(frame.cmd(), frame.payload())?;
                    tracing::info!(%notice, "front notice");
                }
                RESPONSE_WORLDLIST => return Ok(WorldList::from_frame(&frame)?),
                cmd => tracing::debug!(
                    cmd,
                    "front frame ignored while awaiting the worldlist (the legacy's front map routes more opcodes)"
                ),
            }
        }
    })
    .await?;
    let worlds_seen = worldlist.worlds.len();

    let world = match cfg.world_id {
        None => None,
        Some(world_id) => Some(
            worldlist
                .worlds
                .into_iter()
                .find(|world| world.world_id == world_id)
                .ok_or(DriverError::WorldNotFound {
                    world_id,
                    seen: worlds_seen,
                })?,
        ),
    };
    let game_addr = match &world {
        Some(world) => world_addr(world)?,
        None => cfg.game_addr,
    };

    request_login(&mut front, &cfg.user, &cfg.password, &cfg.mac_address).await?;
    let auth = within(cfg.step_timeout, Phase::FrontLogin, async {
        loop {
            let Some(frame) = front.next_frame().await? else {
                return Err(DriverError::ServerClosed {
                    phase: Phase::FrontLogin,
                });
            };
            if frame.cmd_id() == login::RESPONSE_LOGIN {
                return Ok(LoginResponse::from_frame(&frame)?.capture()?);
            }
            tracing::debug!(
                cmd = frame.cmd_id(),
                "front frame ignored while awaiting the login response"
            );
        }
    })
    .await?;
    tracing::info!(user_id = %auth.user_id(), "front login ok");

    // The legacy closes the front session before dialling the game server
    // (`pFrontSession->CloseSocket`, Client-Winsock.cpp:788); dropping the
    // link here produces the same FIN.
    drop(front);

    // ---------------- game service: handshake, login, roster ----------------
    let mut game = GameLink::connect(game_addr).await?;
    let unique = game.handshake().await?;
    tracing::info!(
        addr = %game_addr,
        char_unique = unique.char_unique,
        key1 = unique.key1,
        key2 = unique.key2,
        version = unique.version,
        "game handshake complete"
    );

    request_game_login(&mut game, &auth, &cfg.password).await?;

    // The server answers a game login with CHAR_SLOTS roster frames (its
    // `send_char_list`; the client AND-reduces readiness across the same set),
    // each the 984-byte `CharListEntry` of Step 4.4.
    let roster = within(cfg.step_timeout, Phase::CharList, async {
        let mut roster = Vec::with_capacity(CHAR_SLOTS);
        while roster.len() < CHAR_SLOTS {
            let Some(frame) = game.next_frame().await? else {
                return Err(DriverError::ServerClosed {
                    phase: Phase::CharList,
                });
            };
            if frame.cmd_id() != RESPONSE_CHARLIST {
                return Err(DriverError::UnexpectedOpcode {
                    phase: Phase::CharList,
                    cmd: frame.cmd_id(),
                });
            }
            roster.push(CharListEntry::from_frame(&frame)?);
        }
        Ok(roster)
    })
    .await?;
    tracing::info!(frames = roster.len(), "character roster received");

    // ---------------- world entry: select, banner, character, blueprint ----------------
    let (selected_slot, selected_name) = {
        let selected = roster
            .iter()
            .find(|character| {
                i32::from(character.slot) == cfg.char_slot && !character.name.is_empty()
            })
            .ok_or_else(|| DriverError::SlotEmpty {
                slot: cfg.char_slot,
                offered: roster.iter().map(|character| character.slot).collect(),
            })?;
        (i32::from(selected.slot), selected.name.clone())
    };
    request_select_char(&mut game, selected_slot).await?;
    tracing::info!(slot = selected_slot, name = %selected_name, "select sent");

    let mut entry = WorldEntryState::new(unique.char_unique);
    // Dispatcher state exists from the first entry frame: the five in-world
    // opcodes are routed by the legacy's message map whenever they arrive, so
    // the entry waits dispatch them instead of refusing a legal frame.
    let mut dispatcher = DispatcherState::new(unique.char_unique);

    within(cfg.step_timeout, Phase::WorldEntry, async {
        let mut banner = false;
        let mut loaded = false;
        while !(banner && loaded) {
            let Some(frame) = game.next_frame().await? else {
                return Err(DriverError::ServerClosed {
                    phase: Phase::WorldEntry,
                });
            };
            if is_dispatchable(frame.cmd_id()) {
                dispatch_frame(&mut dispatcher, &frame)?;
                continue;
            }
            match frame.cmd_id() {
                RESPONSE_SERVERINFO if !banner => {
                    if !entry.apply_server_info(
                        frame.cmd(),
                        frame.payload(),
                        SERVICE_AREA_ENGLISH,
                    )? {
                        // The gate dropped it (Client-System.cpp:1682). The
                        // banner is re-parsed here *only* to name its region
                        // in the failure — the happy path parses exactly once.
                        let dropped = parse_server_info(frame.cmd(), frame.payload())?;
                        return Err(DriverError::ServiceAreaMismatch {
                            client: SERVICE_AREA_ENGLISH,
                            banner: dropped.service_area,
                            phase: Phase::WorldEntry,
                        });
                    }
                    banner = true;
                }
                char_info::RESPONSE_LOGIN if !loaded => {
                    // Ok(false) is another character's record — the legacy
                    // returns before SetChar (Client-Login.cpp:116) — so the
                    // wait continues; a parse failure propagates.
                    loaded = entry.apply_login(frame.cmd(), frame.payload())?;
                }
                cmd => {
                    return Err(DriverError::UnexpectedOpcode {
                        phase: Phase::WorldEntry,
                        cmd,
                    });
                }
            }
        }
        Ok(())
    })
    .await?;

    if let Some(village_ref) = cfg.village_ref.filter(|&village_ref| village_ref != 0) {
        // Ref 0 is the legacy's "not a village" sentinel and is never sent
        // (`Client-Login.cpp:279`); the module's range gate refuses it too.
        request_village_struct(&mut game, village_ref).await?;
        entry.mark_village_struct_requested();
        within(cfg.step_timeout, Phase::VillageBlueprint, async {
            loop {
                let Some(frame) = game.next_frame().await? else {
                    return Err(DriverError::ServerClosed {
                        phase: Phase::VillageBlueprint,
                    });
                };
                if is_dispatchable(frame.cmd_id()) {
                    dispatch_frame(&mut dispatcher, &frame)?;
                    continue;
                }
                if frame.cmd_id() == RESPONSE_VILLAGESTRUCT {
                    entry.apply_village_struct(frame.cmd(), frame.payload())?;
                    return Ok(());
                }
                return Err(DriverError::UnexpectedOpcode {
                    phase: Phase::VillageBlueprint,
                    cmd: frame.cmd_id(),
                });
            }
        })
        .await?;
    }

    if !entry.is_in_world() {
        return Err(DriverError::NotInWorld);
    }
    if let Some(local) = entry.char_client() {
        dispatcher.adopt_local(local);
    }
    tracing::info!(
        char_unique = entry.my_char_unique(),
        village_pending = entry.village_struct_pending(),
        "in-world condition satisfied"
    );

    // ---------------- in-world: event loop + heartbeat + dispatcher ----------------
    let shutdown = Shutdown::new();
    let tracker = TaskTracker::new();
    let watcher = spawn_ctrl_c_watcher(shutdown.clone(), &tracker);
    // The production 10 s heartbeat (Step 5.2) under its identity constant, so
    // `is_heartbeat`'s name-based routing applies unchanged; the interval is a
    // field so the hermetic soak (Step 6.3) can scale cadence without touching
    // the identity.
    let tickers = [Ticker::new(HEARTBEAT_TICKER_NAME, cfg.heartbeat_interval)];
    let mut heartbeats_sent = 0u64;
    let mut frames_dispatched = 0u32;

    let loop_result = run_with_shutdown(
        &mut game,
        &tickers,
        &shutdown,
        async |link, ev| -> Result<Flow, DriverError> {
            match ev {
                LoopEvent::Frame(frame) => {
                    dispatch_frame(&mut dispatcher, &frame)?;
                    frames_dispatched += 1;
                    tracing::debug!(cmd = frame.cmd_id(), "in-world frame dispatched");
                    Ok(Flow::Continue)
                }
                LoopEvent::Ticked { name, .. } if name == HEARTBEAT_TICKER_NAME => {
                    // A write failure stops the loop through
                    // `LoopError::Handler`, the legacy's abandon-on-send-failure
                    // (`heartbeat.rs`, send_heartbeat docs).
                    send_heartbeat(link).await?;
                    heartbeats_sent += 1;
                    Ok(Flow::Continue)
                }
                LoopEvent::Ticked { .. } => Ok(Flow::Continue),
            }
        },
    )
    .await;
    match loop_result {
        Ok(()) => {}
        Err(LoopError::Link(source)) => return Err(DriverError::Game(source)),
        Err(LoopError::Handler(source)) => return Err(source),
    }

    // ---------------- teardown (Step 5.3) ----------------
    // The watcher parks on the console event itself — it does not observe the
    // shutdown flag (`watch_signal`, teardown.rs) — so once the loop is over
    // it must be aborted before `terminate`'s bounded drain, or the drain
    // would wait out its whole bound on a task that can never finish
    // (`spawn_ctrl_c_watcher` docs: "drain with a bound rather than assume").
    watcher.abort();
    match watcher.await {
        // Finished before the abort: the console interrupt claimed the flag.
        Ok(Some(cause)) => tracing::info!(?cause, "ctrl+c claimed the shutdown"),
        // Registration failed: the session could only ever end server-side.
        Ok(None) => tracing::warn!(
            "ctrl+c registration failed; this session can only end via server disconnect"
        ),
        Err(join_error) if join_error.is_cancelled() => {}
        Err(join_error) => tracing::warn!(error = %join_error, "ctrl+c watcher failed"),
    }
    terminate(&mut game, &shutdown, &tracker, cfg.drain_bound).await?;
    let cause = shutdown.cause().unwrap_or(Cause::ServerDisconnect);

    Ok(SessionReport {
        front_seed,
        char_unique: unique.char_unique,
        session_seed: (unique.key1, unique.key2),
        server_version: unique.version,
        user_id: auth.user_id().to_owned(),
        world,
        worlds_seen,
        roster,
        selected_slot,
        selected_name,
        entry,
        dispatcher,
        frames_dispatched,
        heartbeats_sent,
        cause,
    })
}

/// Whether `cmd` is one of the five opcodes the Step 5.4 dispatcher owns
/// (`dispatcher.rs` module docs: 6/11/118/200/215, the `ClientMessageMap`
/// rows at `Client-Message.cpp:248/309/310/330/807`).
///
/// The entry waits use this to route interleaved in-world frames rather than
/// refuse a frame the legacy would have handled (module docs, "failure
/// semantics").
fn is_dispatchable(cmd: u16) -> bool {
    matches!(
        cmd,
        char_info::RESPONSE_LOGINMANY
            | dispatcher::RESPONSE_LOGOUT
            | dispatcher::RESPONSE_CHAT
            | dispatcher::RESPONSE_CHARUPDATE
            | dispatcher::RESPONSE_PERSON_IP
    )
}

/// Forms the dial address the worldlist entry advertises — the legacy's
/// "dial where the list says" (`WorldInfo::game_server_ip`, "where the client
/// dials next", `src/auth/world_list.rs`).
///
/// # Errors
///
/// [`DriverError::WorldAddress`] when the ip string is not an `IpAddr`; the
/// port is used as received.
fn world_addr(world: &WorldInfo) -> Result<SocketAddr, DriverError> {
    let ip: IpAddr = world
        .game_server_ip
        .parse()
        .map_err(|_| DriverError::WorldAddress {
            world_id: world.world_id,
            ip: world.game_server_ip.clone(),
            port: world.game_server_port,
        })?;
    Ok(SocketAddr::new(ip, world.game_server_port))
}

/// Awaits a fallible `future` bounded by `budget`, mapping an elapsed timer
/// onto a phase-tagged [`DriverError::StepTimeout`].
///
/// The bound wraps the whole wait (skipped frames included), so a peer that
/// trickles irrelevant frames cannot reset the budget forever.
async fn within<T>(
    budget: Duration,
    phase: Phase,
    future: impl Future<Output = Result<T, DriverError>>,
) -> Result<T, DriverError> {
    tokio::time::timeout(budget, future)
        .await
        .map_err(|_| DriverError::StepTimeout { phase, budget })?
}

/// A required string variable.
fn required_env(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::Missing { name })
}

/// An optional string variable (`None` when absent).
fn optional_env(name: &'static str) -> Result<Option<String>, ConfigError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { name }),
    }
}

/// An optional integer variable.
fn parse_env_int<T>(name: &'static str, raw: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr<Err = std::num::ParseIntError>,
{
    raw.parse::<T>().map_err(|source| ConfigError::Parse {
        name,
        value: raw.to_owned(),
        kind: std::any::type_name::<T>(),
        source,
    })
}

/// Resolves `host:port` from `name`, falling back to `default_spec`.
async fn resolve_env(name: &'static str, default_spec: String) -> Result<SocketAddr, ConfigError> {
    let spec = std::env::var(name).unwrap_or(default_spec);
    resolve_spec(name, spec).await
}

/// Resolves one `host:port` spec (DNS names allowed — the ngrok path
/// `tests/live_integration.rs` documents).
async fn resolve_spec(name: &'static str, spec: String) -> Result<SocketAddr, ConfigError> {
    let (host, port) = spec
        .rsplit_once(':')
        .ok_or_else(|| ConfigError::NotHostPort {
            name,
            spec: spec.clone(),
        })?;
    let port: u16 = port.parse().map_err(|source| ConfigError::Parse {
        name,
        value: spec.clone(),
        kind: "u16 port",
        source,
    })?;
    let mut addrs = lookup_host((host, port))
        .await
        .map_err(|source| ConfigError::Resolve {
            name,
            spec: spec.clone(),
            source,
        })?
        .collect::<Vec<_>>();
    addrs.sort();
    addrs
        .into_iter()
        .next()
        .ok_or_else(|| ConfigError::Resolve {
            name,
            spec,
            source: std::io::Error::other("resolved to zero addresses"),
        })
}

#[cfg(test)]
mod tests;
