-- Sprint 11 / phase C1 / US-11.15 — stretch.
--
-- Minimal view-count storage for shareable links. One row per
-- (page_id, language, day) so a daily chart does not need to scan
-- a full event table, while still counting hits independently of
-- views from the authoring app.
--
-- No PII: `referrer_host` is a host (not a full URL) and null when
-- the request carried no Referer header; `viewer_token` is a
-- sha256 fingerprint of the cookie set on first visit.

CREATE TABLE page_views (
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    language TEXT NOT NULL,
    day DATE NOT NULL,
    referrer_host TEXT,
    viewer_token TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (page_id, language, day, viewer_token, referrer_host)
);

CREATE INDEX page_views_lookup ON page_views (page_id, language, day);

-- Shareable-link metadata lives on pages so the authoring UI can
-- toggle visibility per page without a separate table.
ALTER TABLE pages
    ADD COLUMN share_visibility TEXT NOT NULL DEFAULT 'private'
        CHECK (share_visibility IN ('private', 'workspace', 'public'));
