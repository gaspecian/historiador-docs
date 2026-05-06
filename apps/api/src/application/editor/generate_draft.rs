use std::sync::Arc;

use uuid::Uuid;

use historiador_llm::{TextGenerationClient, TextStream, Turn};

use super::prompts;
use crate::domain::error::ApplicationError;
use crate::domain::value::{Actor, Role};

pub struct GenerateDraftCommand {
    pub brief: String,
    pub language: Option<String>,
    /// Prior conversation turns (oldest first). The handler applies
    /// the turn/character caps before calling `execute`, so this is
    /// ready to forward to the LLM as-is.
    pub history: Vec<Turn>,
}

pub struct DraftStream {
    pub stream: TextStream,
    /// Actor + original brief, so the handler can fire a telemetry
    /// event after the stream terminates.
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub brief: String,
    pub language: Option<String>,
}

pub struct GenerateDraftUseCase {
    text_gen: Arc<dyn TextGenerationClient>,
}

impl GenerateDraftUseCase {
    pub fn new(text_gen: Arc<dyn TextGenerationClient>) -> Self {
        Self { text_gen }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: GenerateDraftCommand,
    ) -> Result<DraftStream, ApplicationError> {
        actor.require_role(Role::Author)?;

        let user_prompt = match &cmd.language {
            Some(lang) => format!("Write in {lang}.\n\n{}", cmd.brief),
            None => cmd.brief.clone(),
        };

        let stream = self
            .text_gen
            .generate_text_stream_with_history(
                prompts::DRAFT_SYSTEM_PROMPT,
                &cmd.history,
                &user_prompt,
            )
            .await
            .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

        Ok(DraftStream {
            stream,
            user_id: actor.user_id,
            workspace_id: actor.workspace_id,
            brief: cmd.brief,
            language: cmd.language,
        })
    }
}
