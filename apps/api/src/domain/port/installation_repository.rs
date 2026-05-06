use async_trait::async_trait;

use crate::domain::entity::Installation;
use crate::domain::error::ApplicationError;

#[async_trait]
pub trait InstallationRepository: Send + Sync {
    async fn get(&self) -> Result<Installation, ApplicationError>;
}
