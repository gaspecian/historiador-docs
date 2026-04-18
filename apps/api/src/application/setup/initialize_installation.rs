use std::sync::Arc;

use historiador_db::password as pw;

use crate::domain::error::ApplicationError;
use crate::domain::port::cipher::Cipher;
use crate::domain::port::llm_probe::LlmProbe;
use crate::domain::port::workspace_repository::{InitializeInstallation, WorkspaceRepository};
use crate::domain::value::{Email, Language};
use crate::infrastructure::llm::probe::LlmProvider;

use super::bcp47;
use super::defaults;

/// Strongly-typed command built by the presentation layer from the
/// HTTP DTO. Values are already trimmed / lightly validated; the use
/// case performs the cross-field and BCP 47 checks.
pub struct InitializeInstallationCommand {
    pub admin_email: String,
    pub admin_password: String,
    pub workspace_name: String,
    pub llm_provider: LlmProvider,
    pub llm_api_key: String,
    pub generation_model: Option<String>,
    pub embedding_model: Option<String>,
    pub languages: Vec<String>,
    pub primary_language: String,
}

pub struct InstallationInitialized {
    pub workspace_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}

pub struct InitializeInstallationUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
    llm_probe: Arc<dyn LlmProbe>,
    cipher: Arc<dyn Cipher>,
}

impl InitializeInstallationUseCase {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        llm_probe: Arc<dyn LlmProbe>,
        cipher: Arc<dyn Cipher>,
    ) -> Self {
        Self {
            workspaces,
            llm_probe,
            cipher,
        }
    }

    pub async fn execute(
        &self,
        cmd: InitializeInstallationCommand,
    ) -> Result<InstallationInitialized, ApplicationError> {
        bcp47::validate_pair(&cmd.languages, &cmd.primary_language)?;
        let email = Email::parse(&cmd.admin_email)?;

        // Probe BEFORE touching the DB so we never hold a transaction
        // open across a network call.
        self.llm_probe
            .probe(cmd.llm_provider, &cmd.llm_api_key)
            .await
            .map_err(|e| {
                ApplicationError::Domain(crate::domain::error::DomainError::Validation(format!(
                    "LLM key rejected: {e}"
                )))
            })?;

        let password_hash = pw::hash(&cmd.admin_password).map_err(ApplicationError::Infrastructure)?;

        let (encrypted_key, base_url): (Option<String>, Option<String>) = match cmd.llm_provider {
            LlmProvider::Ollama => (None, Some(cmd.llm_api_key.trim().to_string())),
            LlmProvider::Test => (None, None),
            LlmProvider::OpenAi | LlmProvider::Anthropic => {
                let ct = self.cipher.encrypt(&cmd.llm_api_key)?;
                (Some(ct), None)
            }
        };

        let generation_model = cmd
            .generation_model
            .unwrap_or_else(|| defaults::generation_model(cmd.llm_provider).to_string());
        let embedding_model = cmd
            .embedding_model
            .unwrap_or_else(|| defaults::embedding_model(cmd.llm_provider).to_string());

        let languages: Vec<Language> = cmd
            .languages
            .iter()
            .cloned()
            .map(Language::from_trusted)
            .collect();
        let primary_language = Language::from_trusted(cmd.primary_language);

        let result = self
            .workspaces
            .initialize(InitializeInstallation {
                workspace_name: cmd.workspace_name,
                languages,
                primary_language,
                llm_provider: cmd.llm_provider.as_db_str().to_string(),
                llm_api_key_encrypted: encrypted_key,
                llm_base_url: base_url,
                generation_model,
                embedding_model,
                admin_email: email,
                admin_password_hash: password_hash,
            })
            .await?;

        Ok(InstallationInitialized {
            workspace_id: result.workspace_id,
            user_id: result.admin_user_id,
        })
    }
}
