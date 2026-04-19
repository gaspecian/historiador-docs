//! Pre-publish readiness check (Sprint 11, phase B6 / US-11.13).
//!
//! Runs when the author clicks "Ready to publish?" Returns a list of
//! structured issues the UI surfaces as blocking warnings. Pure
//! function over a `BlockTree` — no database, no LLM — so it can
//! execute in-request and test cleanly.
//!
//! Checks (order matches the user-facing list):
//!
//! - **empty_section** — a heading with no content blocks before the
//!   next peer-or-shallower heading.
//! - **todo_marker** — any `TODO`, `FIXME`, `XXX` in a block body.
//! - **heading_hierarchy** — heading levels that skip a level
//!   (e.g. H1 directly to H3 without an intervening H2).
//! - **first_heading_not_h1** — a page that opens with an H3+
//!   likely has a missing title.

use historiador_blocks::{Block, BlockKind, BlockTree};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    EmptySection,
    TodoMarker,
    HeadingHierarchy,
    FirstHeadingNotH1,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Issue {
    pub kind: IssueKind,
    pub block_id: Option<String>,
    pub message: String,
}

pub fn check(tree: &BlockTree) -> Vec<Issue> {
    let mut issues = Vec::new();
    issues.extend(check_first_heading(tree));
    issues.extend(check_empty_sections(tree));
    issues.extend(check_heading_hierarchy(tree));
    issues.extend(check_todo_markers(tree));
    issues
}

fn check_first_heading(tree: &BlockTree) -> Vec<Issue> {
    let Some(first_heading) = tree
        .blocks
        .iter()
        .find(|b| matches!(b.kind, BlockKind::Heading { .. }))
    else {
        return Vec::new();
    };
    if let BlockKind::Heading { level } = first_heading.kind {
        if level > 2 {
            return vec![Issue {
                kind: IssueKind::FirstHeadingNotH1,
                block_id: Some(first_heading.id.as_str()),
                message: format!(
                    "A página abre com H{level}. Considere um título de nível 1 ou 2."
                ),
            }];
        }
    }
    Vec::new()
}

fn check_empty_sections(tree: &BlockTree) -> Vec<Issue> {
    let mut out = Vec::new();
    let headings: Vec<(usize, u8, &Block)> = tree
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match &b.kind {
            BlockKind::Heading { level } => Some((i, *level, b)),
            _ => None,
        })
        .collect();

    for (idx, (i, level, heading)) in headings.iter().enumerate() {
        let next_stop = headings
            .iter()
            .skip(idx + 1)
            .find(|(_, lv, _)| *lv <= *level)
            .map(|(j, _, _)| *j)
            .unwrap_or(tree.blocks.len());
        let has_content = tree.blocks[*i + 1..next_stop]
            .iter()
            .any(|b| !matches!(b.kind, BlockKind::Heading { .. }));
        if !has_content {
            out.push(Issue {
                kind: IssueKind::EmptySection,
                block_id: Some(heading.id.as_str()),
                message: format!(
                    "A seção “{}” está vazia — adicione ao menos um parágrafo.",
                    heading_text(heading)
                ),
            });
        }
    }

    out
}

fn check_heading_hierarchy(tree: &BlockTree) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut previous: Option<u8> = None;
    for block in &tree.blocks {
        if let BlockKind::Heading { level } = block.kind {
            if let Some(prev) = previous {
                if level > prev + 1 {
                    out.push(Issue {
                        kind: IssueKind::HeadingHierarchy,
                        block_id: Some(block.id.as_str()),
                        message: format!(
                            "Salto de H{prev} para H{level} — adicione uma subseção intermediária."
                        ),
                    });
                }
            }
            previous = Some(level);
        }
    }
    out
}

fn check_todo_markers(tree: &BlockTree) -> Vec<Issue> {
    let mut out = Vec::new();
    for block in &tree.blocks {
        let body = &block.markdown;
        let lower = body.to_ascii_uppercase();
        let hit = lower.contains("TODO") || lower.contains("FIXME") || lower.contains("XXX");
        if hit {
            out.push(Issue {
                kind: IssueKind::TodoMarker,
                block_id: Some(block.id.as_str()),
                message: "Marcador pendente encontrado (TODO/FIXME/XXX).".to_string(),
            });
        }
    }
    out
}

fn heading_text(block: &Block) -> String {
    block
        .markdown
        .trim_start_matches('#')
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use historiador_blocks::parse_markdown;

    fn tree_from(md: &str) -> BlockTree {
        parse_markdown(md).expect("fixture parses")
    }

    #[test]
    fn empty_tree_has_no_issues() {
        assert!(check(&BlockTree::default()).is_empty());
    }

    #[test]
    fn well_formed_document_has_no_issues() {
        let tree = tree_from("# Title\n\nintro\n\n## Sub\n\nbody\n");
        assert!(check(&tree).is_empty());
    }

    #[test]
    fn empty_section_is_flagged() {
        let tree = tree_from("# Title\n\nintro\n\n## Empty\n\n## Next\n\nbody\n");
        let issues: Vec<_> = check(&tree)
            .into_iter()
            .filter(|i| i.kind == IssueKind::EmptySection)
            .collect();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Empty"));
    }

    #[test]
    fn todo_marker_is_flagged() {
        let tree = tree_from("# Title\n\nTODO: write intro\n");
        let issues: Vec<_> = check(&tree)
            .into_iter()
            .filter(|i| i.kind == IssueKind::TodoMarker)
            .collect();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn heading_hierarchy_skip_is_flagged() {
        let tree = tree_from("# Title\n\nintro\n\n### Too deep\n\nbody\n");
        let issues: Vec<_> = check(&tree)
            .into_iter()
            .filter(|i| i.kind == IssueKind::HeadingHierarchy)
            .collect();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn first_heading_h3_flagged() {
        let tree = tree_from("### Starts too deep\n\nbody\n");
        let issues: Vec<_> = check(&tree)
            .into_iter()
            .filter(|i| i.kind == IssueKind::FirstHeadingNotH1)
            .collect();
        assert_eq!(issues.len(), 1);
    }
}
