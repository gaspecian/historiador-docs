# pt-BR Platform Translation — Design

**Status:** Draft
**Date:** 2026-05-05
**Branch:** `feature/change-language`
**Author:** Gabriel Specian

---

## Goal

All user-visible text in `apps/web` and all API error messages that surface to the UI are in Brazilian Portuguese (pt-BR). Single-language replacement — no i18n framework, no locale switcher, no message catalogs.

## Non-Goals

This is option **A** of the three approaches considered: replace English with pt-BR. Future flexibility for multilingual UI (option B/C) is explicitly deferred.

---

## Strategy

Inline string replacement, layered by user-visible surface across nine focused PRs. Strings stay where they are; we translate them in place. No `next-intl`, no `react-intl`, no extraction step.

API error responses keep stable English **codes** (`{"error": "validation_error", ...}`) and translate only the human **message** field. The web layer continues to surface `err.message` from the API; that message arrives in pt-BR.

---

## Invariants

These constrain every PR. Violations are review blockers.

1. **Error codes stay English.** `apps/api/src/presentation/error.rs` returns `{ "error": "validation_error", "message": "..." }`. `error` is a stable machine-readable identifier — never translate. Only `message` is translated.
2. **OpenAPI/MCP/utoipa surfaces stay English.** `#[utoipa::path(summary, description, ...)]`, MCP tool descriptions, OpenAPI titles. The contract is a developer interface.
3. **AI prompt templates in `prompts/` stay as-is.** Translating them changes AI output behavior, not UI. Out of scope.
4. **Developer docs stay English.** README, CHANGELOG, CLAUDE.md, AGENTS.md, `docs/*.md`, `artifacts/adr/**`, `artifacts/sprints/**`.
5. **Logs and `tracing::*` stay English.** Operations and debugging remain English; only HTTP response `message` strings are translated.
6. **Identifiers, slugs, URLs, env vars, config keys stay English.** No renaming of variables, routes, or columns.
7. **Brand and proper nouns stay as-is.** "Historiador Doc", "MCP", "Chronik", "Postgres", "OpenAI", "Ollama", "Anthropic", etc.
8. **Tests update to match.** Tests asserting on user-visible strings are updated to pt-BR. Tests asserting on API responses migrate to assert on `error` code (machine-stable) rather than `message` (translated) where possible.

---

## Translation Glossary

Canonical terminology. Plan tasks reference this glossary instead of inventing translations ad-hoc.

| English | pt-BR | Notes |
|---|---|---|
| Workspace | Workspace | Keep English. Loanword in pt-BR tech vocabulary. |
| Collection | Coleção | |
| Page | Página | |
| Draft | Rascunho | Already used in `editor/page.tsx:283`. |
| Outline | Esboço | |
| Chunk | Fragmento | User-facing only. Code/log identifiers stay "chunk". |
| Block (editor) | Bloco | |
| Comment | Comentário | |
| Proposal | Proposta | Editor proposal overlay. |
| Autonomy mode | Modo de autonomia | |
| Knowledge base | Base de conhecimento | |
| Provider (LLM) | Provedor | "Provedor de LLM" when ambiguous. |
| Generation model | Modelo de geração | |
| Embedding | Embedding | Keep English. |
| Token | Token | Keep English. |
| Setup / Installation | Configuração inicial | First-run wizard. |
| Setup wizard | Assistente de configuração | |
| Search (verb) / Query (noun) | Buscar / Consulta | |
| Login / Sign in | Entrar | Button label. |
| Sign out / Log out | Sair | |
| Register / Sign up | Criar conta | |
| Activate (account) | Ativar | |
| Invite (verb) / Invite (noun) | Convidar / Convite | |
| Role | Perfil | |
| Admin / Editor / Viewer | Administrador / Editor / Leitor | |
| Save | Salvar | |
| Cancel | Cancelar | |
| Delete | Excluir | Gentler than "Apagar" in Brazilian SaaS. |
| Edit | Editar | |
| Approve | Aprovar | |
| Reject | Rejeitar | |
| Regenerate | Regenerar | |
| Revoke | Revogar | |
| Restore | Restaurar | |
| Loading... | Carregando... | |
| Saved / Unsaved | Salvo / Não salvo | |
| Failed to ... | Não foi possível ... | Idiomatic; avoid literal "Falhou ao". |
| ... required | ... obrigatório | "Title required" → "Título obrigatório". |
| Settings | Configurações | |

---

## Phase Breakdown

Each phase is one PR. Each phase produces a working app — partial translation is acceptable mid-sequence (the app is already partially translated).

### Phase 1 — Setup wizard

**Files:**
- `apps/web/app/setup/page.tsx`

**Surface:** First-run installation wizard (LLM provider selection, admin user creation, workspace languages, MCP token).

### Phase 2 — Auth

**Files:**
- `apps/web/app/(auth)/layout.tsx`
- `apps/web/app/(auth)/login/page.tsx`
- `apps/web/app/(auth)/activate/page.tsx`

**Surface:** Login form, account activation (password setup), error toasts ("Login failed", "Passwords do not match", "Missing activation token").

### Phase 3 — Dashboard chrome

**Files:**
- `apps/web/app/page.tsx`
- `apps/web/app/(dashboard)/layout.tsx`
- `apps/web/app/(dashboard)/dashboard/pages/page.tsx`
- `apps/web/app/(dashboard)/dashboard/pages/[id]/page.tsx`
- `apps/web/components/layout/user-menu.tsx`
- `apps/web/components/collections/collection-tree.tsx`
- `apps/web/components/collections/collection-tree-node.tsx`
- `apps/web/components/collections/create-collection-dialog.tsx`
- `apps/web/components/pages/page-list.tsx`
- `apps/web/components/pages/search-bar.tsx`
- `apps/web/components/pages/draft-publish-toggle.tsx`
- `apps/web/components/pages/publish-confirm-modal.tsx`
- `apps/web/components/pages/version-history-panel.tsx`
- `apps/web/components/pages/language-badges.tsx`
- `apps/web/components/pages/language-tabs.tsx`

### Phase 4 — Editor

Some strings are already translated on this branch; audit each file and complete what's missing.

**Files:**
- `apps/web/app/(dashboard)/editor/page.tsx`
- `apps/web/components/editor/editor-panel.tsx`
- `apps/web/features/editor/editor-v2.tsx`
- `apps/web/features/editor/auto-save-banner.tsx`
- `apps/web/features/editor/split-pane.tsx`
- `apps/web/features/editor/save/save-dialog.tsx`
- `apps/web/features/editor/chat/chat-pane.tsx`
- `apps/web/features/editor/chat/composer.tsx`
- `apps/web/features/editor/chat/message-list.tsx`
- `apps/web/features/editor/outline/outline-card.tsx`
- `apps/web/features/editor/overlay/proposal-panel.tsx`
- `apps/web/features/editor/comments/comment-composer.tsx`
- `apps/web/features/editor/comments/comment-panel.tsx`
- `apps/web/features/editor/autonomy/autonomy-selector.tsx`
- `apps/web/features/editor/autonomy/checkpoint-card.tsx`
- `apps/web/features/editor/templates/template-picker.tsx`
- `apps/web/features/editor/toolbar/inline-toolbar.tsx`
- `apps/web/features/editor/review/commentable-preview.tsx`
- `apps/web/features/editor/canvas/canvas.tsx` (placeholders, empty states)

### Phase 5 — Admin

**Files:**
- `apps/web/app/(dashboard)/dashboard/admin/page.tsx`
- `apps/web/components/admin/llm-settings-form.tsx`
- `apps/web/components/admin/mcp-settings.tsx`
- `apps/web/components/admin/mcp-analytics.tsx`
- `apps/web/components/admin/user-list.tsx`
- `apps/web/components/admin/invite-user-form.tsx`
- `apps/web/components/admin/workspace-config.tsx`
- `apps/web/components/admin/export-section.tsx`

### Phase 6 — UI primitives

**Files:** `apps/web/components/ui/*.tsx` (8 files: badge, button, card, copy-button, dialog, dropdown, input, select, spinner).

Most have no copy. Expect only: spinner aria-label, dialog close button, copy-button success/idle text.

`apps/web/app/design-system/page.tsx` is **out of scope** — developer showcase, not an end-user surface.

### Phase 7 — API error messages (Rust)

**Files:**
- `apps/api/src/presentation/error.rs` — translate `message` strings in `code_and_message()` (the canonical mapping).
- `apps/api/src/application/admin/invite_user.rs:45` (Conflict)
- `apps/api/src/application/admin/update_llm_config.rs:89,119` (Validation)
- `apps/api/src/application/collections/create_collection.rs:31,52` (Validation, Conflict)
- `apps/api/src/application/collections/update_collection.rs:55` (Conflict)
- `apps/api/src/application/pages/create_page.rs:81` (Conflict)
- `apps/api/src/application/pages/update_page.rs:59` (Validation)
- `apps/api/src/application/pages/version_history.rs:142` (Validation)
- `apps/api/src/application/setup/bcp47.rs:12,18,23` (Validation)
- `apps/api/src/application/setup/initialize_installation.rs:77,92` (Validation)
- `apps/api/src/application/setup/list_ollama_models.rs:19` (Validation)
- `apps/api/src/domain/value/email.rs:18,20` (Validation)
- `apps/api/src/domain/value/language.rs:15` (Validation)
- `apps/api/src/domain/value/slug.rs:13,19` (Validation)
- `apps/api/src/infrastructure/chronik/analytics.rs:29` (Validation surfacing to admin)

**Skipped (internal/infrastructure errors that surface as opaque 500s):**
- `apps/api/src/infrastructure/backfill/service.rs:180,194`
- `apps/api/src/infrastructure/chunker/pipeline.rs:76`
- `apps/api/src/infrastructure/token/jwt_issuer.rs:41`
- `apps/api/src/application/editor/iterate_draft.rs:55`
- `apps/api/src/application/editor/generate_draft.rs:59`
- `apps/api/src/bin/load_test_seed.rs`, `apps/api/src/bin/backfill_block_ids.rs` (CLI tools)
- `apps/api/src/presentation/handler/auth.rs:68,148` and `handler/admin/users.rs:57,66` — forward upstream `e.to_string()`; the upstream string (already translated in this phase) is what reaches the user.
- `#[error("...")]` attributes on `ApiError` enum variants (`unauthorized`, `forbidden`, `not found`, `setup required`) — used as the `Display` impl in logs and as input to `code_and_message`'s `code` field. Stay English.

### Phase 8 — Locale (`<html lang>` and `Intl` formatting)

**Files:**
- `apps/web/app/layout.tsx` — `<html lang="pt-BR">`.
- `apps/web/lib/format.ts` *(new file)* — exports `formatDate`, `formatDateTime`, `formatTime`, `formatNumber`.

```ts
const DATE_FMT = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short" });
const DATETIME_FMT = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "short" });
const TIME_FMT = new Intl.DateTimeFormat("pt-BR", { timeStyle: "short" });
const NUMBER_FMT = new Intl.NumberFormat("pt-BR");

export const formatDate = (d: Date | string | number) => DATE_FMT.format(new Date(d));
export const formatDateTime = (d: Date | string | number) => DATETIME_FMT.format(new Date(d));
export const formatTime = (d: Date | string | number) => TIME_FMT.format(new Date(d));
export const formatNumber = (n: number) => NUMBER_FMT.format(n);
```

`Intl` constructors are expensive — cache once at module load.

**Sweep targets:** `.toLocaleTimeString()`, `.toLocaleDateString()`, `.toLocaleString()`, hardcoded date concatenations. Known call sites include:
- `apps/web/features/editor/editor-v2.tsx:190` — `Saved ${savedAt.toLocaleTimeString()}`
- `apps/web/components/pages/version-history-panel.tsx` — version timestamps
- `apps/web/components/admin/mcp-analytics.tsx` — chart axis labels
- `apps/web/components/pages/page-list.tsx` — list timestamps

Replace each with the new helpers.

**Relative time** ("há 2 minutos") at call sites that need it: `Intl.RelativeTimeFormat("pt-BR")` inline. Don't centralize until reused across multiple components.

### Phase 9 — Test sweep

**Rust (`cargo test --workspace`):**
- Tests asserting on `ApiError`/`DomainError` `message` strings → update to pt-BR.
- Tests asserting on HTTP response body text → migrate from `message` to `error` (code) field where possible. If a test only checks `message`, switch to `error`.
- Tests asserting on `Display` impl of error variants — update if pt-BR was introduced; leave alone otherwise.

**Web:**
- No existing UI string-assertion tests found in initial scan. If any surface during sweeps, update assertions.
- Per-phase: `pnpm lint` and manual click-through in `pnpm dev` for the touched flow.

**OpenAPI codegen:**
- Phase 7 must not change utoipa annotations (Invariant 2). After Phase 7, run `pnpm gen:types`. Any non-empty diff to `openapi.yaml` or `packages/types/generated/index.ts` indicates a leaked translation in an annotation — revert it.

---

## Per-Phase Workflow

Each phase PR follows the same checklist:

1. Translate strings per glossary.
2. `cargo fmt --all --check` (if Rust touched).
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings` (if Rust touched).
4. `cargo test --workspace` (if Rust touched).
5. `pnpm lint` (if web touched).
6. `pnpm gen:types` and confirm no diff (if Rust touched in Phase 7 — protects Invariant 2).
7. Manual smoke: walk the touched flow in `pnpm dev`, verify no orphan English, no broken layouts from longer pt-BR words.
8. Commit. Title uses `i18n(scope): translate <surface> to pt-BR`.

---

## Acceptance Criteria

1. All files in Phases 1–7 contain only pt-BR user-visible strings, consistent with the glossary.
2. `<html lang="pt-BR">`. Date/time/number rendering on touched surfaces uses the `Intl` `pt-BR` helpers.
3. `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check` all pass.
4. `pnpm lint` passes.
5. Manual smoke through setup → login → dashboard → editor → admin shows no orphan English strings and no obvious layout breaks (longer pt-BR words may overflow buttons; fix or accept per case).
6. After Phase 7: `pnpm gen:types` produces no diff (proves utoipa annotations were not touched).

---

## Out of Scope

Explicitly not changed by this work:

- `prompts/` directory (AI prompt templates).
- `#[utoipa::path(...)]` summaries/descriptions and MCP tool descriptions.
- README, CHANGELOG, CLAUDE.md, AGENTS.md, `docs/*.md`, `artifacts/adr/**`, `artifacts/sprints/**`.
- `apps/web/app/design-system/page.tsx` (developer showcase).
- Internal `tracing::*` log messages.
- `anyhow!`/`bail!` strings in non-user-facing paths (backfill, chunker, JWT issuer, draft generation, CLI bins).
- Database column/table renames, route paths, env var names, identifiers.
- Workspace `languages` config seeding (ADR-005 content language is orthogonal).
- Email templates (none confirmed to exist).
