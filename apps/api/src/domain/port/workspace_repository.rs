use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::Workspace;
use crate::domain::error::ApplicationError;
use crate::domain::value::{Email, Language};

/// Bundle passed into the transactional setup initialization. All three
/// rows (workspace, admin user, installation flag) must commit together.
#[derive(Debug, Clone)]
pub struct InitializeInstallation {
    pub workspace_name: String,
    pub languages: Vec<Language>,
    pub primary_language: Language,
    pub llm_provider: String,
    pub llm_api_key_encrypted: Option<String>,
    pub llm_base_url: Option<String>,
    pub generation_model: String,
    pub embedding_model: String,
    pub admin_email: Email,
    pub admin_password_hash: String,
}

/// Result of a successful setup initialization.
#[derive(Debug, Clone)]
pub struct InstallationBootstrapped {
    pub workspace_id: Uuid,
    pub admin_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct LlmConfigPatch {
    pub llm_provider: String,
    /// `None` means "leave the existing ciphertext untouched".
    pub llm_api_key_encrypted: Option<String>,
    pub llm_base_url: Option<String>,
    pub generation_model: String,
    pub embedding_model: String,
}

#[async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Workspace>, ApplicationError>;

    async fn find_singleton(&self) -> Result<Option<Workspace>, ApplicationError>;

    /// Atomic setup — inserts the workspace, the initial admin user,
    /// and flips the installation flag in a single transaction.
    async fn initialize(
        &self,
        input: InitializeInstallation,
    ) -> Result<InstallationBootstrapped, ApplicationError>;

    /// Rotate the MCP bearer token hash. Returns true iff a row was
    /// updated.
    async fn update_mcp_token(
        &self,
        workspace_id: Uuid,
        new_token_hash: &str,
    ) -> Result<bool, ApplicationError>;

    async fn update_llm_config(
        &self,
        workspace_id: Uuid,
        patch: LlmConfigPatch,
    ) -> Result<bool, ApplicationError>;
}
