//! MOD-05: Non-blocking event loop & heartbeat, micro-step by micro-step
//! per `CONTRIBUTING.md`.
//!
//! - Step 5.1: [`event_loop`], the `tokio::select!` receive loop with a
//!   strict 100 ms read budget so an idle Game link never blocks its task,
//!   while background tickers keep firing.

pub mod event_loop;
