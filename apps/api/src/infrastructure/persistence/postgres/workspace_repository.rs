use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::{installation, users, workspaces};

use crate::domain::entity::Workspace;
use crate::domain::error::ApplicationError;
use crate::domain::port::workspace_repository::{
    InitializeInstallation, InstallationBootstrapped, LlmConfigPatch, WorkspaceRepository,
};

use super::mapper;

pub struct PostgresWorkspaceRepository {
    pool: PgPool,
}

impl PostgresWorkspaceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkspaceRepository for PostgresWorkspaceRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Workspace>, ApplicationError> {
        let row = workspaces::find_by_id(&self.pool, id).await?;
        Ok(row.map(mapper::workspace))
    }

    async fn find_singleton(&self) -> Result<Option<Workspace>, ApplicationError> {
        let row = workspaces::find_singleton(&self.pool).await?;
        Ok(row.map(mapper::workspace))
    }

    async fn initialize(
        &self,
        input: InitializeInstallation,
    ) -> Result<InstallationBootstrapped, ApplicationError> {
        // Collect language tags as owned strings so the transaction
        // can borrow them without outliving the loop.
        let languages: Vec<String> = input
            .languages
            .iter()
            .map(|l| l.as_str().to_string())
            .collect();

        let mut tx = self.pool.begin().await.map_err(anyhow::Error::from)?;

        let workspace_id = workspaces::insert(
            &mut tx,
            workspaces::NewWorkspace {
                name: &input.workspace_name,
                languages: &languages,
                primary_language: input.primary_language.as_str(),
                llm_provider: &input.llm_provider,
                llm_api_key_encrypted: input.llm_api_key_encrypted.as_deref(),
                llm_base_url: input.llm_base_url.as_deref(),
                generation_model: &input.generation_model,
                embedding_model: &input.embedding_model,
            },
        )
        .await?;

        let admin_user_id = users::insert_admin(
            &mut tx,
            workspace_id,
            input.admin_email.as_str(),
            &input.admin_password_hash,
        )
        .await?;

        installation::mark_complete(&mut tx).await?;

        tx.commit().await.map_err(anyhow::Error::from)?;

        Ok(InstallationBootstrapped {
            workspace_id,
            admin_user_id,
        })
    }

    async fn update_mcp_token(
        &self,
        workspace_id: Uuid,
        new_token_hash: &str,
    ) -> Result<bool, ApplicationError> {
        let affected =
            workspaces::update_mcp_token(&self.pool, workspace_id, new_token_hash).await?;
        Ok(affected > 0)
    }

    async fn update_llm_config(
        &self,
        workspace_id: Uuid,
        patch: LlmConfigPatch,
    ) -> Result<bool, ApplicationError> {
        let affected = workspaces::update_llm_config(
            &self.pool,
            workspace_id,
            workspaces::LlmConfigPatch {
                llm_provider: &patch.llm_provider,
                llm_api_key_encrypted: patch.llm_api_key_encrypted.as_deref(),
                llm_base_url: patch.llm_base_url.as_deref(),
                generation_model: &patch.generation_model,
                embedding_model: &patch.embedding_model,
            },
        )
        .await?;
        Ok(affected > 0)
    }
}
