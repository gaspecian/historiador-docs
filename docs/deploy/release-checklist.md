# v1.0.0 Release Checklist

This is the Week 2 Friday runbook for Sprint 10 item #12. Run top to
bottom on a clean checkout. Every step is idempotent except the tag
and the visibility flip.

## 0. Preconditions

- [ ] You are on `feature/sprint-10` (or the PR has merged into `main`)
  and your working tree is clean.
- [ ] You have `gh` installed and authenticated as the repo owner
  (`gh auth status`).
- [ ] You have `docker` running if you want to verify the production
  image builds locally (optional).

## 1. Final CI gate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --bins
pnpm -C apps/web lint
pnpm -C packages/types build
cargo audit
```

All six must be green. Stop if any fail.

## 2. Load test gate (MCP p95 < 2 s)

This step is **gating** per the Sprint 10 DoD. Skipping it is only OK
if `docs/performance.md` already has a dated entry with
`p95 ≤ 2000ms` within the last 7 days and no retrieval-path code has
changed since.

```bash
# Bring up the stack.
docker compose up -d

# Wait for Chronik to be healthy, then seed.
cargo run --release -p historiador_api --bin load-test-seed -- --chunks 10000

# Start api + mcp (production mode if you want to mirror prod):
cargo run --release -p historiador_api --bin api &
cargo run --release -p historiador_mcp --bin mcp &

# Complete the setup wizard at http://localhost:3000 the first time
# only, then grab the MCP bearer token from Admin → MCP Server.
export MCP_BEARER_TOKEN="<token>"

# Fire the harness. --emit-report appends to docs/performance.md.
./scripts/load-test/run.sh --emit-report
```

If the script exits non-zero: do not tag. Follow the tuning levers in
`docs/performance.md` (Chronik HNSW params → pool size → query
embedding cache → flamegraph). Document the remediation before tagging.

## 3. Production compose smoke test

Optional but strongly recommended before the first public deploy.

```bash
# Build the api/mcp/web images locally.
docker compose -f docker-compose.prod.yml build

# Stand them up (needs .env with real values for JWT_SECRET,
# APP_ENCRYPTION_KEY, MCP_BEARER_TOKEN, PUBLIC_BASE_URL).
docker compose -f docker-compose.prod.yml up -d

# Health checks.
curl -fsS http://localhost:3002/health
curl -fsS -H "Authorization: Bearer $MCP_BEARER_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
     http://localhost:3002/mcp | jq .

# Tear down.
docker compose -f docker-compose.prod.yml down
```

## 4. GitHub metadata

Before tagging, make sure the repo has the labels and milestones the
issues depend on.

```bash
# Labels
for label in \
  "bug:d73a4a" \
  "enhancement:a2eeef" \
  "documentation:0075ca" \
  "performance:fbca04" \
  "multilingual:bfdadc" \
  "good first issue:7057ff" \
  "v1.1:c5def5" \
  "v2:e99695"
do
  name="${label%%:*}"
  color="${label##*:}"
  gh label create "$name" --color "$color" --force
done

# Milestones
gh api repos/:owner/:repo/milestones -X POST -f title="v1.1.0" -f state=open
gh api repos/:owner/:repo/milestones -X POST -f title="v2.0.0"  -f state=open
```

## 5. File the v1.1 deferrals (from the code review)

```bash
# 1. Editor WebSocket rebuild per ADR-008
gh issue create --milestone v1.1.0 --label enhancement --label v1.1 \
  --title "Rebuild editor on WebSocket per ADR-008" \
  --body "$(cat <<'EOF'
ADR-009 ratifies SSE for v1.0 but the long-term target remains the
ADR-008 vision: WebSocket transport with explicit conversation /
generation modes and inline section click-to-edit.

Scope:
- WebSocket handler in apps/api with the EditorMessage envelope
  documented in ADR-008
- Frontend: split the current SSE-based useEditorStream into a
  WebSocket client that routes conversation vs generation_chunk
  messages to separate panes
- Section click-to-edit: section_focus message type wired from the
  preview pane into the AI system prompt
- Dual-write conversations into the Chronik editor-conversations:stream
  topic alongside the Postgres editor_conversations table

See code review finding 4.3.
EOF
)"

# 2. Route-level RBAC middleware
gh issue create --milestone v1.1.0 --label enhancement --label v1.1 \
  --title "Add route-level RBAC middleware as defence-in-depth" \
  --body "Audit today's per-use-case Actor::require_role calls;
add an axum::middleware::from_fn group at the route level that
double-checks role for /admin/* (Admin), /pages/* (Author+), /search/*
and /editor/* (Author+), /export/* (Author+). Per code review
finding 5.2."

# 3. Chunker metadata
gh issue create --milestone v1.1.0 --label enhancement --label v1.1 \
  --title "Propagate author_id and last_updated into chunk payloads" \
  --body "Both fields already live on page_versions; the chunker only
needs to receive them in its input and surface them on the ChunkRef
result so analytics + freshness hints can use them. Per code review
finding 5.3."

# 4. Email delivery
gh issue create --milestone v1.1.0 --label enhancement --label v1.1 \
  --title "Native email delivery for invitations" \
  --body "Invitations currently return an activation URL the admin
must share manually. Add SMTP or provider-based (SES, SendGrid,
Postmark) delivery behind a configuration flag."

# 5. Native Ollama embeddings
gh issue create --milestone v1.1.0 --label enhancement --label v1.1 \
  --title "Native Ollama embedding client (replace stub)" \
  --body "historiador_llm::OllamaEmbeddingClient today falls back
to the stub embedder. Wire it against Ollama's /api/embeddings
endpoint and its supported embedding models."
```

## 6. File ≥ 5 `good first issue` tickets

```bash
gh issue create --label "good first issue" --label documentation \
  --title "Add pt-PT to the supported languages list" \
  --body "Historiador Doc ships with en, pt-BR, es, fr baked into
the synthetic load test chunks. Add pt-PT to the suggested BCP 47
tags in the setup wizard (apps/web/app/setup/...) and any docs that
list examples."

gh issue create --label "good first issue" --label enhancement \
  --title "Improve the 'JWT_SECRET too short' error message" \
  --body "apps/api/src/main.rs crashes at boot if JWT_SECRET is
missing or too short. The current error string points at the env var
but does not say HOW to generate one. Improve the message to include
an example command (\`openssl rand -base64 32\`) and a link to
docs/security.md."

gh issue create --label "good first issue" --label enhancement \
  --title "Dark mode for the admin dashboard" \
  --body "apps/web uses Tailwind 4 with Next.js 16. Add a dark mode
toggle that persists via localStorage + a \`data-theme\` attribute
on <html>. Start from the admin layout (apps/web/app/admin/...) and
work out."

gh issue create --label "good first issue" --label documentation \
  --title "Replace scripts/archive/setup-vexfs.sh with a migration guide" \
  --body "The script is archived but operators running an older fork
on VexFS need a step-by-step path to Chronik. Write
docs/migrations/vexfs-to-chronik.md that covers: (a) export from
VexFS, (b) spin up Chronik, (c) re-index via /admin/workspace/reindex."

gh issue create --label "good first issue" --label multilingual \
  --title "Add French translations to the setup wizard strings" \
  --body "The setup wizard (apps/web/app/setup/...) currently hard-
codes Portuguese / English. Externalize the strings into a simple
dictionary keyed by locale and ship an fr-FR translation."
```

## 7. Tag + release

**This is the point of no return.** Triple-check the CHANGELOG entry
for 1.0.0 before continuing.

```bash
# Tag. The message is the CHANGELOG entry verbatim.
git tag -a v1.0.0 -m "v1.0.0 — public release

See CHANGELOG.md for the full entry."

# Push the tag.
git push origin v1.0.0

# Publish the GitHub release. --generate-notes pulls from CHANGELOG.
gh release create v1.0.0 \
  --title "v1.0.0 — Historiador Doc" \
  --notes-file <(sed -n '/## \[1.0.0\]/,/## \[/p' CHANGELOG.md | sed '$d')
```

## 8. Flip repo to public

**Also a point of no return.** Check one more time that there are no
accidental secrets in the repo (`git log --all -p | grep -E
'sk-[A-Za-z0-9]{20,}|ghp_|gho_'`).

```bash
gh repo edit --visibility public --accept-visibility-change-consequences
```

## 9. Close the Sprint 10 branch

```bash
# Merge the sprint branch if you haven't already.
gh pr create --base main --head feature/sprint-10 \
  --title "Sprint 10 — v1.0 Hardening" \
  --body-file artifacts/sprints/sprint-10.md

# After the PR merges:
git checkout main
git pull
git branch -d feature/sprint-10
git push origin --delete feature/sprint-10
```

## 10. Announce

Suggested copy for a LinkedIn / Twitter / Discord post:

> Historiador Doc v1.0.0 is out. Self-hosted documentation with a
> built-in MCP server — write markdown, your AI tools query
> structure-aware chunks over JSON-RPC 2.0. Multilingual by default,
> Rust + Next.js, AGPL-3.0. github.com/gaspecian/historiador-docs
