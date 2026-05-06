-- Sprint 10: Editor conversation persistence (code review finding 4.3
-- minimum-bar fix; ADR-009 records the SSE-on-Postgres path).
--
-- Each row is the conversation transcript for one (page, language, user)
-- triple: messages are stored as a JSONB array of {role, content, ts}
-- objects. The table is upserted via ON CONFLICT so "save the current
-- conversation" is a single round-trip.
--
-- The Chronik `editor-conversations:stream` topic remains provisioned
-- for v1.1, where the WebSocket rebuild per ADR-008 will dual-write
-- every message as a durable event.

CREATE TABLE editor_conversations (
    page_id     UUID        NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    language    TEXT        NOT NULL,
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    messages    JSONB       NOT NULL DEFAULT '[]'::jsonb,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (page_id, language, user_id)
);

-- Per-user recency lookups: "show me my in-progress pages, newest first"
CREATE INDEX editor_conversations_user_updated_idx
    ON editor_conversations(user_id, updated_at DESC);

-- The API role owns the table implicitly via migration. MCP role does
-- NOT need access — editor conversations are an authoring concern.
