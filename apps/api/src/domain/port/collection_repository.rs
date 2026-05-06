use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::Collection;
use crate::domain::error::ApplicationError;
use crate::domain::value::Slug;

#[derive(Debug, Clone)]
pub struct NewCollection {
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: Slug,
}

#[derive(Debug, Clone, Default)]
pub struct CollectionPatch {
    pub name: Option<String>,
    pub slug: Option<Slug>,
    /// `Some(None)` means "move to workspace root"; `None` means "do
    /// not change the parent".
    pub parent_id: Option<Option<Uuid>>,
}

#[async_trait]
pub trait CollectionRepository: Send + Sync {
    async fn insert(&self, input: NewCollection) -> Result<Collection, ApplicationError>;

    async fn find_by_id(
        &self,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<Collection>, ApplicationError>;

    async fn list_by_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<Collection>, ApplicationError>;

    async fn update(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        patch: CollectionPatch,
    ) -> Result<Option<Collection>, ApplicationError>;

    /// Cascading delete — children and pages go with it. Returns true
    /// iff a row was removed.
    async fn delete(&self, id: Uuid, workspace_id: Uuid) -> Result<bool, ApplicationError>;
}
