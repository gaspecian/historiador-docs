# ADR-005: Multilingual Documentation Architecture — Installation-Time Language Configuration

**Status:** Accepted
**Date:** 2026-04-08
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Historiador Doc targets companies globally. A Brazilian company may need documentation in Portuguese and English. A European enterprise may require English, French, and German. A global company might require five or more languages across its knowledge base.

The platform must handle multilingual documentation without becoming complex for non-technical authors. The key tension is between **flexibility** (letting each author or page define its own language) and **enforceability** (ensuring the company's language standards are consistently met across all documentation).

The system also needs to ensure the MCP retrieval layer handles multilingual queries and responses correctly — an AI tool querying in Spanish should retrieve the Spanish-language chunks if they exist.

Key constraints:
- Language configuration must be enforceable company-wide — not optional per-author
- Non-technical authors should not need to understand or manage language settings
- The AI editor must assist authors in producing documentation in each required language
- Chunk metadata must include language for MCP-side filtering
- Configuration should be set at installation time by IT, not discovered post-deployment

---

## Decision

**Language support is defined as a global workspace configuration set during installation.** The IT admin specifies one or more required languages in the first-run setup wizard. This configuration:

- Drives the AI editor to prompt authors for content in each required language
- Is applied uniformly to all pages — there are no per-page or per-author language overrides in v1
- Is stored in the workspace settings table in PostgreSQL
- Is included as metadata on every chunk written to VexFS

Pages missing required language versions are visually flagged in the dashboard but are not blocked from publishing in v1.

---

## Options Considered

### Option A: Installation-Time Global Configuration ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Enforceability | High — a single setting governs all content |
| Author experience | Simple — authors follow the system prompt |
| IT control | Strong — set once, applies everywhere |
| Flexibility | Low — no per-page or per-author overrides |
| Implementation complexity | Low-Medium |

**Pros:**
- IT defines the standard once at installation; no ongoing configuration burden
- Authors never need to think about language settings — the editor prompts them automatically
- Consistent enforcement across the entire knowledge base
- Simple data model: workspace has a `languages: string[]` field; all pages and chunks inherit it
- Straightforward MCP behavior: all chunks carry a `language` tag; queries can be filtered by language

**Cons:**
- No flexibility for edge cases (e.g., one collection that should only be in English within a workspace configured for three languages)
- If the company's language requirements change, updating the configuration does not retroactively flag existing pages that may now be missing a language version
- Per-collection language overrides are a frequent feature request in multilingual documentation tools — not available in v1

---

### Option B: Per-Page Language Configuration

| Dimension | Assessment |
|-----------|------------|
| Enforceability | Low — each author decides |
| Author experience | Complex — authors must manage language settings per page |
| IT control | Weak — no workspace-level enforcement |
| Flexibility | High |
| Implementation complexity | Medium |

**Pros:**
- Maximum flexibility — a page about a French-only regulation can be French-only
- Authors who are domain experts can decide what translations are needed

**Cons:**
- Without enforcement, language coverage becomes inconsistent — some pages have all required languages, others have only one
- Non-technical authors may not know which languages are required
- Creates a support and auditing problem: "which pages are missing a Portuguese version?" requires a dashboard query rather than being the default state
- Contradicts the democratic, opinionated design philosophy — puts too much configuration responsibility on authors

**Rejected** — too much author burden, too little enforcement.

---

### Option C: Per-Author Language Assignment (each author writes in their native language)

| Dimension | Assessment |
|-----------|------------|
| Enforceability | Medium |
| Author experience | Simple for individuals, complex for coordination |
| IT control | Medium |
| Flexibility | Medium |
| Implementation complexity | High — requires translation workflow |

**Pros:**
- Authors write in the language they know best
- Could enable a "translation request" workflow where one author writes in one language and requests translation from another

**Cons:**
- Requires a translation coordination workflow that is significantly beyond v1 scope
- Does not ensure a single page is available in all required languages — it depends on author coordination
- Introduces a new persona (translator) and a new workflow state (pending translation)
- Premature complexity for v1

**Rejected** — translation workflow is v2 scope at earliest.

---

## Data Model

### Workspace Language Configuration

```typescript
// PostgreSQL: workspace table
interface Workspace {
  id: string;
  name: string;
  languages: string[];        // e.g. ["pt-BR", "en-US"] — BCP 47 language tags
  primary_language: string;   // e.g. "pt-BR" — used as default in editor
  created_at: Date;
  updated_at: Date;
}
```

### Page Language Versions

```typescript
// PostgreSQL: page_versions table
interface PageVersion {
  page_id: string;
  language: string;           // BCP 47 language tag
  title: string;
  content_markdown: string;
  status: 'draft' | 'published';
  author_id: string;
  created_at: Date;
  updated_at: Date;
}
```

A single `page` record has one or more `page_version` records — one per language. The page is considered complete when all workspace `languages` have a published `page_version`.

### Chunk Language Metadata

```typescript
// VexFS chunk metadata
interface Chunk {
  page_id: string;
  language: string;           // BCP 47 language tag — same as page_version.language
  heading_path: string[];
  content: string;
  token_count: number;
  section_index: number;
  oversized: boolean;
}
```

---

## MCP Query Language Handling

The MCP server handles language in queries as follows:

1. **No language specified in query**: return best-matching chunks across all languages, ordered by semantic similarity score. Include `language` in each chunk's source attribution so the AI tool can display it.
2. **Language specified in query** (e.g., via a query parameter or MCP tool argument): filter VexFS results to chunks matching the specified language before scoring.
3. **Query language detection** (v2 consideration): automatically detect the language of the incoming query and prefer chunks in the same language — not implemented in v1.

---

## Author Experience in the AI Editor

When a workspace is configured for multiple languages (e.g., `["pt-BR", "en-US"]`):

1. The editor presents a language tab for each configured language
2. The primary language tab is shown first and focused by default
3. As the author writes in the primary language, the AI can offer to auto-draft the secondary language version — the author reviews and edits before saving
4. The publish button shows the completion status of each language version:
   - ✅ Portuguese — complete
   - ⚠️ English — draft
5. Publishing with incomplete language versions shows a warning ("English version is a draft — publish anyway?") but does not block the action in v1

---

## Consequences

**Easier:**
- IT admins have a clear, single point of control for language standards
- Dashboard can trivially show which pages are missing required language versions
- MCP chunk metadata is always consistent — every chunk has a `language` field
- The AI editor knows exactly what language to use for each draft — no author decision required

**Harder:**
- Per-collection language overrides require a data model change — not straightforward to add post-v1 without a migration
- If a company changes its required languages after deployment, existing pages must be audited manually (no automatic retroactive flagging in v1)
- The AI editor auto-draft feature for secondary languages requires a second LLM call per publish — must be asynchronous to avoid blocking the publish action

**Must revisit:**
- Per-collection language configuration is a predictable v2 request — the data model should be designed to accommodate a `collection_languages` override without a full schema migration
- Automatic language detection for MCP queries (query language → prefer matching chunks) is a high-value v2 improvement that can be added without schema changes

---

## Action Items

1. [ ] Implement `languages` and `primary_language` fields on the workspace model; include in the first-run setup wizard
2. [ ] Implement `page_versions` table with per-language content storage
3. [ ] Update the AI editor UI to show language tabs matching workspace configuration
4. [ ] Implement page completeness indicator in the dashboard (flag pages missing required language versions)
5. [ ] Ensure all VexFS chunk writes include the `language` field from the page version
6. [ ] Implement language filter support in the MCP server query handler
7. [ ] Define the BCP 47 language tag list supported in the setup wizard (must be supported by the configured LLM)
