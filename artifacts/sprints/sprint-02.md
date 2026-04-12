# Sprint 2 — Authentication & Setup

**Dates:** Week 2 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Any user can log in, and a new installation configures itself through a setup wizard that captures the admin account, LLM API key, and workspace language configuration.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | Carryover from Sprint 1 stretch items if not completed |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0 (Must Ship)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **First-run setup wizard API** | 3 pts | `POST /setup/init` — accepts admin email/password, LLM provider + API key (validated by a test call), workspace name, language config (`languages[]`, `primary_language`). Sets a `setup_complete` flag. All requests blocked until setup completes. Wrapped in a single database transaction — either fully commits or rolls back. |
| 2 | **JWT authentication** | 2 pts | `POST /auth/login` returns signed JWT. Axum middleware validates token on protected routes. Token expiry + refresh. Passwords hashed with `argon2`. |
| 3 | **Role-based access control (RBAC)** | 2 pts | Three roles: Admin, Author, Viewer. Route-level guards in Axum middleware. Role stored on `users` table. Enforced on all content and admin endpoints. |
| 4 | **User invite endpoint** | 1 pt | `POST /admin/users/invite` creates a pending user record with email + role, returns an invite token. User activates via `POST /auth/activate`. No email sending in v1 — admin copies the activation link. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| OpenAPI generation pipeline (Sprint 1 carryover) | 1 pt | If not completed in Sprint 1 stretch. |
| LLM API key validation endpoint | 1 pt | `POST /admin/settings/llm/test` makes a minimal API call to confirm the key works. Useful in the setup wizard UI. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `argon2` crate integration complexity | Password hashing setup slower than expected | Budget half a day for crypto crate setup. `argon2` is well-documented in Rust. |
| Setup wizard transaction edge cases | Partially initialized workspace if setup fails mid-way | Use a database transaction for the entire init sequence — either fully commits or rolls back cleanly. |

---

## Definition of Done

- [ ] `POST /setup/init` creates a workspace, admin user, and language config in a single transaction
- [ ] `POST /auth/login` returns a valid JWT; expired tokens are rejected with 401
- [ ] Protected routes return 403 for insufficient role, 401 for missing/invalid token
- [ ] `POST /admin/users/invite` returns an activation link
- [ ] `cargo clippy --deny warnings` passes; `cargo test` passes
- [ ] CI green

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — begin setup wizard API |
| Wednesday | Mid-sprint: JWT auth + RBAC complete |
| Friday EOD | Sprint end — full auth flow working end-to-end via `curl` |
