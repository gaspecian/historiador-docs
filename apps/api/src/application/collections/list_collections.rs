use std::sync::Arc;

use crate::domain::entity::Collection;
use crate::domain::error::ApplicationError;
use crate::domain::port::collection_repository::CollectionRepository;
use crate::domain::value::{Actor, Role};

pub struct ListCollectionsUseCase {
    collections: Arc<dyn CollectionRepository>,
}

impl ListCollectionsUseCase {
    pub fn new(collections: Arc<dyn CollectionRepository>) -> Self {
        Self { collections }
    }

    pub async fn execute(&self, actor: Actor) -> Result<Vec<Collection>, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        self.collections.list_by_workspace(actor.workspace_id).await
    }
}
