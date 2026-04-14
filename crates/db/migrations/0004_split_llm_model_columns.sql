-- ============================================================
-- Sprint 8 — separate LLM generation and embedding model config.
--
-- Before this migration the workspace carried only `llm_provider` and
-- an encrypted API key. Sprint 8 introduces Ollama, where the
-- generation model and embedding model are independent strings that
-- the admin picks (e.g. `llama3.1:8b` + `nomic-embed-text`). Making
-- these explicit columns also benefits OpenAI deployments that want
-- to pin a specific embedding model.
--
-- Backfill picks each provider's historical defaults so existing
-- installations keep working after an in-place upgrade.
-- ============================================================

ALTER TABLE workspaces
    ADD COLUMN generation_model TEXT,
    ADD COLUMN embedding_model  TEXT,
    ADD COLUMN llm_base_url     TEXT;  -- used by Ollama; null for cloud providers

UPDATE workspaces SET
    generation_model = CASE llm_provider
        WHEN 'openai'    THEN 'gpt-4o-mini'
        WHEN 'anthropic' THEN 'claude-haiku-4-5-20251001'
        WHEN 'ollama'    THEN 'llama3.1:8b'
        ELSE 'stub'
    END,
    embedding_model = CASE llm_provider
        WHEN 'openai'    THEN 'text-embedding-3-small'
        WHEN 'anthropic' THEN 'text-embedding-3-small'
        WHEN 'ollama'    THEN 'nomic-embed-text'
        ELSE 'stub'
    END
WHERE generation_model IS NULL;

ALTER TABLE workspaces
    ALTER COLUMN generation_model SET NOT NULL,
    ALTER COLUMN embedding_model  SET NOT NULL;
