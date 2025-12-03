use thiserror::Error;
use std::io;

pub type Result<T> = std::result::Result<T, HauskiError>;

#[derive(Debug, Error)]
pub enum HauskiError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Serde JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Db(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Internal error: {0}")]
    Internal(String),

    // NOTE: We don't use #[from] here because `anyhow::Error` does not implement `std::error::Error`
    // in our vendored version (to avoid coherence issues), which `thiserror` requires for #[from]/#[source].
    // By omitting #[source], this variant is treated as data and doesn't expose the source error via `std::error::Error::source`.
    #[error("Other error: {0}")]
    Other(anyhow::Error),
}

impl From<anyhow::Error> for HauskiError {
    fn from(err: anyhow::Error) -> Self {
        HauskiError::Other(err)
    }
}
