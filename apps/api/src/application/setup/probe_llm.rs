use std::sync::Arc;

use crate::domain::error::ApplicationError;
use crate::domain::port::llm_probe::LlmProbe;
use crate::infrastructure::llm::probe::LlmProvider;

pub struct ProbeLlmResult {
    pub success: bool,
    pub message: String,
}

pub struct ProbeLlmUseCase {
    probe: Arc<dyn LlmProbe>,
}

impl ProbeLlmUseCase {
    pub fn new(probe: Arc<dyn LlmProbe>) -> Self {
        Self { probe }
    }

    pub async fn execute(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> Result<ProbeLlmResult, ApplicationError> {
        Ok(match self.probe.probe(provider, api_key, base_url).await {
            Ok(()) => ProbeLlmResult {
                success: true,
                message: "connection successful".into(),
            },
            Err(e) => ProbeLlmResult {
                success: false,
                message: format!("{e}"),
            },
        })
    }
}
