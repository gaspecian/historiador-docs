//! Property-based round-trip tests per ADR-010.
//!
//! Invariant: for any markdown in the supported CommonMark subset,
//! `parse → serialize → parse` must yield an identical `BlockTree`.
//! The first parse canonicalises whitespace; the second is
//! idempotent. Silent data loss here breaks the anchor stability that
//! diffs (ADR-013) and comments (ADR-016) depend on.

use historiador_blocks::{parse_markdown, serialize_markdown};
use proptest::prelude::*;

/// A single markdown block that we know how to generate.
#[derive(Debug, Clone)]
enum GenBlock {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(String),
    UnorderedList(Vec<String>),
    OrderedList(Vec<String>),
    Code {
        lang: Option<String>,
        body: String,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

impl GenBlock {
    fn render(&self) -> String {
        match self {
            Self::Heading { level, text } => {
                let hashes: String = "#".repeat(*level as usize);
                format!("{hashes} {text}\n")
            }
            Self::Paragraph(text) => format!("{text}\n"),
            Self::UnorderedList(items) => {
                let mut out = String::new();
                for item in items {
                    out.push_str("- ");
                    out.push_str(item);
                    out.push('\n');
                }
                out
            }
            Self::OrderedList(items) => {
                let mut out = String::new();
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&format!("{}. {item}\n", i + 1));
                }
                out
            }
            Self::Code { lang, body } => {
                let fence = "```";
                let lang_str = lang.as_deref().unwrap_or("");
                format!("{fence}{lang_str}\n{body}\n{fence}\n")
            }
            Self::Table { headers, rows } => {
                let mut out = String::new();
                out.push_str("| ");
                out.push_str(&headers.join(" | "));
                out.push_str(" |\n");
                out.push('|');
                for _ in headers {
                    out.push_str(" --- |");
                }
                out.push('\n');
                for row in rows {
                    out.push_str("| ");
                    out.push_str(&row.join(" | "));
                    out.push_str(" |\n");
                }
                out
            }
        }
    }
}

// --- strategies ---

fn safe_text() -> impl Strategy<Value = String> {
    // ASCII letters, digits, spaces. No markdown-structural chars so
    // we don't accidentally introduce new blocks.
    "[a-zA-Z0-9 ]{1,40}"
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty", |s| !s.is_empty())
}

fn safe_single_word() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9]{0,12}".prop_map(|s| s.to_string())
}

fn code_body() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 \n(){};=+\\-*/]{1,120}"
        .prop_map(|s| {
            // Avoid closing fences inside the body — they would end the
            // block prematurely.
            s.replace("```", "")
        })
        .prop_filter("non-empty", |s| !s.trim().is_empty())
}

fn gen_heading() -> impl Strategy<Value = GenBlock> {
    (1u8..=6, safe_text()).prop_map(|(level, text)| GenBlock::Heading { level, text })
}

fn gen_paragraph() -> impl Strategy<Value = GenBlock> {
    safe_text().prop_map(GenBlock::Paragraph)
}

fn gen_unordered_list() -> impl Strategy<Value = GenBlock> {
    prop::collection::vec(safe_text(), 1..5).prop_map(GenBlock::UnorderedList)
}

fn gen_ordered_list() -> impl Strategy<Value = GenBlock> {
    prop::collection::vec(safe_text(), 1..5).prop_map(GenBlock::OrderedList)
}

fn gen_code() -> impl Strategy<Value = GenBlock> {
    (prop::option::of(safe_single_word()), code_body())
        .prop_map(|(lang, body)| GenBlock::Code { lang, body })
}

fn gen_table() -> impl Strategy<Value = GenBlock> {
    let headers = prop::collection::vec(safe_single_word(), 2..4);
    headers
        .prop_flat_map(|h| {
            let col_count = h.len();
            let rows = prop::collection::vec(
                prop::collection::vec(safe_single_word(), col_count..=col_count),
                1..4,
            );
            (Just(h), rows)
        })
        .prop_map(|(headers, rows)| GenBlock::Table { headers, rows })
}

fn gen_block() -> impl Strategy<Value = GenBlock> {
    prop_oneof![
        gen_heading(),
        gen_paragraph(),
        gen_unordered_list(),
        gen_ordered_list(),
        gen_code(),
        gen_table(),
    ]
}

fn gen_document() -> impl Strategy<Value = String> {
    prop::collection::vec(gen_block(), 1..8).prop_map(|blocks| {
        let mut out = String::new();
        for (i, b) in blocks.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&b.render());
        }
        out
    })
}

proptest! {
    // `cargo test` default case count is 256; override up to 1024 via
    // PROPTEST_CASES=1024 for local deep runs. CI sticks with defaults
    // to keep per-run time predictable. The plan calls for ≥10k over
    // a dedicated nightly run — tuned there, not here.
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn round_trip_preserves_tree(md in gen_document()) {
        let first = parse_markdown(&md).unwrap();
        let serialised = serialize_markdown(&first);
        let second = parse_markdown(&serialised).unwrap();
        prop_assert_eq!(first, second);
    }

    #[test]
    fn round_trip_preserves_block_ids(md in gen_document()) {
        let first = parse_markdown(&md).unwrap();
        let original_ids: Vec<_> = first.blocks.iter().map(|b| b.id.clone()).collect();
        let serialised = serialize_markdown(&first);
        let second = parse_markdown(&serialised).unwrap();
        let round_ids: Vec<_> = second.blocks.iter().map(|b| b.id.clone()).collect();
        prop_assert_eq!(original_ids, round_ids);
    }

    #[test]
    fn serialize_is_idempotent_after_one_pass(md in gen_document()) {
        let tree = parse_markdown(&md).unwrap();
        let first_md = serialize_markdown(&tree);
        let again = parse_markdown(&first_md).unwrap();
        let second_md = serialize_markdown(&again);
        prop_assert_eq!(first_md, second_md);
    }
}
