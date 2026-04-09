#!/usr/bin/env bash
#
# Creates the two Postgres roles + schema-level privileges that enforce
# ADR-003 at the database layer:
#   * historiador_api — owns everything inside the public schema
#   * historiador_mcp — read-only consumer
#
# Why schema grants live here (and not in the 0001 migration):
# Postgres 15+ does not grant CREATE/USAGE on schema public to non-owners
# by default. The migration is run by historiador_api at api boot, which
# means historiador_api cannot grant *itself* schema-level privileges —
# only the superuser can. So this script (which runs as the superuser on
# first Postgres boot via /docker-entrypoint-initdb.d/) handles
# schema-level grants; the 0001 migration handles table-level grants,
# which historiador_api can issue because it owns the tables it creates.
#
# The script is idempotent — safe if the postgres volume is recreated.
set -euo pipefail

: "${POSTGRES_API_PASSWORD:?POSTGRES_API_PASSWORD is required}"
: "${POSTGRES_MCP_PASSWORD:?POSTGRES_MCP_PASSWORD is required}"

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- --- Role creation -----------------------------------------------------
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

    -- --- Database-level grants --------------------------------------------
    -- CREATE ON DATABASE lets historiador_api install trusted extensions
    -- (pgcrypto, citext) during migrations. Without it, the 0001 migration
    -- fails on CREATE EXTENSION with "permission denied". CONNECT is
    -- obviously required for both roles to log in.
    GRANT CONNECT, CREATE ON DATABASE "${POSTGRES_DB}" TO historiador_api;
    GRANT CONNECT ON DATABASE "${POSTGRES_DB}" TO historiador_mcp;

    -- --- Schema-level grants ----------------------------------------------
    -- historiador_api needs CREATE + USAGE so it can run migrations and
    -- own the tables those migrations create. It becomes the effective
    -- schema owner for Historiador's tables.
    GRANT USAGE, CREATE ON SCHEMA public TO historiador_api;

    -- historiador_mcp needs USAGE to query the tables historiador_api
    -- grants it SELECT on via the 0001 migration.
    GRANT USAGE ON SCHEMA public TO historiador_mcp;
EOSQL
