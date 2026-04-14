//! Setup gate: until `POST /setup/init` has run successfully, every
//! route except `/health` and `/setup/init` returns 423 Locked.
//!
//! The flag is cached in `AppState.setup_complete` (AtomicBool) —
//! seeded once at startup from the `installation` row, flipped once
//! by the setup handler on successful commit. No per-request DB
//! read.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::error::ApiError;
use crate::state::AppState;

/// Paths that remain reachable while setup is incomplete.
fn is_allowed_pre_setup(path: &str) -> bool {
    matches!(
        path,
        "/health" | "/setup/init" | "/setup/probe" | "/setup/ollama-models"
    )
}

pub async fn setup_gate(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if state.setup_complete.load(Ordering::Acquire) {
        return Ok(next.run(request).await);
    }
    if is_allowed_pre_setup(request.uri().path()) {
        return Ok(next.run(request).await);
    }
    Err(ApiError::SetupRequired)
}
