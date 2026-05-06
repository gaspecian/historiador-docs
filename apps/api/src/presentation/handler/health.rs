use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

/// Response body for `GET /health`. Exposed as an OpenAPI schema so
/// stretch item 7 (openapi-typescript) can generate a matching
/// TypeScript type for the frontend.
#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub git_sha: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "service is healthy", body = HealthResponse)
    ),
    tag = "system"
)]
pub async fn handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        git_sha: state.git_sha.clone(),
    })
}
