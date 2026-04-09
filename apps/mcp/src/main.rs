// Sprint 1 stub. Item 4 will populate this with a readonly Postgres pool,
// Axum /health endpoint, and graceful shutdown. Sprint 2 adds the MCP
// protocol handler at POST /mcp.
//
// INVARIANT (ADR-003): This binary must NEVER call
// historiador_db::run_migrations — the readonly role lacks DDL privileges
// and MCP has no business owning schema evolution.
fn main() {}
