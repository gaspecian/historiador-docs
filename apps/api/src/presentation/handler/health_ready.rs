//! Readiness probe — reports the boot-time vector store backfill
//! status. Distinct from `/health` (liveness) so load balancers can
//! hold off traffic until backfill completes without ever reporting
//! the process as dead.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::infrastructure::backfill::BackfillState;
use crate::state::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub backfill: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[utoipa::path(
    get,
    path = "/health/ready",
    operation_id = "health_ready",
    responses(
        (status = 200, description = "service is ready (or backfill disabled / completed)", body = ReadyResponse),
        (status = 503, description = "backfill is running or failed", body = ReadyResponse),
    ),
    tag = "system"
)]
pub async fn handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = state
        .backfill_state
        .read()
        .expect("backfill state lock poisoned")
        .clone();
    response_for(&snapshot)
}

fn response_for(state: &BackfillState) -> (StatusCode, Json<ReadyResponse>) {
    match state {
        BackfillState::Disabled => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready",
                backfill: "disabled",
                synced: None,
                failed: None,
                reason: None,
            }),
        ),
        BackfillState::Running => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready",
                backfill: "running",
                synced: None,
                failed: None,
                reason: None,
            }),
        ),
        BackfillState::Completed { synced, failed } => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready",
                backfill: "completed",
                synced: Some(*synced),
                failed: Some(*failed),
                reason: None,
            }),
        ),
        BackfillState::Failed { reason } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready",
                backfill: "failed",
                synced: None,
                failed: None,
                reason: Some(reason.clone()),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_200_ready() {
        let (code, body) = response_for(&BackfillState::Disabled);
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body.status, "ready");
        assert_eq!(body.backfill, "disabled");
    }

    #[test]
    fn running_returns_503_not_ready() {
        let (code, body) = response_for(&BackfillState::Running);
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.backfill, "running");
    }

    #[test]
    fn completed_returns_200_with_counts() {
        let (code, body) = response_for(&BackfillState::Completed {
            synced: 3,
            failed: 1,
        });
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body.backfill, "completed");
        assert_eq!(body.synced, Some(3));
        assert_eq!(body.failed, Some(1));
    }

    #[test]
    fn failed_returns_503_with_reason() {
        let (code, body) = response_for(&BackfillState::Failed {
            reason: "kaboom".into(),
        });
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.backfill, "failed");
        assert_eq!(body.reason.as_deref(), Some("kaboom"));
    }
}
