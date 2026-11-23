use openscale_domain::DomainError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid uuid in database: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("invalid timestamp in database: {0}")]
    Timestamp(#[from] chrono::ParseError),

    #[error("domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("invalid enum value '{value}' for column '{column}'")]
    InvalidEnumValue { column: &'static str, value: String },
}
