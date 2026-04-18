//! Router construction shared between `main.rs` and integration tests.
//!
//! Keeping this in one place means the e2e test exercises the exact
//! route tree, middleware stack, and state layout that production
//! runs — no "test-only wrapper" divergence.

use std::sync::Arc;

use axum::{middleware, routing::get, Router};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::presentation::handler::health;
use crate::presentation::middleware::setup_gate::setup_gate;
use crate::presentation::openapi::ApiDoc;
use crate::routes;
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        .route("/health", get(health::handler))
        .nest("/auth", routes::auth_router())
        .nest("/setup", routes::setup_router())
        .nest("/pages", routes::pages_router())
        .nest("/collections", routes::collections_router())
        .nest("/admin", routes::admin_router())
        .nest("/editor", routes::editor_router())
        .nest("/export", routes::export_router())
        .layer(middleware::from_fn_with_state(state.clone(), setup_gate))
        // Internal routes — no setup gate, no JWT auth. Protected by
        // network topology (localhost/Docker only).
        .nest("/internal", routes::internal_router())
        .with_state(state);

    // Swagger UI at /docs, OpenAPI JSON at /api-docs/openapi.json.
    // Merged *after* `with_state` so the swagger router (which has no
    // state) doesn't conflict, and *outside* the setup gate so the
    // contract is readable even before the installation is configured.
    Router::new()
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api_routes)
        .layer(TraceLayer::new_for_http())
}
