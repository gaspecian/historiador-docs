-- ============================================================
-- Historiador Doc — initial schema (Sprint 1, Item 3)
--
-- Derived from:
--   ADR-001 (VexFS is retrieval source of truth)
--   ADR-003 (MCP has zero write access — GRANTs enforce this)
--   ADR-005 (multilingual workspace + per-language page_versions)
--   PRD v1.1: nested collections, page states, user roles, workspace
--             language config
--
-- Roles historiador_api and historiador_mcp are created by
-- docker/postgres/init/10-roles.sh on first Postgres boot. This
-- migration only GRANTs privileges to them. The split enforces the
-- ADR-003 read-only invariant at the database layer, independent of
-- how the service containers are configured.
-- ============================================================

-- ---- extensions ----
CREATE EXTENSION IF NOT EXISTS "pgcrypto";  -- gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS "citext";    -- case-insensitive email

-- ---- enums ----
CREATE TYPE user_role AS ENUM ('admin', 'author', 'viewer');
CREATE TYPE page_status AS ENUM ('draft', 'published');

-- ---- workspaces ----
-- v1 has a single workspace per install; the table exists so the
-- schema is forward-compatible with the ADR'd v2 multi-workspace
-- consideration without a destructive migration.
CREATE TABLE workspaces (
    id                         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name                       TEXT        NOT NULL,
    languages                  TEXT[]      NOT NULL,                 -- BCP 47 tags
    primary_language           TEXT        NOT NULL,
    llm_provider               TEXT        NOT NULL DEFAULT 'openai',
    llm_api_key_encrypted      TEXT,                                 -- nullable until setup wizard runs
    mcp_bearer_token_hash      TEXT,                                 -- set by wizard; rotated via admin panel
    created_at                 TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                 TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT primary_language_in_languages
        CHECK (primary_language = ANY(languages))
);

-- ---- users ----
CREATE TABLE users (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id   UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    email          CITEXT      NOT NULL,
    password_hash  TEXT        NOT NULL,
    role           user_role   NOT NULL DEFAULT 'author',
    active         BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, email)
);

-- ---- sessions ----
-- Created in Sprint 1 even though auth is Sprint 2 work, so the schema
-- shape is stable and future migrations stay additive.
CREATE TABLE sessions (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash   TEXT        NOT NULL UNIQUE,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user_id_idx ON sessions(user_id);
CREATE INDEX sessions_expires_at_idx ON sessions(expires_at);

-- ---- collections ----
-- Nested via adjacency list (unbounded depth, recommended ≤3).
-- Tree queries use recursive CTE:
--   WITH RECURSIVE tree AS (
--     SELECT id, parent_id, name, 0 AS depth
--       FROM collections
--      WHERE parent_id IS NULL
--     UNION ALL
--     SELECT c.id, c.parent_id, c.name, t.depth + 1
--       FROM collections c
--       JOIN tree t ON c.parent_id = t.id
--   ) SELECT * FROM tree ORDER BY depth, name;
CREATE TABLE collections (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    parent_id     UUID        REFERENCES collections(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    slug          TEXT        NOT NULL,
    sort_order    INTEGER     NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, parent_id, slug)
);
CREATE INDEX collections_parent_id_idx    ON collections(parent_id);
CREATE INDEX collections_workspace_id_idx ON collections(workspace_id);

-- ---- pages ----
CREATE TABLE pages (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id   UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    collection_id  UUID        REFERENCES collections(id) ON DELETE SET NULL,
    slug           TEXT        NOT NULL,
    status         page_status NOT NULL DEFAULT 'draft',
    created_by     UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, collection_id, slug)
);
CREATE INDEX pages_workspace_id_idx  ON pages(workspace_id);
CREATE INDEX pages_collection_id_idx ON pages(collection_id);
CREATE INDEX pages_status_idx        ON pages(status);

-- ---- page_versions ----
-- Per-language content storage (ADR-005 core data model). A page is
-- considered complete when all workspace `languages` have a published
-- version; the dashboard flags gaps, the system does not block publish.
CREATE TABLE page_versions (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id           UUID        NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    language          TEXT        NOT NULL,               -- BCP 47
    title             TEXT        NOT NULL,
    content_markdown  TEXT        NOT NULL,
    status            page_status NOT NULL DEFAULT 'draft',
    author_id         UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (page_id, language)
);
CREATE INDEX page_versions_page_id_idx  ON page_versions(page_id);
CREATE INDEX page_versions_language_idx ON page_versions(language);

-- ---- chunks ----
-- Metadata only. Embeddings live in VexFS (ADR-001). `vexfs_ref` is an
-- opaque pointer (uuid or provider-specific id) understood by the
-- VectorStore trait implementation in crates/db/src/vector_store.rs.
-- This table is the join point between Postgres and VexFS: row exists
-- iff the corresponding embedding has been written.
CREATE TABLE chunks (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    page_version_id  UUID        NOT NULL REFERENCES page_versions(id) ON DELETE CASCADE,
    heading_path     TEXT[]      NOT NULL,
    section_index    INTEGER     NOT NULL,
    token_count      INTEGER     NOT NULL,
    oversized        BOOLEAN     NOT NULL DEFAULT FALSE,
    language         TEXT        NOT NULL,
    vexfs_ref        TEXT        NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (page_version_id, section_index)
);
CREATE INDEX chunks_page_version_id_idx ON chunks(page_version_id);
CREATE INDEX chunks_language_idx        ON chunks(language);

-- ============================================================
-- updated_at triggers
-- ============================================================

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER workspaces_updated_at    BEFORE UPDATE ON workspaces    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER users_updated_at         BEFORE UPDATE ON users         FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER collections_updated_at   BEFORE UPDATE ON collections   FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER pages_updated_at         BEFORE UPDATE ON pages         FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER page_versions_updated_at BEFORE UPDATE ON page_versions FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ============================================================
-- Role GRANTs — enforces ADR-003 at the database layer.
--
-- historiador_api: full CRUD on every table
-- historiador_mcp: SELECT only, and only on tables needed for
--                  source attribution in MCP responses. NOT on
--                  users or sessions — MCP has no business
--                  reading authentication data.
-- ============================================================

-- API role (read/write owner)
GRANT CONNECT ON DATABASE historiador TO historiador_api;
GRANT USAGE ON SCHEMA public TO historiador_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO historiador_api;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO historiador_api;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO historiador_api;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO historiador_api;

-- MCP role (read-only, restricted surface)
GRANT CONNECT ON DATABASE historiador TO historiador_mcp;
GRANT USAGE ON SCHEMA public TO historiador_mcp;
GRANT SELECT ON workspaces, collections, pages, page_versions, chunks TO historiador_mcp;
-- Explicitly NO grants on users, sessions, or the sqlx migrations table.
