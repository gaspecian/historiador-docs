use std::sync::Arc;

use crate::domain::entity::Collection;
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::collection_repository::{CollectionPatch, CollectionRepository};
use crate::domain::value::{Actor, Role, Slug};
use crate::util::slugify;

use super::commands::UpdateCollectionCommand;

pub struct UpdateCollectionUseCase {
    collections: Arc<dyn CollectionRepository>,
}

impl UpdateCollectionUseCase {
    pub fn new(collections: Arc<dyn CollectionRepository>) -> Self {
        Self { collections }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: UpdateCollectionCommand,
    ) -> Result<Collection, ApplicationError> {
        actor.require_role(Role::Author)?;

        let new_slug = cmd
            .name
            .as_deref()
            .map(|n| Slug::parse(slugify(n)))
            .transpose()?;

        let result = self
            .collections
            .update(
                cmd.id,
                actor.workspace_id,
                CollectionPatch {
                    name: cmd.name,
                    slug: new_slug,
                    parent_id: cmd.parent_id,
                },
            )
            .await
            .map_err(map_slug_conflict)?;

        result.ok_or_else(|| DomainError::NotFound.into())
    }
}

fn map_slug_conflict(err: ApplicationError) -> ApplicationError {
    if let ApplicationError::Infrastructure(ref any_err) = err {
        let msg = any_err.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            return DomainError::Conflict("collection slug conflict".into()).into();
        }
    }
    err
}
