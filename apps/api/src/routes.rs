//! Route group nests. Every feature area has a placeholder router so
//! Sprint 2 can drop real handlers into the right module without
//! touching `main.rs`.

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

/// Authentication routes — login, logout, setup wizard, session refresh.
pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 2): POST /login, POST /logout, POST /setup
}

/// Page CRUD + publish + chunker trigger.
pub fn pages_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 2): GET /, POST /, GET /:id, PUT /:id, POST /:id/publish
}

/// Nested collection tree management.
pub fn collections_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 2): GET /, POST /, PUT /:id, DELETE /:id (with recursive CTE)
}

/// Admin — user management, workspace settings, MCP token rotation.
pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 2): GET /users, POST /users/invite, PATCH /workspace, POST /mcp-token/rotate
}
