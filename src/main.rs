//! optimon — a concurrent monitoring daemon for NVIDIA Optimus laptops.
//!
//! v0.1 runs independent watcher tasks that poll GPU and display state and log
//! meaningful changes. The main task loads configuration, starts the watchers,
//! and waits for Ctrl+C before signalling a graceful shutdown.

mod action;
mod config;
mod display;
mod error;
mod gpu;
mod watcher;

use std::path::PathBuf;

use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

use config::Config;

const CONFIG_PATH: &str = "config.toml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load(config_path())?;
    init_tracing(&config.general.log_level);

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting optimon");
    tracing::debug!(?config, "loaded configuration");

    // `false` = running, `true` = shutdown requested.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handles = watcher::spawn_all(&config, shutdown_rx);

    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown requested, stopping watchers");

    // Ignore send errors: they only occur if every receiver has already
    // dropped, in which case there is nothing left to stop.
    let _ = shutdown_tx.send(true);
    for handle in handles {
        if let Err(e) = handle.await {
            tracing::warn!(error = %e, "watcher task did not shut down cleanly");
        }
    }

    tracing::info!("stopped");
    Ok(())
}

/// Resolve the config path, honoring an optional `OPTIMON_CONFIG` override.
fn config_path() -> PathBuf {
    std::env::var_os("OPTIMON_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CONFIG_PATH))
}

/// Initialize `tracing`. The `RUST_LOG` environment variable, if set, takes
/// precedence over the configured log level.
fn init_tracing(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("optimon={level}")));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
