-- Sprint 11 / ADR-014 — autonomy mode per page.
--
-- Stored alongside page_versions so each (page, language) version
-- carries its own mode. Default "propose" is the safest stance for
-- new pages; workspace-level default overrides ship via
-- workspaces.default_autonomy_mode (also added here).
--
-- CHECK constraint mirrors the values the application-layer enum
-- accepts. Keep in sync with
-- apps/api/src/infrastructure/telemetry/editor.rs::AutonomyMode.

ALTER TABLE page_versions
    ADD COLUMN autonomy_mode TEXT NOT NULL DEFAULT 'propose'
        CHECK (autonomy_mode IN ('propose', 'checkpointed', 'autonomous'));

ALTER TABLE workspaces
    ADD COLUMN default_autonomy_mode TEXT NOT NULL DEFAULT 'propose'
        CHECK (default_autonomy_mode IN ('propose', 'checkpointed', 'autonomous'));
