//! Binary entry point: the Step 6.1 end-to-end driver over environment config.
//!
//! Everything orchestration-shaped lives in [`goonzu_client::client::driver`]
//! (`src/client/driver.rs`) so the hermetic Step 6.2 harness can drive the
//! exact same code path; this file is environment -> config -> runtime ->
//! exit codes, nothing more.
//!
//! Step 8.4 (`WO-8.4.md` §7.1) composes that entry on Windows: the window
//! pump thread runs beside the driver session, the stage trigger is
//! consumed from the driver's frozen `Entered` record (D24) and posted at
//! the window as `WM_ENTER_WORLD`, and the exit codes below are
//! unchanged. Off-Windows the headless body runs exactly as it always
//! has (D29) — no window, no trigger.
//!
//! Exit codes (WO-6.1 §5): `0` a session that ended at `Stage::TornDown`, `1`
//! a runtime failure (the failing stage is part of the logged error), `2` a
//! configuration error (a required variable missing or unparseable).

use goonzu_client::client::driver::{self, DriverConfig, DriverError};
use goonzu_client::client::teardown::Shutdown;
use goonzu_client::CLIENT_VERSION;

/// The composed window's handle slot: filled once `main`'s ready channel
/// delivers it (D25 step 5), read by [`EnterWorldTrigger`] (D24).
#[cfg(windows)]
use std::sync::{Arc, OnceLock};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    #[cfg(windows)]
    composed().await;
    #[cfg(not(windows))]
    headless().await;
}

/// The off-Windows entry (WO-8.4 D29): today's headless body, unchanged —
/// `fmt::init()` at INFO, environment to config, `driver::run`, the
/// delivered exit match. The `#[cfg]` boundary *is* the portability story:
/// no flag, no mode (D29).
#[cfg(not(windows))]
async fn headless() {
    tracing_subscriber::fmt::init();
    tracing::info!(version = CLIENT_VERSION, "goonzu client starting");

    let config = match DriverConfig::from_env().await {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "driver configuration is invalid");
            std::process::exit(2);
        }
    };

    let shutdown = Shutdown::new();
    match driver::run(&config, &shutdown).await {
        Ok(outcome) => {
            tracing::info!(
                stage = %outcome.stage,
                worlds = outcome.worlds,
                char_unique = outcome.char_unique,
                char_slot = outcome.char_slot,
                village = ?outcome.village,
                "session complete"
            );
        }
        // A configuration `run` itself refuses is still a configuration error.
        Err(DriverError::Config(error)) => {
            tracing::error!(%error, "driver configuration is invalid");
            std::process::exit(2);
        }
        Err(error) => {
            tracing::error!(%error, "session failed");
            std::process::exit(1);
        }
    }
}

/// The Windows composition (WO-8.4 §7.1, D24–D26): the window pump on one
/// `std::thread`, the driver on this runtime thread, ordered by the ready
/// channel; the DEBUG subscriber installed here lives for all of `main` and
/// carries the stage trigger. The seam itself is `graphics::window`
/// (§7.2); the exit arms at the bottom are byte-for-byte the delivered
/// ones (WO-6.1 §5).
#[cfg(windows)]
async fn composed() {
    use goonzu_client::client::commands;
    use goonzu_client::client::teardown::Cause;
    use goonzu_client::graphics::window;
    use goonzu_client::input::InputFeed;
    use std::sync::mpsc;
    use tracing_subscriber::prelude::*;

    // D24/§7.1 step 1: the subscriber first, so the starting info! record
    // is captured; the HWND slot is still empty, so no trigger can fire yet
    // (the signal is DEBUG — §2.2-2's disclosed stdout-volume change).
    let hwnd_slot: Arc<OnceLock<isize>> = Arc::new(OnceLock::new());
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .finish()
        .with(EnterWorldTrigger {
            hwnd: Arc::clone(&hwnd_slot),
        });
    // Thread-scoped and held for all of `main`: the current-thread runtime
    // polls the driver on this very thread, so the Layer sees every driver
    // record (`e2e_loopback.rs` `capture_subscriber` callsite argument).
    let _default = tracing::subscriber::set_default(subscriber);

    tracing::info!(version = CLIENT_VERSION, "goonzu client starting");

    // §7.1 step 2: config before any window exists — the config-error
    // contract never flashes a window (exit 2, unchanged).
    let config = match DriverConfig::from_env().await {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "driver configuration is invalid");
            std::process::exit(2);
        }
    };

    // §7.1 steps 3-4: the frozen channel and the one window thread. When
    // the demo returns for any reason the thread claims the session
    // shutdown with the frozen cause (D26 — the GUI close is the user
    // interrupt's analogue; `src/client/**` may not add a third `Cause`,
    // so the terminal record logs `cause=CtrlC`, documented never silent).
    let shutdown = Shutdown::new();
    let (sender, receiver) = commands::channel();
    let feed = InputFeed::new(sender);
    let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
    let window_shutdown = shutdown.clone();
    let window_handle = std::thread::spawn(move || {
        let _ = window::run_demo_with(Some(feed), ready_tx);
        let _ = window_shutdown.trigger(Cause::CtrlC);
    });

    // §7.1 step 5: exactly one ready delivery, awaited BEFORE the driver
    // starts — the trigger's target provably exists before the first
    // `Entered` record (D25 step 5).
    match ready_rx.recv() {
        Ok(Ok(hwnd)) => {
            let _ = hwnd_slot.set(hwnd);
        }
        Ok(Err(why)) => {
            tracing::error!(%why, "the demo window could not be opened");
            let _ = window_handle.join();
            std::process::exit(1);
        }
        Err(_) => {
            tracing::error!("the window thread panicked");
            let _ = window_handle.join();
            std::process::exit(1);
        }
    }

    // §7.1 step 6: the production Ctrl+C watcher arms exactly as `run`
    // arms it (`driver.rs:475-481`), with the command channel wired in
    // (was `driver::run` — the WO-8.3 divergence (a) closed).
    let result = driver::run_with_commands(&config, &shutdown, receiver).await;

    // §7.1 step 7: close + join before ANY exit (D26) — the pump is woken
    // by `WM_CLOSE`, never blocked on `main`, and the window thread never
    // waits on `main` (the ready send precedes the pump loop; the shutdown
    // flag is atomic) — no lock ordering exists to deadlock.
    window::close_demo();
    let _ = window_handle.join();

    // §7.1 step 8: byte-for-byte the delivered arms (WO-6.1 §5).
    match result {
        Ok(outcome) => {
            tracing::info!(
                stage = %outcome.stage,
                worlds = outcome.worlds,
                char_unique = outcome.char_unique,
                char_slot = outcome.char_slot,
                village = ?outcome.village,
                "session complete"
            );
        }
        // A configuration `run` itself refuses is still a configuration error.
        Err(DriverError::Config(error)) => {
            tracing::error!(%error, "driver configuration is invalid");
            std::process::exit(2);
        }
        Err(error) => {
            tracing::error!(%error, "session failed");
            std::process::exit(1);
        }
    }
}

/// D24 — the one signal the composition consumes (never a Win32 type,
/// never an event built here): the frozen driver record's metadata name
/// (`driver.rs:850` message literal) and the `outcome` field's Debug
/// rendering (`DispatchOutcome::Entered`, `dispatcher.rs:766,994-997`).
///
/// cfg: compiled on Windows (the Layer calls it) and under test (T57's
/// subject). Off-Windows non-test builds have no caller, and a dead
/// private function would fail `RUSTFLAGS=-D warnings` — the D29
/// portability boundary, disclosed in the delivery note.
#[cfg(any(windows, test))]
fn enter_world_signal(event_name: &str, outcome_debug: &str) -> bool {
    event_name == "in-world frame dispatched" && outcome_debug.starts_with("Entered")
}

/// D24 — the composition's stage trigger: a `tracing` `Layer` over the
/// driver's frozen `Entered` record (§7.1). Its `on_event` visits the
/// event's `outcome` field, applies [`enter_world_signal`],
/// and on `true` posts `WM_ENTER_WORLD` at the composed window through
/// `graphics::window::post_enter_world` — an unset HWND slot means no
/// target, and D25 orders the slot's fill before the driver starts, so
/// the skip arm is provably unreachable in the composition.
#[cfg(windows)]
struct EnterWorldTrigger {
    /// The composed window's handle (D25 step 5).
    hwnd: Arc<OnceLock<isize>>,
}

/// The `outcome` field's Debug rendering, when the visited event carries
/// one (every other field type falls through `Visit`'s defaults).
#[cfg(windows)]
struct OutcomeVisitor {
    /// `Some` once a field named `outcome` was visited.
    outcome: Option<String>,
}

#[cfg(windows)]
impl tracing::field::Visit for OutcomeVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "outcome" {
            self.outcome = Some(format!("{value:?}"));
        }
    }
}

#[cfg(windows)]
impl<S> tracing_subscriber::Layer<S> for EnterWorldTrigger
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = OutcomeVisitor { outcome: None };
        event.record(&mut visitor);
        let Some(outcome) = visitor.outcome else {
            return; // no `outcome` field — not the driver's dispatch record
        };
        if !enter_world_signal(event.metadata().name(), &outcome) {
            return; // the wrong message, or the wrong outcome of it (E48)
        }
        if let Some(hwnd) = self.hwnd.get() {
            goonzu_client::graphics::window::post_enter_world(*hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::enter_world_signal;

    /// T57 (WO-8.4 §9, E48) — the trigger predicate admits **exactly**
    /// the stage signal: the frozen driver record's message name *and* an
    /// `Entered` outcome, together. Every other `DispatchOutcome`
    /// rendering of that message, every other message carrying an
    /// `Entered` rendering (including the one-token prefix typo), and the
    /// empty string are refused — and the pure two-string function never
    /// panics on garbage. Expectations are §2.4-4's hand table, never
    /// read back from the code under test (F-07).
    #[test]
    fn the_enter_world_trigger_filter_admits_only_the_stage_signal() {
        const NAME: &str = "in-world frame dispatched";
        const ENTERED: &str = "Entered { char_uniques: [3], npcs: 0 }";

        // The one firing row (driver.rs:850 message literal +
        // dispatcher.rs:766 `Entered`, produced only by opcode 6).
        assert!(
            enter_world_signal(NAME, ENTERED),
            "E48: the frozen driver record with an Entered outcome must fire"
        );

        // E48 outcome half: every other DispatchOutcome rendering of the
        // same message must not fire (`dispatcher.rs:749-799`) — a chat
        // frame re-arming the feed on every line is exactly what probe 4
        // simulates.
        for outcome in [
            "Chat(ChatMessage { declared_size: 20, char_unique: 3, mode: 1, \
              is_game_master: false, system_txt: 0, level: 0, \
              player_name: \"someone\", text: \"hi\" })",
            "Removed { char_unique: 3, applied: true }",
            "Applied { char_unique: 3, target: Unmatched }",
            "Ordered { char_unique: 3, target: Unmatched, order: Move { x: 100, y: 100 } }",
            "Unhandled { cmd: 999 }",
            "Unmatched",
            "",
        ] {
            assert!(
                !enter_world_signal(NAME, outcome),
                "E48: outcome {outcome:?} of the stage message must not fire"
            );
        }

        // E48 name half: an Entered rendering of any other record must not
        // fire — the prefix typo row included (probe 3's target).
        for name in [
            "heartbeat sent",
            "session ended",
            "in-world order echo reported",
            "in-world frame dispatch",
        ] {
            assert!(
                !enter_world_signal(name, ENTERED),
                "E48: message {name:?} must not fire even with an Entered outcome"
            );
        }

        // Never panics: any/empty/garbage inputs (WO-8.4 §9 T57).
        for (name, outcome) in [
            ("", ""),
            ("", ENTERED),
            (NAME, ""),
            ("in-world frame dispatched\u{0}", "Entered {"),
            ("\u{10FFFD}", "Entered"),
        ] {
            let _fired = enter_world_signal(name, outcome); // no panic is the assertion
        }
    }
}
