//! Configuration loading and defaults.
//!
//! The daemon reads a `config.toml` at startup. Every field has a sensible
//! default so a missing or partial file still yields a usable configuration.

use std::path::Path;

use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub gpu: Gpu,
    pub display: Display,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct General {
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Gpu {
    pub poll_interval_secs: u64,
    pub temp_threshold_c: u32,
    pub util_threshold_pct: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Display {
    pub poll_interval_secs: u64,
}

impl Config {
    /// Load configuration from `path`. A missing file is not an error; the
    /// built-in defaults are returned instead.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        match std::fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).map_err(Error::from),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(Error::Io(e)),
        }
    }
}

impl Default for General {
    fn default() -> Self {
        General {
            log_level: "info".to_string(),
        }
    }
}

impl Default for Gpu {
    fn default() -> Self {
        Gpu {
            poll_interval_secs: 4,
            temp_threshold_c: 3,
            util_threshold_pct: 10,
        }
    }
}

impl Default for Display {
    fn default() -> Self {
        Display {
            poll_interval_secs: 4,
        }
    }
}
