//! Step 5.1 — the non-blocking receive loop (`CONTRIBUTING.md:95-98`).
//!
//! One [`GameLink`], one task: poll for the next inbound frame under a
//! **strict [`READ_POLL_BUDGET`] (100 ms)**, and in the same `tokio::select!`
//! fire any background [`Ticker`] whose deadline came due. An idle socket
//! therefore never blocks the task longer than one budget, and a stream of
//! frames never starves the tickers.
//!
//! # The defect this replaces (legacy → port)
//!
//! The legacy client pumps the network once per frame from a Win32 message
//! loop: an inner `for (i = 0; i < 10; ++i)` (`main.cpp:1549-1581`) drains
//! pending `GetMessage`s and otherwise `Sleep(10)`s — and that sleep exists
//! **only in game mode**, `if(gamemode == 1) Sleep(10);` (`main.cpp:1579`) —
//! so the *idle in-game* cadence is `Sleep(10)` x 10 = **100 ms per
//! `NetworkOperation()`** (`main.cpp:1625-1630`, `:1628` →
//! `Client-Winsock.cpp:925-928`; `NetworkOperation()` itself is ungated by
//! mode, `main.cpp:1628`, so outside game mode the legacy pump busy-polls).
//! That 100 ms is this module's [`READ_POLL_BUDGET`], carried over as a strict
//! *per-read* bound rather than a whole-frame `Sleep`.
//!
//! # Why the 100 ms budget is necessary here (decision record)
//!
//! The cadence condition is **necessary** for this port, on three grounds:
//!
//! 1. **Specification:** `CONTRIBUTING.md:97` defines Step 5.1 as "a strict
//!    100ms read timeout (`tokio::time::timeout`)" — the budget is this
//!    step's named deliverable, not an incidental constant.
//! 2. **Legacy regime mapping:** the legacy sleeps only while `gamemode == 1`
//!    (`main.cpp:1579`); this loop *is* that regime's pump — it runs over the
//!    game link established in Steps 4.1-4.4, i.e. exactly the in-game phase,
//!    so the gated condition holds wherever the loop runs. The ungated
//!    busy-poll cadence of the other states belongs to surfaces this loop
//!    does not model and is out of scope here.
//! 3. **Cancellation cadence:** the timeout is the only thing that drops an
//!    in-flight `next_frame` future on a silent socket — that periodic drop is
//!    what `a_frame_split_across_two_read_windows_still_reassembles` pins as
//!    cancel-safety, and it keeps the session's own 5 s read window
//!    (`net/session.rs:93`) from ever being reached from inside the loop.
//!    Utilized at exactly one site: the `tokio::time::timeout(READ_POLL_BUDGET,
//!    link.next_frame())` read arm of the `select!` below.
//!
//! ## The verified legacy drain model (`GameServerAction`)
//!
//! `Client-Winsock.cpp:796-922` drains the game link's recv buffer as a
//! **snapshot-bounded indexed loop**: `int count = GetPacketCount()` is taken
//! once (`:850`), then `for (int i = 0; i < count; ++i)` (`:864`) pops each
//! packet (`GetFirstPacket` `:866`, decode `:872`, decompress `:896-912`,
//! dispatch `:914`, `RemoveFirstPacket` `:916`). It never waits for a packet
//! that has not arrived; packets that land mid-drain wait for the next pump.
//!
//! ## How this loop mirrors it
//!
//! - **Non-blocking semantics for buffered data.** `GameLink::next_frame`
//!   (`game/entry.rs:417-429`) returns an already-decoded frame *immediately*
//!   (`:419-421`, no `.await`), so a backlog drains back-to-back across
//!   `select!` iterations — one frame per cycle instead of the legacy's bulk
//!   `for` loop; bytes still arriving are awaited under the budget (next
//!   point), never without bound.
//! - **Blocking is replaced by a bounded wait, not removed.** The legacy's
//!   defect is that `GetMessage` *blocks* the pump until a window message
//!   arrives (`main.cpp:1553`), so a quiet game server stalls the whole loop.
//!   `tokio::select!` + `timeout(READ_POLL_BUDGET, next_frame())` turns that
//!   block into a bounded wait that expires every 100 ms — the loop stays live
//!   on a silent socket.
//! - **Deliberate refinement (documented deviation).** The legacy finishes its
//!   whole snapshot before any timer is examined; this loop re-evaluates the
//!   earliest ticker deadline *between* frames, because both branches race in
//!   `select!`. A due ticker therefore fires within one frame-handling time of
//!   its deadline even under sustained inbound traffic — which is the 5.1
//!   acceptance ("frames ... while continuing background tickers without
//!   latency", `CONTRIBUTING.md:98`) — and it cannot starve either side:
//!   frame delivery never waits on a ticker
//!   (`frames_are_delivered_while_tickers_keep_firing`) and ticker dispatch
//!   never waits on a frame
//!   (`an_idle_socket_does_not_block_the_loop_and_tickers_keep_firing`).
//!
//! The server port models the same link as **one task per socket**
//! (`gz-net/session.rs`, whose per-session worker owns its own read loop), so a
//! single-task `select!` here is the client-side mirror, not a new architecture.
//!
//! # Cancellation safety (why a 100 ms drop loses nothing)
//!
//! `GameLink::next_frame`'s **only** `.await` is `session.read(&mut buf)`
//! (`game/entry.rs:417-429`): the decoder is checked first, then one read, then
//! `decoder.feed` runs **synchronously** before the next loop iteration. So the
//! future is dropped *only* at a `session.read` await point, and at that point:
//!
//! - bytes not yet handed to the process are still in the kernel socket buffer
//!   (tokio's `TcpStream` read is cancel-safe), so the next poll reads them;
//! - bytes already read have already been `feed`-ed into the decoder, which is
//!   owned by the link and persists across polls, so a partial frame stays
//!   buffered.
//!
//! Nothing is half-applied: there is no await *between* a completed read and its
//! `feed`. A frame split across two (or N) budget windows therefore reassembles
//! byte-for-byte — pinned by
//! `a_frame_split_across_two_read_windows_still_reassembles`.
//!
//! The 100 ms budget also strictly dominates the session's own
//! [`crate::net::session::DEFAULT_READ_TIMEOUT`] (5 s): the budget expires and
//! the future is dropped long before the session's read window could, so a
//! [`crate::net::session::SessionError::ReadTimeout`] is never observed from inside the loop during
//! ordinary idling (`net/session.rs` documents that a read timeout is *not* a
//! teardown; here it simply cannot surface).
//!
//! # Tickers are checked, not awaited per-ticker
//!
//! Each [`Ticker`] carries a `name` and an `interval`; the loop tracks one
//! next-fire deadline per ticker and awaits only the **earliest** in the
//! `select!` (a single `sleep_until`, or a `pending()` future when no ticker
//! exists), so N tickers cost no extra dependencies and no N-way select. When a
//! deadline is reached, every ticker whose deadline has passed fires once and
//! its deadline advances to the next whole interval boundary — the legacy's
//! `now - last >= interval` check (`main.cpp:1625-1630`), which coalesces
//! intervals missed while the loop was busy instead of bursting to catch up. A
//! zero `interval` can never be awaited without a hot spin, so it is treated as
//! disabled and never fires.
//!
//! # Test addresses are synthetic (F-01/F-02)
//!
//! As on every link so far (QC decisions on CAR-2026-10-24-RETRO01), the live
//! `goonzu-rs` server is unavailable, so the suite drives the loop over a
//! **loopback** `GameLink` whose peer writes real link-ciphered frames with a
//! fresh `Cipher::link_cipher()` — the same pairing the game service uses
//! (`main.rs:680`). Read windows are pinned small so the behaviour is observable
//! in milliseconds. F-07: the loop is never its own expectation source; every
//! frame the peer writes is a handwritten `(cmd, payload)` and the assertions
//! compare against those literals.

use crate::game::entry::{GameError, GameLink};
use crate::net::codec::Frame;
use std::time::Duration;
use tokio::time::{sleep_until, Instant};

/// The strict per-read budget: how long the loop waits on
/// [`GameLink::next_frame`] before it re-polls its tickers.
///
/// 100 ms — the legacy's *in-game* idle pump cadence: `Sleep(10)` x 10 inner
/// iterations per `NetworkOperation()` (`main.cpp:1549-1581`, `:1625-1630`),
/// with the sleep itself existing only `if(gamemode == 1)` (`main.cpp:1579`).
/// See the module docs' decision record for why that gated cadence is
/// *necessary* here rather than incidental. Applied as a **bound on one
/// wait**, not a sleep: a frame that arrives 1 ms after the loop starts
/// waiting is handed over immediately, not after 100 ms.
pub const READ_POLL_BUDGET: Duration = Duration::from_millis(100);

/// A background timer the loop fires on a fixed interval.
///
/// Step 5.2's heartbeat is the first real ticker (a 10 s keepalive); the loop
/// accepts any number of them and awaits only the earliest deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticker {
    /// A stable label for the tick, delivered in [`LoopEvent::Ticked`] so a
    /// handler can tell multiple tickers apart.
    pub name: &'static str,
    /// The interval between fires. Must be positive — a zero interval is
    /// treated as disabled and never fires (module docs, "tickers are
    /// checked").
    pub interval: Duration,
}

impl Ticker {
    /// A ticker that fires every `interval`, labelled `name`.
    #[must_use]
    pub const fn new(name: &'static str, interval: Duration) -> Self {
        Self { name, interval }
    }
}

/// One thing the loop hands to the handler.
#[derive(Debug)]
pub enum LoopEvent {
    /// A complete inbound frame, already deciphered and framed by the link.
    Frame(Frame),
    /// A [`Ticker`] fired: its `name` and the running `fired` count for that
    /// ticker (1 on the first fire).
    Ticked {
        /// The firing ticker's [`Ticker::name`].
        name: &'static str,
        /// That ticker's fire ordinal, starting at 1.
        fired: u64,
    },
}

/// What the handler asks the loop to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Keep running.
    Continue,
    /// Return from [`run`] successfully after this event.
    Break,
}

/// Why [`run`] stopped with a failure.
#[derive(Debug, thiserror::Error)]
pub enum LoopError<E: std::error::Error + 'static> {
    /// The link failed: a session/socket error, a refused stream (sticky
    /// framing/integrity poison), or a codec failure — the loop is finished,
    /// exactly as the legacy returns out of the receive path on a decode
    /// failure (`Client-Winsock.cpp:872-890`).
    #[error(transparent)]
    Link(#[from] GameError),
    /// The handler returned an error for an event. Constructed explicitly
    /// (`.map_err(LoopError::Handler)`) rather than via `#[from]`, because a
    /// blanket `impl<E> From<E> for LoopError<E>` would overlap
    /// `impl From<GameError> for LoopError<E>` when `E = GameError`.
    #[error(transparent)]
    Handler(E),
}

/// The outcome of one `select!` poll, produced before any handler runs so the
/// futures borrowing the link are already dropped.
enum Wake {
    /// The read budget elapsed with no complete frame: an idle poll.
    Idle,
    /// A complete inbound frame arrived.
    Frame(Frame),
    /// The peer closed the connection cleanly (read returned 0 / EOF).
    Closed,
    /// The link failed.
    Failed(GameError),
    /// At least one ticker deadline was reached.
    Ticks,
}

/// One ticker's runtime state: when it next fires and how many times it has.
struct TickerState {
    name: &'static str,
    interval: Duration,
    deadline: Instant,
    fired: u64,
}

/// Waits until `deadline`, or forever when there is no ticker.
///
/// A single future for the earliest ticker deadline, so the `select!` needs one
/// branch regardless of how many tickers exist.
async fn ticker_wait(deadline: Option<Instant>) {
    match deadline {
        Some(at) => sleep_until(at).await,
        // No ticker: this branch never resolves, leaving the read as the only
        // live branch.
        None => std::future::pending::<()>().await,
    }
}

/// Runs the non-blocking receive loop over `link` until it closes, fails, or
/// the handler breaks.
///
/// Each iteration `select!`s between a frame read bounded by
/// [`READ_POLL_BUDGET`] and the earliest [`Ticker`] deadline, then dispatches
/// the resulting [`LoopEvent`] to `handler`. The handler receives `&mut link`
/// so it can respond (e.g. send a keepalive) on the same task.
///
/// Returns `Ok(())` when the peer closes cleanly or the handler returns
/// [`Flow::Break`]; [`LoopError::Link`] when the link fails, [`LoopError::Handler`]
/// when the handler errors. A handler error stops the loop immediately.
///
/// # Errors
///
/// As documented above; see each variant.
pub async fn run<F, E>(
    link: &mut GameLink,
    tickers: &[Ticker],
    mut handler: F,
) -> Result<(), LoopError<E>>
where
    F: FnMut(&mut GameLink, LoopEvent) -> Result<Flow, E>,
    E: std::error::Error + 'static,
{
    // Zero-interval tickers are dropped here (never fire), so the catch-up
    // advance below can always make progress on a positive interval.
    let mut states: Vec<TickerState> = tickers
        .iter()
        .filter(|t| !t.interval.is_zero())
        .map(|t| TickerState {
            name: t.name,
            interval: t.interval,
            deadline: Instant::now() + t.interval,
            fired: 0,
        })
        .collect();

    loop {
        // Only the earliest deadline is awaited (module docs).
        let earliest = states.iter().map(|s| s.deadline).min();

        // Both futures borrow nothing the handler needs: the read borrows
        // `link`, the ticker wait borrows nothing. The arm bodies only *produce*
        // a `Wake`; they run no handler, so the `&mut link` the read future
        // holds is released the moment this statement ends.
        let wake = tokio::select! {
            read = tokio::time::timeout(READ_POLL_BUDGET, link.next_frame()) => {
                match read {
                    // The 100 ms budget elapsed with no frame: an idle poll.
                    Err(_elapsed) => Wake::Idle,
                    Ok(Ok(Some(frame))) => Wake::Frame(frame),
                    // Clean close (EOF): buffered frames were already handed out
                    // before this, so end the loop.
                    Ok(Ok(None)) => Wake::Closed,
                    Ok(Err(err)) => Wake::Failed(err),
                }
            }
            _ = ticker_wait(earliest) => Wake::Ticks,
        };

        match wake {
            Wake::Idle => {}
            Wake::Closed => return Ok(()),
            Wake::Failed(err) => return Err(LoopError::Link(err)),
            Wake::Frame(frame) => {
                let flow = handler(link, LoopEvent::Frame(frame)).map_err(LoopError::Handler)?;
                if flow == Flow::Break {
                    return Ok(());
                }
            }
            Wake::Ticks => {
                let now = Instant::now();
                // Fire every ticker whose deadline has passed, in order; each
                // advances to the next whole-interval boundary (coalescing any
                // intervals missed while the loop was busy — the legacy's
                // `now - last >= interval` check, not a burst).
                for state in states.iter_mut() {
                    if state.deadline <= now {
                        state.fired += 1;
                        while state.deadline <= now {
                            state.deadline += state.interval;
                        }
                        let flow = handler(
                            link,
                            LoopEvent::Ticked {
                                name: state.name,
                                fired: state.fired,
                            },
                        )
                        .map_err(LoopError::Handler)?;
                        if flow == Flow::Break {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
