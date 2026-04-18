use std::sync::Arc;

use crate::auth::tokens as rt;
use crate::domain::error::ApplicationError;
use crate::domain::port::session_repository::SessionRepository;

pub struct LogoutUseCase {
    sessions: Arc<dyn SessionRepository>,
}

impl LogoutUseCase {
    pub fn new(sessions: Arc<dyn SessionRepository>) -> Self {
        Self { sessions }
    }

    pub async fn execute(&self, refresh_token: &str) -> Result<(), ApplicationError> {
        let hash = rt::sha256_hex(refresh_token);
        self.sessions.delete_by_token_hash(&hash).await?;
        Ok(())
    }
}
