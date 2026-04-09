use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Top-level error type for route handlers. Sprint 2 will grow this
/// with typed variants (NotFound, Forbidden, ValidationError, ...)
/// that map to appropriate HTTP status codes. In Sprint 1 every error
/// becomes a 500 — we have no handlers that can fail yet.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(error = ?self, "unhandled api error");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_server_error" })),
        )
            .into_response()
    }
}
