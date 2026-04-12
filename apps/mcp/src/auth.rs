//! Bearer token authentication middleware for the MCP server.
//!
//! Compares the SHA-256 hash of the incoming token against the
//! pre-computed hash in `McpState`. The `/health` endpoint is exempt
//! (handled by router structure, not by this middleware).

use std::sync::Arc;

use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::state::McpState;

pub async fn bearer_auth(
    State(state): State<Arc<McpState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(t) => {
            let hash: [u8; 32] = Sha256::digest(t.as_bytes()).into();
            if hash == state.bearer_token_hash {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
