//! Domain + application error types. Carries **no HTTP status codes**:
//! the presentation layer maps these into `ApiError` / HTTP responses.

use thiserror::Error;

/// Business-rule error — produced by use cases or ports when a domain
/// invariant is violated. Always safe to reflect back to the caller.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Conflict(String),

    #[error("forbidden")]
    Forbidden,
}

/// Error returned by ports and use cases. Wraps a `DomainError` for
/// business violations and an opaque `anyhow::Error` for adapter-level
/// failures (database down, network error, bug). The presentation
/// layer turns the former into 4xx responses and the latter into 500.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Infrastructure(#[from] anyhow::Error),
}
