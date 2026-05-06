//! `historiador_blocks` — typed block tree for the AI editor canvas.
//!
//! ADR-010 separates two representations of a page:
//!   - markdown in `page_versions.content_markdown` is the source of truth
//!   - a block tree is the rendering / mutation model for the editor
//!
//! This crate bridges them. Each top-level markdown block gets a stable
//! UUIDv7 ID persisted as an HTML comment (`<!-- block:<uuid> -->`)
//! immediately before the block. IDs survive round-trip so the
//! proposal overlay (ADR-013) and inline comments (ADR-016) can anchor
//! reliably across edits.
//!
//! Round-trip property:
//!   `parse(serialize(parse(md))) == parse(md)`
//!
//! The first parse canonicalises non-semantic whitespace, which is why
//! the invariant is stated as "idempotent after one pass" rather than
//! "serialize is the inverse of parse".

pub mod parse;
pub mod serialize;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable block identifier. UUIDv7 gives monotonic ordering when IDs
/// are compared, which helps debug replay scenarios even though block
/// order in the tree is authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub Uuid);

impl BlockId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(s: &str) -> Option<Self> {
        Uuid::parse_str(s).ok().map(Self)
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for BlockId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Semantic classification of a top-level markdown block. Stored
/// alongside the raw markdown so downstream consumers (LLM tool calls
/// in A4, Tiptap schema in A5) can filter without re-parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockKind {
    Heading {
        level: u8,
    },
    Paragraph,
    List {
        ordered: bool,
    },
    Code {
        language: Option<String>,
    },
    Table,
    Callout {
        variant: CalloutVariant,
    },
    /// Anything the classifier does not recognise falls into `Other`
    /// so round-trip is never lossy. Most common case: blockquotes
    /// that are not GFM alerts.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalloutVariant {
    Note,
    Tip,
    Warning,
    Danger,
    Important,
    Caution,
}

impl CalloutVariant {
    pub fn from_alert_tag(tag: &str) -> Option<Self> {
        match tag.to_ascii_uppercase().as_str() {
            "NOTE" => Some(Self::Note),
            "TIP" => Some(Self::Tip),
            "WARNING" => Some(Self::Warning),
            "DANGER" => Some(Self::Danger),
            "IMPORTANT" => Some(Self::Important),
            "CAUTION" => Some(Self::Caution),
            _ => None,
        }
    }
}

/// A single block: identifier, classification, and the canonical
/// markdown that represents it. The canonical form (rather than the
/// raw input) is what serialize emits, so the second parse always
/// produces an identical tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    #[serde(flatten)]
    pub kind: BlockKind,
    /// Canonical markdown for this block. Does *not* include the
    /// `<!-- block:<uuid> -->` comment — that is added by `serialize`.
    pub markdown: String,
}

/// The tree for a page. "Tree" is aspirational: in this first cut
/// every block is a top-level sibling. Callouts and lists may become
/// real trees in a later phase; the API here accommodates that growth.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlockTree {
    pub blocks: Vec<Block>,
}

impl BlockTree {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Look up a block by ID. Used by the A4 dispatcher when applying
    /// block ops.
    pub fn find(&self, id: &BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| &b.id == id)
    }

    pub fn find_mut(&mut self, id: &BlockId) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| &b.id == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    #[error("markdown parse failed: {0}")]
    Parse(String),
}

// Convenience re-exports so consumers have a single import line.
pub use parse::parse_markdown;
pub use serialize::serialize_markdown;
