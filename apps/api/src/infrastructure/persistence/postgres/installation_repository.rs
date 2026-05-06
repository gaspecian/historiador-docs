use async_trait::async_trait;
use sqlx::PgPool;

use historiador_db::postgres::installation;

use crate::domain::entity::Installation;
use crate::domain::error::ApplicationError;
use crate::domain::port::installation_repository::InstallationRepository;

use super::mapper;

pub struct PostgresInstallationRepository {
    pool: PgPool,
}

impl PostgresInstallationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InstallationRepository for PostgresInstallationRepository {
    async fn get(&self) -> Result<Installation, ApplicationError> {
        let row = installation::get(&self.pool).await?;
        Ok(mapper::installation(row))
    }
}
