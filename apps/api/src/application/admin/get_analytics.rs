use std::sync::Arc;

use crate::domain::error::ApplicationError;
use crate::domain::port::query_analytics::{McpQueryStats, QueryAnalytics};
use crate::domain::value::{Actor, Role};

pub struct GetMcpAnalyticsUseCase {
    analytics: Arc<dyn QueryAnalytics>,
}

impl GetMcpAnalyticsUseCase {
    pub fn new(analytics: Arc<dyn QueryAnalytics>) -> Self {
        Self { analytics }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        days: Option<i32>,
    ) -> Result<McpQueryStats, ApplicationError> {
        actor.require_role(Role::Admin)?;
        let days = days.unwrap_or(7).clamp(1, 30);
        self.analytics.mcp_query_stats(days).await
    }
}
