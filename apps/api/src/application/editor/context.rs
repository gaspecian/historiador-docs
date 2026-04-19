//! Canvas-aware context assembly (Sprint 11, phase A6).
//!
//! Each time the user sends a chat turn the server rebuilds a fresh
//! context block for the LLM: the current outline (A9 fills this in),
//! a truncated view of the canvas markdown, the user's selection and
//! cursor position, and the most recent conversation history.
//!
//! The output is a plain string pasted into the system prompt. A7's
//! prompt template formats it; downstream phases (A8 intake, A10
//! proposal overlay, A11 autonomy) consume the same context.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inputs to the context assembler. Any field may be empty.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ContextInputs {
    /// User's text selection on the canvas, if any.
    pub selection_text: Option<String>,
    /// Block the cursor is in, so the agent knows where the user is
    /// focused.
    pub cursor_block_id: Option<String>,
    /// Full canvas markdown. Will be truncated to `MAX_CANVAS_CHARS`.
    pub canvas_markdown: String,
    /// Outline items (A9 wires this).
    pub outline: Vec<OutlineEntry>,
    /// Recent conversation turns (oldest → newest).
    pub recent_history: Vec<HistoryTurn>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutlineEntry {
    pub heading: String,
    pub level: u8,
    pub block_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoryTurn {
    pub role: String,
    pub content: String,
}

/// Assembled context ready to be pasted into the system prompt.
#[derive(Debug, Clone, Serialize)]
pub struct AssembledContext {
    pub text: String,
    pub canvas_truncated: bool,
    pub bytes: usize,
}

pub const MAX_CANVAS_CHARS: usize = 4_000;
pub const MAX_HISTORY_TURNS: usize = 10;

pub fn assemble(inputs: ContextInputs) -> AssembledContext {
    let mut out = String::new();

    // Outline
    if !inputs.outline.is_empty() {
        out.push_str("## Approved outline\n");
        for entry in &inputs.outline {
            let indent = " ".repeat(((entry.level as usize).saturating_sub(1)) * 2);
            out.push_str(&format!("{indent}- {}\n", entry.heading));
        }
        out.push('\n');
    }

    // Cursor / selection
    if let Some(cursor_id) = &inputs.cursor_block_id {
        if Uuid::parse_str(cursor_id).is_ok() {
            out.push_str(&format!("## Cursor\nBlock: {cursor_id}\n\n"));
        }
    }
    if let Some(sel) = &inputs.selection_text {
        let trimmed = sel.trim();
        if !trimmed.is_empty() {
            out.push_str("## User selection\n");
            // Cap selection at 2k chars to avoid runaway context on
            // a ctrl+A select-all.
            let snippet: String = trimmed.chars().take(2_000).collect();
            out.push_str(&snippet);
            out.push_str("\n\n");
        }
    }

    // Canvas (truncated)
    let (canvas_snippet, truncated) = truncate_canvas(&inputs.canvas_markdown);
    if !canvas_snippet.is_empty() {
        out.push_str("## Canvas (markdown");
        if truncated {
            out.push_str(", truncated");
        }
        out.push_str(")\n");
        out.push_str(&canvas_snippet);
        out.push_str("\n\n");
    }

    // Recent history
    if !inputs.recent_history.is_empty() {
        out.push_str("## Recent conversation\n");
        let skip = inputs
            .recent_history
            .len()
            .saturating_sub(MAX_HISTORY_TURNS);
        for turn in inputs.recent_history.iter().skip(skip) {
            out.push_str(&format!("- **{}**: {}\n", turn.role, turn.content));
        }
        out.push('\n');
    }

    let bytes = out.len();
    AssembledContext {
        text: out,
        canvas_truncated: truncated,
        bytes,
    }
}

fn truncate_canvas(markdown: &str) -> (String, bool) {
    if markdown.len() <= MAX_CANVAS_CHARS {
        return (markdown.to_string(), false);
    }
    // Truncate on a character boundary by taking the first
    // MAX_CANVAS_CHARS chars. Appends an explicit "…truncated" marker
    // so the model knows the view is partial.
    let mut snippet: String = markdown.chars().take(MAX_CANVAS_CHARS).collect();
    snippet.push_str("\n\n…(canvas truncated)");
    (snippet, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_yield_empty_context() {
        let c = assemble(ContextInputs::default());
        assert_eq!(c.text, "");
        assert!(!c.canvas_truncated);
    }

    #[test]
    fn outline_is_indented_by_level() {
        let inputs = ContextInputs {
            outline: vec![
                OutlineEntry {
                    heading: "Top".into(),
                    level: 1,
                    block_id: None,
                },
                OutlineEntry {
                    heading: "Sub".into(),
                    level: 2,
                    block_id: None,
                },
            ],
            ..Default::default()
        };
        let c = assemble(inputs);
        assert!(c.text.contains("- Top"));
        assert!(c.text.contains("  - Sub"));
    }

    #[test]
    fn cursor_is_included_only_for_valid_uuid() {
        let bad = ContextInputs {
            cursor_block_id: Some("not-a-uuid".into()),
            ..Default::default()
        };
        assert!(!assemble(bad).text.contains("Cursor"));

        let good = ContextInputs {
            cursor_block_id: Some("01960000-0000-7000-8000-000000000001".into()),
            ..Default::default()
        };
        assert!(assemble(good)
            .text
            .contains("01960000-0000-7000-8000-000000000001"));
    }

    #[test]
    fn canvas_longer_than_cap_is_truncated() {
        let inputs = ContextInputs {
            canvas_markdown: "a".repeat(MAX_CANVAS_CHARS + 500),
            ..Default::default()
        };
        let c = assemble(inputs);
        assert!(c.canvas_truncated);
        assert!(c.text.contains("truncated"));
    }

    #[test]
    fn history_is_capped_at_max_turns() {
        let inputs = ContextInputs {
            recent_history: (0..MAX_HISTORY_TURNS + 5)
                .map(|i| HistoryTurn {
                    role: "user".into(),
                    content: format!("turn {i}"),
                })
                .collect(),
            ..Default::default()
        };
        let c = assemble(inputs);
        assert!(c.text.contains(&format!("turn {}", MAX_HISTORY_TURNS + 4)));
        assert!(!c.text.contains("turn 0"));
    }

    #[test]
    fn selection_is_capped_at_two_thousand_chars() {
        let inputs = ContextInputs {
            selection_text: Some("s".repeat(3_000)),
            ..Default::default()
        };
        let c = assemble(inputs);
        // 2000 s's plus headers
        let count = c.text.matches('s').count();
        assert!(count >= 2_000);
        assert!(count < 2_100);
    }
}
