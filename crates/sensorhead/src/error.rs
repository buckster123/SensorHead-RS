//! Parse and contract errors. These are never a fake success.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("thermal frame length {got}, expected {expected}")]
    ThermalFrameLen { got: usize, expected: usize },
}

pub type Result<T> = std::result::Result<T, Error>;
