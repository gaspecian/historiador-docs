//! Route group nests. Each sub-router is mounted into the top-level
//! app tree by [`crate::app::build_router`].

use axum::{routing::post, Router};
use std::sync::Arc;

use crate::admin;
use crate::auth;
use crate::setup;
use crate::state::AppState;

pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(auth::handlers::login))
        .route("/refresh", post(auth::handlers::refresh))
        .route("/logout", post(auth::handlers::logout))
        .route("/activate", post(auth::handlers::activate))
}

pub fn setup_router() -> Router<Arc<AppState>> {
    Router::new().route("/init", post(setup::handler::init))
}

pub fn pages_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 3+): GET /, POST /, GET /:id, PUT /:id, POST /:id/publish
}

pub fn collections_router() -> Router<Arc<AppState>> {
    Router::new()
    // TODO(Sprint 3+): GET /, POST /, PUT /:id, DELETE /:id (with recursive CTE)
}

pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new().route("/users/invite", post(admin::users::invite))
}
