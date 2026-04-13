use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrmError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("record not found")]
    NotFound,

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("migration error: {0}")]
    Migration(String),
}
