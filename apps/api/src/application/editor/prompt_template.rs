//! Renders the v1 agent prompt template (Sprint 11, phase A7).
//!
//! The template lives in `prompts/agent/v1.md` and ships in the
//! binary via the Sprint 11 `LoadedPrompt` loader. This module
//! substitutes three variables: `{{tools}}` is rendered from
//! `historiador_tools::block_op_tools()`, `{{mode}}` is filled per
//! turn based on server-side state (A8 sets the intake gate), and
//! `{{context}}` receives the output of
//! `editor::context::assemble()`.

use historiador_tools::ToolSpec;

#[derive(Debug, Clone, Copy)]
pub enum PromptMode {
    /// Canvas empty + no approved outline — hold back tool calls
    /// and ask 2–4 clarifying questions (A8).
    Intake,
    /// Conversation mode. The agent is thinking with the user; no
    /// tool calls unless the user explicitly asks for a change.
    Conversation,
    /// Generation mode. The agent may use canvas tools.
    Generation,
}

impl PromptMode {
    pub fn directive(self) -> &'static str {
        match self {
            PromptMode::Intake => {
                "The canvas is empty and no outline has been approved. Do NOT emit \
                 any tool call on this turn. Ask 2–4 clarifying questions about the \
                 audience, goal, and shape of the page before you write."
            }
            PromptMode::Conversation => {
                "You are in conversation mode. Reply with prose only — no tool \
                 calls — unless the user explicitly asks for a canvas change in \
                 this turn."
            }
            PromptMode::Generation => {
                "You are in generation mode. You may emit tool calls to modify the \
                 canvas. Work in sections; pause at headings in checkpointed mode."
            }
        }
    }
}

/// Render the full system prompt for a turn. `template_body` is the
/// contents of `prompts/agent/v1.md`.
pub fn render_prompt(
    template_body: &str,
    mode: PromptMode,
    tools: &[ToolSpec],
    assembled_context: &str,
) -> String {
    let tools_rendered = render_tools(tools);
    template_body
        .replace("{{tools}}", &tools_rendered)
        .replace("{{mode}}", mode.directive())
        .replace(
            "{{context}}",
            if assembled_context.is_empty() {
                "(no context yet)"
            } else {
                assembled_context
            },
        )
}

fn render_tools(tools: &[ToolSpec]) -> String {
    if tools.is_empty() {
        return "(no tools available)".to_string();
    }
    let mut out = String::new();
    for tool in tools {
        out.push_str(&format!("- **{}** — {}\n", tool.name, tool.description));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str =
        "persona\n\ntools:\n{{tools}}\n\nmode:\n{{mode}}\n\ncontext:\n{{context}}\n";

    #[test]
    fn substitutes_mode_directive() {
        let out = render_prompt(TEMPLATE, PromptMode::Intake, &[], "");
        assert!(out.contains("Do NOT emit any tool call"));
        assert!(!out.contains("{{mode}}"));
    }

    #[test]
    fn renders_tools_as_bulleted_list() {
        let tools = historiador_tools::block_op_tools();
        let out = render_prompt(TEMPLATE, PromptMode::Generation, &tools, "");
        assert!(out.contains("- **insert_block**"));
        assert!(out.contains("- **replace_block**"));
        assert!(out.contains("- **append_to_section**"));
        assert!(out.contains("- **delete_block**"));
        assert!(out.contains("- **suggest_block_change**"));
    }

    #[test]
    fn context_placeholder_fires_when_empty() {
        let out = render_prompt(TEMPLATE, PromptMode::Conversation, &[], "");
        assert!(out.contains("(no context yet)"));
    }

    #[test]
    fn real_context_is_spliced_through() {
        let out = render_prompt(
            TEMPLATE,
            PromptMode::Conversation,
            &[],
            "## Cursor\nBlock: 0196-...\n",
        );
        assert!(out.contains("Block: 0196-..."));
    }

    #[test]
    fn every_placeholder_is_consumed() {
        let tools = historiador_tools::block_op_tools();
        let out = render_prompt(TEMPLATE, PromptMode::Intake, &tools, "context body");
        assert!(!out.contains("{{"));
    }

    #[test]
    fn snapshot_intake_mode_empty_canvas() {
        // Golden-ish snapshot for the intake scenario: blank canvas,
        // no selection, no outline. The assertions check the stable
        // marker strings instead of a byte-exact snapshot so prompt
        // wording tweaks do not force a ritual test update.
        let tools = historiador_tools::block_op_tools();
        let out = render_prompt(TEMPLATE, PromptMode::Intake, &tools, "");
        assert!(out.contains("Do NOT emit any tool call"));
        assert!(out.contains("- **insert_block**"));
        assert!(out.contains("(no context yet)"));
    }

    #[test]
    fn snapshot_generation_mode_with_cursor() {
        let tools = historiador_tools::block_op_tools();
        let context = "## Cursor\nBlock: 01960000-0000-7000-8000-000000000001\n";
        let out = render_prompt(TEMPLATE, PromptMode::Generation, &tools, context);
        assert!(out.contains("generation mode"));
        assert!(out.contains("01960000-0000-7000-8000-000000000001"));
    }

    #[test]
    fn snapshot_conversation_mode_with_selection() {
        let tools = historiador_tools::block_op_tools();
        let context = "## User selection\nthe tricky bit\n";
        let out = render_prompt(TEMPLATE, PromptMode::Conversation, &tools, context);
        assert!(out.contains("conversation mode"));
        assert!(out.contains("the tricky bit"));
    }
}
