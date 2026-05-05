-- ============================================================
-- Drop the workspaces.embedding_model column.
--
-- Chronik handles embeddings server-side via its topic-level
-- vector.embedding.* configuration; the workspace-scoped column
-- is vestigial from the pre-Chronik (VexFS) era. The application
-- code stopped referencing it in the EmbeddingClient retirement
-- (see artifacts/plans/2026-05-01-embedding-client-retirement-plan.md).
-- ============================================================

ALTER TABLE workspaces
    DROP COLUMN embedding_model;
