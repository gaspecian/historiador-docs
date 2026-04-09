use axum::Json;
use serde::Serialize;

/// MCP's own health response. Deliberately not shared with the API's
/// `HealthResponse`: MCP is not part of the REST API OpenAPI contract
/// (it speaks the MCP protocol in Sprint 2), so the shape is local.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub service: &'static str,
}

pub async fn handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        service: "mcp",
    })
}
