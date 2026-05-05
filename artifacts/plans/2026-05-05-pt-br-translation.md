# pt-BR Platform Translation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all user-visible English text in `apps/web` and all user-facing API error messages in `apps/api` with Brazilian Portuguese, single-language.

**Architecture:** Inline string replacement, no i18n framework. Strings are translated in place per the glossary in [artifacts/specs/2026-05-05-pt-br-translation-design.md](../specs/2026-05-05-pt-br-translation-design.md). API keeps stable English `error` codes; only the human `message` field is translated. OpenAPI/MCP/utoipa surfaces, prompt templates, and developer docs stay English (see Invariants in spec).

**Tech Stack:** Next.js 16 + React 19 (web), Axum + thiserror (API), `Intl.*` for formatting. No new dependencies.

**Reference:** Read [artifacts/specs/2026-05-05-pt-br-translation-design.md](../specs/2026-05-05-pt-br-translation-design.md) before starting. The glossary there is the canonical translation reference. When in doubt, follow it; when not in glossary, follow style rules ("Failed to ..." → "Não foi possível ...", "... required" → "... obrigatório").

**Working state:** Branch `feature/change-language` already has many files partially translated. Most tasks are *audit-and-finish*, not greenfield translation.

---

## Per-Task Workflow (apply to every task)

Every task ends with this sequence:

1. **Audit:** Run the per-file grep below; fix anything found.
2. **Lint:** `pnpm lint` (web tasks) or `cargo clippy --workspace --all-targets --all-features -- -D warnings` (Rust tasks).
3. **Build/types:** `pnpm --filter web type-check` if the file imports types; `cargo check --workspace` for Rust.
4. **Test:** `cargo test --workspace` for Rust tasks.
5. **Manual smoke:** `pnpm --filter web dev` and walk the affected flow if web; for Rust handler changes, hit the endpoint with `curl` and confirm pt-BR `message` field.
6. **Commit** with conventional message `i18n(<scope>): translate <surface> to pt-BR`.

**Audit grep** (run inside the touched file's directory):

```bash
# Strings that look like English (capital + lowercase + space + lowercase word)
grep -nE '"[A-Z][a-z]+ [a-z]+' <file>
# JSX text nodes that look English
grep -nE '>[A-Z][a-z]+( [a-z]+)+<' <file>
# Common English UI verbs
grep -nE '"(Save|Cancel|Delete|Edit|Login|Logout|Loading|Error|Submit|Create|Update|Failed|Success|Confirm|Close|Open|Download|Upload|Copy|Restore|Regenerate|Revoke|Approve|Reject|Activate|Deactivate|Invite)( |\.|"|\?)' <file>
```

If a match is genuinely a developer-facing string (constant key, error code, log message), leave it alone. Only translate user-visible content per Invariant 5 in the spec.

---

## Phase 1 — Setup wizard

### Task 1: Finish setup wizard translation

**Files:**
- Modify: `apps/web/app/setup/page.tsx`

Most strings are already pt-BR. Only fallback error strings remain English.

- [ ] **Step 1: Translate the three `"Connection failed"`/`"Setup failed"` fallback strings**

In `apps/web/app/setup/page.tsx`:

```
Line 152: msg || `Failed to list models (HTTP ${modelsRes.status})`
       → msg || `Não foi possível listar os modelos (HTTP ${modelsRes.status})`

Line 159: message: err instanceof Error ? err.message : "Connection failed",
       → message: err instanceof Error ? err.message : "Falha na conexão",

Line 192: throw new Error(body.message || "Setup failed");
       → throw new Error(body.message || "Falha na configuração");

Line 201: setError(err instanceof Error ? err.message : "Setup failed");
       → setError(err instanceof Error ? err.message : "Falha na configuração");
```

- [ ] **Step 2: Audit the file**

Run: `grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/setup/page.tsx`
Expected: matches are limited to `Spinner`, JSX class names with capitalized English (e.g., `text-amber-600`), `BCP 47`, model names ("OpenAI", "Anthropic", "Ollama"), or template/code samples (`ollama pull`). No genuine UI English remains.

- [ ] **Step 3: Run web checks**

```bash
pnpm --filter web lint
pnpm --filter web type-check
```

Expected: PASS.

- [ ] **Step 4: Smoke test**

Start the API + web (`cargo run -p historiador_api --bin api` then `cd apps/web && pnpm dev`). Open `http://localhost:3000`. Walk the setup wizard end to end on a fresh DB. Confirm: zero English strings; setup completes successfully; auto-login redirects to `/dashboard/pages`.

- [ ] **Step 5: Commit**

```bash
git add apps/web/app/setup/page.tsx
git commit -m "i18n(setup): translate fallback error strings to pt-BR"
```

---

## Phase 2 — Auth

### Task 2: Translate `(auth)/login/page.tsx`

**Files:** `apps/web/app/(auth)/login/page.tsx`

Already mostly pt-BR. Only the error fallback is English.

- [ ] **Step 1: Translate the fallback error**

```
Line 68: setError(err instanceof Error ? err.message : "Login failed");
      → setError(err instanceof Error ? err.message : "Falha ao entrar");
```

- [ ] **Step 2: Audit, lint, type-check**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/\(auth\)/login/page.tsx
pnpm --filter web lint
pnpm --filter web type-check
```

Expected: only matches are `Nexian Tech` brand and `historiador.doc` mono text — leave both. PASS on lint and type-check.

- [ ] **Step 3: Smoke test**

`pnpm dev` → submit login with a wrong password → confirm error toast says `Falha ao entrar` (or the pt-BR API message).

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/\(auth\)/login/page.tsx
git commit -m "i18n(auth): translate login fallback error to pt-BR"
```

### Task 3: Translate `(auth)/activate/page.tsx`

**Files:** `apps/web/app/(auth)/activate/page.tsx`

Most strings are still English. Full translation needed.

- [ ] **Step 1: Translate every user-visible string**

In `apps/web/app/(auth)/activate/page.tsx`:

```
Line 39: setError("Password must be at least 12 characters");
      → setError("A senha precisa ter no mínimo 12 caracteres");

Line 43: setError("Passwords do not match");
      → setError("As senhas não coincidem");

Line 47: setError("Missing activation token");
      → setError("Token de ativação ausente");

Line 73: setError(err instanceof Error ? err.message : "Activation failed");
      → setError(err instanceof Error ? err.message : "Falha ao ativar a conta");

Line 83: <h1 className="text-xl font-bold">Account activated</h1>
      → <h1 className="text-xl font-bold">Conta ativada</h1>

Line 84: <p className="text-sm text-text-tertiary">Redirecting to login...</p>
      → <p className="text-sm text-text-tertiary">Redirecionando para o login…</p>

Line 94: <h1 className="text-2xl font-bold">Activate your account</h1>
      → <h1 className="text-2xl font-bold">Ative sua conta</h1>

Line 96: Set a password to complete your registration
      → Defina uma senha para concluir seu cadastro

Line 102: label="Password"
       → label="Senha"

Line 106: placeholder="Min. 12 characters"
       → placeholder="Mín. 12 caracteres"

Line 111: label="Confirm password"
       → label="Confirmar senha"

Line 122: {loading ? "Activating..." : "Activate account"}
       → {loading ? "Ativando…" : "Ativar conta"}
```

- [ ] **Step 2: Audit, lint, type-check**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/\(auth\)/activate/page.tsx
pnpm --filter web lint
pnpm --filter web type-check
```

Expected: no English UI strings remain; lint and type-check PASS.

- [ ] **Step 3: Smoke test**

Trigger a fake invite (or use existing admin invite endpoint) → open the activation URL → confirm all strings are pt-BR; submit mismatched passwords → confirm `As senhas não coincidem`.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/\(auth\)/activate/page.tsx
git commit -m "i18n(auth): translate activate page to pt-BR"
```

### Task 4: Translate `(auth)/layout.tsx`

**Files:** `apps/web/app/(auth)/layout.tsx`

- [ ] **Step 1: Read the file and translate any user-visible strings per glossary**

```bash
cat apps/web/app/\(auth\)/layout.tsx
```

If the file is purely structural (only React layout wrapper with no copy), no changes needed — proceed to commit empty (skip this task) or simply move on. If copy is present, translate it.

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/\(auth\)/layout.tsx
```

Expected: no UI English strings.

- [ ] **Step 3: If changes were made, lint and commit**

```bash
pnpm --filter web lint
git add apps/web/app/\(auth\)/layout.tsx
git commit -m "i18n(auth): translate auth layout to pt-BR"
```

If no changes were needed, skip the commit and move to Phase 3.

---

## Phase 3 — Dashboard chrome

### Task 5: Dashboard layout and landing

**Files:**
- `apps/web/app/page.tsx`
- `apps/web/app/(dashboard)/layout.tsx`
- `apps/web/components/layout/user-menu.tsx`

- [ ] **Step 1: Read each file and identify English strings**

```bash
cat apps/web/app/page.tsx
cat apps/web/app/\(dashboard\)/layout.tsx
cat apps/web/components/layout/user-menu.tsx
```

For each English string found, translate per glossary:
- "Sign out" / "Log out" → "Sair"
- "Settings" → "Configurações"
- "Profile" → "Perfil"
- "Admin" → "Administração"
- "Pages" → "Páginas"
- "Editor" → "Editor"
- "Dashboard" → "Painel"
- "Workspace" stays "Workspace"
- "Documentation" → "Documentação"
- "Loading..." → "Carregando…"

Apply `Edit` for each occurrence.

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/page.tsx apps/web/app/\(dashboard\)/layout.tsx apps/web/components/layout/user-menu.tsx
```

Expected: no UI English remains; only brand strings, type names, or class fragments.

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/app/page.tsx apps/web/app/\(dashboard\)/layout.tsx apps/web/components/layout/user-menu.tsx
git commit -m "i18n(dashboard): translate layout and user menu to pt-BR"
```

### Task 6: Collections components

**Files:**
- `apps/web/components/collections/collection-tree.tsx`
- `apps/web/components/collections/collection-tree-node.tsx`
- `apps/web/components/collections/create-collection-dialog.tsx`

Known English strings (from initial scan):

```
create-collection-dialog.tsx:32:
  setError(err instanceof Error ? err.message : "Failed to create collection");
  → setError(err instanceof Error ? err.message : "Não foi possível criar a coleção");

create-collection-dialog.tsx:41:
  placeholder="Collection name"
  → placeholder="Nome da coleção"
```

- [ ] **Step 1: Apply known translations and audit each file**

For each file run `cat <file>` and translate any other English strings found per glossary. Common ones likely present:
- "Create collection" → "Criar coleção"
- "New collection" → "Nova coleção"
- "Cancel" → "Cancelar"
- "Save" → "Salvar"
- "No collections yet" → "Nenhuma coleção ainda"
- "Drag to reorder" → "Arraste para reordenar"
- "Children" / "Subcollections" → "Sub-coleções"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/components/collections/*.tsx
```

Expected: no UI English remains.

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/collections/
git commit -m "i18n(collections): translate collection tree and dialog to pt-BR"
```

### Task 7: Page list, search, draft toggle, language widgets

**Files:**
- `apps/web/components/pages/page-list.tsx`
- `apps/web/components/pages/search-bar.tsx`
- `apps/web/components/pages/draft-publish-toggle.tsx`
- `apps/web/components/pages/publish-confirm-modal.tsx`
- `apps/web/components/pages/language-badges.tsx`
- `apps/web/components/pages/language-tabs.tsx`

Known English strings:

```
publish-confirm-modal.tsx:17:
  <h3 className="text-lg font-semibold">Incomplete language coverage</h3>
  → <h3 className="text-lg font-semibold">Cobertura de idiomas incompleta</h3>
```

- [ ] **Step 1: Read every file, translate per glossary**

Common terms to expect:
- "Search pages" / "Search..." → "Buscar páginas" / "Buscar…"
- "No pages yet" → "Nenhuma página ainda"
- "Draft" → "Rascunho"
- "Published" → "Publicada"
- "Publish" → "Publicar"
- "Unpublish" → "Despublicar"
- "Title" → "Título"
- "Last updated" → "Última atualização"
- "Languages" / "Language" → "Idiomas" / "Idioma"
- "Missing" → "Faltando"
- "Complete" → "Completo"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/components/pages/*.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/pages/
git commit -m "i18n(pages): translate page list, search, language widgets to pt-BR"
```

### Task 8: Page detail screens

**Files:**
- `apps/web/app/(dashboard)/dashboard/pages/page.tsx`
- `apps/web/app/(dashboard)/dashboard/pages/[id]/page.tsx`
- `apps/web/components/pages/version-history-panel.tsx`

Known English strings:

```
[id]/page.tsx:67:
  return <div className="text-center py-8 text-text-tertiary">Page not found</div>;
  → return <div className="text-center py-8 text-text-tertiary">Página não encontrada</div>;

[id]/page.tsx:155:
  title="Version history"
  → title="Histórico de versões"

[id]/page.tsx:169:
  <span aria-label="More actions">⋮</span>
  → <span aria-label="Mais ações">⋮</span>

version-history-panel.tsx:206:
  {restoring ? "Restoring..." : "Restore as draft"}
  → {restoring ? "Restaurando…" : "Restaurar como rascunho"}
```

- [ ] **Step 1: Apply known translations and read each file for any remaining English**

Common terms:
- "Page not found" → "Página não encontrada"
- "More actions" → "Mais ações"
- "Version history" → "Histórico de versões"
- "Versions" → "Versões"
- "Compare" → "Comparar"
- "Restore" → "Restaurar"
- "View" → "Visualizar"
- "Edit" → "Editar"
- "Delete" → "Excluir"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/app/\(dashboard\)/dashboard/pages/*.tsx apps/web/app/\(dashboard\)/dashboard/pages/\[id\]/page.tsx apps/web/components/pages/version-history-panel.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/app/\(dashboard\)/dashboard/pages/ apps/web/components/pages/version-history-panel.tsx
git commit -m "i18n(pages): translate page detail and version history to pt-BR"
```

---

## Phase 4 — Editor

### Task 9: Editor page chrome and panels

**Files:**
- `apps/web/app/(dashboard)/editor/page.tsx`
- `apps/web/components/editor/editor-panel.tsx`
- `apps/web/features/editor/editor-v2.tsx`
- `apps/web/features/editor/auto-save-banner.tsx`
- `apps/web/features/editor/split-pane.tsx`

Known English strings:

```
components/editor/editor-panel.tsx:75:
  placeholder="Describe the document you want to create..."
  → placeholder="Descreva o documento que você quer criar…"

components/editor/editor-panel.tsx:91:
  placeholder="Describe what to change..."
  → placeholder="Descreva o que mudar…"

features/editor/editor-v2.tsx:190:
  {savedAt ? `Saved ${savedAt.toLocaleTimeString()}` : "Not saved yet"}
  → {savedAt ? `Salvo ${savedAt.toLocaleTimeString()}` : "Ainda não salvo"}
```

(Phase 8 will replace `toLocaleTimeString()` with the `pt-BR` formatter — leave the call as-is for now; Phase 8 picks it up.)

- [ ] **Step 1: Apply known translations + read each file**

Common editor terms:
- "Save" → "Salvar"
- "Saving..." → "Salvando…"
- "Saved" / "Unsaved" → "Salvo" / "Não salvo"
- "Auto-save enabled" → "Salvamento automático ativado"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' \
  apps/web/app/\(dashboard\)/editor/page.tsx \
  apps/web/components/editor/editor-panel.tsx \
  apps/web/features/editor/editor-v2.tsx \
  apps/web/features/editor/auto-save-banner.tsx \
  apps/web/features/editor/split-pane.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/app/\(dashboard\)/editor/page.tsx apps/web/components/editor/editor-panel.tsx apps/web/features/editor/editor-v2.tsx apps/web/features/editor/auto-save-banner.tsx apps/web/features/editor/split-pane.tsx
git commit -m "i18n(editor): translate editor page chrome and panels to pt-BR"
```

### Task 10: Editor chat

**Files:**
- `apps/web/features/editor/chat/chat-pane.tsx`
- `apps/web/features/editor/chat/composer.tsx`
- `apps/web/features/editor/chat/message-list.tsx`

Known English strings:

```
chat/composer.tsx:59:
  aria-label="Send"
  → aria-label="Enviar"
```

`chat/composer.tsx:8` has a JSDoc comment referencing `"Skip discovery"` — that's documentation about a UI label. The actual UI label probably exists somewhere (search for it in the file). If found, translate to "Pular descoberta"; if it's just a JSDoc reference, leave it (developer-facing comment).

- [ ] **Step 1: Apply known + read each file**

Common chat terms:
- "Send" → "Enviar"
- "New message" → "Nova mensagem"
- "Thinking..." → "Pensando…"
- "Skip discovery" → "Pular descoberta"
- "AI assistant" → "Assistente"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/features/editor/chat/*.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/features/editor/chat/
git commit -m "i18n(editor): translate editor chat to pt-BR"
```

### Task 11: Outline, proposal overlay, comments

**Files:**
- `apps/web/features/editor/outline/outline-card.tsx`
- `apps/web/features/editor/overlay/proposal-panel.tsx`
- `apps/web/features/editor/comments/comment-composer.tsx`
- `apps/web/features/editor/comments/comment-panel.tsx`
- `apps/web/features/editor/review/commentable-preview.tsx`

Known English strings:

```
outline/outline-card.tsx:33:
  {approved ? "Outline approved" : "Proposed outline"}
  → {approved ? "Esboço aprovado" : "Esboço proposto"}

overlay/proposal-panel.tsx:88:
  aria-label="Accept"
  → aria-label="Aceitar"

overlay/proposal-panel.tsx:96:
  aria-label="Reject"
  → aria-label="Rejeitar"
```

`outline/outline-card.tsx:8` has a JSDoc comment about `"Approve outline"` — that's developer-facing, leave it.

- [ ] **Step 1: Apply known + read each file**

Common terms:
- "Approve" / "Reject" → "Aprovar" / "Rejeitar"
- "Accept" → "Aceitar"
- "Resolve" → "Resolver"
- "Comment" → "Comentar" (verb) / "Comentário" (noun)
- "Reply" → "Responder"
- "Outline" → "Esboço"
- "Proposal" → "Proposta"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' \
  apps/web/features/editor/outline/*.tsx \
  apps/web/features/editor/overlay/*.tsx \
  apps/web/features/editor/comments/*.tsx \
  apps/web/features/editor/review/*.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/features/editor/outline/ apps/web/features/editor/overlay/ apps/web/features/editor/comments/ apps/web/features/editor/review/
git commit -m "i18n(editor): translate outline, proposal, and comments to pt-BR"
```

### Task 12: Save dialog, autonomy, templates, toolbar, canvas

**Files:**
- `apps/web/features/editor/save/save-dialog.tsx`
- `apps/web/features/editor/autonomy/autonomy-selector.tsx`
- `apps/web/features/editor/autonomy/checkpoint-card.tsx`
- `apps/web/features/editor/templates/template-picker.tsx`
- `apps/web/features/editor/toolbar/inline-toolbar.tsx`
- `apps/web/features/editor/canvas/canvas.tsx`

Save dialog and templates already heavily pt-BR. Autonomy selector has English label `"Checkpointed"` (from initial scan):

```
autonomy/autonomy-selector.tsx:34:
  label: "Checkpointed",
  → label: "Com checkpoint",
```

(Confirm by reading the file — the array of options likely also has "Manual" and "Autonomous"; translate to "Manual" / "Autônomo" if present.)

- [ ] **Step 1: Read each file, translate per glossary**

Common terms:
- "Manual" → "Manual"
- "Autonomous" → "Autônomo"
- "Checkpointed" → "Com checkpoint"
- "Run" → "Executar"
- "Pause" → "Pausar"
- "Resume" → "Retomar"
- "Bold" / "Italic" → "Negrito" / "Itálico"
- "Heading" → "Título"
- "Code" → "Código"
- "Empty document" → "Documento vazio"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' \
  apps/web/features/editor/save/*.tsx \
  apps/web/features/editor/autonomy/*.tsx \
  apps/web/features/editor/templates/*.tsx \
  apps/web/features/editor/toolbar/*.tsx \
  apps/web/features/editor/canvas/canvas.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/features/editor/save/ apps/web/features/editor/autonomy/ apps/web/features/editor/templates/ apps/web/features/editor/toolbar/ apps/web/features/editor/canvas/canvas.tsx
git commit -m "i18n(editor): translate save dialog, autonomy, templates, toolbar to pt-BR"
```

---

## Phase 5 — Admin

### Task 13: Admin page and LLM settings

**Files:**
- `apps/web/app/(dashboard)/dashboard/admin/page.tsx`
- `apps/web/components/admin/llm-settings-form.tsx`

Known English strings (LLM settings):

```
llm-settings-form.tsx:15:
  { value: "", label: "Select a provider…" },
  → { value: "", label: "Selecione um provedor…" },

llm-settings-form.tsx:133:
  setProbeMessage("Generation model is required.");
  → setProbeMessage("Modelo de geração obrigatório.");

llm-settings-form.tsx:210:
  label="Base URL (optional)"
  → label="Base URL (opcional)"

llm-settings-form.tsx:227, 257, 264:
  label="Generation model"
  → label="Modelo de geração"

llm-settings-form.tsx:240:
  label="Ollama base URL"
  → label="URL base do Ollama"
```

Admin page (`apps/(dashboard)/dashboard/admin/page.tsx`) likely has tab labels and section headings — read and translate:
- "Users" / "User management" → "Usuários" / "Gestão de usuários"
- "LLM" → "LLM"
- "MCP" → "MCP"
- "Workspace" → "Workspace"
- "Analytics" → "Análises"
- "Export" → "Exportação"

- [ ] **Step 1: Apply known translations and read both files**

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' \
  apps/web/app/\(dashboard\)/dashboard/admin/page.tsx \
  apps/web/components/admin/llm-settings-form.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/app/\(dashboard\)/dashboard/admin/page.tsx apps/web/components/admin/llm-settings-form.tsx
git commit -m "i18n(admin): translate admin page and LLM settings to pt-BR"
```

### Task 14: MCP settings + analytics

**Files:**
- `apps/web/components/admin/mcp-settings.tsx`
- `apps/web/components/admin/mcp-analytics.tsx`

Known English strings:

```
mcp-settings.tsx:74:
  {workspace.has_mcp_token ? "Token is set" : "No token configured"}
  → {workspace.has_mcp_token ? "Token configurado" : "Nenhum token configurado"}

mcp-settings.tsx:84:
  {loading ? "Regenerating..." : "Regenerate Token"}
  → {loading ? "Regenerando…" : "Regenerar token"}

mcp-analytics.tsx:22:
  setError(e instanceof Error ? e.message : "Failed to load analytics");
  → setError(e instanceof Error ? e.message : "Não foi possível carregar as análises");

mcp-analytics.tsx:79:
  <h3 className="text-sm font-medium mb-2">Queries by day</h3>
  → <h3 className="text-sm font-medium mb-2">Consultas por dia</h3>

mcp-analytics.tsx:123:
  <h3 className="text-sm font-medium mb-2">Top query topics</h3>
  → <h3 className="text-sm font-medium mb-2">Tópicos de consulta mais frequentes</h3>

mcp-analytics.tsx:160:
  <th className="text-right py-1 font-medium">Last seen</th>
  → <th className="text-right py-1 font-medium">Última vez</th>
```

- [ ] **Step 1: Apply translations and read both files for additional strings**

Other common terms:
- "Revoke" → "Revogar"
- "Copy token" → "Copiar token"
- "Token (visible once)" → "Token (visível apenas uma vez)"
- "Are you sure?" → "Tem certeza?"
- "Total queries" → "Total de consultas"
- "Topic" / "Count" → "Tópico" / "Contagem"

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/components/admin/mcp-settings.tsx apps/web/components/admin/mcp-analytics.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/admin/mcp-settings.tsx apps/web/components/admin/mcp-analytics.tsx
git commit -m "i18n(admin): translate MCP settings and analytics to pt-BR"
```

### Task 15: User list and invite

**Files:**
- `apps/web/components/admin/user-list.tsx`
- `apps/web/components/admin/invite-user-form.tsx`

Known English strings:

```
user-list.tsx:18:
  if (!confirm("Deactivate this user?")) return;
  → if (!confirm("Desativar este usuário?")) return;

invite-user-form.tsx:40:
  setError(err instanceof Error ? err.message : "Invite failed");
  → setError(err instanceof Error ? err.message : "Não foi possível enviar o convite");
```

Other common strings:
- "Email" → "E-mail"
- "Role" → "Perfil"
- "Status" → "Situação"
- "Active" / "Inactive" → "Ativo" / "Inativo"
- "Last login" → "Último login"
- "Invite user" → "Convidar usuário"
- "Send invite" → "Enviar convite"
- "Deactivate" / "Reactivate" → "Desativar" / "Reativar"

- [ ] **Step 1: Apply known + read each file**

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/components/admin/user-list.tsx apps/web/components/admin/invite-user-form.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/admin/user-list.tsx apps/web/components/admin/invite-user-form.tsx
git commit -m "i18n(admin): translate user list and invite form to pt-BR"
```

### Task 16: Workspace config and export

**Files:**
- `apps/web/components/admin/workspace-config.tsx`
- `apps/web/components/admin/export-section.tsx`

Known English strings:

```
workspace-config.tsx:15:
  <span className="font-medium text-text-secondary">Workspace name</span>
  → <span className="font-medium text-text-secondary">Nome do workspace</span>

workspace-config.tsx:23:
  <span className="font-medium text-text-secondary">Generation model</span>
  → <span className="font-medium text-text-secondary">Modelo de geração</span>

workspace-config.tsx:27:
  <span className="font-medium text-text-secondary">Primary language</span>
  → <span className="font-medium text-text-secondary">Idioma principal</span>

export-section.tsx:37:
  "Download workspace as markdown"
  → "Baixar workspace como markdown"
```

Other common strings:
- "LLM provider" → "Provedor de LLM"
- "Languages" → "Idiomas"
- "Export" → "Exportar"
- "Downloading..." → "Baixando…"
- "Export complete" → "Exportação concluída"

- [ ] **Step 1: Apply known + read each file**

- [ ] **Step 2: Audit**

```bash
grep -nE '"[A-Z][a-z]+ [a-z]+|>[A-Z][a-z]+( [a-z]+)+<' apps/web/components/admin/workspace-config.tsx apps/web/components/admin/export-section.tsx
```

- [ ] **Step 3: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/admin/workspace-config.tsx apps/web/components/admin/export-section.tsx
git commit -m "i18n(admin): translate workspace config and export to pt-BR"
```

---

## Phase 6 — UI primitives

### Task 17: UI primitive defaults

**Files:** all `*.tsx` in `apps/web/components/ui/` (8 files: badge, button, card, copy-button, dialog, dropdown, input, select, spinner).

Most are headless and have no copy. Sweep for any default strings.

- [ ] **Step 1: Read each file**

```bash
ls apps/web/components/ui/
for f in apps/web/components/ui/*.tsx; do echo "=== $f ==="; cat "$f"; done
```

- [ ] **Step 2: Translate any default strings found**

Likely candidates:
- `copy-button.tsx`: `"Copied!"` / `"Copy"` → `"Copiado!"` / `"Copiar"`
- `spinner.tsx`: `aria-label="Loading"` → `aria-label="Carregando"`
- `dialog.tsx`: close button `aria-label="Close"` → `aria-label="Fechar"`

If a primitive accepts text via props with no English fallback string, no change needed.

- [ ] **Step 3: Audit**

```bash
grep -nE '"[A-Z][a-z]+( [a-z]+)*"' apps/web/components/ui/*.tsx | grep -v 'className\|//\|import \|from \"\|"http\|var(--\|interface \|type '
```

- [ ] **Step 4: Lint, type-check, commit**

```bash
pnpm --filter web lint
pnpm --filter web type-check
git add apps/web/components/ui/
git commit -m "i18n(ui): translate primitive default strings to pt-BR"
```

If no changes were needed, skip the commit and proceed to Phase 7.

---

## Phase 7 — API error messages (Rust)

### Task 18: Translate canonical `code_and_message` mapping

**Files:** `apps/api/src/presentation/error.rs`

The `code_and_message` function is the canonical map from error variant to (status, code, message). Translate the `message` strings; **leave `code` strings English** (they are stable identifiers per Invariant 1).

- [ ] **Step 1: Apply translation**

In `apps/api/src/presentation/error.rs:36-66`:

```rust
fn code_and_message(&self) -> (StatusCode, &'static str, String) {
    match self {
        ApiError::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "autenticação necessária".into(),
        ),
        ApiError::Forbidden => (
            StatusCode::FORBIDDEN,
            "forbidden",
            "permissão insuficiente".into(),
        ),
        ApiError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "recurso não encontrado".into(),
        ),
        ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, "validation_error", msg.clone()),
        ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
        ApiError::SetupRequired => (
            StatusCode::LOCKED,
            "setup_required",
            "configuração da instalação não foi concluída".into(),
        ),
        ApiError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_server_error",
            "erro interno do servidor".into(),
        ),
    }
}
```

The `#[error("...")]` attributes on `ApiError` enum variants stay English — they're used by `Display` for logs (Invariant 5).

- [ ] **Step 2: Run tests**

```bash
cargo test -p historiador_api
```

If tests fail asserting on the old English `message` strings, fix them — switch the assertion to the `error` (code) field where possible, or update the expected `message`. Example:

```rust
// Before
assert!(body.contains("authentication required"));
// Prefer this
assert_eq!(json["error"], "unauthorized");
// Or this if message must be checked
assert!(body.contains("autenticação necessária"));
```

- [ ] **Step 3: Run clippy and fmt**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 4: Verify OpenAPI spec did not drift**

```bash
pnpm gen:types
git diff openapi.yaml packages/types/generated/index.ts
```

Expected: empty diff. The translation didn't touch utoipa annotations, so no codegen drift.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/presentation/error.rs
git commit -m "i18n(api): translate canonical error messages to pt-BR"
```

### Task 19: Translate domain value object validation messages

**Files:**
- `apps/api/src/domain/value/email.rs`
- `apps/api/src/domain/value/language.rs`
- `apps/api/src/domain/value/slug.rs`

These are reused across handlers, so translating them propagates broadly.

- [ ] **Step 1: Apply translations**

In `apps/api/src/domain/value/email.rs`:

```rust
// Line 18
.ok_or_else(|| DomainError::Validation("email must contain '@'".into()))?;
// →
.ok_or_else(|| DomainError::Validation("e-mail precisa conter '@'".into()))?;

// Line 20-22
return Err(DomainError::Validation(
    "email has empty local or domain part".into(),
));
// →
return Err(DomainError::Validation(
    "e-mail tem parte local ou domínio vazio".into(),
));
```

In `apps/api/src/domain/value/language.rs:15`:

```rust
return Err(DomainError::Validation("language tag is empty".into()));
// →
return Err(DomainError::Validation("a tag de idioma está vazia".into()));
```

In `apps/api/src/domain/value/slug.rs`:

```rust
// Line 13
return Err(DomainError::Validation("slug is empty".into()));
// →
return Err(DomainError::Validation("o slug está vazio".into()));

// Lines 19-21
return Err(DomainError::Validation(
    "slug must contain only lowercase letters, digits, and hyphens".into(),
));
// →
return Err(DomainError::Validation(
    "o slug deve conter apenas letras minúsculas, dígitos e hifens".into(),
));
```

- [ ] **Step 2: Run tests for each value module**

```bash
cargo test -p historiador_api domain::value
```

If tests assert on the English message, update them to assert on the pt-BR message (or to assert on `DomainError::Validation` variant without inspecting the inner string where possible).

- [ ] **Step 3: Lint and fmt**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/domain/value/email.rs apps/api/src/domain/value/language.rs apps/api/src/domain/value/slug.rs
git commit -m "i18n(domain): translate value object validation messages to pt-BR"
```

### Task 20: Translate setup-flow validation messages

**Files:**
- `apps/api/src/application/setup/bcp47.rs`
- `apps/api/src/application/setup/initialize_installation.rs`
- `apps/api/src/application/setup/list_ollama_models.rs`

- [ ] **Step 1: Apply translations**

In `apps/api/src/application/setup/bcp47.rs`:

```rust
// Line 12
DomainError::Validation(format!("invalid BCP 47 language tag: {tag}")).into(),
// →
DomainError::Validation(format!("tag de idioma BCP 47 inválida: {tag}")).into(),

// Line 18
DomainError::Validation(format!("invalid BCP 47 primary_language: {primary}")).into(),
// →
DomainError::Validation(format!("primary_language BCP 47 inválido: {primary}")).into(),

// Line 23
DomainError::Validation("primary_language must be one of languages".into()).into(),
// →
DomainError::Validation("primary_language precisa estar em languages".into()).into(),
```

In `apps/api/src/application/setup/initialize_installation.rs`:

```rust
// Lines 77-79
return Err(ApplicationError::Domain(DomainError::Validation(
    "openai requires either an API key or a custom base URL".into(),
)));
// →
return Err(ApplicationError::Domain(DomainError::Validation(
    "openai exige uma chave de API ou uma URL base personalizada".into(),
)));

// Line 92
ApplicationError::Domain(DomainError::Validation(format!("LLM key rejected: {e}")))
// →
ApplicationError::Domain(DomainError::Validation(format!("chave de LLM rejeitada: {e}")))
```

In `apps/api/src/application/setup/list_ollama_models.rs`:

```rust
// Lines 19-21
ApplicationError::Domain(DomainError::Validation(format!(
    "Ollama unreachable: {e}"
)))
// →
ApplicationError::Domain(DomainError::Validation(format!(
    "Ollama inacessível: {e}"
)))
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p historiador_api application::setup
```

Update test assertions as needed (prefer asserting on `error` code over `message`).

- [ ] **Step 3: Lint and fmt**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/application/setup/
git commit -m "i18n(setup): translate setup validation messages to pt-BR"
```

### Task 21: Translate page/collection/admin application errors

**Files:**
- `apps/api/src/application/admin/invite_user.rs`
- `apps/api/src/application/admin/update_llm_config.rs`
- `apps/api/src/application/collections/create_collection.rs`
- `apps/api/src/application/collections/update_collection.rs`
- `apps/api/src/application/pages/create_page.rs`
- `apps/api/src/application/pages/update_page.rs`
- `apps/api/src/application/pages/version_history.rs`
- `apps/api/src/infrastructure/chronik/analytics.rs`

- [ ] **Step 1: Apply known translations**

Concrete translations:

```rust
// admin/invite_user.rs:45-48
return Err(DomainError::Conflict(
    "a user with this email already exists in the workspace".into(),
)
.into());
// →
return Err(DomainError::Conflict(
    "já existe um usuário com este e-mail no workspace".into(),
)
.into());

// admin/update_llm_config.rs:89-91
return Err(ApplicationError::Domain(DomainError::Validation(
    "openai requires either an API key or a custom base URL".into(),
)));
// →
return Err(ApplicationError::Domain(DomainError::Validation(
    "openai exige uma chave de API ou uma URL base personalizada".into(),
)));

// admin/update_llm_config.rs:119
ApplicationError::Domain(DomainError::Validation(format!("LLM rejected: {e}")))
// →
ApplicationError::Domain(DomainError::Validation(format!("LLM rejeitou: {e}")))

// collections/create_collection.rs:31
DomainError::Validation("parent collection not found".into())
// →
DomainError::Validation("coleção pai não encontrada".into())

// collections/create_collection.rs:52
DomainError::Conflict("collection slug already exists".into())
// →
DomainError::Conflict("o slug da coleção já existe".into())

// collections/update_collection.rs:55
DomainError::Conflict("collection slug conflict".into())
// →
DomainError::Conflict("conflito de slug de coleção".into())

// pages/create_page.rs:81-83
return ApplicationError::Domain(DomainError::Conflict(
    "page with this slug already exists in the collection".into(),
));
// →
return ApplicationError::Domain(DomainError::Conflict(
    "já existe uma página com este slug na coleção".into(),
));

// pages/update_page.rs:59-62
return Err(DomainError::Validation(
    "page is published — revert to draft before editing".into(),
)
.into());
// →
return Err(DomainError::Validation(
    "a página está publicada — reverta para rascunho antes de editar".into(),
)
.into());

// pages/version_history.rs:142-145
return Err(DomainError::Validation(
    "page is published — revert to draft before restoring".into(),
)
.into());
// →
return Err(DomainError::Validation(
    "a página está publicada — reverta para rascunho antes de restaurar".into(),
)
.into());

// infrastructure/chronik/analytics.rs:29
DomainError::Validation("analytics unavailable — Chronik not configured".into())
// →
DomainError::Validation("análises indisponíveis — Chronik não configurado".into())
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p historiador_api application
```

Update test assertions.

- [ ] **Step 3: Lint and fmt**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/application/admin/ apps/api/src/application/collections/ apps/api/src/application/pages/ apps/api/src/infrastructure/chronik/analytics.rs
git commit -m "i18n(api): translate application-layer error messages to pt-BR"
```

---

## Phase 8 — Locale (`<html lang>` and `Intl` formatting)

### Task 22: Add `pt-BR` formatting helpers and set `<html lang>`

**Files:**
- Create: `apps/web/lib/format.ts`
- Modify: `apps/web/app/layout.tsx`

- [ ] **Step 1: Create the formatter module**

Create `apps/web/lib/format.ts`:

```ts
const DATE_FMT = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short" });
const DATETIME_FMT = new Intl.DateTimeFormat("pt-BR", {
  dateStyle: "short",
  timeStyle: "short",
});
const TIME_FMT = new Intl.DateTimeFormat("pt-BR", { timeStyle: "short" });
const NUMBER_FMT = new Intl.NumberFormat("pt-BR");

export const formatDate = (d: Date | string | number): string =>
  DATE_FMT.format(new Date(d));

export const formatDateTime = (d: Date | string | number): string =>
  DATETIME_FMT.format(new Date(d));

export const formatTime = (d: Date | string | number): string =>
  TIME_FMT.format(new Date(d));

export const formatNumber = (n: number): string => NUMBER_FMT.format(n);
```

- [ ] **Step 2: Set HTML lang**

In `apps/web/app/layout.tsx`, change the root html tag's `lang` attribute from `en` (or whatever it is) to `pt-BR`:

```tsx
<html lang="pt-BR">
```

Read the file first to find the existing line, then `Edit` to swap it.

- [ ] **Step 3: Lint and type-check**

```bash
pnpm --filter web lint
pnpm --filter web type-check
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/lib/format.ts apps/web/app/layout.tsx
git commit -m "i18n(locale): add pt-BR Intl helpers and set html lang"
```

### Task 23: Replace `toLocale*` call sites with `format.ts` helpers

**Files (sweep):** anywhere in `apps/web` that calls `.toLocaleTimeString()`, `.toLocaleDateString()`, or `.toLocaleString()` on a Date.

- [ ] **Step 1: Find all call sites**

```bash
grep -rnE '\.toLocale(Time|Date|)String\(' apps/web --include="*.tsx" --include="*.ts" | grep -v node_modules | grep -v ".next"
```

Known call site:
- `apps/web/features/editor/editor-v2.tsx:190` — `Saved ${savedAt.toLocaleTimeString()}` → `Salvo ${formatTime(savedAt)}`

- [ ] **Step 2: Replace each call site**

For each result, switch to the appropriate helper:
- `.toLocaleTimeString()` → `formatTime(date)`
- `.toLocaleDateString()` → `formatDate(date)`
- `.toLocaleString()` (date+time) → `formatDateTime(date)`

Add the import at the top of each file: `import { formatDate, formatDateTime, formatTime } from "@/lib/format";` (only the helpers used).

- [ ] **Step 3: Lint and type-check**

```bash
pnpm --filter web lint
pnpm --filter web type-check
```

- [ ] **Step 4: Smoke test**

`pnpm dev` → open the editor, save a draft → confirm timestamp renders in pt-BR format ("01/05/2026" or "13:42"). Open version history → confirm timestamps are pt-BR.

- [ ] **Step 5: Commit**

```bash
git add apps/web
git commit -m "i18n(locale): use pt-BR Intl helpers for date/time formatting"
```

---

## Phase 9 — Test sweep and final verification

### Task 24: Rust test sweep

- [ ] **Step 1: Run the full Rust test suite**

```bash
cargo test --workspace
```

Expected: PASS. If anything fails, the failure should be a test asserting on a translated English string.

- [ ] **Step 2: For each failing test, prefer asserting on the error code over the message**

Example failure:

```rust
assert!(response_body.contains("authentication required"));
```

Replace with:

```rust
let json: serde_json::Value = serde_json::from_str(&response_body).unwrap();
assert_eq!(json["error"], "unauthorized");
```

If the test cannot use the code (e.g., it's asserting on `DomainError::Validation` discriminant only), update the message string to its pt-BR equivalent.

- [ ] **Step 3: Re-run tests until green**

```bash
cargo test --workspace
```

- [ ] **Step 4: Lint and fmt**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "i18n(tests): update assertions for pt-BR error messages"
```

### Task 25: Web lint and end-to-end smoke

- [ ] **Step 1: Run web lint and type-check**

```bash
pnpm --filter web lint
pnpm --filter web type-check
```

Expected: PASS.

- [ ] **Step 2: Confirm OpenAPI spec did not drift**

```bash
pnpm gen:types
git status openapi.yaml packages/types/generated/index.ts
```

Expected: clean. If diff appears, an utoipa annotation was translated by mistake — revert.

- [ ] **Step 3: End-to-end smoke**

Reset the database (`docker compose down -v && docker compose up -d`), start the API (`cargo run -p historiador_api --bin api`), start web (`cd apps/web && pnpm dev`).

Walk these flows:
1. Setup wizard end-to-end → confirm zero English.
2. Login with admin → confirm error fallback in pt-BR if password is wrong.
3. Create a collection → confirm dialog in pt-BR.
4. Create a page → confirm editor chrome in pt-BR.
5. Save a draft → confirm "Salvo HH:MM" in pt-BR formatter.
6. Admin → invite a user → confirm form in pt-BR.
7. Admin → MCP analytics → confirm chart titles in pt-BR.
8. Admin → workspace config → confirm field labels in pt-BR.

Note any orphan English strings; fix them in the appropriate file and amend the relevant phase commit (or stage as a follow-up).

- [ ] **Step 4: Final commit**

If any orphan strings were fixed during smoke:

```bash
git add -A
git commit -m "i18n: fix orphan English strings caught during smoke test"
```

If smoke passed clean, no commit needed.

### Task 26: Open PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feature/change-language
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create --base main --title "i18n: translate platform UI and API error messages to pt-BR" --body "$(cat <<'EOF'
## Summary

- Replaces all user-visible English text in `apps/web` with Brazilian Portuguese (pt-BR), single-language.
- Translates user-facing API error messages in `apps/api`. Stable English error codes are preserved (`{"error": "validation_error", ...}`).
- Adds `apps/web/lib/format.ts` with cached `Intl` formatters for pt-BR date, datetime, time, and number rendering. Sets `<html lang="pt-BR">`.
- Out of scope: utoipa/MCP/OpenAPI surfaces, prompt templates in `prompts/`, developer docs, log strings, internal `anyhow!`/`bail!` strings.

Spec: [artifacts/specs/2026-05-05-pt-br-translation-design.md](artifacts/specs/2026-05-05-pt-br-translation-design.md)

## Test plan

- [ ] cargo test --workspace passes
- [ ] cargo clippy --workspace --all-targets --all-features -- -D warnings passes
- [ ] cargo fmt --all --check passes
- [ ] pnpm --filter web lint passes
- [ ] pnpm --filter web type-check passes
- [ ] pnpm gen:types produces empty diff (proves utoipa unchanged)
- [ ] Manual smoke: setup → login → dashboard → editor → admin shows zero orphan English

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Acceptance Criteria

1. All files in Phases 1–7 contain only pt-BR user-visible strings, consistent with the glossary.
2. `<html lang="pt-BR">`. Date/time rendering on touched surfaces uses `Intl` `pt-BR` helpers.
3. `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check` all pass.
4. `pnpm --filter web lint` and `pnpm --filter web type-check` pass.
5. Manual smoke through setup → login → dashboard → editor → admin shows zero orphan English strings.
6. After Phase 7: `pnpm gen:types` produces no diff (proves utoipa annotations were not touched).
