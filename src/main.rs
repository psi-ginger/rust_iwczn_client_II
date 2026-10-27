//! Binary entry point: the Step 6.1 end-to-end driver over environment config.
//!
//! Everything orchestration-shaped lives in [`goonzu_client::client::driver`]
//! (`src/client/driver.rs`) so the hermetic Step 6.2 harness can drive the
//! exact same code path; this file is environment -> config -> runtime ->
//! exit codes, nothing more.
//!
//! Exit codes (WO-6.1 §5): `0` a session that ended at `Stage::TornDown`, `1`
//! a runtime failure (the failing stage is part of the logged error), `2` a
//! configuration error (a required variable missing or unparseable).

use goonzu_client::client::driver::{self, DriverConfig, DriverError};
use goonzu_client::client::teardown::Shutdown;
use goonzu_client::CLIENT_VERSION;

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
