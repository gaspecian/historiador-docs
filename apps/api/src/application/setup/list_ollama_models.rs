use crate::domain::error::{ApplicationError, DomainError};

pub struct OllamaModel {
    pub name: String,
    pub size_bytes: u64,
}

pub struct ListOllamaModelsUseCase;

impl ListOllamaModelsUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, base_url: &str) -> Result<Vec<OllamaModel>, ApplicationError> {
        let tags = historiador_llm::list_ollama_models(base_url)
            .await
            .map_err(|e| {
                ApplicationError::Domain(DomainError::Validation(format!(
                    "Ollama unreachable: {e}"
                )))
            })?;
        Ok(tags
            .into_iter()
            .map(|t| OllamaModel {
                name: t.name,
                size_bytes: t.size,
            })
            .collect())
    }
}

impl Default for ListOllamaModelsUseCase {
    fn default() -> Self {
        Self::new()
    }
}
