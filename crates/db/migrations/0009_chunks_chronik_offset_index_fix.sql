-- 0009: tighten the chunks_chronik_offset_idx partial-index predicate
-- to match the "skip rows where either column is NULL" contract from
-- migration 0008's comment. A row with partition set but offset NULL
-- is malformed (partition without an offset is not a valid Chronik
-- address), so the index has no business covering it.

DROP INDEX IF EXISTS chunks_chronik_offset_idx;

CREATE INDEX chunks_chronik_offset_idx
    ON chunks(chronik_partition, chronik_offset)
    WHERE chronik_partition IS NOT NULL
      AND chronik_offset    IS NOT NULL;
