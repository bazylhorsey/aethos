use thiserror::Error;

#[derive(Debug, Error)]
pub enum AethosError {
    #[error("HTTP error: {0}")]
    Http(#[from] http::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Not found")]
    NotFound,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Internal error: {0}")]
    Internal(String),
}
