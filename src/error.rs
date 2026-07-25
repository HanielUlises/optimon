//! Crate-wide error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse configuration: {0}")]
    Config(#[from] toml::de::Error),

    #[error("command `{command}` failed: {message}")]
    Command { command: String, message: String },
}
