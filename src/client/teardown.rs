//! Step 5.3 — graceful teardown & signal handling (`CONTRIBUTING.md:152-155`).
//!
//! The client must stop cleanly on **either** exit cause — `Ctrl+C` *or* the
//! server disconnecting — "close sockets cleanly without leaking tasks",
//! and terminate with **zero orphaned tokio background tasks** (the
//! acceptance criterion, verbatim). This module is that policy: one
//! first-wins [`Shutdown`] flag, one task-accounting [`TaskTracker`], one
//! [`terminate`] that closes the link and drains the tracker.
//!
//! # What the legacy does (F-05 parity citations)
//!
//! The legacy client is a Win32 **GUI** program: there is no `signal()`,
//! `SetConsoleCtrlHandler`, or `SIGINT` handler anywhere in the client tree
//! (tree-wide grep) — it has no console to be interrupted on. Its exit chain
//! is window-message driven, and this module maps onto it one-to-one:
//!
//! - `WM_DESTROY` (`mainfunction.cpp:1598`) is the teardown itself: it runs
//!   `delete pclClient` (the client destructor tears down its sessions) and
//!   then `PostQuitMessage(0)` (`mainfunction.cpp:1656`).
//! - `PostQuitMessage` is what makes `GetMessage` return 0 in the main pump
//!   (`main.cpp:1553-1556`), so `WinMain` returns `msg.wParam` and the
//!   process ends — the "loop stops" half of the port's contract.
//! - A 2000 ms quit timer ends the same way: `if ((pclClient->CurrentClock -
//!   dwQuitTimer) > 2000)` → `DestroyWindow(hwnd)` (`main.cpp:1584-1620`),
//!   i.e. back into `WM_DESTROY`.
//! - The socket close itself is `ITZSession::CloseSocket`
//!   (`ITZSession.cpp:194-203`): `_closesocket()` under the session's
//!   critical section. It runs from the session destructor
//!   (`ITZSession.cpp:33`), and the front link is also closed explicitly on
//!   the exit path (`Client-Winsock.cpp:788`).
//! - The *server-side* disconnect is **detected, not signalled**: once the
//!   game session falls out of `SESSION_STATE_ESTABLISHED` after login
//!   (`Client-Winsock.cpp:834-844`), the client raises the exit-confirm
//!   dialog; the user confirms and the `WM_DESTROY` chain above runs.
//!
//! # What this module adds (port divergence, stated honestly)
//!
//! 1. **Ctrl+C is port-added.** The legacy has no console interrupt, so
//!    [`Cause::CtrlC`] and the [`spawn_ctrl_c_watcher`] adapter built on
//!    `tokio::signal::ctrl_c` have **no legacy counterpart** — no citation
//!    is invented for them (F-05). They exist because this port also runs
//!    as a foreground process where SIGINT is the ordinary way out. The
//!    *behaviour they feed* — stop the loop, close the socket, end the
//!    process — is exactly the legacy's `WM_DESTROY` chain.
//! 2. **One flag, first cause wins.** [`Shutdown::trigger`] is a
//!    compare-exchange: the first cause to arrive owns the shutdown and a
//!    later one is dropped (its `bool` answer says whether it won). A
//!    Ctrl+C racing a server drop therefore resolves to exactly one cause
//!    instead of two teardowns.
//! 3. **Detected disconnect becomes the default cause.** The loop marks
//!    [`Cause::ServerDisconnect`] when the peer closes or the link fails
//!    (`event_loop::run_with_shutdown`), mirroring the legacy's detection at
//!    `Client-Winsock.cpp:834-844`; [`terminate`] claims an *unclaimed*
//!    shutdown as [`Cause::ServerDisconnect`] too, because by the time
//!    teardown runs, "someone asked to stop" with no console cause in
//!    flight can only mean the link is gone.
//!
//! # The teardown contract
//!
//! [`terminate`] runs three phases, **all of which always execute** (an
//! early error is remembered, never short-circuiting the rest):
//!
//! 1. claim the cause (unless a `Ctrl+C` already owns it);
//! 2. close the game link — a write-side shutdown so the server sees a clean
//!    EOF (`GameLink::close` → `ITZSession::CloseSocket` parity);
//! 3. [`TaskTracker::drain`] every spawned background task within a caller's
//!    bound, returning [`DrainError::Timeout`] — never hanging, never
//!    claiming success with orphans still live.
//!
//! The receive loop honours the flag at the top of each iteration, so a
//! Ctrl+C that arrives mid-wait is acted on after at most **one read
//! budget** (`event_loop::READ_POLL_BUDGET`, 100 ms) — bounded shutdown
//! latency, pinned by `tests::triggered_shutdown_stops_the_loop_within_one_read_budget`.
//!
//! # Testing notes (F-01/F-07)
//!
//! A real `Ctrl+C` is **not** hermetically deliverable on Windows: the
//! obvious trick, `GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)`, signals the
//! whole console group — i.e. cargo and the harness running this suite. So
//! the signal path is tested through its **injected-future seam**:
//! [`watch_signal`] takes the future factory, and the production adapter in
//! [`spawn_ctrl_c_watcher`] is the one-line `tokio::signal::ctrl_c`
//! pass-through (manually verified behaviour, F-01 style; the tests here
//! cover its task accounting). Every expectation comes from outside the unit
//! under test: loopback peers the test itself closes, literal durations, and
//! wall-clock-style `Instant` bounds measured around the call (F-07).

use crate::game::entry::GameLink;
use std::future::Future;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// How often [`TaskTracker::drain`] re-checks the live count while waiting.
///
/// Port-chosen: a 1 ms poll keeps a clean shutdown imperceptible (a task
/// that finishes immediately is caught within one interval) while leaving
/// plenty of resolution for the caller's bound.
pub const DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Why the client is shutting down.
///
/// First wins: the cause stored in [`Shutdown`] at trigger time is the one
/// reported for the whole teardown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cause {
    /// A console interrupt arrived (port-added — the legacy GUI client has
    /// no console signal; module docs, "port divergence").
    CtrlC,
    /// The server dropped, the link failed, or teardown ran with no cause
    /// claimed — the legacy's detected-disconnect path
    /// (`Client-Winsock.cpp:834-844`).
    ServerDisconnect,
}

impl Cause {
    /// The flag's on-wire encoding inside [`Shutdown`]'s atomic (`0` =
    /// untriggered).
    const fn code(self) -> u8 {
        match self {
            Self::CtrlC => 1,
            Self::ServerDisconnect => 2,
        }
    }

    /// Inverse of [`Cause::code`] — `None` for the untriggered `0` (and any
    /// value this module did not write).
    const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::CtrlC),
            2 => Some(Self::ServerDisconnect),
            _ => None,
        }
    }
}

/// The one shutdown flag shared by the signal watcher, the receive loop, and
/// any background task that wants to know it is time to stop.
///
/// `Clone` is cheap (`Arc`) and every clone observes the same flag: hand one
/// to each participant (module docs, "one flag, first cause wins").
#[derive(Debug, Clone, Default)]
pub struct Shutdown {
    /// `0` untriggered, otherwise [`Cause::code`].
    flag: Arc<AtomicU8>,
}

impl Shutdown {
    /// A shutdown that has not been triggered.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Claims the shutdown for `cause`.
    ///
    /// Returns `true` only for the first caller: later triggers lose the
    /// compare-exchange and change nothing, so the recorded cause is stable
    /// for the whole teardown.
    pub fn trigger(&self, cause: Cause) -> bool {
        self.flag
            .compare_exchange(0, cause.code(), Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Whether any cause has claimed the shutdown.
    #[must_use]
    pub fn is_triggered(&self) -> bool {
        self.flag.load(Ordering::SeqCst) != 0
    }

    /// The winning cause, or `None` while nobody has claimed it.
    #[must_use]
    pub fn cause(&self) -> Option<Cause> {
        Cause::from_code(self.flag.load(Ordering::SeqCst))
    }
}

/// Why [`TaskTracker::drain`] gave up: the bound elapsed with tasks still
/// live. Produced **instead of** `Ok(())` — a drain that cannot finish never
/// reports success (the step's "zero orphaned tasks" acceptance).
#[derive(Debug, thiserror::Error)]
pub enum DrainError {
    /// `bound` elapsed while `live` tracked task(s) had not finished.
    #[error("{live} background task(s) still live after the {bound:?} drain bound")]
    Timeout {
        /// Tasks still counted live when the bound elapsed.
        live: usize,
        /// The caller's drain bound.
        bound: Duration,
    },
}

/// Why [`terminate`] could not complete cleanly.
#[derive(Debug, thiserror::Error)]
pub enum TeardownError {
    /// The write-side shutdown of the game link failed. The drain still ran
    /// (module docs, "the teardown contract").
    #[error(transparent)]
    Close(#[from] crate::game::entry::GameError),
    /// The drain bound elapsed with background tasks still live
    /// ([`DrainError::Timeout`]).
    #[error(transparent)]
    Drain(#[from] DrainError),
}

/// Counts the client's background tasks so teardown can prove they are all
/// gone — the mechanism behind the step's zero-orphan acceptance criterion.
///
/// Every task must be spawned through [`TaskTracker::spawn`]; a raw
/// `tokio::spawn` is invisible to the drain and therefore a leak this type
/// cannot detect (documented, not hidden).
#[derive(Debug, Clone, Default)]
pub struct TaskTracker {
    /// Live (spawned, not yet finished/aborted/panicked) task count.
    live: Arc<AtomicUsize>,
}

/// Decrements the live count exactly once, when the owning task's future is
/// dropped — normal completion, `abort()`, and panics alike, so a killed
/// task is never left counted (orphan detection stays honest).
#[derive(Debug)]
struct LiveGuard(Arc<AtomicUsize>);

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl TaskTracker {
    /// A tracker with zero live tasks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many spawned tasks have not finished yet.
    #[must_use]
    pub fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// Spawns `future` on the tokio runtime, counted until it ends.
    ///
    /// The count is taken **before** the task can run (a task that finishes
    /// immediately must not decrement a count that was never incremented),
    /// and released by the internal `LiveGuard` when the future is dropped.
    ///
    /// # Panics
    ///
    /// As `tokio::spawn`: outside a runtime. The count is left incremented
    /// in that case only because the panic itself is fatal to the caller's
    /// operation.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.live.fetch_add(1, Ordering::SeqCst);
        let guard = LiveGuard(Arc::clone(&self.live));
        tokio::spawn(async move {
            let _guard = guard;
            future.await
        })
    }

    /// Waits until every tracked task has finished, or `bound` elapses.
    ///
    /// Polls [`TaskTracker::live`] every [`DRAIN_POLL_INTERVAL`] — a
    /// tokio-time sleep, so the caller's bound is exact under a paused test
    /// clock and never a busy spin in production.
    ///
    /// # Errors
    ///
    /// [`DrainError::Timeout`] carrying the still-live count: an orphan is
    /// *reported*, never silently accepted.
    pub async fn drain(&self, bound: Duration) -> Result<(), DrainError> {
        let deadline = Instant::now() + bound;
        loop {
            let live = self.live.load(Ordering::SeqCst);
            if live == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(DrainError::Timeout { live, bound });
            }
            tokio::time::sleep(DRAIN_POLL_INTERVAL).await;
        }
    }
}

/// Awaits `signal` and, if it reports success, claims the shutdown as
/// [`Cause::CtrlC`] — the **injectable seam** for the signal path.
///
/// `signal` is a future factory rather than a hard-wired
/// `tokio::signal::ctrl_c` so the seam can be driven hermetically (module
/// docs, "testing notes"). Production passes `tokio::signal::ctrl_c`.
///
/// Returns `Some(Cause::CtrlC)` only when *this call* claimed the flag — if
/// the shutdown was already triggered (e.g. the server dropped first), the
/// console event loses and the existing cause stands. A **registration
/// failure** returns `None` and touches nothing: a signal we could not hook
/// must never masquerade as one that fired.
///
/// # Errors surfacing here
///
/// None: `signal`'s `Err` becomes `None`, not a panic or a false trigger.
pub async fn watch_signal<F, Fut>(shutdown: Shutdown, mut signal: F) -> Option<Cause>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<()>>,
{
    match signal().await {
        Ok(()) => shutdown.trigger(Cause::CtrlC).then_some(Cause::CtrlC),
        // Registration/delivery failure: never claim a cause we did not get.
        Err(_) => None,
    }
}

/// The production Ctrl+C watcher: [`watch_signal`] over
/// `tokio::signal::ctrl_c`, spawned as a **tracked** background task.
///
/// The task lives until a console interrupt arrives (or registration fails),
/// which is why teardown must [`TaskTracker::drain`] with a bound rather
/// than assume — see `tests::spawn_ctrl_c_watcher_is_accounted_and_abortable`.
pub fn spawn_ctrl_c_watcher(
    shutdown: Shutdown,
    tracker: &TaskTracker,
) -> JoinHandle<Option<Cause>> {
    tracker.spawn(async move { watch_signal(shutdown, tokio::signal::ctrl_c).await })
}

/// The full Step 5.3 teardown: claim the cause, close the link, drain the
/// tasks (module docs, "the teardown contract").
///
/// All three phases run even if an earlier one errors — a failed socket
/// close still gets its task drain, and a failed drain still happened *after*
/// the socket was closed. The first error is what comes back.
///
/// `shutdown` is only ever *claimed* here, never waited on: pass the flag
/// already set (Ctrl+C) or not, and this call supplies
/// [`Cause::ServerDisconnect`] for the unclaimed case.
///
/// # Errors
///
/// [`TeardownError::Close`] when the write-side shutdown fails;
/// [`TeardownError::Drain`] when `drain_bound` elapses with tasks still
/// live (module docs, "zero orphaned tasks").
pub async fn terminate(
    link: &mut GameLink,
    shutdown: &Shutdown,
    tracker: &TaskTracker,
    drain_bound: Duration,
) -> Result<(), TeardownError> {
    // Phase 1: default an unclaimed shutdown to the server-side cause (an
    // exit with no console event in flight means the link is gone — the
    // legacy's detected-disconnect reading, Client-Winsock.cpp:834-844). A
    // Ctrl+C already recorded keeps its claim; a second trigger is a no-op.
    shutdown.trigger(Cause::ServerDisconnect);

    // Phase 2 and 3 both run unconditionally; each result is captured
    // before the other is awaited, so one failure never skips the other.
    let closing = link.close().await.map_err(TeardownError::Close);
    let drained = tracker
        .drain(drain_bound)
        .await
        .map_err(TeardownError::Drain);

    closing?;
    drained?;
    Ok(())
}

#[cfg(test)]
mod tests;
