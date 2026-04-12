-- ============================================================
-- Historiador Doc — auth & setup schema deltas (Sprint 2)
--
-- Adds:
--   1. installation singleton (global setup-complete flag)
--   2. pending-user columns on users (invite flow)
--
-- The installation row is intentionally a singleton (id = 1) —
-- v1 is single-workspace-per-install, but the flag lives in its
-- own table rather than on workspaces so that "setup complete"
-- can be asserted before any workspace exists (e.g. to gate
-- /setup/init from being called twice).
--
-- users.password_hash becomes nullable so an invited user can
-- exist before choosing their password at activation. A CHECK
-- constraint keeps the two states mutually exclusive: every row
-- either has a password (activated) or an invite token (pending).
-- ============================================================

-- ---- installation singleton ----
CREATE TABLE installation (
    id              SMALLINT    PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    setup_complete  BOOLEAN     NOT NULL DEFAULT FALSE,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO installation (id) VALUES (1);

-- ---- users: pending-user support ----
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN invite_token_hash TEXT;
ALTER TABLE users ADD COLUMN invite_expires_at TIMESTAMPTZ;

CREATE INDEX users_invite_token_hash_idx
    ON users(invite_token_hash)
    WHERE invite_token_hash IS NOT NULL;

-- Pending rows have an invite token and no password; activated rows
-- have a password and no invite token. Enforced at the DB layer so a
-- bug in the application cannot produce a row in both states.
ALTER TABLE users ADD CONSTRAINT users_invite_xor_password CHECK (
    (password_hash IS NOT NULL AND invite_token_hash IS NULL) OR
    (password_hash IS NULL     AND invite_token_hash IS NOT NULL)
);

-- ============================================================
-- Role GRANTs — installation stays invisible to historiador_mcp.
-- MCP has no reason to know install state; keeping it out of the
-- read role is cheap defence in depth.
-- ============================================================
-- (no GRANT statement = no access for historiador_mcp)
