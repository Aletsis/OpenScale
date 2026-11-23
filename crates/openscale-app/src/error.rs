use openscale_domain::DomainError;
use openscale_driver::DriverError;
use openscale_storage::StorageError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("scale {0} is not registered in ScaleManager")]
    ScaleNotRegistered(Uuid),

    #[error("scale actor channel closed")]
    ActorUnavailable,

    #[error("domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("driver error: {0}")]
    Driver(#[from] DriverError),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}
