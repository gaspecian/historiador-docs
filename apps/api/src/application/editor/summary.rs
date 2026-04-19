//! TL;DR summary for exports (Sprint 11, phase B7 / US-11.14).
//!
//! Pure helpers for the export pipeline. `prepend_tldr` injects a
//! `## TL;DR` section at the top of the markdown; `draft_summary`
//! produces a heuristic summary as a fallback when the LLM
//! integration is unavailable. A later phase swaps the heuristic
//! for a one-shot LLM call that the author can edit before export.

use historiador_blocks::{BlockKind, BlockTree};

pub const MAX_SUMMARY_WORDS: usize = 120;

/// Prepend a `## TL;DR` section to the markdown. If `summary` is
/// empty the markdown is returned unchanged.
pub fn prepend_tldr(markdown: &str, summary: &str) -> String {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return markdown.to_string();
    }
    let capped = cap_words(trimmed, MAX_SUMMARY_WORDS);
    format!("## TL;DR\n\n{capped}\n\n{}", markdown.trim_start())
}

/// Clamp a summary string to at most `max_words` whitespace-
/// separated tokens. Cuts on a word boundary and appends `…` when
/// truncated.
pub fn cap_words(text: &str, max_words: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= max_words {
        return text.trim().to_string();
    }
    let mut out = words[..max_words].join(" ");
    out.push('…');
    out
}

/// Heuristic summary from a BlockTree. Takes the first paragraph
/// that follows a heading; caps at `MAX_SUMMARY_WORDS`. Returns an
/// empty string when the page has no prose. Used as the initial
/// value for the export dialog's summary textarea.
pub fn draft_summary(tree: &BlockTree) -> String {
    let mut saw_heading = false;
    for block in &tree.blocks {
        match &block.kind {
            BlockKind::Heading { .. } => saw_heading = true,
            BlockKind::Paragraph if saw_heading => {
                return cap_words(block.markdown.trim(), MAX_SUMMARY_WORDS);
            }
            BlockKind::Paragraph => {
                return cap_words(block.markdown.trim(), MAX_SUMMARY_WORDS);
            }
            _ => {}
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use historiador_blocks::parse_markdown;

    #[test]
    fn empty_summary_returns_markdown_unchanged() {
        let md = "# Hi\n\nbody\n";
        assert_eq!(prepend_tldr(md, ""), md);
        assert_eq!(prepend_tldr(md, "  \n\n"), md);
    }

    #[test]
    fn tldr_is_prepended_with_markdown_heading() {
        let md = "# Hi\n\nbody\n";
        let out = prepend_tldr(md, "A short summary.");
        assert!(out.starts_with("## TL;DR\n\nA short summary."));
        assert!(out.contains("# Hi"));
    }

    #[test]
    fn cap_words_truncates_and_appends_ellipsis() {
        let txt = "one two three four five six seven eight nine ten";
        assert_eq!(cap_words(txt, 4), "one two three four…");
    }

    #[test]
    fn cap_words_leaves_short_text_alone() {
        let txt = "just three words";
        assert_eq!(cap_words(txt, 10), "just three words");
    }

    #[test]
    fn draft_summary_picks_first_paragraph() {
        let tree = parse_markdown("# Title\n\nThe intro paragraph explains the topic.\n").unwrap();
        let s = draft_summary(&tree);
        assert!(s.contains("intro paragraph"));
    }

    #[test]
    fn draft_summary_is_empty_when_no_paragraphs() {
        let tree = parse_markdown("# Only heading\n").unwrap();
        assert!(draft_summary(&tree).is_empty());
    }

    #[test]
    fn draft_summary_respects_word_cap() {
        let long_para: String = (0..200)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let md = format!("# T\n\n{long_para}\n");
        let tree = parse_markdown(&md).unwrap();
        let s = draft_summary(&tree);
        let words = s.split_whitespace().count();
        // MAX_SUMMARY_WORDS words plus the "…" on the last token.
        assert!(words <= MAX_SUMMARY_WORDS);
        assert!(s.ends_with('…'));
    }
}
