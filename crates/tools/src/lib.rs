//! `historiador_tools` — shared LLM tool specifications for the AI editor.
//!
//! One ToolSpec per canvas operation. Every caller that emits block ops
//! references the same specs here so provider adapters (`AnthropicClient`,
//! `OpenAiClient`, `OllamaClient`) translate from one authoritative
//! schema. See ADR-011 §125–143.
//!
//! Full-canvas overwrite is intentionally absent: the tool surface is
//! the mechanism that enforces US-11.06's "block-level only" invariant.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A tool the LLM can call. `input_schema` is a JSON Schema draft-07
/// document; providers translate it into their native tool format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// A tool call emitted by the LLM. `arguments` is validated against
/// the matching ToolSpec's `input_schema` before dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("schema validation failed: {0}")]
    InvalidArguments(String),
}

/// The Sprint 11 block-op tools. Stable ordering — the LLM sees them
/// rendered in this order. Note the absence of any `replace_document`
/// variant: full-canvas rewrites are impossible at the schema level.
pub fn block_op_tools() -> Vec<ToolSpec> {
    vec![
        insert_block(),
        replace_block(),
        append_to_section(),
        delete_block(),
        suggest_block_change(),
    ]
}

pub fn insert_block() -> ToolSpec {
    ToolSpec {
        name: "insert_block",
        description: "Insert a new block at a position relative to an existing block. Use for adding new paragraphs, headings, lists, code, tables, or callouts.",
        input_schema: json!({
            "type": "object",
            "required": ["anchor_block_id", "position", "block"],
            "properties": {
                "anchor_block_id": {
                    "type": "string",
                    "description": "Stable block ID the new block is positioned relative to."
                },
                "position": {
                    "type": "string",
                    "enum": ["before", "after"],
                    "description": "Insert the new block before or after the anchor."
                },
                "block": { "$ref": "#/definitions/Block" }
            },
            "definitions": block_definitions()
        }),
    }
}

pub fn replace_block() -> ToolSpec {
    ToolSpec {
        name: "replace_block",
        description: "Replace an existing block's content in place. The block ID is preserved. Use for rewriting paragraphs, correcting code, or updating tables.",
        input_schema: json!({
            "type": "object",
            "required": ["block_id", "block"],
            "properties": {
                "block_id": {
                    "type": "string",
                    "description": "Stable ID of the block to replace."
                },
                "block": { "$ref": "#/definitions/Block" }
            },
            "definitions": block_definitions()
        }),
    }
}

pub fn append_to_section() -> ToolSpec {
    ToolSpec {
        name: "append_to_section",
        description: "Append one or more blocks at the end of a section (identified by its heading block). Use when drafting a section incrementally.",
        input_schema: json!({
            "type": "object",
            "required": ["section_heading_id", "blocks"],
            "properties": {
                "section_heading_id": {
                    "type": "string",
                    "description": "Stable ID of the heading block whose section receives the appended blocks."
                },
                "blocks": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "$ref": "#/definitions/Block" }
                }
            },
            "definitions": block_definitions()
        }),
    }
}

pub fn delete_block() -> ToolSpec {
    ToolSpec {
        name: "delete_block",
        description:
            "Delete a single block by ID. Reversible via the proposal overlay if in Propose mode.",
        input_schema: json!({
            "type": "object",
            "required": ["block_id"],
            "properties": {
                "block_id": {
                    "type": "string",
                    "description": "Stable ID of the block to delete."
                }
            }
        }),
    }
}

pub fn suggest_block_change() -> ToolSpec {
    ToolSpec {
        name: "suggest_block_change",
        description: "Propose a tracked change to a block without applying it. Renders as an inline suggestion the author can accept or reject. Use for grammar fixes, tone adjustments, or alternative phrasings.",
        input_schema: json!({
            "type": "object",
            "required": ["block_id", "suggested_block", "rationale"],
            "properties": {
                "block_id": {
                    "type": "string",
                    "description": "Stable ID of the block the suggestion targets."
                },
                "suggested_block": { "$ref": "#/definitions/Block" },
                "rationale": {
                    "type": "string",
                    "description": "One-sentence explanation of why the change is proposed."
                }
            },
            "definitions": block_definitions()
        }),
    }
}

/// The Block schema. Referenced by every tool that inserts or replaces
/// content. Mirrors the typed block tree emitted by `historiador_blocks`
/// (Phase A2) — keep in sync when that crate lands.
fn block_definitions() -> Value {
    json!({
        "Block": {
            "oneOf": [
                { "$ref": "#/definitions/Heading" },
                { "$ref": "#/definitions/Paragraph" },
                { "$ref": "#/definitions/List" },
                { "$ref": "#/definitions/Code" },
                { "$ref": "#/definitions/Table" },
                { "$ref": "#/definitions/Callout" }
            ]
        },
        "Heading": {
            "type": "object",
            "required": ["kind", "level", "text"],
            "properties": {
                "kind": { "const": "heading" },
                "level": { "type": "integer", "minimum": 1, "maximum": 6 },
                "text": { "type": "string", "minLength": 1 }
            }
        },
        "Paragraph": {
            "type": "object",
            "required": ["kind", "text"],
            "properties": {
                "kind": { "const": "paragraph" },
                "text": { "type": "string", "minLength": 1 }
            }
        },
        "List": {
            "type": "object",
            "required": ["kind", "ordered", "items"],
            "properties": {
                "kind": { "const": "list" },
                "ordered": { "type": "boolean" },
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" }
                }
            }
        },
        "Code": {
            "type": "object",
            "required": ["kind", "body"],
            "properties": {
                "kind": { "const": "code" },
                "language": { "type": ["string", "null"] },
                "body": { "type": "string" }
            }
        },
        "Table": {
            "type": "object",
            "required": ["kind", "headers", "rows"],
            "properties": {
                "kind": { "const": "table" },
                "headers": { "type": "array", "items": { "type": "string" } },
                "rows": {
                    "type": "array",
                    "items": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        },
        "Callout": {
            "type": "object",
            "required": ["kind", "variant", "text"],
            "properties": {
                "kind": { "const": "callout" },
                "variant": {
                    "type": "string",
                    "enum": ["note", "tip", "warning", "danger"]
                },
                "text": { "type": "string", "minLength": 1 }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_op_tools_returns_five_specs_in_stable_order() {
        let tools = block_op_tools();
        let names: Vec<_> = tools.iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec![
                "insert_block",
                "replace_block",
                "append_to_section",
                "delete_block",
                "suggest_block_change",
            ]
        );
    }

    #[test]
    fn no_tool_exposes_full_document_replacement() {
        let tools = block_op_tools();
        for tool in &tools {
            assert_ne!(tool.name, "replace_document");
            let schema = tool.input_schema.to_string();
            assert!(
                !schema.contains("\"replace_document\""),
                "tool {} leaked a replace_document reference",
                tool.name
            );
        }
    }

    #[test]
    fn every_spec_has_required_fields() {
        for tool in block_op_tools() {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema["required"].is_array());
        }
    }

    #[test]
    fn insert_block_requires_anchor_position_block() {
        let spec = insert_block();
        let required = spec.input_schema["required"].as_array().unwrap();
        let names: Vec<_> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"anchor_block_id"));
        assert!(names.contains(&"position"));
        assert!(names.contains(&"block"));
    }

    #[test]
    fn replace_block_requires_block_id_not_path() {
        let spec = replace_block();
        let required = spec.input_schema["required"].as_array().unwrap();
        let names: Vec<_> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(
            names.contains(&"block_id"),
            "replace_block must require block_id (anchor) per ADR-010"
        );
    }

    #[test]
    fn specs_serialize_as_valid_json() {
        for tool in block_op_tools() {
            let json = serde_json::to_string(&tool).expect("tool must serialize");
            assert!(json.contains(tool.name));
        }
    }
}
