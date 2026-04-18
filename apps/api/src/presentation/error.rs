use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::domain::error::{ApplicationError, DomainError};

/// Top-level error type for route handlers.
///
/// Every variant maps to a known HTTP status. `Internal` is the only
/// variant that logs at `ERROR` level — client-triggered errors
/// (401/403/404/etc) are expected traffic and stay quiet.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Conflict(String),

    #[error("setup required")]
    SetupRequired,

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    fn code_and_message(&self) -> (StatusCode, &'static str, String) {
        match self {
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".into(),
            ),
            ApiError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "insufficient role".into(),
            ),
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "resource not found".into(),
            ),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, "validation_error", msg.clone()),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            ApiError::SetupRequired => (
                StatusCode::LOCKED,
                "setup_required",
                "installation setup is not complete".into(),
            ),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                "internal server error".into(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        if let ApiError::Internal(err) = &self {
            tracing::error!(error = ?err, "internal api error");
        }
        let (status, code, message) = self.code_and_message();
        (status, Json(json!({ "error": code, "message": message }))).into_response()
    }
}

impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::NotFound => ApiError::NotFound,
            DomainError::Validation(msg) => ApiError::Validation(msg),
            DomainError::Conflict(msg) => ApiError::Conflict(msg),
            DomainError::Forbidden => ApiError::Forbidden,
        }
    }
}

impl From<ApplicationError> for ApiError {
    fn from(e: ApplicationError) -> Self {
        match e {
            ApplicationError::Domain(d) => d.into(),
            ApplicationError::Infrastructure(err) => ApiError::Internal(err),
        }
    }
}
