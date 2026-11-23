use openscale_domain::DomainError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("not connected to scale")]
    NotConnected,

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("timeout waiting for scale response")]
    Timeout,

    #[error("protocol error (code {code:?}): {message}")]
    Protocol { code: Option<i32>, message: String },

    #[error("authentication required by scale")]
    AuthenticationRequired,

    #[error("unsupported operation for this driver: {0}")]
    Unsupported(&'static str),

    #[error("record not found")]
    NotFound,

    #[error("domain error: {0}")]
    Domain(#[from] DomainError),
}
