use std::sync::Arc;

use uuid::Uuid;

use historiador_llm::{TextGenerationClient, TextStream};

use super::prompts;
use crate::domain::error::ApplicationError;
use crate::domain::value::{Actor, Role};

pub struct IterateDraftCommand {
    pub current_draft: String,
    pub instruction: String,
}

pub struct IterateStream {
    pub stream: TextStream,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub instruction: String,
}

pub struct IterateDraftUseCase {
    text_gen: Arc<dyn TextGenerationClient>,
}

impl IterateDraftUseCase {
    pub fn new(text_gen: Arc<dyn TextGenerationClient>) -> Self {
        Self { text_gen }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: IterateDraftCommand,
    ) -> Result<IterateStream, ApplicationError> {
        actor.require_role(Role::Author)?;

        let user_prompt = format!(
            "## Current Draft\n\n{}\n\n## Instruction\n\n{}",
            cmd.current_draft, cmd.instruction
        );

        let stream = self
            .text_gen
            .generate_text_stream(prompts::ITERATE_SYSTEM_PROMPT, &user_prompt)
            .await
            .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

        Ok(IterateStream {
            stream,
            user_id: actor.user_id,
            workspace_id: actor.workspace_id,
            instruction: cmd.instruction,
        })
    }
}
