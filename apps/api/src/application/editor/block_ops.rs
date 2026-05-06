//! Block-op dispatcher (Sprint 11, phase A4).
//!
//! Receives a `historiador_llm::ToolCallChunk` from a provider,
//! resolves it to a typed block op, and either (a) applies it to a
//! mutable `BlockTree` directly (autonomous mode) or (b) produces a
//! `Proposal` record for the overlay (propose/checkpointed modes —
//! A10/A11 wire this side).
//!
//! Four safety guarantees:
//! 1. Every op references a stable `BlockId` (ADR-010). A bare
//!    `replace_document` tool does not exist; `replace_block`
//!    requires a `block_id` and refuses an empty one.
//! 2. Unknown tool names return `DispatchError::UnknownTool` instead
//!    of silently dropping — the caller logs and telemeters it.
//! 3. Any attempt to touch every block in the tree fires
//!    `EditorMetrics::record_canvas_overwrite_attempt()` — the
//!    US-11.06 invariant.
//! 4. Block payload parsing is type-driven via serde, not free-form
//!    JSON Schema — the `block` field deserialises into a typed
//!    `BlockPayload` so malformed arguments fail cleanly.

use historiador_blocks::{Block, BlockId, BlockKind, BlockTree, CalloutVariant};
use historiador_llm::ToolCallChunk;
use serde::Deserialize;
use uuid::Uuid;

use crate::infrastructure::telemetry::editor::EditorMetrics;

/// A proposal produced by the dispatcher. A10 renders this in the
/// overlay; in autonomous mode the caller can apply immediately via
/// [`Proposal::apply`].
#[derive(Debug, Clone, PartialEq)]
pub enum Proposal {
    Insert {
        proposal_id: Uuid,
        anchor_block_id: BlockId,
        position: InsertPosition,
        new_block: Block,
    },
    Replace {
        proposal_id: Uuid,
        block_id: BlockId,
        original: Block,
        replacement: Block,
    },
    Append {
        proposal_id: Uuid,
        section_heading_id: BlockId,
        new_blocks: Vec<Block>,
    },
    Delete {
        proposal_id: Uuid,
        block_id: BlockId,
        original: Block,
    },
    Suggest {
        proposal_id: Uuid,
        block_id: BlockId,
        original: Block,
        suggested: Block,
        rationale: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InsertPosition {
    Before,
    After,
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("invalid arguments for {tool}: {source}")]
    InvalidArguments {
        tool: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("block not found: {0}")]
    BlockNotFound(BlockId),
    #[error("refused: {0}")]
    Refused(String),
}

/// Parse and validate a tool call against the current tree. Produces
/// a proposal; does NOT mutate the tree. Mutation happens at
/// application time (A10 on accept, A11 autonomous-mode auto-accept).
pub fn dispatch(
    tree: &BlockTree,
    call: &ToolCallChunk,
    metrics: &EditorMetrics,
) -> Result<Proposal, DispatchError> {
    match call.name.as_str() {
        "insert_block" => dispatch_insert(tree, call),
        "replace_block" => dispatch_replace(tree, call, metrics),
        "append_to_section" => dispatch_append(tree, call),
        "delete_block" => dispatch_delete(tree, call),
        "suggest_block_change" => dispatch_suggest(tree, call),
        other => Err(DispatchError::UnknownTool(other.to_string())),
    }
}

/// Apply a proposal to the tree. Used by the autonomous-mode path
/// and by A10's Accept button.
pub fn apply(tree: &mut BlockTree, proposal: &Proposal) -> Result<(), DispatchError> {
    match proposal {
        Proposal::Insert {
            anchor_block_id,
            position,
            new_block,
            ..
        } => {
            let idx = tree
                .blocks
                .iter()
                .position(|b| &b.id == anchor_block_id)
                .ok_or_else(|| DispatchError::BlockNotFound(anchor_block_id.clone()))?;
            let insert_at = match position {
                InsertPosition::Before => idx,
                InsertPosition::After => idx + 1,
            };
            tree.blocks.insert(insert_at, new_block.clone());
            Ok(())
        }
        Proposal::Replace {
            block_id,
            replacement,
            ..
        } => {
            let idx = tree
                .blocks
                .iter()
                .position(|b| &b.id == block_id)
                .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;
            tree.blocks[idx] = replacement.clone();
            Ok(())
        }
        Proposal::Append {
            section_heading_id,
            new_blocks,
            ..
        } => {
            let idx = tree
                .blocks
                .iter()
                .position(|b| &b.id == section_heading_id)
                .ok_or_else(|| DispatchError::BlockNotFound(section_heading_id.clone()))?;
            // Append at the end of the section — i.e., right before
            // the next heading of equal-or-shallower level, or at
            // the end of the tree if this is the last section.
            let target_level = heading_level(&tree.blocks[idx]).unwrap_or(0);
            let mut insert_at = tree.blocks.len();
            for (j, block) in tree.blocks.iter().enumerate().skip(idx + 1) {
                if let Some(level) = heading_level(block) {
                    if level <= target_level {
                        insert_at = j;
                        break;
                    }
                }
            }
            for (offset, new) in new_blocks.iter().enumerate() {
                tree.blocks.insert(insert_at + offset, new.clone());
            }
            Ok(())
        }
        Proposal::Delete { block_id, .. } => {
            let idx = tree
                .blocks
                .iter()
                .position(|b| &b.id == block_id)
                .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;
            tree.blocks.remove(idx);
            Ok(())
        }
        Proposal::Suggest {
            block_id,
            suggested,
            ..
        } => {
            // `suggest_block_change` applies only when the user
            // accepts it, and "accept" means "replace". Applying the
            // suggestion collapses it to a Replace.
            let idx = tree
                .blocks
                .iter()
                .position(|b| &b.id == block_id)
                .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;
            tree.blocks[idx] = suggested.clone();
            Ok(())
        }
    }
}

// --- per-tool dispatch ---

#[derive(Debug, Deserialize)]
struct InsertArgs {
    anchor_block_id: String,
    position: InsertPosition,
    block: BlockPayload,
}

fn dispatch_insert(tree: &BlockTree, call: &ToolCallChunk) -> Result<Proposal, DispatchError> {
    let args: InsertArgs = serde_json::from_value(call.arguments.clone()).map_err(|e| {
        DispatchError::InvalidArguments {
            tool: call.name.clone(),
            source: e,
        }
    })?;

    let anchor_id = parse_block_id(&args.anchor_block_id, &call.name)?;
    if tree.find(&anchor_id).is_none() {
        return Err(DispatchError::BlockNotFound(anchor_id));
    }

    Ok(Proposal::Insert {
        proposal_id: Uuid::now_v7(),
        anchor_block_id: anchor_id,
        position: args.position,
        new_block: args.block.into_block(BlockId::new()),
    })
}

#[derive(Debug, Deserialize)]
struct ReplaceArgs {
    block_id: String,
    block: BlockPayload,
}

fn dispatch_replace(
    tree: &BlockTree,
    call: &ToolCallChunk,
    metrics: &EditorMetrics,
) -> Result<Proposal, DispatchError> {
    let args: ReplaceArgs = serde_json::from_value(call.arguments.clone()).map_err(|e| {
        DispatchError::InvalidArguments {
            tool: call.name.clone(),
            source: e,
        }
    })?;

    if args.block_id.trim().is_empty() {
        // Empty ID is the "replace the whole document" smell — refuse
        // and telemeter. The tool schema already forbids this, but
        // the runtime guard belts-and-braces the invariant.
        metrics.record_canvas_overwrite_attempt();
        return Err(DispatchError::Refused(
            "replace_block requires a non-empty block_id (ADR-010)".to_string(),
        ));
    }

    let block_id = parse_block_id(&args.block_id, &call.name)?;
    let original = tree
        .find(&block_id)
        .cloned()
        .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;
    let mut replacement = args.block.into_block(block_id.clone());
    // Preserve the original ID on replace — ADR-010.
    replacement.id = block_id.clone();

    Ok(Proposal::Replace {
        proposal_id: Uuid::now_v7(),
        block_id,
        original,
        replacement,
    })
}

#[derive(Debug, Deserialize)]
struct AppendArgs {
    section_heading_id: String,
    blocks: Vec<BlockPayload>,
}

fn dispatch_append(tree: &BlockTree, call: &ToolCallChunk) -> Result<Proposal, DispatchError> {
    let args: AppendArgs = serde_json::from_value(call.arguments.clone()).map_err(|e| {
        DispatchError::InvalidArguments {
            tool: call.name.clone(),
            source: e,
        }
    })?;

    if args.blocks.is_empty() {
        return Err(DispatchError::Refused(
            "append_to_section requires at least one block".to_string(),
        ));
    }

    let heading_id = parse_block_id(&args.section_heading_id, &call.name)?;
    let heading = tree
        .find(&heading_id)
        .ok_or_else(|| DispatchError::BlockNotFound(heading_id.clone()))?;
    if heading_level(heading).is_none() {
        return Err(DispatchError::Refused(format!(
            "block {heading_id} is not a heading — append_to_section targets a section"
        )));
    }

    let new_blocks = args
        .blocks
        .into_iter()
        .map(|p| p.into_block(BlockId::new()))
        .collect();

    Ok(Proposal::Append {
        proposal_id: Uuid::now_v7(),
        section_heading_id: heading_id,
        new_blocks,
    })
}

#[derive(Debug, Deserialize)]
struct DeleteArgs {
    block_id: String,
}

fn dispatch_delete(tree: &BlockTree, call: &ToolCallChunk) -> Result<Proposal, DispatchError> {
    let args: DeleteArgs = serde_json::from_value(call.arguments.clone()).map_err(|e| {
        DispatchError::InvalidArguments {
            tool: call.name.clone(),
            source: e,
        }
    })?;
    let block_id = parse_block_id(&args.block_id, &call.name)?;
    let original = tree
        .find(&block_id)
        .cloned()
        .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;

    Ok(Proposal::Delete {
        proposal_id: Uuid::now_v7(),
        block_id,
        original,
    })
}

#[derive(Debug, Deserialize)]
struct SuggestArgs {
    block_id: String,
    suggested_block: BlockPayload,
    rationale: String,
}

fn dispatch_suggest(tree: &BlockTree, call: &ToolCallChunk) -> Result<Proposal, DispatchError> {
    let args: SuggestArgs = serde_json::from_value(call.arguments.clone()).map_err(|e| {
        DispatchError::InvalidArguments {
            tool: call.name.clone(),
            source: e,
        }
    })?;

    let block_id = parse_block_id(&args.block_id, &call.name)?;
    let original = tree
        .find(&block_id)
        .cloned()
        .ok_or_else(|| DispatchError::BlockNotFound(block_id.clone()))?;
    let mut suggested = args.suggested_block.into_block(block_id.clone());
    suggested.id = block_id.clone();

    Ok(Proposal::Suggest {
        proposal_id: Uuid::now_v7(),
        block_id,
        original,
        suggested,
        rationale: args.rationale,
    })
}

// --- typed block payload ---

/// What the LLM emits inside `insert_block.block`, `replace_block.block`,
/// etc. Mirrors the JSON schema in `historiador_tools::block_definitions()`.
/// Deserialisation is the validation — invalid shapes fail serde.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockPayload {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
    },
    List {
        ordered: bool,
        items: Vec<String>,
    },
    Code {
        #[serde(default)]
        language: Option<String>,
        body: String,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Callout {
        variant: CalloutVariant,
        text: String,
    },
}

impl BlockPayload {
    /// Render the payload into a `Block` by building the canonical
    /// markdown for the kind. We emit plain forms so the round-trip
    /// proptest (A2) covers them.
    pub fn into_block(self, id: BlockId) -> Block {
        match self {
            BlockPayload::Heading { level, text } => {
                let level = level.clamp(1, 6);
                let hashes: String = "#".repeat(level as usize);
                let markdown = format!("{hashes} {text}\n");
                Block {
                    id,
                    kind: BlockKind::Heading { level },
                    markdown,
                }
            }
            BlockPayload::Paragraph { text } => {
                let markdown = format!("{text}\n");
                Block {
                    id,
                    kind: BlockKind::Paragraph,
                    markdown,
                }
            }
            BlockPayload::List { ordered, items } => {
                let mut md = String::new();
                for (i, item) in items.iter().enumerate() {
                    if ordered {
                        md.push_str(&format!("{}. {item}\n", i + 1));
                    } else {
                        md.push_str("- ");
                        md.push_str(item);
                        md.push('\n');
                    }
                }
                Block {
                    id,
                    kind: BlockKind::List { ordered },
                    markdown: md,
                }
            }
            BlockPayload::Code { language, body } => {
                let fence = "```";
                let lang = language.clone().unwrap_or_default();
                let markdown = format!("{fence}{lang}\n{body}\n{fence}\n");
                Block {
                    id,
                    kind: BlockKind::Code { language },
                    markdown,
                }
            }
            BlockPayload::Table { headers, rows } => {
                let mut md = String::new();
                md.push_str("| ");
                md.push_str(&headers.join(" | "));
                md.push_str(" |\n");
                md.push('|');
                for _ in &headers {
                    md.push_str(" --- |");
                }
                md.push('\n');
                for row in &rows {
                    md.push_str("| ");
                    md.push_str(&row.join(" | "));
                    md.push_str(" |\n");
                }
                Block {
                    id,
                    kind: BlockKind::Table,
                    markdown: md,
                }
            }
            BlockPayload::Callout { variant, text } => {
                let tag = match variant {
                    CalloutVariant::Note => "NOTE",
                    CalloutVariant::Tip => "TIP",
                    CalloutVariant::Warning => "WARNING",
                    CalloutVariant::Danger => "DANGER",
                    CalloutVariant::Important => "IMPORTANT",
                    CalloutVariant::Caution => "CAUTION",
                };
                let mut md = String::new();
                md.push_str(&format!("> [!{tag}]\n"));
                for line in text.lines() {
                    md.push_str("> ");
                    md.push_str(line);
                    md.push('\n');
                }
                Block {
                    id,
                    kind: BlockKind::Callout { variant },
                    markdown: md,
                }
            }
        }
    }
}

// --- helpers ---

fn parse_block_id(s: &str, tool_name: &str) -> Result<BlockId, DispatchError> {
    BlockId::parse(s.trim()).ok_or_else(|| DispatchError::InvalidArguments {
        tool: tool_name.to_string(),
        source: serde::de::Error::custom(format!("not a valid UUID: {s}")),
    })
}

fn heading_level(block: &Block) -> Option<u8> {
    match &block.kind {
        BlockKind::Heading { level } => Some(*level),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use historiador_blocks::parse_markdown;
    use serde_json::json;

    fn make_tree() -> BlockTree {
        // Two peer H2 sections under one H1 so we can assert
        // append_to_section respects the next-peer-heading boundary.
        parse_markdown(
            "<!-- block:019d0000-0000-7000-8000-000000000001 -->\n\n# Intro\n\n\
             <!-- block:019d0000-0000-7000-8000-000000000002 -->\n\nAn intro paragraph.\n\n\
             <!-- block:019d0000-0000-7000-8000-000000000003 -->\n\n## Section 1\n\n\
             <!-- block:019d0000-0000-7000-8000-000000000004 -->\n\nSection 1 body.\n\n\
             <!-- block:019d0000-0000-7000-8000-000000000005 -->\n\n## Section 2\n\n\
             <!-- block:019d0000-0000-7000-8000-000000000006 -->\n\nSection 2 body.\n",
        )
        .expect("fixture parses")
    }

    fn call(name: &str, args: serde_json::Value) -> ToolCallChunk {
        ToolCallChunk {
            call_id: "c".into(),
            name: name.into(),
            arguments: args,
        }
    }

    #[test]
    fn insert_block_before_anchor_produces_insert_proposal() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let call = call(
            "insert_block",
            json!({
                "anchor_block_id": "019d0000-0000-7000-8000-000000000002",
                "position": "before",
                "block": { "kind": "paragraph", "text": "new para" }
            }),
        );

        let p = dispatch(&tree, &call, &metrics).unwrap();
        match p {
            Proposal::Insert {
                position,
                new_block,
                ..
            } => {
                assert_eq!(position, InsertPosition::Before);
                assert!(matches!(new_block.kind, BlockKind::Paragraph));
            }
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    #[test]
    fn replace_block_refuses_empty_id_and_records_overwrite_metric() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let c = call(
            "replace_block",
            json!({
                "block_id": "",
                "block": { "kind": "paragraph", "text": "oops" }
            }),
        );

        let err = dispatch(&tree, &c, &metrics).unwrap_err();
        assert!(matches!(err, DispatchError::Refused(_)));
        assert_eq!(metrics.snapshot().canvas_overwrite_events, 1);
    }

    #[test]
    fn replace_block_preserves_original_id() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let c = call(
            "replace_block",
            json!({
                "block_id": "019d0000-0000-7000-8000-000000000002",
                "block": { "kind": "paragraph", "text": "updated" }
            }),
        );

        let p = dispatch(&tree, &c, &metrics).unwrap();
        match p {
            Proposal::Replace {
                block_id,
                replacement,
                ..
            } => {
                assert_eq!(block_id.as_str(), "019d0000-0000-7000-8000-000000000002");
                assert_eq!(replacement.id, block_id);
            }
            other => panic!("expected Replace, got {other:?}"),
        }
    }

    #[test]
    fn delete_block_returns_proposal_with_original_preserved() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let c = call(
            "delete_block",
            json!({ "block_id": "019d0000-0000-7000-8000-000000000004" }),
        );
        let p = dispatch(&tree, &c, &metrics).unwrap();
        match p {
            Proposal::Delete { original, .. } => {
                assert!(matches!(original.kind, BlockKind::Paragraph));
            }
            other => panic!("expected Delete, got {other:?}"),
        }
    }

    #[test]
    fn unknown_tool_returns_unknown_tool_error() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let c = call("replace_document", json!({"body": "..."}));
        let err = dispatch(&tree, &c, &metrics).unwrap_err();
        assert!(matches!(err, DispatchError::UnknownTool(ref n) if n == "replace_document"));
    }

    #[test]
    fn apply_insert_and_delete_round_trip() {
        let mut tree = make_tree();
        let before = tree.len();
        let metrics = EditorMetrics::new();

        let c = call(
            "insert_block",
            json!({
                "anchor_block_id": "019d0000-0000-7000-8000-000000000001",
                "position": "after",
                "block": { "kind": "paragraph", "text": "injected" }
            }),
        );
        let p = dispatch(&tree, &c, &metrics).unwrap();
        apply(&mut tree, &p).unwrap();
        assert_eq!(tree.len(), before + 1);

        // Delete the block we just inserted.
        let inserted_id = match &p {
            Proposal::Insert { new_block, .. } => new_block.id.clone(),
            _ => unreachable!(),
        };
        let del = call("delete_block", json!({ "block_id": inserted_id.as_str() }));
        let p2 = dispatch(&tree, &del, &metrics).unwrap();
        apply(&mut tree, &p2).unwrap();
        assert_eq!(tree.len(), before);
    }

    #[test]
    fn append_to_section_inserts_before_next_peer_heading() {
        let mut tree = make_tree();
        let metrics = EditorMetrics::new();
        // Append to Section 1 (H2). The next peer heading is
        // Section 2 (also H2), which bounds the section.
        let c = call(
            "append_to_section",
            json!({
                "section_heading_id": "019d0000-0000-7000-8000-000000000003",
                "blocks": [{ "kind": "paragraph", "text": "appended" }]
            }),
        );
        let p = dispatch(&tree, &c, &metrics).unwrap();
        apply(&mut tree, &p).unwrap();

        let appended_index = tree
            .blocks
            .iter()
            .position(|b| {
                matches!(&b.kind, BlockKind::Paragraph) && b.markdown.contains("appended")
            })
            .expect("appended block exists");
        let section_two_index = tree
            .blocks
            .iter()
            .position(|b| b.markdown.contains("Section 2"))
            .expect("Section 2 heading exists");
        assert!(
            appended_index < section_two_index,
            "appended block must land inside Section 1, before Section 2"
        );
    }

    #[test]
    fn invalid_json_arguments_return_invalid_arguments() {
        let tree = make_tree();
        let metrics = EditorMetrics::new();
        let c = call("insert_block", json!({ "this is": "garbage" }));
        let err = dispatch(&tree, &c, &metrics).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidArguments { .. }));
    }
}
