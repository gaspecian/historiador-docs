use std::sync::Arc;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::cipher::Cipher;
use crate::domain::port::llm_probe::LlmProbe;
use crate::domain::port::page_repository::PageRepository;
use crate::domain::port::workspace_repository::{LlmConfigPatch, WorkspaceRepository};
use crate::domain::value::{Actor, Role};
use crate::infrastructure::llm::probe::LlmProvider;

pub struct UpdateLlmConfigCommand {
    pub llm_provider: LlmProvider,
    /// Empty string → keep the existing secret/base URL; otherwise
    /// probe + persist the new value.
    pub llm_api_key: String,
    pub generation_model: String,
    pub embedding_model: String,
}

pub struct UpdateLlmConfigResult {
    pub requires_reindex: bool,
    pub affected_page_versions: i64,
    pub requires_restart: bool,
}

pub struct UpdateLlmConfigUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
    pages: Arc<dyn PageRepository>,
    probe: Arc<dyn LlmProbe>,
    cipher: Arc<dyn Cipher>,
}

impl UpdateLlmConfigUseCase {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        pages: Arc<dyn PageRepository>,
        probe: Arc<dyn LlmProbe>,
        cipher: Arc<dyn Cipher>,
    ) -> Self {
        Self {
            workspaces,
            pages,
            probe,
            cipher,
        }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: UpdateLlmConfigCommand,
    ) -> Result<UpdateLlmConfigResult, ApplicationError> {
        actor.require_role(Role::Admin)?;

        let ws = self
            .workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        // Only probe when the admin rotates the secret.
        if !cmd.llm_api_key.is_empty() {
            self.probe
                .probe(cmd.llm_provider, &cmd.llm_api_key)
                .await
                .map_err(|e| {
                    ApplicationError::Domain(DomainError::Validation(format!(
                        "LLM rejected: {e}"
                    )))
                })?;
        }

        let (encrypted_key, base_url): (Option<String>, Option<String>) = match cmd.llm_provider {
            LlmProvider::Ollama => {
                if cmd.llm_api_key.is_empty() {
                    (None, ws.llm_base_url.clone())
                } else {
                    (None, Some(cmd.llm_api_key.trim().to_string()))
                }
            }
            LlmProvider::Test => (None, None),
            LlmProvider::OpenAi | LlmProvider::Anthropic => {
                if cmd.llm_api_key.is_empty() {
                    (None, None)
                } else {
                    let ct = self.cipher.encrypt(&cmd.llm_api_key)?;
                    (Some(ct), None)
                }
            }
        };

        self.workspaces
            .update_llm_config(
                actor.workspace_id,
                LlmConfigPatch {
                    llm_provider: cmd.llm_provider.as_db_str().to_string(),
                    llm_api_key_encrypted: encrypted_key,
                    llm_base_url: base_url,
                    generation_model: cmd.generation_model.clone(),
                    embedding_model: cmd.embedding_model.clone(),
                },
            )
            .await?;

        let embedding_changed = cmd.embedding_model != ws.embedding_model;
        let affected_page_versions = if embedding_changed {
            self.pages
                .find_all_published_in_workspace(actor.workspace_id)
                .await?
                .len() as i64
        } else {
            0
        };

        let generation_changed = cmd.generation_model != ws.generation_model
            || cmd.llm_provider.as_db_str() != ws.llm_provider;

        Ok(UpdateLlmConfigResult {
            requires_reindex: embedding_changed && affected_page_versions > 0,
            affected_page_versions,
            requires_restart: generation_changed,
        })
    }
}
