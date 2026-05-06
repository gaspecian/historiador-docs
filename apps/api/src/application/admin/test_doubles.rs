//! Minimal test doubles for admin use-case unit tests.
//!
//! Lives behind `#[cfg(test)]` and is only exposed inside the
//! `application::admin` module tree. Each double is intentionally as
//! small as the unit tests need.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::domain::entity::Workspace;
use crate::domain::error::ApplicationError;
use crate::domain::port::cipher::Cipher;
use crate::domain::port::llm_probe::{LlmProbe, LlmProvider};
use crate::domain::port::workspace_repository::{
    InitializeInstallation, InstallationBootstrapped, LlmConfigPatch, WorkspaceRepository,
};
use crate::domain::value::Language;

// ---------- workspace ----------

/// Captures the most recent `update_llm_config` patch and the most
/// recent `initialize` input, and returns a preconfigured workspace
/// from `find_by_id` / `find_singleton`.
pub(crate) struct InMemoryWorkspaceRepository {
    workspace: Workspace,
    pub last_patch: Mutex<Option<LlmConfigPatch>>,
    pub last_init: Mutex<Option<InitializeInstallation>>,
}

impl InMemoryWorkspaceRepository {
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            last_patch: Mutex::new(None),
            last_init: Mutex::new(None),
        }
    }
}

#[async_trait]
impl WorkspaceRepository for InMemoryWorkspaceRepository {
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Workspace>, ApplicationError> {
        Ok(Some(self.workspace.clone()))
    }

    async fn find_singleton(&self) -> Result<Option<Workspace>, ApplicationError> {
        Ok(Some(self.workspace.clone()))
    }

    async fn initialize(
        &self,
        input: InitializeInstallation,
    ) -> Result<InstallationBootstrapped, ApplicationError> {
        let workspace_id = self.workspace.id;
        *self.last_init.lock().unwrap() = Some(input);
        Ok(InstallationBootstrapped {
            workspace_id,
            admin_user_id: Uuid::new_v4(),
        })
    }

    async fn update_mcp_token(
        &self,
        _workspace_id: Uuid,
        _new_token_hash: &str,
    ) -> Result<bool, ApplicationError> {
        unimplemented!("not used by these tests")
    }

    async fn update_llm_config(
        &self,
        _workspace_id: Uuid,
        patch: LlmConfigPatch,
    ) -> Result<bool, ApplicationError> {
        *self.last_patch.lock().unwrap() = Some(patch);
        Ok(true)
    }
}

// ---------- probe ----------

#[derive(Debug, Clone)]
pub(crate) struct ProbeCall {
    pub provider: LlmProvider,
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Always-OK probe that records each invocation.
pub(crate) struct AcceptingProbe {
    pub calls: Mutex<Vec<ProbeCall>>,
}

impl AcceptingProbe {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LlmProbe for AcceptingProbe {
    async fn probe(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> anyhow::Result<()> {
        self.calls.lock().unwrap().push(ProbeCall {
            provider,
            api_key: api_key.to_string(),
            base_url: base_url.map(|s| s.to_string()),
        });
        Ok(())
    }
}

/// Always-fail probe.
#[allow(dead_code)]
pub(crate) struct RejectingProbe;

#[async_trait]
impl LlmProbe for RejectingProbe {
    async fn probe(
        &self,
        _provider: LlmProvider,
        _api_key: &str,
        _base_url: Option<&str>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("rejected by test stub")
    }
}

// ---------- cipher ----------

/// Wraps the plaintext as `enc:<plaintext>` so tests can assert that
/// encryption ran without pulling in real AES-GCM.
pub(crate) struct StubCipher;

impl Cipher for StubCipher {
    fn encrypt(&self, plaintext: &str) -> Result<String, ApplicationError> {
        Ok(format!("enc:{plaintext}"))
    }

    fn decrypt(&self, ciphertext: &str) -> Result<String, ApplicationError> {
        Ok(ciphertext
            .strip_prefix("enc:")
            .unwrap_or(ciphertext)
            .to_string())
    }
}

// ---------- workspace fixture ----------

pub(crate) fn make_workspace(
    llm_provider: &str,
    api_key_encrypted: Option<&str>,
    base_url: Option<&str>,
) -> Workspace {
    Workspace {
        id: Uuid::new_v4(),
        name: "test-workspace".to_string(),
        languages: vec![Language::from_trusted("en")],
        primary_language: Language::from_trusted("en"),
        llm_provider: llm_provider.to_string(),
        llm_api_key_encrypted: api_key_encrypted.map(|s| s.to_string()),
        llm_base_url: base_url.map(|s| s.to_string()),
        generation_model: "gpt-4o-mini".to_string(),
        mcp_bearer_token_hash: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
