use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid department id: {0}")]
    InvalidDepartmentId(u32),

    #[error("invalid PLU number: {0}")]
    InvalidPluNumber(u64),

    #[error("PLU key {did}:{plu} exceeds XS 8-byte key limits (DID 0..=99, PLU 1..=9999999999999)")]
    PluKeyOutOfXsRange { did: u32, plu: u64 },

    #[error("invalid price in cents: {0}")]
    InvalidPrice(i64),

    #[error("permission denied: role {role:?} lacks {permission:?}")]
    PermissionDenied { role: String, permission: String },

    #[error("validation error: {0}")]
    Validation(String),
}
