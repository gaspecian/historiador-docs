# Contributing to Historiador Doc

Thank you for considering a contribution. This guide gets you from a
clean clone to a merged PR in under two hours.

## Project shape

Historiador Doc is a mixed-language monorepo:

- **Rust backend** (Cargo workspace): `apps/api`, `apps/mcp`,
  `crates/db`, `crates/chunker`, `crates/llm`.
- **Next.js frontend** (pnpm workspace + Turborepo): `apps/web`.
- **Shared TypeScript types**: `packages/types`, auto-generated from
  the Rust OpenAPI contract.

Before changing architecture, skim [`CLAUDE.md`](CLAUDE.md) and the
relevant ADRs under [`artifacts/adr/`](artifacts/adr/). The
"Critical Invariants" section of `CLAUDE.md` is the shortest summary
of what must not drift.

## Local setup

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust | stable (pinned via `rust-toolchain.toml`) | Installs rustfmt + clippy on first `cargo` invocation |
| Node.js | 20+ | Required by Next.js 16 |
| pnpm | corepack-enabled | Run `corepack enable` once if it isn't already |
| Docker | Compose v2 | Runs Postgres, Ollama, Chronik-Stream |

### First run

```bash
git clone https://github.com/gaspecian/historiador-docs.git
cd historiador-docs

cp .env.example .env              # never commit .env
corepack enable                   # one-time

docker compose up -d              # Postgres + Ollama + Chronik
pnpm install                      # installs web + types deps

cargo run -p historiador_api --bin api
# in another terminal:
cd apps/web && pnpm dev
# in a third terminal (optional — only if you're touching MCP):
cargo run -p historiador_mcp --bin mcp
```

Open <http://localhost:3000>, complete the setup wizard, and you're
live.

### Environment variables

- `.env.example` documents every required and optional variable. Keys
  shorter than their documented minimums (`JWT_SECRET` ≥ 32 chars,
  `APP_ENCRYPTION_KEY` = base64 of 32 bytes) will crash the API at
  boot — that's intentional.
- The MCP binary reads `DATABASE_URL_READONLY`; the API reads
  `DATABASE_URL_READWRITE`. Do not collapse the two into one variable.
- Local dev can leave `ALLOW_IN_MEMORY_VECTOR_STORE=true` so the app
  still boots when Chronik isn't up. Production defaults to `false`
  (see [`docs/security.md`](docs/security.md)).

## Running the test suite

```bash
# Rust: unit + library tests (no Postgres required)
cargo test --workspace --lib --bins

# Rust: e2e tests that spin up an in-process API + test DB
cargo test --workspace --test '*'

# Rust: lint (CI denies all warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Rust: format check (CI requires)
cargo fmt --all --check

# Frontend
pnpm -C apps/web lint
pnpm -C packages/types build      # tsc --noEmit
```

A single Rust test: `cargo test -p historiador_db test_name_fragment`.

## OpenAPI / TypeScript codegen

Anything that changes the HTTP contract must flow through this pipeline:

1. Add or change `#[utoipa::path]` / `ToSchema` annotations in Rust.
2. Regenerate:
   ```bash
   pnpm gen:types        # runs gen:openapi → build:types
   ```
3. Commit both `openapi.yaml` and `packages/types/generated/index.ts`.

CI runs the same pipeline and fails if it produces a diff, so if you
forget step 2 your PR will turn red.

## Database migrations

- Migrations live in [`crates/db/migrations/`](crates/db/migrations/)
  and are embedded into the API binary via `sqlx::migrate!`.
- They run automatically on API startup. There's no manual step for
  local dev.
- Naming: `NNNN_short_description.sql`. Keep them numerically ordered
  and never edit a merged migration — write a new one that compensates.
- New tables must get grants matching ADR-003: the `historiador_api`
  role owns them implicitly; add an explicit `GRANT SELECT TO
  historiador_mcp` only if MCP needs to read them.

## PR conventions

### Branching

- Branch names: `feature/<what>`, `fix/<what>`, `docs/<what>`,
  `chore/<what>`. No personal namespaces.
- Rebase on `main` (or the current feature/<sprint> branch) before
  opening the PR.

### Commit messages

Conventional Commits:

- `feat(<scope>): …`
- `fix(<scope>): …`
- `docs(<scope>): …`
- `chore(<scope>): …`
- `refactor(<scope>): …`
- `style(<scope>): …`

Write the *why* in the body, not the *what*. A reviewer can see the
what from the diff. Reference the ADR or issue number when the
change is driven by one.

Co-authors should appear as `Co-Authored-By: Name <email>` lines at
the bottom.

### Pull requests

- Title mirrors the top commit subject.
- Body: a short summary + a "Test plan" bullet list. The repo's
  default template takes care of this.
- One logical change per PR. Two unrelated fixes = two PRs.
- Passing CI is required for merge. Do not push merges while CI is red.

### Review flow

- Request review from a code owner listed for the touched crate /
  package.
- Reviews use [Conventional Comments](https://conventionalcomments.org/)
  prefixes (`praise:`, `question:`, `suggestion:`, `issue:`) so authors
  can tell what's blocking vs. what's nitpick.

## Issue labels

| Label | Use |
|-------|-----|
| `bug` | A defect in released behavior |
| `enhancement` | New feature or improvement |
| `documentation` | Docs-only change |
| `performance` | Retrieval or rendering speed |
| `multilingual` | Language handling, BCP 47, language-specific quirks |
| `good first issue` | Small, well-scoped starter tasks |
| `v1.1` | Tracked for the v1.1 milestone |
| `v2` | Tracked for the v2 milestone |

## ADRs

Architectural decisions are recorded in
[`artifacts/adr/`](artifacts/adr/). The rules:

- ADRs are **append-only**. To change a decision, write a new ADR that
  supersedes the old one (see how ADR-006 supersedes ADR-004 and
  ADR-007 supersedes ADR-001).
- Don't edit accepted ADRs retroactively. Add a new "Supersedes"
  reference to the new ADR.
- If your change touches a PRD-level invariant, update both the ADR
  and the "Resolved Decisions" section of the PRD in the same PR.

## When to ask

If a feature spans multiple crates or invalidates an ADR, open a
discussion issue first. The cost of aligning on direction before
writing code is far lower than the cost of unwinding a 500-line PR
that went the wrong way.
