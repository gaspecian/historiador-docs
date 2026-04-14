-- Sprint 7: Page version history (immutable audit trail).
--
-- The existing `page_versions` table retains its UNIQUE(page_id, language)
-- constraint and continues to represent the "current working copy" for
-- each language. This new table is an append-only history: every save
-- and publish creates a snapshot that can be browsed and restored.

CREATE TABLE page_version_history (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id          UUID        NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    language         TEXT        NOT NULL,
    title            TEXT        NOT NULL,
    content_markdown TEXT        NOT NULL,
    is_published     BOOLEAN     NOT NULL DEFAULT FALSE,
    author_id        UUID        REFERENCES users(id) ON DELETE SET NULL,
    version_number   INTEGER     NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- No updated_at: rows are immutable once written.
    UNIQUE (page_id, language, version_number)
);

-- Primary lookup: "show me history for this page in this language, newest first"
CREATE INDEX pvh_page_lang_created_idx
    ON page_version_history(page_id, language, created_at DESC);

-- API role (historiador_api) owns the table implicitly via migration.
-- MCP role does NOT need access — history is an authoring-API concern.
