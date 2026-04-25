-- 0008: replace opaque vexfs_ref with the (partition, offset) pair
-- Chronik 2.4.1 returns from /_vector/<topic>/search hits.
--
-- chunks.vexfs_ref was a placeholder for the old "opaque pointer to a
-- VexFS doc id" model. Chronik does not assign user-controllable doc
-- ids; instead, every produced record is addressable by its
-- (partition, offset) within the topic. The MCP enrichment join now
-- keys on (chronik_partition, chronik_offset).

ALTER TABLE chunks DROP COLUMN vexfs_ref;

ALTER TABLE chunks
    ADD COLUMN chronik_partition INTEGER,
    ADD COLUMN chronik_offset    BIGINT;

-- Old chunk rows (if any) cannot be back-filled — they reference a
-- vector store that never received them. Keep the rows for audit but
-- mark the offsets as NULL; the MCP enricher must skip rows where
-- either column is NULL.

CREATE INDEX chunks_chronik_offset_idx
    ON chunks(chronik_partition, chronik_offset)
    WHERE chronik_partition IS NOT NULL;
