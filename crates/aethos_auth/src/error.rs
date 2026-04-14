use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("failed to hash password: {0}")]
    HashError(String),
    #[error("invalid token")]
    InvalidToken,
}
