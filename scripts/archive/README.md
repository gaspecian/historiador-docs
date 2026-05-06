# scripts/archive/

Scripts that are kept for historical reference but are no longer part
of the normal dev or deploy path. They may still work, but nothing in
the current architecture requires running them.

## `setup-vexfs.sh`

Vendored VexFS at a pinned SHA and applied two local patches (submodule
workaround + entrypoint fix) so the `docker compose --profile vector`
path could spin up a VexFS container.

**Superseded by:** [ADR-007 — Chronik-Stream](../../artifacts/adr/ADR-007-chronik-stream.md).
Chronik-Stream replaces VexFS as the retrieval substrate and ships as
a prebuilt image (`ghcr.io/lspecian/chronik-stream`) — no vendoring is
needed. The `crates/db::vector_store::ChronikVectorStore` is the
production implementation as of Sprint 7.

The script is left in place for anyone running a fork on the old
VexFS path, but new installations should follow the standard
`docker compose up -d` flow documented in the README.
