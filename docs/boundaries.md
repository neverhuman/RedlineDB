# Boundaries

Authoritative boundary declaration: [`agent/boundaries.toml`](../agent/boundaries.toml).

## Data-access boundary

This is a single-crate CLI harness with **no in-process database driver**. The
only database access is a subprocess invocation of an external `sqlite3` /
`psql` shell, driven from:

- `src/sqlite_parity/engine.rs` — builds SQL probe scripts, runs them through
  the reference `sqlite3` CLI.
- `src/sqlite_parity/rql_phase1.rs` — lowers SQL text to JSON (dense with SQL
  keyword literals by design).
- `src/beyond_sqlite/engine.rs` and `src/beyond_sqlite/oracle.rs` — drive
  `psql` for the PostgreSQL-class oracle.

These four files are the adapter seam. The jankurai wrong-layer-DB detector
recognizes only `db/`, `migrations/`, and `crates/adapters/` as data layers,
none of which a flat single-crate harness has — so `agent/audit-policy.toml`
scopes those files out of the product-language scan and documents the rationale.
The reference CLI name is centralized as `sqlite_parity::REFERENCE_CLI_BIN` so
the renderer/CLI modules never carry a raw DB-client marker.

## Generated zones

Generated artifacts are declared in
[`.jankurai/generated-zones.toml`](../.jankurai/generated-zones.toml). Do not
hand-edit them — regenerate via the declared command
(`cargo run -p xtask -- generate`).

## Output contract

The JSONL raw-record shape (`schemas/raw-record.schema.json`) and the release
manifest shape (`schemas/release-manifest.schema.json`) are consumed by
RedlineDB CI. Field-level compatibility is a hard contract; the
`release_manifest_integrity` test guards artifact drift.

## Cross-runtime / ownership

Ownership and proof routing live in
[`agent/owner-map.json`](../agent/owner-map.json) and
[`agent/test-map.json`](../agent/test-map.json); per-cell guidance is in
`ops/AGENTS.md`.
