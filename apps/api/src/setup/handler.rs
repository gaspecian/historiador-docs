//! `POST /setup/init` — first-run installation.
//!
//! Flow:
//!   1. Fast-fail if `installation.setup_complete` is already TRUE.
//!   2. Validate the request DTO (email format, password length,
//!      BCP 47 tags, `primary_language ∈ languages`).
//!   3. Probe the LLM provider with the supplied key. **Runs before
//!      the DB transaction** — we never hold a transaction open
//!      across a network call.
//!   4. Open a transaction: insert workspace → insert admin user →
//!      mark installation complete → commit.
//!   5. Flip the cached setup-complete flag so the gate middleware
//!      stops returning 423.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::{extract::State, Json};
use historiador_db::{
    password,
    postgres::{installation, users, workspaces},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::error::ApiError;
use crate::setup::llm_probe::LlmProvider;
use crate::state::AppState;

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct SetupRequest {
    #[validate(email)]
    pub admin_email: String,

    #[validate(length(min = 12, max = 256))]
    pub admin_password: String,

    #[validate(length(min = 1, max = 100))]
    pub workspace_name: String,

    pub llm_provider: LlmProvider,

    #[validate(length(min = 1, max = 512))]
    pub llm_api_key: String,

    #[validate(length(min = 1, max = 16))]
    pub languages: Vec<String>,

    #[validate(length(min = 2, max = 32))]
    pub primary_language: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SetupResponse {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub setup_complete: bool,
}

/// Validate a BCP 47 language tag. Accepts the small subset used by
/// v1 (2–3 letter primary tag, optional 2-letter region subtag).
/// Tighten later if/when we allow script/variant subtags.
fn is_valid_bcp47(tag: &str) -> bool {
    // Lazy static via once_cell would save a recompile per call but
    // setup runs at most once; a local construction is fine.
    let re = Regex::new(r"^[a-z]{2,3}(-[A-Z]{2})?$").unwrap();
    re.is_match(tag)
}

fn validate_languages(languages: &[String], primary: &str) -> Result<(), ApiError> {
    for tag in languages {
        if !is_valid_bcp47(tag) {
            return Err(ApiError::Validation(format!(
                "invalid BCP 47 language tag: {tag}"
            )));
        }
    }
    if !is_valid_bcp47(primary) {
        return Err(ApiError::Validation(format!(
            "invalid BCP 47 primary_language: {primary}"
        )));
    }
    if !languages.iter().any(|l| l == primary) {
        return Err(ApiError::Validation(
            "primary_language must be one of languages".into(),
        ));
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/setup/init",
    request_body = SetupRequest,
    responses(
        (status = 200, description = "installation initialized", body = SetupResponse),
        (status = 400, description = "validation error or LLM key rejected"),
        (status = 409, description = "setup already complete"),
    ),
    tag = "setup"
)]
pub async fn init(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, ApiError> {
    // 1. Gate: not already complete.
    if state.setup_complete.load(Ordering::Acquire) {
        return Err(ApiError::Conflict("setup already complete".into()));
    }

    // 2a. DTO validation.
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    // 2b. Language cross-field + BCP 47.
    validate_languages(&body.languages, &body.primary_language)?;

    // 3. LLM probe — before we touch the DB. Network timeout is 10s
    //    (configured in HttpLlmProbe::default).
    state
        .llm_probe
        .probe(body.llm_provider, &body.llm_api_key)
        .await
        .map_err(|e| ApiError::Validation(format!("LLM key rejected: {e}")))?;

    // 4. Secrets we persist: hash the admin password, encrypt the
    //    LLM key. Both are pure CPU work, no await.
    let password_hash = password::hash(&body.admin_password).map_err(ApiError::Internal)?;
    let encrypted_key = state
        .cipher
        .encrypt(&body.llm_api_key)
        .map_err(ApiError::Internal)?;

    // 5. Transactional insert.
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let workspace_id = workspaces::insert(
        &mut tx,
        workspaces::NewWorkspace {
            name: &body.workspace_name,
            languages: &body.languages,
            primary_language: &body.primary_language,
            llm_provider: body.llm_provider.as_db_str(),
            llm_api_key_encrypted: &encrypted_key,
        },
    )
    .await
    .map_err(ApiError::Internal)?;

    let user_id =
        users::insert_admin(&mut tx, workspace_id, &body.admin_email, &password_hash)
            .await
            .map_err(ApiError::Internal)?;

    installation::mark_complete(&mut tx)
        .await
        .map_err(ApiError::Internal)?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    // 6. Flip the cached flag so the gate middleware stops blocking.
    state.setup_complete.store(true, Ordering::Release);

    Ok(Json(SetupResponse {
        workspace_id,
        user_id,
        setup_complete: true,
    }))
}

// ---- probe (test connection without completing setup) ----

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ProbeRequest {
    pub llm_provider: LlmProvider,
    #[serde(default)]
    pub llm_api_key: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProbeResponse {
    pub success: bool,
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/setup/probe",
    request_body = ProbeRequest,
    responses(
        (status = 200, description = "probe result", body = ProbeResponse),
    ),
    tag = "setup"
)]
pub async fn probe(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProbeRequest>,
) -> Result<Json<ProbeResponse>, ApiError> {
    match state.llm_probe.probe(body.llm_provider, &body.llm_api_key).await {
        Ok(()) => Ok(Json(ProbeResponse {
            success: true,
            message: "connection successful".into(),
        })),
        Err(e) => Ok(Json(ProbeResponse {
            success: false,
            message: format!("{e}"),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcp47_accepts_common_tags() {
        assert!(is_valid_bcp47("en"));
        assert!(is_valid_bcp47("pt-BR"));
        assert!(is_valid_bcp47("en-US"));
        assert!(is_valid_bcp47("fra"));
    }

    #[test]
    fn bcp47_rejects_bad_shapes() {
        assert!(!is_valid_bcp47(""));
        assert!(!is_valid_bcp47("EN"));
        assert!(!is_valid_bcp47("pt_BR"));
        assert!(!is_valid_bcp47("english"));
        assert!(!is_valid_bcp47("pt-br"));
    }

    #[test]
    fn primary_must_be_in_languages() {
        let res = validate_languages(&["pt-BR".into(), "en-US".into()], "es-ES");
        assert!(res.is_err());
    }

    #[test]
    fn valid_language_config_passes() {
        let res = validate_languages(&["pt-BR".into(), "en-US".into()], "pt-BR");
        assert!(res.is_ok());
    }
}
