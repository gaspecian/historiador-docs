//! `historiador_mcp` library surface.
//!
//! Exposes just enough of the internal wiring for integration tests to
//! build an in-process MCP router without touching the real env or DB.
//! Production code lives in `src/main.rs` which re-uses these exports.

pub mod application;
pub mod auth;
pub mod health;
pub mod infrastructure;
pub mod jsonrpc;
pub mod query;
pub mod state;

pub use state::McpState;

use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

/// Construct the MCP Axum router from a pre-built state. Shared between
/// `main.rs` (production) and integration tests (in-process).
pub fn build_router(state: Arc<McpState>) -> Router {
    let authed_routes = Router::new()
        .route("/mcp", post(jsonrpc::handler))
        .route("/query", post(query::handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::bearer_auth,
        ));

    Router::new()
        .route("/health", get(health::handler))
        .merge(authed_routes)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
