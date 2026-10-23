//! Binary entry point for the GoonZu client.
//!
//! Step 1.1 boots the minimal runtime scaffold: initialize structured logging
//! and start the single-threaded tokio runtime on which later steps (TCP
//! sessions, framed codec, event loop) will be scheduled.

use goonzu_client::CLIENT_VERSION;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    tracing::info!(version = CLIENT_VERSION, "goonzu client scaffold ready");
}
