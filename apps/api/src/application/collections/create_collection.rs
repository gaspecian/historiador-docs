use std::sync::Arc;

use crate::domain::entity::Collection;
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::collection_repository::{CollectionRepository, NewCollection};
use crate::domain::value::{Actor, Role, Slug};
use crate::util::slugify;

use super::commands::CreateCollectionCommand;

pub struct CreateCollectionUseCase {
    collections: Arc<dyn CollectionRepository>,
}

impl CreateCollectionUseCase {
    pub fn new(collections: Arc<dyn CollectionRepository>) -> Self {
        Self { collections }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: CreateCollectionCommand,
    ) -> Result<Collection, ApplicationError> {
        actor.require_role(Role::Author)?;

        if let Some(parent_id) = cmd.parent_id {
            self.collections
                .find_by_id(parent_id, actor.workspace_id)
                .await?
                .ok_or_else(|| DomainError::Validation("parent collection not found".into()))?;
        }

        let slug = Slug::parse(slugify(&cmd.name))?;

        self.collections
            .insert(NewCollection {
                workspace_id: actor.workspace_id,
                parent_id: cmd.parent_id,
                name: cmd.name,
                slug,
            })
            .await
            .map_err(map_slug_conflict)
    }
}

fn map_slug_conflict(err: ApplicationError) -> ApplicationError {
    if let ApplicationError::Infrastructure(ref any_err) = err {
        let msg = any_err.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            return DomainError::Conflict("collection slug already exists".into()).into();
        }
    }
    err
}
