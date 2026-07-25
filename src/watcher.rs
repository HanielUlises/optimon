//! Watcher tasks.
//!
//! Each watcher runs as an independent Tokio task on its own polling interval.
//! Watchers keep a private in-memory snapshot of the last observed state and
//! emit a log line only when the state changes meaningfully. All watchers
//! observe a shared shutdown signal so `main` can stop them cleanly.

use std::collections::BTreeMap;

use tokio::sync::watch;
use tokio::time::{self, Duration, MissedTickBehavior};

use crate::config::Config;
use crate::display::{self, Monitor};
use crate::gpu::{ActiveGpu, GpuProbe, NvidiaStats};

/// Spawn every watcher and return their join handles.
///
/// `shutdown` is a receiver that flips to `true` when the daemon should stop.
pub fn spawn_all(
    config: &Config,
    shutdown: watch::Receiver<bool>,
) -> Vec<tokio::task::JoinHandle<()>> {
    vec![
        tokio::spawn(gpu_watcher(config.gpu.clone(), shutdown.clone())),
        tokio::spawn(display_watcher(config.display.clone(), shutdown)),
    ]
}

/// Poll GPU state: the active PRIME GPU and NVIDIA telemetry.
async fn gpu_watcher(config: crate::config::Gpu, mut shutdown: watch::Receiver<bool>) {
    // GpuProbe (and its NVML handle) is not `Send`-friendly to hold across
    // awaits, but we only ever touch it between ticks on this task, so a local
    // binding is fine.
    let probe = GpuProbe::new();
    let mut ticker = interval(config.poll_interval_secs);

    let mut last_gpu: Option<ActiveGpu> = None;
    let mut last_stats: Option<NvidiaStats> = None;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let gpu = probe.active_gpu();
                if last_gpu != Some(gpu) {
                    match last_gpu {
                        Some(prev) => tracing::info!(from = %prev, to = %gpu, "active GPU changed"),
                        None => tracing::info!(gpu = %gpu, "active GPU"),
                    }
                    last_gpu = Some(gpu);
                }

                if let Some(stats) = probe.nvidia_stats() {
                    if stats_changed(last_stats, stats, &config) {
                        tracing::info!(
                            temp_c = stats.temperature_c,
                            util_pct = stats.utilization_pct,
                            "NVIDIA telemetry",
                        );
                        last_stats = Some(stats);
                    }
                } else if last_stats.take().is_some() {
                    tracing::info!("NVIDIA GPU no longer reachable");
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::debug!("gpu watcher stopping");
                    return;
                }
            }
        }
    }
}

/// Poll connected displays.
async fn display_watcher(config: crate::config::Display, mut shutdown: watch::Receiver<bool>) {
    let mut ticker = interval(config.poll_interval_secs);
    let mut last: Option<BTreeMap<String, Monitor>> = None;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let current = match display::connected_monitors() {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to query displays");
                        continue;
                    }
                };

                if let Some(prev) = &last {
                    log_display_diff(prev, &current);
                } else {
                    for m in current.values() {
                        tracing::info!(output = %m.name, resolution = ?m.resolution, "display present");
                    }
                }
                last = Some(current);
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::debug!("display watcher stopping");
                    return;
                }
            }
        }
    }
}

/// Whether NVIDIA telemetry moved enough to be worth logging.
fn stats_changed(last: Option<NvidiaStats>, current: NvidiaStats, config: &crate::config::Gpu) -> bool {
    match last {
        None => true,
        Some(prev) => {
            prev.temperature_c.abs_diff(current.temperature_c) >= config.temp_threshold_c
                || prev.utilization_pct.abs_diff(current.utilization_pct)
                    >= config.util_threshold_pct
        }
    }
}

/// Log connections, disconnections, and resolution changes between two scans.
fn log_display_diff(prev: &BTreeMap<String, Monitor>, current: &BTreeMap<String, Monitor>) {
    for (name, monitor) in current {
        match prev.get(name) {
            None => tracing::info!(output = %name, resolution = ?monitor.resolution, "monitor connected"),
            Some(old) if old.resolution != monitor.resolution => {
                tracing::info!(
                    output = %name,
                    from = ?old.resolution,
                    to = ?monitor.resolution,
                    "monitor resolution changed",
                );
            }
            Some(_) => {}
        }
    }

    for name in prev.keys() {
        if !current.contains_key(name) {
            tracing::info!(output = %name, "monitor disconnected");
        }
    }
}

/// Build an interval timer that skips missed ticks rather than bursting to
/// catch up (which would happen if the system was suspended).
fn interval(secs: u64) -> time::Interval {
    let mut ticker = time::interval(Duration::from_secs(secs.max(1)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}
