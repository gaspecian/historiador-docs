//! Bearer token authentication middleware for the MCP server.
//!
//! Compares the SHA-256 hash of the incoming token against the
//! pre-computed hash in `McpState` using `subtle::ConstantTimeEq`. The
//! `/health` endpoint is exempt (handled by router structure, not by
//! this middleware).
//!
//! Hashing both sides to fixed-length digests is what makes the
//! comparison resistant to length-based timing side-channels;
//! `ConstantTimeEq` makes the byte-by-byte comparison resistant to
//! prefix-based timing side-channels.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

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
            if verify_token_hash(&hash, &state.bearer_token_hash) {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Constant-time comparison of two 32-byte SHA-256 digests.
///
/// Extracted for direct test coverage — exercising `bearer_auth`
/// requires a full `McpState`, which is overkill for a pure
/// comparison invariant.
fn verify_token_hash(provided: &[u8; 32], expected: &[u8; 32]) -> bool {
    provided.ct_eq(expected).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(s: &str) -> [u8; 32] {
        Sha256::digest(s.as_bytes()).into()
    }

    #[test]
    fn accepts_matching_token() {
        let expected = digest("correct-bearer-token");
        let provided = digest("correct-bearer-token");
        assert!(verify_token_hash(&provided, &expected));
    }

    #[test]
    fn rejects_wrong_token() {
        let expected = digest("correct-bearer-token");
        let provided = digest("wrong-bearer-token");
        assert!(!verify_token_hash(&provided, &expected));
    }

    /// Regression test for finding 4.1 of the v1.0 code review.
    ///
    /// A raw-byte `==` comparison on the plaintext tokens would leak
    /// prefix information through timing. The defence is twofold:
    /// (a) both sides are hashed to fixed-length digests, so a shorter
    /// guess does not produce a shorter comparison, and (b) the digest
    /// comparison itself is constant-time via `subtle::ConstantTimeEq`.
    /// The observable behaviour we assert here: a token that is a
    /// prefix of the real token must return false, and must do so
    /// through the same code path as any other mismatch.
    #[test]
    fn rejects_prefix_of_real_token() {
        let expected = digest("correct-bearer-token");
        let provided = digest("correct-bearer"); // prefix attack attempt
        assert!(!verify_token_hash(&provided, &expected));
    }

    #[test]
    fn rejects_empty_token() {
        let expected = digest("correct-bearer-token");
        let provided = digest("");
        assert!(!verify_token_hash(&provided, &expected));
    }
}
