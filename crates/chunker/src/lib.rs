//! `historiador_chunker` — structure-aware markdown chunker.
//!
//! Implements the algorithm specified in ADR-002: parse markdown to a
//! `comrak` AST and emit heading-delimited chunks (with paragraph-
//! boundary fallback for oversized sections). Code blocks, tables, and
//! lists are atomic AST nodes and are never split.

use comrak::{
    arena_tree::Node,
    nodes::{Ast, NodeValue},
    parse_document, Arena, Options,
};
use serde::Serialize;
use std::cell::RefCell;

// ---- public types ----

/// A single chunk produced by the structure-aware splitter.
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Heading hierarchy path, e.g. `["APIs", "Authentication"]`.
    /// Empty for content before any heading (flat pages).
    pub heading_path: Vec<String>,
    /// The markdown content of this chunk.
    pub content: String,
    /// Estimated token count (whitespace-based word count).
    pub token_count: usize,
    /// Sequential index across all chunks in the page (0, 1, 2, ...).
    pub section_index: usize,
    /// True if this chunk exceeds `max_tokens` but could not be split
    /// further (e.g., a single large code block).
    pub oversized: bool,
}

/// Configuration for the chunker.
pub struct ChunkConfig {
    /// Maximum tokens per chunk before splitting. Default: 512 per ADR-002.
    pub max_tokens: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self { max_tokens: 512 }
    }
}

/// Errors from chunking.
#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    #[error("empty markdown input")]
    EmptyInput,
}

// ---- internal types ----

/// A section collected during AST traversal, before splitting.
struct Section {
    heading_path: Vec<String>,
    /// Individual content blocks (paragraphs, code blocks, tables, etc.)
    /// Each block is rendered markdown text and whether it's atomic (code/table).
    blocks: Vec<ContentBlock>,
}

struct ContentBlock {
    text: String,
    token_count: usize,
    /// Atomic blocks (code blocks, tables) are never split.
    atomic: bool,
}

// ---- public API ----

/// Parse markdown and split into structure-aware chunks per ADR-002.
pub fn chunk_markdown(markdown: &str, config: &ChunkConfig) -> Result<Vec<Chunk>, ChunkError> {
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        return Err(ChunkError::EmptyInput);
    }

    let arena = Arena::new();
    let root = parse_document(&arena, trimmed, &Options::default());

    let sections = collect_sections(root);
    let chunks = split_sections_into_chunks(sections, config);

    Ok(chunks)
}

// ---- AST traversal ----

/// Walk the AST top-level children, grouping content under heading-
/// delimited sections. Headings push/pop the heading_path stack.
fn collect_sections<'a>(root: &'a Node<'a, RefCell<Ast>>) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    // Track heading text + level together so same-level headings
    // replace each other and deeper headings are popped correctly.
    let mut heading_stack: Vec<(String, u8)> = Vec::new();
    let mut current_blocks: Vec<ContentBlock> = Vec::new();

    for child in root.children() {
        let node_data = child.data.borrow();
        match &node_data.value {
            NodeValue::Heading(heading) => {
                // Flush the current section before starting a new one.
                if !current_blocks.is_empty() || sections.is_empty() {
                    let heading_path = heading_stack.iter().map(|(t, _)| t.clone()).collect();
                    sections.push(Section {
                        heading_path,
                        blocks: std::mem::take(&mut current_blocks),
                    });
                }

                // Pop all headings at this level or deeper, then push.
                let level = heading.level;
                while heading_stack.last().is_some_and(|(_, l)| *l >= level) {
                    heading_stack.pop();
                }
                let heading_text = collect_text(child);
                heading_stack.push((heading_text, level));
            }
            NodeValue::CodeBlock(_) => {
                let text = render_node(child);
                let token_count = estimate_tokens(&text);
                current_blocks.push(ContentBlock {
                    text,
                    token_count,
                    atomic: true,
                });
            }
            NodeValue::Table(_) => {
                let text = render_node(child);
                let token_count = estimate_tokens(&text);
                current_blocks.push(ContentBlock {
                    text,
                    token_count,
                    atomic: true,
                });
            }
            _ => {
                // Paragraphs, lists, block quotes, HTML blocks, etc.
                let text = render_node(child);
                let token_count = estimate_tokens(&text);
                if token_count > 0 {
                    current_blocks.push(ContentBlock {
                        text,
                        token_count,
                        atomic: false,
                    });
                }
            }
        }
    }

    // Flush the last section.
    let heading_path: Vec<String> = heading_stack.iter().map(|(t, _)| t.clone()).collect();
    if !current_blocks.is_empty() {
        sections.push(Section {
            heading_path: heading_path.clone(),
            blocks: current_blocks,
        });
    } else if sections.is_empty() {
        // Edge case: document has only headings with no content.
        sections.push(Section {
            heading_path,
            blocks: Vec::new(),
        });
    }

    sections
}

/// Split sections into chunks, applying the max_tokens threshold.
fn split_sections_into_chunks(sections: Vec<Section>, config: &ChunkConfig) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut section_index = 0usize;

    for section in sections {
        if section.blocks.is_empty() {
            continue;
        }

        let total_tokens: usize = section.blocks.iter().map(|b| b.token_count).sum();

        if total_tokens <= config.max_tokens {
            // Entire section fits in one chunk.
            let content = join_blocks(&section.blocks);
            chunks.push(Chunk {
                heading_path: section.heading_path,
                content,
                token_count: total_tokens,
                section_index,
                oversized: false,
            });
            section_index += 1;
        } else {
            // Section exceeds max_tokens — split at block boundaries.
            let mut current_text = String::new();
            let mut current_tokens = 0usize;

            for block in &section.blocks {
                if block.atomic && block.token_count > config.max_tokens {
                    // Flush any accumulated non-atomic content first.
                    if current_tokens > 0 {
                        chunks.push(Chunk {
                            heading_path: section.heading_path.clone(),
                            content: current_text.clone(),
                            token_count: current_tokens,
                            section_index,
                            oversized: current_tokens > config.max_tokens,
                        });
                        section_index += 1;
                        current_text.clear();
                        current_tokens = 0;
                    }
                    // Emit the oversized atomic block as its own chunk.
                    chunks.push(Chunk {
                        heading_path: section.heading_path.clone(),
                        content: block.text.clone(),
                        token_count: block.token_count,
                        section_index,
                        oversized: true,
                    });
                    section_index += 1;
                    continue;
                }

                // Would adding this block exceed the limit?
                if current_tokens > 0
                    && current_tokens + block.token_count > config.max_tokens
                {
                    // Flush the current accumulator.
                    chunks.push(Chunk {
                        heading_path: section.heading_path.clone(),
                        content: current_text.clone(),
                        token_count: current_tokens,
                        section_index,
                        oversized: false,
                    });
                    section_index += 1;
                    current_text.clear();
                    current_tokens = 0;
                }

                if !current_text.is_empty() {
                    current_text.push('\n');
                }
                current_text.push_str(&block.text);
                current_tokens += block.token_count;
            }

            // Flush remaining content.
            if current_tokens > 0 {
                chunks.push(Chunk {
                    heading_path: section.heading_path.clone(),
                    content: current_text,
                    token_count: current_tokens,
                    section_index,
                    oversized: current_tokens > config.max_tokens,
                });
                section_index += 1;
            }
        }
    }

    chunks
}

// ---- helpers ----

/// Simple whitespace-based word count as token estimate (adequate for v1).
fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Collect the plain-text content of a node and its children.
fn collect_text<'a>(node: &'a Node<'a, RefCell<Ast>>) -> String {
    let mut text = String::new();
    collect_text_recursive(node, &mut text);
    text
}

fn collect_text_recursive<'a>(node: &'a Node<'a, RefCell<Ast>>, buf: &mut String) {
    let data = node.data.borrow();
    if let NodeValue::Text(ref t) = data.value {
        buf.push_str(t);
    } else if let NodeValue::Code(ref c) = data.value {
        buf.push_str(&c.literal);
    }
    for child in node.children() {
        collect_text_recursive(child, buf);
    }
}

/// Render a single AST node back to CommonMark markdown.
fn render_node<'a>(node: &'a Node<'a, RefCell<Ast>>) -> String {
    let mut buf = Vec::new();
    comrak::format_commonmark(node, &Options::default(), &mut buf)
        .expect("commonmark rendering should not fail");
    let text = String::from_utf8_lossy(&buf).trim().to_string();
    text
}

/// Join block texts with newlines.
fn join_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- tests ----

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ChunkConfig {
        ChunkConfig::default()
    }

    fn small_config() -> ChunkConfig {
        ChunkConfig { max_tokens: 10 }
    }

    #[test]
    fn standard_sections_produce_separate_chunks() {
        let md = "\
## Getting Started

This is the getting started section with some content.

## Installation

This is the installation section with different content.

## Configuration

This is the configuration section with more content.
";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading_path, vec!["Getting Started"]);
        assert_eq!(chunks[1].heading_path, vec!["Installation"]);
        assert_eq!(chunks[2].heading_path, vec!["Configuration"]);
    }

    #[test]
    fn nested_headings_build_heading_path() {
        let md = "\
## APIs

Overview of APIs.

### Authentication

Details about authentication.

### Authorization

Details about authorization.

## Other

Other content.
";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        assert!(chunks.len() >= 3);
        assert_eq!(chunks[0].heading_path, vec!["APIs"]);
        assert_eq!(chunks[1].heading_path, vec!["APIs", "Authentication"]);
        assert_eq!(chunks[2].heading_path, vec!["APIs", "Authorization"]);
        assert_eq!(chunks[3].heading_path, vec!["Other"]);
    }

    #[test]
    fn code_block_never_split() {
        let md = "\
## Example

Here is some code:

```rust
fn main() {
    println!(\"Hello, world!\");
}
```

And some follow-up text.
";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        // The entire section should be one chunk (all fits under 512 tokens).
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("fn main()"));
        assert!(chunks[0].content.contains("Hello, world!"));
    }

    #[test]
    fn flat_page_produces_single_chunk_with_empty_heading_path() {
        let md = "This is a flat page with no headings. Just plain content.";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].heading_path.is_empty());
        assert!(chunks[0].content.contains("flat page"));
    }

    #[test]
    fn oversized_section_splits_at_paragraph_boundaries() {
        // Use small max_tokens to force splitting.
        let md = "\
## Big Section

First paragraph with several words to fill up tokens here.

Second paragraph with even more words to exceed the small limit.

Third paragraph adding yet more content to test splitting behavior.
";
        let config = small_config();
        let chunks = chunk_markdown(md, &config).unwrap();
        // Should produce multiple chunks, all with the same heading_path.
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        for chunk in &chunks {
            assert_eq!(chunk.heading_path, vec!["Big Section"]);
        }
    }

    #[test]
    fn oversized_code_block_emitted_as_single_oversized_chunk() {
        // Create a code block that exceeds the small token limit.
        let mut code_lines = String::from("## Code\n\n```\n");
        for i in 0..50 {
            code_lines.push_str(&format!("line {i} of the very long code block here\n"));
        }
        code_lines.push_str("```\n");

        let config = small_config();
        let chunks = chunk_markdown(&code_lines, &config).unwrap();
        // Find the chunk containing the code block.
        let code_chunk = chunks.iter().find(|c| c.content.contains("line 0")).unwrap();
        assert!(code_chunk.oversized, "code block chunk should be marked oversized");
        // The code block should not be split.
        assert!(code_chunk.content.contains("line 49"));
    }

    #[test]
    fn table_is_atomic() {
        let md = "\
## Data

| Name | Value |
|------|-------|
| foo  | 1     |
| bar  | 2     |
";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("foo"));
        assert!(chunks[0].content.contains("bar"));
    }

    #[test]
    fn section_index_is_sequential() {
        let md = "\
## One

Content one.

## Two

Content two.

## Three

Content three.
";
        let chunks = chunk_markdown(md, &default_config()).unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.section_index, i, "section_index mismatch at {i}");
        }
    }

    #[test]
    fn empty_input_returns_error() {
        let result = chunk_markdown("", &default_config());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChunkError::EmptyInput));
    }

    #[test]
    fn whitespace_only_input_returns_error() {
        let result = chunk_markdown("   \n\n  ", &default_config());
        assert!(result.is_err());
    }
}
