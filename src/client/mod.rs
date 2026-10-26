//! MOD-05: Non-blocking event loop & heartbeat, micro-step by micro-step
//! per `CONTRIBUTING.md`.
//!
//! - Step 5.1: [`event_loop`], the `tokio::select!` receive loop with a
//!   strict 100 ms read budget so an idle Game link never blocks its task,
//!   while background tickers keep firing.
//! - Step 5.2: [`heartbeat`], the 10-second keepalive ticker that pushes the
//!   header-only opcode 100 frame over the established game session.
//! - Step 5.3: [`teardown`], the graceful exit path — one first-wins
//!   shutdown flag (Ctrl+C or server disconnect), tracked background tasks,
//!   and a bounded drain, plus the loop's [`event_loop::run_with_shutdown`]
//!   flavour that honours the flag.

pub mod event_loop;
pub mod heartbeat;
pub mod teardown;
