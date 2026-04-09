# ADR-002: Document Chunking Strategy — Structure-Aware

**Status:** Accepted
**Date:** 2026-04-08
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

When a documentation page is published, Historiador Doc must split its content into discrete chunks stored in VexFS for semantic retrieval. The chunking strategy directly determines retrieval quality: poor chunking produces irrelevant or incomplete MCP responses; good chunking ensures the right knowledge unit is returned for any given query.

Documentation pages in Historiador Doc are markdown files with rich structural elements: headings (H1–H3), sections, paragraphs, lists, code blocks, and tables. The chunking strategy must preserve this structure to the maximum extent possible while producing chunks of a size suitable for LLM context windows.

Key constraints:
- Chunks must be semantically coherent — a chunk should answer one question or describe one concept
- Chunk metadata must include heading hierarchy (e.g., `Engineering > APIs > Authentication`) for source attribution in MCP responses
- Code blocks must never be split mid-block
- Chunks must carry a `language` field to support multilingual workspaces
- Maximum chunk size: configurable, default 512 tokens

---

## Decision

**Use structure-aware chunking** — the chunker parses the markdown AST and splits content at heading boundaries, using paragraph boundaries as the fallback split point for sections that exceed the maximum token threshold.

Fixed-size (character or token count) splitting is explicitly rejected for v1.

Implementation: use the `comrak` library (Rust) to parse markdown into a CommonMark AST, then walk the AST to extract heading-delimited sections as primary chunk candidates. The chunker lives in the `crates/chunker` crate and is shared between the API and MCP Cargo workspace members.

---

## Options Considered

### Option A: Structure-Aware (heading-boundary) ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Retrieval quality | High — each chunk maps to one logical topic |
| Implementation complexity | Medium — requires markdown AST parsing |
| Chunk coherence | High — chunks are semantically complete sections |
| Metadata richness | High — heading path is a natural source of attribution metadata |
| Code block preservation | Native — AST treats code blocks as atomic nodes |

**Pros:**
- Chunks are inherently meaningful — they correspond to how a human would navigate the document
- Heading hierarchy is automatically available as metadata (`Engineering / APIs / Authentication`)
- Code blocks, tables, and lists are treated as atomic units by the AST parser — no mid-block splits
- Directly compatible with how the AI editor structures pages (heading-driven sections)
- MCP responses include natural source attribution ("from the Authentication section of the APIs page")

**Cons:**
- Requires markdown AST parsing (remark or similar) — slightly more complex than naive splitting
- Sections with no headings (flat documents) require a fallback strategy
- Very short sections may produce undersized chunks that reduce retrieval precision

---

### Option B: Fixed-Size Token Splitting

| Dimension | Assessment |
|-----------|------------|
| Retrieval quality | Low-Medium — chunks frequently break mid-concept |
| Implementation complexity | Low — tokenize and split at N tokens with overlap |
| Chunk coherence | Low — no semantic alignment |
| Metadata richness | Low — chunk position in document, nothing more |
| Code block preservation | Poor — code blocks are split at token boundaries |

**Pros:**
- Trivial to implement — no AST parsing required
- Predictable and uniform chunk sizes
- Overlap (e.g., 20% token overlap between adjacent chunks) partially mitigates mid-concept splitting

**Cons:**
- Consistently breaks at semantically arbitrary points — a chunk might start mid-sentence or mid-procedure
- Code blocks split mid-block produce unusable chunks for technical documentation
- No meaningful metadata beyond byte offset
- Retrieval quality for documentation (structured, procedural content) is significantly worse than structure-aware splitting
- Rejected in v1 — not appropriate for documentation workloads

---

### Option C: Semantic Sentence Splitting (NLP-based)

| Dimension | Assessment |
|-----------|------------|
| Retrieval quality | Medium-High — sentence-level coherence |
| Implementation complexity | High — requires sentence boundary detection, often LLM-assisted |
| Chunk coherence | Medium — sentences are coherent but lose section context |
| Metadata richness | Low — no structural metadata |
| Code block preservation | Poor without special handling |

**Pros:**
- High coherence at the sentence level
- Can group related sentences by semantic similarity before chunking

**Cons:**
- Requires an additional NLP pass (or LLM call) to identify sentence boundaries and semantic groupings — adds latency and cost to the publish pipeline
- Loses the document's structural hierarchy as metadata
- Adds a significant dependency (spaCy, NLTK, or an LLM call per page)
- Premature optimization for v1 workloads — structure-aware chunking is sufficient and more explainable
- Better suited as a future enhancement for unstructured text; documentation is inherently structured

---

## Trade-off Analysis

Structure-aware chunking has a higher implementation cost than fixed-size splitting, but the quality difference is decisive for documentation content. Documentation pages created by Historiador Doc's AI editor are inherently structured — the editor produces heading-delimited sections by design. This makes structure-aware chunking a natural fit: the chunker and the editor are aligned on the same document model.

Semantic splitting offers marginal quality improvement over structure-aware for well-structured documents while adding significant complexity and latency. It is not justified for v1.

The `remark` AST parsing approach is well-supported in the Node.js ecosystem, widely used, and adds minimal dependency weight to the backend.

---

## Chunking Algorithm (Specification)

```
Input: markdown string, max_tokens (default: 512), language

1. Parse markdown to AST using remark
2. Walk AST, collecting heading nodes and their subtree content
3. For each heading node:
   a. Extract all content under this heading (until the next heading of equal or higher level)
   b. Measure token count
   c. If token count ≤ max_tokens → emit as one chunk
   d. If token count > max_tokens → split at paragraph boundaries, never mid-paragraph
      - Code block nodes are never split; if a code block alone exceeds max_tokens, emit it as an oversized chunk with a warning flag
4. For each chunk, emit:
   {
     content: string,
     heading_path: string[],   // e.g. ["Engineering", "APIs", "Authentication"]
     token_count: number,
     language: string,
     page_id: string,
     section_index: number,
     oversized: boolean
   }
5. Write all chunks to VexFS with embeddings
```

---

## Consequences

**Easier:**
- MCP responses include meaningful source attribution ("from the Authentication section")
- Authors can predict how their content will be chunked — what they write as a section becomes a retrievable unit
- Code blocks in technical documentation are preserved intact
- Future gap detection (v2) can identify gaps at the section/heading level, not just the page level

**Harder:**
- Flat pages (no headings) need a documented fallback behavior and author guidance
- The chunker must be updated if the AI editor is ever allowed to produce non-markdown output formats
- Oversized single-section pages (a single H2 with 2,000 words of prose) will produce large chunks with lower retrieval precision — authors should be guided toward shorter, more focused sections

**Must revisit:**
- If retrieval quality is insufficient for a particular content type (e.g., highly structured tables, step-by-step procedures), evaluate section-level semantic re-ranking as a v2 enhancement
- Monitor oversized chunk frequency in production telemetry; if common, consider auto-suggesting heading insertion to authors

---

## Action Items

1. [ ] Implement markdown chunker using `comrak` AST parser in `crates/chunker`
2. [ ] Define the `Chunk` struct in Rust and expose it via the OpenAPI schema as a TypeScript type
3. [ ] Write unit tests covering: standard sections, nested headings, code blocks, flat pages (no headings), oversized sections
4. [ ] Define the author guidance in the editor UI for flat/unstructured pages ("Add headings to improve AI retrieval quality")
5. [ ] Validate chunk quality by hand against 10 representative documentation pages before beta
