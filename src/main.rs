//! Binary entry point: the Step 6.1 end-to-end driver over environment config.
//!
//! Everything orchestration-shaped lives in [`goonzu_client::client::driver`]
//! (`src/client/driver.rs`) so the hermetic Step 6.2 harness can drive the
//! exact same code path; this file is config + logging + runtime + exit
//! codes, nothing more.
//!
//! Exit codes: `0` clean session (any first-wins teardown cause), `2`
//! configuration error (a required variable missing or unparseable), `1`
//! session failure (the phase-tagged error is logged before exit).

use goonzu_client::client::driver::{self, DriverConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = match DriverConfig::from_env().await {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "driver configuration is invalid");
            std::process::exit(2);
        }
    };

    match driver::run(&config).await {
        Ok(report) => {
            tracing::info!(
                char_unique = report.char_unique,
                user_id = %report.user_id,
                selected_slot = report.selected_slot,
                selected_name = %report.selected_name,
                frames_dispatched = report.frames_dispatched,
                heartbeats_sent = report.heartbeats_sent,
                ?report.cause,
                "session complete"
            );
        }
        Err(error) => {
            tracing::error!(%error, "session failed");
            std::process::exit(1);
        }
    }
}
