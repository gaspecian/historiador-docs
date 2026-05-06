//! BlockTree → markdown. Emits a `<!-- block:<uuid> -->` comment
//! immediately before each block so `parse` can rebind the ID on the
//! next round-trip.

use crate::BlockTree;

/// Serialize a `BlockTree` to markdown with block-ID comments.
///
/// Output shape:
///
/// ```markdown
/// <!-- block:01966c... -->
/// # Heading
///
/// <!-- block:01966d... -->
/// A paragraph.
/// ```
///
/// Each block is preceded by its comment and a single newline; blocks
/// are separated by a blank line. The canonical block markdown already
/// includes its own trailing newline (comrak's `format_commonmark`
/// guarantees this), so we do not add another.
pub fn serialize_markdown(tree: &BlockTree) -> String {
    let mut out = String::new();
    for (i, block) in tree.blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str("<!-- block:");
        out.push_str(&block.id.as_str());
        out.push_str(" -->\n\n");
        out.push_str(block.markdown.trim_end_matches('\n'));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_markdown, Block, BlockId, BlockKind, BlockTree};

    #[test]
    fn round_trip_heading_and_paragraph() {
        let original = "# Hello\n\nWorld.\n";
        let tree = parse_markdown(original).unwrap();
        let serialised = serialize_markdown(&tree);
        let round = parse_markdown(&serialised).unwrap();
        assert_eq!(tree, round);
    }

    #[test]
    fn round_trip_code_block_preserves_language() {
        let original = "```python\nprint('hi')\n```\n";
        let tree = parse_markdown(original).unwrap();
        let serialised = serialize_markdown(&tree);
        let round = parse_markdown(&serialised).unwrap();
        assert_eq!(tree, round);
        match &round.blocks[0].kind {
            BlockKind::Code { language } => assert_eq!(language.as_deref(), Some("python")),
            other => panic!("expected Code, got {other:?}"),
        }
    }

    #[test]
    fn round_trip_preserves_block_ids() {
        let id = BlockId::new();
        let block = Block {
            id: id.clone(),
            kind: BlockKind::Paragraph,
            markdown: "hello\n".to_string(),
        };
        let tree = BlockTree::new(vec![block]);
        let md = serialize_markdown(&tree);
        let round = parse_markdown(&md).unwrap();
        assert_eq!(round.blocks[0].id, id);
    }

    #[test]
    fn round_trip_preserves_order_across_multiple_blocks() {
        let original = "# one\n\nalpha\n\n# two\n\nbeta\n";
        let tree = parse_markdown(original).unwrap();
        let md = serialize_markdown(&tree);
        let round = parse_markdown(&md).unwrap();
        assert_eq!(tree, round);
        assert_eq!(round.len(), 4);
    }

    #[test]
    fn serialize_of_empty_tree_is_empty_string() {
        let tree = BlockTree::default();
        assert_eq!(serialize_markdown(&tree), "");
    }
}
