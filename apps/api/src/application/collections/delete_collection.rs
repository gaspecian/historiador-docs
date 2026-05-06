use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::collection_repository::CollectionRepository;
use crate::domain::value::{Actor, Role};

pub struct DeleteCollectionUseCase {
    collections: Arc<dyn CollectionRepository>,
}

impl DeleteCollectionUseCase {
    pub fn new(collections: Arc<dyn CollectionRepository>) -> Self {
        Self { collections }
    }

    pub async fn execute(&self, actor: Actor, id: Uuid) -> Result<(), ApplicationError> {
        actor.require_role(Role::Author)?;
        let deleted = self.collections.delete(id, actor.workspace_id).await?;
        if !deleted {
            return Err(DomainError::NotFound.into());
        }
        Ok(())
    }
}
