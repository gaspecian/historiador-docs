//! Markdown → BlockTree. Extracts block-ID comments from HTML block
//! siblings and binds them to the next top-level content block. Any
//! block that arrives without a prefixed `<!-- block:UUID -->` gets a
//! freshly minted UUIDv7.

use comrak::{
    arena_tree::Node,
    format_commonmark,
    nodes::{Ast, NodeValue},
    parse_document, Arena, ExtensionOptions, Options,
};
use std::cell::RefCell;

use crate::{Block, BlockError, BlockId, BlockKind, BlockTree, CalloutVariant};

/// Parse markdown into a `BlockTree`.
///
/// Top-level HTML comments of the form `<!-- block:<uuid> -->` are
/// consumed as ID hints for the following block. Comments that do not
/// match the pattern are treated as regular HTML blocks (they become
/// `BlockKind::Other`), which keeps user-authored HTML round-tripping.
pub fn parse_markdown(markdown: &str) -> Result<BlockTree, BlockError> {
    let arena = Arena::new();
    let options = parse_options();
    let root = parse_document(&arena, markdown, &options);

    let mut blocks: Vec<Block> = Vec::new();
    let mut pending_id: Option<BlockId> = None;

    for child in root.children() {
        let node_value = child.data.borrow().value.clone();
        match node_value {
            NodeValue::HtmlBlock(html) => {
                if let Some(id) = extract_block_id(&html.literal) {
                    // Consume the comment — do not emit it as a block.
                    pending_id = Some(id);
                    continue;
                }
                if is_list_disambiguator(&html.literal) {
                    // Comrak emits `<!-- end list -->` between two
                    // consecutive lists of different types so they do
                    // not merge on re-parse. It is a serializer
                    // artifact, not authored content. Drop it so the
                    // round-trip stays stable.
                    continue;
                }
                // A non-ID HTML block is content: emit as Other.
                let markdown_text = render_node(child, &options)?;
                let id = pending_id.take().unwrap_or_default();
                blocks.push(Block {
                    id,
                    kind: BlockKind::Other,
                    markdown: markdown_text,
                });
            }
            other => {
                let kind = classify(&other, child);
                let markdown_text = render_node(child, &options)?;
                let id = pending_id.take().unwrap_or_default();
                blocks.push(Block {
                    id,
                    kind,
                    markdown: markdown_text,
                });
            }
        }
    }

    Ok(BlockTree::new(blocks))
}

/// Comrak options shared by parse + serialize. Keeping them in one
/// place ensures the round-trip uses a single CommonMark dialect.
pub(crate) fn parse_options() -> Options<'static> {
    let mut options = Options::default();
    let mut ext = ExtensionOptions::default();
    ext.strikethrough = true;
    ext.table = true;
    ext.tasklist = true;
    ext.autolink = true;
    ext.footnotes = true;
    options.extension = ext;
    options
}

/// Scan an HTML block for `<!-- block:<uuid> -->` and return the UUID.
fn extract_block_id(html: &str) -> Option<BlockId> {
    let trimmed = html.trim();
    let inner = trimmed.strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let uuid_part = inner.strip_prefix("block:")?.trim();
    BlockId::parse(uuid_part)
}

/// Comrak emits `<!-- end list -->` between two consecutive lists of
/// different types (ordered ↔ unordered) so they do not merge on
/// re-parse. We drop it on parse because it is a serializer artifact
/// the user never authored.
fn is_list_disambiguator(html: &str) -> bool {
    let trimmed = html.trim();
    let Some(inner) = trimmed
        .strip_prefix("<!--")
        .and_then(|s| s.strip_suffix("-->"))
    else {
        return false;
    };
    inner.trim() == "end list"
}

/// Classify a top-level AST node by its value. Callout detection is
/// best-effort: a blockquote whose first paragraph starts with
/// `[!NOTE]`, `[!TIP]`, etc. becomes a Callout; anything else stays
/// Other (round-trip preserves the markdown either way).
fn classify<'a>(value: &NodeValue, node: &'a Node<'a, RefCell<Ast>>) -> BlockKind {
    match value {
        NodeValue::Heading(h) => BlockKind::Heading { level: h.level },
        NodeValue::Paragraph => BlockKind::Paragraph,
        NodeValue::List(list) => BlockKind::List {
            ordered: list.list_type == comrak::nodes::ListType::Ordered,
        },
        NodeValue::CodeBlock(code) => BlockKind::Code {
            language: if code.info.is_empty() {
                None
            } else {
                Some(code.info.clone())
            },
        },
        NodeValue::Table(_) => BlockKind::Table,
        NodeValue::BlockQuote => detect_alert(node).unwrap_or(BlockKind::Other),
        _ => BlockKind::Other,
    }
}

fn detect_alert<'a>(node: &'a Node<'a, RefCell<Ast>>) -> Option<BlockKind> {
    let first = node.first_child()?;
    let first_value = first.data.borrow().value.clone();
    if !matches!(first_value, NodeValue::Paragraph) {
        return None;
    }
    let text = collect_text(first);
    let trimmed = text.trim_start();
    let after_bracket = trimmed.strip_prefix("[!")?;
    let (tag, _) = after_bracket.split_once(']')?;
    let variant = CalloutVariant::from_alert_tag(tag)?;
    Some(BlockKind::Callout { variant })
}

/// Collect the plain text of a node's descendants — used only for
/// alert-tag sniffing, so newline handling is coarse.
fn collect_text<'a>(node: &'a Node<'a, RefCell<Ast>>) -> String {
    let mut buf = String::new();
    push_text(node, &mut buf);
    buf
}

fn push_text<'a>(node: &'a Node<'a, RefCell<Ast>>, buf: &mut String) {
    let value = node.data.borrow().value.clone();
    match value {
        NodeValue::Text(ref t) => buf.push_str(t),
        NodeValue::Code(ref c) => buf.push_str(&c.literal),
        NodeValue::HtmlInline(ref h) => buf.push_str(h),
        NodeValue::LineBreak | NodeValue::SoftBreak => buf.push(' '),
        _ => {
            for child in node.children() {
                push_text(child, buf);
            }
        }
    }
}

/// Render a single AST node back to canonical markdown. We render one
/// node at a time so the output contains exactly the block's content
/// (no extra leading/trailing whitespace beyond what comrak emits).
///
/// Strips trailing `<!-- end list -->` markers. Comrak emits these
/// when a list has a sibling list of a different type, to stop the
/// two from merging on re-parse — but our serializer puts every block
/// behind its own `<!-- block:<uuid> -->` marker, so lists can never
/// merge regardless. Keeping the disambiguator would make the stored
/// markdown unstable across round-trips.
fn render_node<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &Options,
) -> Result<String, BlockError> {
    let mut buf: Vec<u8> = Vec::new();
    format_commonmark(node, options, &mut buf).map_err(|e| BlockError::Parse(e.to_string()))?;
    let s = String::from_utf8(buf).map_err(|e| BlockError::Parse(e.to_string()))?;
    Ok(strip_trailing_list_disambiguator(s))
}

fn strip_trailing_list_disambiguator(mut s: String) -> String {
    loop {
        let trimmed = s.trim_end_matches('\n');
        if let Some(before) = trimmed.strip_suffix("<!-- end list -->") {
            s.truncate(before.len());
            // Leave a single trailing newline so blocks remain well-
            // formed for the serializer.
            while s.ends_with("\n\n") {
                s.pop();
            }
            if !s.ends_with('\n') {
                s.push('\n');
            }
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_block_id_from_comment() {
        let id = extract_block_id("<!-- block:01234567-89ab-7cde-8123-456789abcdef -->");
        assert!(id.is_some());
    }

    #[test]
    fn ignores_non_block_comments() {
        assert!(extract_block_id("<!-- ordinary comment -->").is_none());
        assert!(extract_block_id("<!-- block: not-a-uuid -->").is_none());
    }

    #[test]
    fn parses_a_single_heading_and_paragraph() {
        let md = "# Hello\n\nWorld.\n";
        let tree = parse_markdown(md).unwrap();
        assert_eq!(tree.len(), 2);
        assert!(matches!(
            tree.blocks[0].kind,
            BlockKind::Heading { level: 1 }
        ));
        assert!(matches!(tree.blocks[1].kind, BlockKind::Paragraph));
    }

    #[test]
    fn assigns_fresh_ids_when_comments_absent() {
        let md = "# A\n\nB\n";
        let tree = parse_markdown(md).unwrap();
        assert_ne!(tree.blocks[0].id, tree.blocks[1].id);
    }

    #[test]
    fn honours_block_id_comments() {
        let uuid = "01934f83-1b5f-7a4c-8b2e-abc123456789";
        let md = format!("<!-- block:{uuid} -->\n\n# hi\n");
        let tree = parse_markdown(&md).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.blocks[0].id.as_str(), uuid);
    }

    #[test]
    fn recognises_callouts() {
        let md = "> [!WARNING]\n> be careful\n";
        let tree = parse_markdown(md).unwrap();
        assert_eq!(tree.len(), 1);
        assert!(matches!(
            tree.blocks[0].kind,
            BlockKind::Callout {
                variant: CalloutVariant::Warning
            }
        ));
    }

    #[test]
    fn classifies_code_blocks_with_language() {
        let md = "```rust\nfn main() {}\n```\n";
        let tree = parse_markdown(md).unwrap();
        match &tree.blocks[0].kind {
            BlockKind::Code { language } => assert_eq!(language.as_deref(), Some("rust")),
            other => panic!("expected Code, got {other:?}"),
        }
    }

    #[test]
    fn classifies_tables() {
        let md = "| h1 | h2 |\n| --- | --- |\n| a | b |\n";
        let tree = parse_markdown(md).unwrap();
        assert!(matches!(tree.blocks[0].kind, BlockKind::Table));
    }
}
