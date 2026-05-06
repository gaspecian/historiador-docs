use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::port::event_producer::{DomainEvent, EventProducer};

pub struct LogMcpQueryCommand {
    pub workspace_id: Uuid,
    pub query_text: String,
    pub result_count: i32,
}

pub struct LogMcpQueryUseCase {
    events: Arc<dyn EventProducer>,
}

impl LogMcpQueryUseCase {
    pub fn new(events: Arc<dyn EventProducer>) -> Self {
        Self { events }
    }

    pub async fn execute(&self, cmd: LogMcpQueryCommand) -> Result<(), ApplicationError> {
        self.events
            .publish(DomainEvent::McpQueryLogged {
                workspace_id: cmd.workspace_id,
                query_text: cmd.query_text,
                result_count: cmd.result_count as i64,
            })
            .await
    }
}
