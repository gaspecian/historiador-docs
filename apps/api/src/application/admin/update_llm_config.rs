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
    /// Optional. Required to be `Some` when `llm_provider == OpenAi`
    /// AND `llm_api_key` is empty AND no key is on file.
    /// Persisted verbatim (after trim + trailing-slash strip)
    /// into `workspaces.llm_base_url`.
    pub base_url: Option<String>,
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

        let normalized_url = cmd
            .base_url
            .as_deref()
            .map(|u| u.trim().trim_end_matches('/').to_string())
            .filter(|u| !u.is_empty());

        let (encrypted_key, base_url): (Option<String>, Option<String>) = match cmd.llm_provider {
            LlmProvider::Ollama => {
                // Backwards compat: Ollama still carries its URL via llm_api_key.
                if cmd.llm_api_key.is_empty() {
                    (None, ws.llm_base_url.clone())
                } else {
                    (None, Some(cmd.llm_api_key.trim().to_string()))
                }
            }
            LlmProvider::Test => (None, None),
            LlmProvider::Anthropic => {
                if cmd.llm_api_key.is_empty() {
                    (None, None)
                } else {
                    let ct = self.cipher.encrypt(&cmd.llm_api_key)?;
                    (Some(ct), None)
                }
            }
            LlmProvider::OpenAi => {
                // Reject the no-url + no-key + no-existing-key combo.
                if normalized_url.is_none()
                    && cmd.llm_api_key.is_empty()
                    && ws.llm_api_key_encrypted.is_none()
                {
                    return Err(ApplicationError::Domain(DomainError::Validation(
                        "openai requires either an API key or a custom base URL".into(),
                    )));
                }
                let encrypted = if cmd.llm_api_key.is_empty() {
                    // Empty key + URL set → user intends "no auth" → wipe stored key.
                    // Empty key + URL not set → preserve existing key (don't rotate).
                    if normalized_url.is_some() {
                        None
                    } else {
                        ws.llm_api_key_encrypted.clone()
                    }
                } else {
                    Some(self.cipher.encrypt(&cmd.llm_api_key)?)
                };
                (encrypted, normalized_url.clone())
            }
        };

        // Probe when the admin rotates the secret OR when only a URL
        // is supplied (validates a no-auth endpoint at config time).
        if !cmd.llm_api_key.is_empty() || normalized_url.is_some() {
            self.probe
                .probe(
                    cmd.llm_provider,
                    &cmd.llm_api_key,
                    normalized_url.as_deref(),
                )
                .await
                .map_err(|e| {
                    ApplicationError::Domain(DomainError::Validation(format!("LLM rejected: {e}")))
                })?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::admin::test_doubles::{
        make_workspace, AcceptingProbe, EmptyPageRepository, InMemoryWorkspaceRepository,
        StubCipher,
    };
    use crate::domain::value::Role;
    use uuid::Uuid;

    fn admin_actor(workspace_id: Uuid) -> Actor {
        Actor {
            user_id: Uuid::new_v4(),
            workspace_id,
            role: Role::Admin,
        }
    }

    fn build_use_case(
        ws_repo: Arc<InMemoryWorkspaceRepository>,
        probe: Arc<AcceptingProbe>,
    ) -> UpdateLlmConfigUseCase {
        UpdateLlmConfigUseCase::new(
            ws_repo,
            Arc::new(EmptyPageRepository),
            probe,
            Arc::new(StubCipher),
        )
    }

    #[tokio::test]
    async fn openai_with_base_url_and_key_persists_both() {
        let ws = make_workspace("openai", Some("enc:old-key"), None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(
            admin_actor(workspace_id),
            UpdateLlmConfigCommand {
                llm_provider: LlmProvider::OpenAi,
                llm_api_key: "new-key".to_string(),
                base_url: Some("https://litellm.example/v1".to_string()),
                generation_model: "gpt-4o-mini".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
            },
        )
        .await
        .expect("succeeds");

        let patch = ws_repo.last_patch.lock().unwrap().clone().expect("patch");
        assert_eq!(patch.llm_provider, "openai");
        assert_eq!(
            patch.llm_base_url,
            Some("https://litellm.example/v1".to_string())
        );
        assert_eq!(patch.llm_api_key_encrypted, Some("enc:new-key".to_string()));
    }

    #[tokio::test]
    async fn openai_with_base_url_and_empty_key_persists_url_clears_key() {
        let ws = make_workspace("openai", Some("enc:old-key"), None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(
            admin_actor(workspace_id),
            UpdateLlmConfigCommand {
                llm_provider: LlmProvider::OpenAi,
                llm_api_key: String::new(),
                base_url: Some("https://my-server".to_string()),
                generation_model: "gpt-4o-mini".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
            },
        )
        .await
        .expect("succeeds");

        let patch = ws_repo.last_patch.lock().unwrap().clone().expect("patch");
        assert_eq!(patch.llm_base_url, Some("https://my-server".to_string()));
        assert_eq!(patch.llm_api_key_encrypted, None);
    }

    #[tokio::test]
    async fn openai_no_url_empty_key_preserves_existing_key() {
        let ws = make_workspace("openai", Some("enc:existing"), None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe.clone());

        uc.execute(
            admin_actor(workspace_id),
            UpdateLlmConfigCommand {
                llm_provider: LlmProvider::OpenAi,
                llm_api_key: String::new(),
                base_url: None,
                generation_model: "gpt-4o-mini".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
            },
        )
        .await
        .expect("succeeds");

        let patch = ws_repo.last_patch.lock().unwrap().clone().expect("patch");
        assert_eq!(
            patch.llm_api_key_encrypted,
            Some("enc:existing".to_string())
        );
        assert_eq!(patch.llm_base_url, None);
        // No probe call when no key is rotated and no URL was supplied.
        assert!(probe.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn openai_no_url_no_key_no_existing_rejects_validation() {
        let ws = make_workspace("openai", None, None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo, probe);

        let result = uc
            .execute(
                admin_actor(workspace_id),
                UpdateLlmConfigCommand {
                    llm_provider: LlmProvider::OpenAi,
                    llm_api_key: String::new(),
                    base_url: None,
                    generation_model: "gpt-4o-mini".to_string(),
                    embedding_model: "text-embedding-3-small".to_string(),
                },
            )
            .await;

        match result {
            Err(ApplicationError::Domain(DomainError::Validation(_))) => {}
            Err(other) => panic!("expected Domain(Validation), got {other:?}"),
            Ok(_) => panic!("expected validation failure"),
        }
    }

    #[tokio::test]
    async fn openai_normalizes_trailing_slash() {
        let ws = make_workspace("openai", Some("enc:old"), None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo.clone(), probe);

        uc.execute(
            admin_actor(workspace_id),
            UpdateLlmConfigCommand {
                llm_provider: LlmProvider::OpenAi,
                llm_api_key: "k".to_string(),
                base_url: Some("https://api/v1/".to_string()),
                generation_model: "gpt-4o-mini".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
            },
        )
        .await
        .expect("succeeds");

        let patch = ws_repo.last_patch.lock().unwrap().clone().expect("patch");
        assert_eq!(patch.llm_base_url, Some("https://api/v1".to_string()));
    }

    #[tokio::test]
    async fn openai_with_url_calls_probe_with_url() {
        let ws = make_workspace("openai", Some("enc:old"), None);
        let workspace_id = ws.id;
        let ws_repo = Arc::new(InMemoryWorkspaceRepository::new(ws));
        let probe = Arc::new(AcceptingProbe::new());
        let uc = build_use_case(ws_repo, probe.clone());

        uc.execute(
            admin_actor(workspace_id),
            UpdateLlmConfigCommand {
                llm_provider: LlmProvider::OpenAi,
                llm_api_key: "k".to_string(),
                base_url: Some("https://my".to_string()),
                generation_model: "gpt-4o-mini".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
            },
        )
        .await
        .expect("succeeds");

        let calls = probe.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].base_url.as_deref(), Some("https://my"));
        assert_eq!(calls[0].api_key, "k");
        assert!(matches!(calls[0].provider, LlmProvider::OpenAi));
    }
}
