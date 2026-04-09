#!/usr/bin/env bash
#
# Creates the two Postgres roles that enforce ADR-003 at the database layer:
#   * historiador_api — read/write owner of the schema
#   * historiador_mcp — read-only consumer (no users/sessions access)
#
# This script runs ONCE on first Postgres boot, mounted into
# /docker-entrypoint-initdb.d/ by docker-compose.yml. It is idempotent via
# DO $$ ... IF NOT EXISTS $$ guards, so it is safe to leave in place even if
# the postgres volume is recreated.
#
# The actual GRANT statements live in the 0001 migration; those run later
# (by the api container on boot). Role creation must happen first so the
# GRANTs have a target.
set -euo pipefail

: "${POSTGRES_API_PASSWORD:?POSTGRES_API_PASSWORD is required}"
: "${POSTGRES_MCP_PASSWORD:?POSTGRES_MCP_PASSWORD is required}"

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = 'historiador_api') THEN
            CREATE ROLE historiador_api LOGIN PASSWORD '${POSTGRES_API_PASSWORD}';
        ELSE
            ALTER ROLE historiador_api WITH LOGIN PASSWORD '${POSTGRES_API_PASSWORD}';
        END IF;

        IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = 'historiador_mcp') THEN
            CREATE ROLE historiador_mcp LOGIN PASSWORD '${POSTGRES_MCP_PASSWORD}';
        ELSE
            ALTER ROLE historiador_mcp WITH LOGIN PASSWORD '${POSTGRES_MCP_PASSWORD}';
        END IF;
    END
    \$\$;
EOSQL
