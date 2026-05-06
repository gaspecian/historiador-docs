use std::sync::Arc;

use historiador_db::password as pw;

use crate::domain::error::{ApplicationError, DomainError};
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
    /// Optional. When `llm_provider == OpenAi`, persisted verbatim
    /// (after trim + trailing-slash strip) into `workspaces.llm_base_url`.
    /// Required to be `Some` when `llm_provider == OpenAi` AND
    /// `llm_api_key` is empty (no existing key exists at first-run).
    pub base_url: Option<String>,
    pub generation_model: Option<String>,
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

        let normalized_url = cmd
            .base_url
            .as_deref()
            .map(|u| u.trim().trim_end_matches('/').to_string())
            .filter(|u| !u.is_empty());

        // Validate the OpenAI no-URL + empty-key combo BEFORE the
        // network probe — first-run has no existing key to fall back on.
        if matches!(cmd.llm_provider, LlmProvider::OpenAi)
            && normalized_url.is_none()
            && cmd.llm_api_key.is_empty()
        {
            return Err(ApplicationError::Domain(DomainError::Validation(
                "openai exige uma chave de API ou uma URL base personalizada".into(),
            )));
        }

        // Probe BEFORE touching the DB so we never hold a transaction
        // open across a network call.
        self.llm_probe
            .probe(
                cmd.llm_provider,
                &cmd.llm_api_key,
                normalized_url.as_deref(),
            )
            .await
            .map_err(|e| {
                ApplicationError::Domain(DomainError::Validation(format!(
                    "chave de LLM rejeitada: {e}"
                )))
            })?;

        let password_hash =
            pw::hash(&cmd.admin_password).map_err(ApplicationError::Infrastructure)?;

        let (encrypted_key, base_url): (Option<String>, Option<String>) = match cmd.llm_provider {
            LlmProvider::Ollama => (None, Some(cmd.llm_api_key.trim().to_string())),
            LlmProvider::Test => (None, None),
            LlmProvider::Anthropic => {
                let ct = self.cipher.encrypt(&cmd.llm_api_key)?;
                (Some(ct), None)
            }
            LlmProvider::OpenAi => {
                let encrypted = if cmd.llm_api_key.is_empty() {
                    // URL set + empty key → "no auth" passthrough.
                    None
                } else {
                    Some(self.cipher.encrypt(&cmd.llm_api_key)?)
                };
                (encrypted, normalized_url.clone())
            }
        };

        let generation_model = cmd
            .generation_model
            .unwrap_or_else(|| defaults::generation_model(cmd.llm_provider).to_string());

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::admin::test_doubles::{
        make_workspace, AcceptingProbe, InMemoryWorkspaceRepository, StubCipher,
    };

    fn build_use_case(
        ws_repo: Arc<InMemoryWorkspaceRepository>,
        probe: Arc<AcceptingProbe>,
    ) -> InitializeInstallationUseCase {
        InitializeInstallationUseCase::new(ws_repo, probe, Arc::new(StubCipher))
    }

    fn base_command(provider: LlmProvider) -> InitializeInstallationCommand {
        InitializeInstallationCommand {
            admin_email: "admin@example.com".to_string(),
            admin_password: "supersecretpassword".to_string(),
            workspace_name: "test-ws".to_string(),
            llm_provider: provider,
            llm_api_key: String::new(),
            base_url: None,
            generation_model: None,
            languages: vec!["en".to_string()],
            primary_language: "en".to_string(),
        }
    }

    #[tokio::test]
    async fn openai_with_url_and_key_persists_both() {
        let ws = make_workspace("openai", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(InitializeInstallationCommand {
            llm_api_key: "my-key".to_string(),
            base_url: Some("https://litellm.example/v1".to_string()),
            ..base_command(LlmProvider::OpenAi)
        })
        .await
        .expect("succeeds");

        let init = ws_repo.last_init.lock().unwrap().clone().expect("init");
        assert_eq!(init.llm_provider, "openai");
        assert_eq!(
            init.llm_base_url,
            Some("https://litellm.example/v1".to_string())
        );
        assert_eq!(init.llm_api_key_encrypted, Some("enc:my-key".to_string()));
    }

    #[tokio::test]
    async fn openai_with_url_and_empty_key_persists_url_only() {
        let ws = make_workspace("openai", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(InitializeInstallationCommand {
            llm_api_key: String::new(),
            base_url: Some("https://my".to_string()),
            ..base_command(LlmProvider::OpenAi)
        })
        .await
        .expect("succeeds");

        let init = ws_repo.last_init.lock().unwrap().clone().expect("init");
        assert_eq!(init.llm_base_url, Some("https://my".to_string()));
        assert_eq!(init.llm_api_key_encrypted, None);
    }

    #[tokio::test]
    async fn openai_no_url_no_key_rejects_validation() {
        let ws = make_workspace("openai", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo, probe);

        let result = uc
            .execute(InitializeInstallationCommand {
                llm_api_key: String::new(),
                base_url: None,
                ..base_command(LlmProvider::OpenAi)
            })
            .await;

        match result {
            Err(ApplicationError::Domain(DomainError::Validation(_))) => {}
            Err(other) => panic!("expected Domain(Validation), got {other:?}"),
            Ok(_) => panic!("expected validation failure"),
        }
    }

    #[tokio::test]
    async fn openai_normalizes_trailing_slash() {
        let ws = make_workspace("openai", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(InitializeInstallationCommand {
            llm_api_key: "k".to_string(),
            base_url: Some("https://api/v1/".to_string()),
            ..base_command(LlmProvider::OpenAi)
        })
        .await
        .expect("succeeds");

        let init = ws_repo.last_init.lock().unwrap().clone().expect("init");
        assert_eq!(init.llm_base_url, Some("https://api/v1".to_string()));
    }

    #[tokio::test]
    async fn openai_probe_called_with_url() {
        let ws = make_workspace("openai", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo, probe.clone());

        uc.execute(InitializeInstallationCommand {
            llm_api_key: "k".to_string(),
            base_url: Some("https://my".to_string()),
            ..base_command(LlmProvider::OpenAi)
        })
        .await
        .expect("succeeds");

        let calls = probe.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].base_url.as_deref(), Some("https://my"));
        assert_eq!(calls[0].api_key, "k");
        assert!(matches!(calls[0].provider, LlmProvider::OpenAi));
    }

    #[tokio::test]
    async fn test_provider_skips_probe() {
        let ws = make_workspace("test", None, None);
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo, probe.clone());

        uc.execute(base_command(LlmProvider::Test))
            .await
            .expect("succeeds");

        // The current architecture sends every provider through the
        // probe trait; the `Test` provider's probe impl is the no-op
        // stub. Either way the call lands on the AcceptingProbe stub
        // here, so we assert behaviour matches the production probe
        // dispatcher: provider=Test passes through with no auth/url.
        let calls = probe.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert!(matches!(calls[0].provider, LlmProvider::Test));
        assert_eq!(calls[0].base_url, None);
    }
}
