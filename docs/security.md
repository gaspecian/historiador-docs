# Security

This document summarises the security posture of Historiador Doc as it
entered the v1.0 release hardening in Sprint 10, and the steps operators
should take before exposing an instance to the internet. It is a living
document — open an issue or a PR if any claim here drifts from the code.

Last reviewed: 2026-04-18 (Sprint 10 Week 1 DoD gate).

---

## Dependency advisories

`cargo audit` is green as of the v1.0 hardening cycle. The CI job runs
it on every PR via `.github/workflows/ci.yml`.

During Sprint 10 the following advisories were addressed:

- **RUSTSEC-2026-0098** and **RUSTSEC-2026-0099** on `rustls-webpki`
  0.103.10 — bumped to 0.103.12 via `cargo update -p rustls-webpki`.
  Both advisories relate to name-constraint handling in X.509
  certificates; neither affected Historiador Doc directly, but the
  transitive dependency was patched proactively.

Run `cargo audit` locally if you maintain a fork or a custom deploy to
re-verify at your cut:

```bash
cargo install cargo-audit
cargo audit
```

---

## Authentication and authorisation

### JWT (user-facing API)

- **Algorithm:** HS256.
- **Boot-time validation:** `apps/api` crashes at startup if
  `JWT_SECRET` is missing or shorter than 32 characters.
  Same rule for `APP_ENCRYPTION_KEY` (must decode to 32 bytes).
- **Password hashing:** Argon2id with the `argon2` crate defaults.
- **Session revocation:** refresh tokens are stored in the `sessions`
  table; `/auth/logout` deletes the row. Invalidated refresh tokens
  cannot mint new access tokens.

### Bearer token (MCP endpoint, port 3002)

- The MCP server is the single externally-exposed service. Its bearer
  token is compared against a pre-computed SHA-256 digest in
  `apps/mcp/src/auth.rs::bearer_auth`, using `subtle::ConstantTimeEq`.
  The result is that:
  - Length is never leaked (both sides are fixed-length 32-byte digests).
  - Prefix matches do not short-circuit the comparison.
- Regression test `rejects_prefix_of_real_token` in the same file
  guards against any future refactor that re-introduces a raw `==`.
- Rotate the token via the dashboard: **Admin → MCP Server → Regenerate
  Token**.
- Tokens must arrive in the `Authorization: Bearer <token>` header. See
  README for the Claude Desktop configuration.

### Handler auth audit

Every non-public handler in `apps/api/src/presentation/handler/` takes
the `AuthUser` extractor. The public set is limited to:

- `GET /health`
- `POST /setup/init`, `POST /setup/probe`, `POST /setup/ollama-models`
  (gated by the 423 Locked setup middleware once the installation
  wizard has completed)
- `POST /auth/login`, `POST /auth/refresh`, `POST /auth/logout`,
  `POST /auth/activate`
- `POST /internal/mcp-log` — **internal-only**, intentionally not
  JWT-gated. See "Internal endpoints" below.

Each `AuthUser`-taking handler delegates authorisation to its use case
via `Actor::require_role(...)`. Role enforcement is use-case-layer
today; route-level middleware as defence-in-depth is tracked for v1.1
(review finding 5.2).

### Internal endpoints

The `/internal/*` router is mounted outside the setup gate and takes no
JWT. It is intended only for service-to-service calls on the internal
Docker network (currently: `/internal/mcp-log` is how the MCP binary
proxies query telemetry back into the API so Chronik ingestion stays
inside the writable-role boundary).

Operators must not expose `/internal/*` to the public network. The
provided `docker-compose.yml` binds every non-MCP port to `127.0.0.1`;
the forthcoming `docker-compose.prod.yml` reasserts this and only
exposes the MCP port (3002).

---

## MCP protocol (JSON-RPC 2.0)

- The `/mcp` endpoint implements protocol version `2025-03-26` with the
  `initialize`, `tools/list`, and `tools/call` methods.
- Tool execution errors are returned inside the result object with
  `isError: true` per the MCP spec, not as JSON-RPC error codes. This
  keeps tool-level diagnostic text visible to clients without requiring
  them to parse error payloads.
- The custom-REST `/query` alias is retained for the internal web UI
  and is not part of the MCP public contract. It takes the same bearer
  middleware as `/mcp`.
- Notifications (requests with no `id`) always receive `204 No Content`
  per JSON-RPC 2.0. Malformed notifications are logged but not replied to.

---

## Data durability (ADR-007, Sprint 10 item #3)

- The vector store of record is **Chronik-Stream**. Postgres stores
  content and metadata (ADR-001 invariant).
- The in-memory vector store fallback is gated behind the
  `ALLOW_IN_MEMORY_VECTOR_STORE` environment variable, which **defaults
  to `false`**. With the default, both `apps/api` and `apps/mcp` fail
  at boot if Chronik is not reachable — they will not silently fall
  back to an in-memory store that drops all chunks on every restart.
- For local development, the provided `.env.example` sets the flag to
  `true`. Both binaries emit a loud `WARN` on every boot when the flag
  is active.
- Operators deploying Historiador Doc to production must leave the flag
  unset or set it to `false`.

---

## Postgres role separation (ADR-003)

Two roles are provisioned by `docker/postgres/init/10-roles.sh`:

- **`historiador_api`** — used by `apps/api` via
  `DATABASE_URL_READWRITE`. Owns every table via migrations and has
  full CRUD.
- **`historiador_mcp`** — used by `apps/mcp` via
  `DATABASE_URL_READONLY`. Has `SELECT` only on the whitelisted subset
  required for query execution: `workspaces`, `collections`, `pages`,
  `page_versions`, `chunks`.

This zero-write invariant for MCP is enforced at the database layer,
not just in application code. Even a bug in `apps/mcp` that tried to
issue an `INSERT` would be rejected by the connection's grant set.

---

## Transport security

The binaries speak plain HTTP. Production deployments terminate TLS at
a reverse proxy. A sample `docs/deploy/nginx.conf` ships as part of the
Sprint 10 Week 2 release artifacts and shows:

- TLS cert selection for the MCP port (3002)
- `proxy_set_header Authorization` preservation so the bearer token
  reaches the application
- Aggressive timeouts for the MCP endpoint since JSON-RPC `tools/call`
  invocations are expected to respond in under 2 s (see
  `docs/performance.md` once published)

If you deploy without the sample proxy, ensure your own setup:

- Binds the MCP port on `0.0.0.0` only at the proxy layer (never on
  the app binary).
- Preserves the `Authorization` header.
- Does not buffer the MCP response for longer than your SSE timeout.

---

## Secrets hygiene

- `.env.example` contains only obvious dev placeholders
  (`JWT_SECRET=dev-jwt-secret-do-not-use-in-production-xxxxxx`,
  `APP_ENCRYPTION_KEY=AAAA…=` base64-zeros). It is safe to commit.
- `.env` is gitignored; `git grep -E 'sk-[A-Za-z0-9]{20,}'` across the
  tree returns nothing as of this review.
- LLM provider keys are stored **encrypted at rest** in the
  `workspaces` table using `APP_ENCRYPTION_KEY` + AES-GCM (see
  `apps/api/src/infrastructure/crypto/`). Rotate the key by re-running
  the setup wizard; existing ciphertext remains readable until the key
  is rotated explicitly.

---

## Reporting vulnerabilities

Historiador Doc does not yet have a formal disclosure channel. Until
one is set up, email the maintainer
(<gabriel.specian@nexiantech.com.br>) with "SECURITY" in the subject.
Please do not file a public GitHub issue for a vulnerability that has
not yet been patched.

---

## Deferred to v1.1

- **Route-level RBAC middleware** (review finding 5.2): defence-in-depth
  so a future contributor cannot forget to call `Actor::require_role`.
  Tracked under the `v1.1.0` milestone.
- **Editor WebSocket rebuild with inline section editing** per ADR-008
  (review finding 4.3 complete fix). ADR-009 ratifies SSE for v1.0.
- **Chunker author + freshness metadata** (review finding 5.3).
