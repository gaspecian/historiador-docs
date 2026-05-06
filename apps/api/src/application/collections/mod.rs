//! Collection use cases.

pub mod commands;
pub mod create_collection;
pub mod delete_collection;
pub mod list_collections;
pub mod update_collection;

pub use commands::{CreateCollectionCommand, UpdateCollectionCommand};
pub use create_collection::CreateCollectionUseCase;
pub use delete_collection::DeleteCollectionUseCase;
pub use list_collections::ListCollectionsUseCase;
pub use update_collection::UpdateCollectionUseCase;
