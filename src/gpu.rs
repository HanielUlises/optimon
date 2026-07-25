//! GPU state probing.
//!
//! Two distinct pieces of information are exposed:
//!
//! * The *active* GPU as selected by PRIME (`prime-select`), i.e. whether the
//!   system is currently rendering on the Intel iGPU or the NVIDIA dGPU.
//! * Live NVIDIA telemetry (temperature and utilization), read through NVML
//!   when the library and driver are available, with a `nvidia-smi` fallback.

use std::fmt;
use std::process::Command;

use nvml_wrapper::Nvml;

use crate::error::Error;

/// The GPU PRIME is currently configured to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveGpu {
    Intel,
    Nvidia,
    /// PRIME "on-demand" mode, where render offloading is decided per process.
    OnDemand,
    Unknown,
}

impl fmt::Display for ActiveGpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ActiveGpu::Intel => "intel",
            ActiveGpu::Nvidia => "nvidia",
            ActiveGpu::OnDemand => "on-demand",
            ActiveGpu::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// A snapshot of live NVIDIA telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NvidiaStats {
    /// Core temperature in degrees Celsius.
    pub temperature_c: u32,
    /// GPU utilization as a percentage (0-100).
    pub utilization_pct: u32,
}

/// Queries GPU state. Holds an optional NVML handle so the library is
/// initialized once and reused across polls.
pub struct GpuProbe {
    nvml: Option<Nvml>,
}

impl GpuProbe {
    /// Initialize the probe. NVML initialization failure is non-fatal: the
    /// probe simply falls back to `nvidia-smi` for telemetry.
    pub fn new() -> Self {
        let nvml = match Nvml::init() {
            Ok(nvml) => Some(nvml),
            Err(e) => {
                tracing::debug!(error = %e, "NVML unavailable, will fall back to nvidia-smi");
                None
            }
        };
        GpuProbe { nvml }
    }

    /// Read the active GPU from `prime-select query`.
    pub fn active_gpu(&self) -> ActiveGpu {
        let output = match Command::new("prime-select").arg("query").output() {
            Ok(o) => o,
            Err(e) => {
                tracing::debug!(error = %e, "prime-select not available");
                return ActiveGpu::Unknown;
            }
        };

        if !output.status.success() {
            return ActiveGpu::Unknown;
        }

        match String::from_utf8_lossy(&output.stdout).trim() {
            "intel" => ActiveGpu::Intel,
            "nvidia" => ActiveGpu::Nvidia,
            "on-demand" => ActiveGpu::OnDemand,
            _ => ActiveGpu::Unknown,
        }
    }

    /// Read NVIDIA telemetry. Returns `None` when no NVIDIA GPU is reachable
    /// (e.g. the dGPU is powered down under PRIME).
    pub fn nvidia_stats(&self) -> Option<NvidiaStats> {
        if let Some(nvml) = &self.nvml {
            match Self::nvidia_stats_nvml(nvml) {
                Ok(stats) => return Some(stats),
                Err(e) => tracing::trace!(error = %e, "NVML query failed, trying nvidia-smi"),
            }
        }
        Self::nvidia_stats_smi()
    }

    fn nvidia_stats_nvml(nvml: &Nvml) -> Result<NvidiaStats, Error> {
        let device = nvml.device_by_index(0).map_err(|e| Error::Command {
            command: "nvml".to_string(),
            message: e.to_string(),
        })?;

        let temperature_c = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .map_err(|e| Error::Command {
                command: "nvml".to_string(),
                message: e.to_string(),
            })?;

        let utilization_pct = device
            .utilization_rates()
            .map_err(|e| Error::Command {
                command: "nvml".to_string(),
                message: e.to_string(),
            })?
            .gpu;

        Ok(NvidiaStats {
            temperature_c,
            utilization_pct,
        })
    }

    fn nvidia_stats_smi() -> Option<NvidiaStats> {
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=temperature.gpu,utilization.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let line = String::from_utf8_lossy(&output.stdout);
        let line = line.trim().lines().next()?;
        let mut parts = line.split(',').map(str::trim);

        let temperature_c = parts.next()?.parse().ok()?;
        let utilization_pct = parts.next()?.parse().ok()?;

        Some(NvidiaStats {
            temperature_c,
            utilization_pct,
        })
    }
}

impl Default for GpuProbe {
    fn default() -> Self {
        Self::new()
    }
}
