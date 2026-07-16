# redline-central

[![Jankurai](https://img.shields.io/badge/Jankurai-governed-blue)](agent/repo-score.md)

Agent entrypoint: [`AGENTS.md`](AGENTS.md).

The **centralized RedlineDB for the whole system**: one shared `redlinedb-server` (in Docker) that
every project connects to, with a Redline-only production abstraction and per-project table-name
prefixes. Lives beside `redline-core` under `redline-split/`; the server binary stays in redline-core,
this subrepo dockerizes it and provides the remote client + shim. Bundled SQLite is available only
through the explicit `sqlite-parity` feature for parity and migration checks.

## Layout
- `crates/redlinedb-client` — sync remote client for `redlinedb-server` (framed TCP), rusqlite-shaped.
- `crates/db-shim` — Redline production abstraction (+ `{ns}` prefixing, `.env`) with explicit
  SQLite parity/migration mode.
- `docker/` — `Dockerfile` + `docker-compose.yml` for the central server.
- `.env.example` — `DB_BACKEND` / `DB_DSN` / `DB_NAMESPACE` template a consumer copies.

## Quick start

```sh
cargo test --locked --workspace --all-targets
DB_BACKEND=sqlite DB_DSN=:memory: DB_NAMESPACE=demo \
  cargo run --locked -p db-shim --features sqlite-parity --bin db-shim-parity
```

## Run the central DB
Docker (recommended):
```
cd docker && docker compose up -d      # serves 0.0.0.0:6033, persistent volume /data
```
Or locally:
```
redlinedb-server --database ./central.redline --listen 127.0.0.1:6033   # from redline-core
```

## Status
- ✅ **`redlinedb-client` proven** — full round-trip (DDL / parameterized DML / transaction / query)
  **and concurrent multi-client** against one shared server, which bypasses RedlineDB's embedded
  exclusive-`flock` limit (multiple processes cannot open one embedded dir; the server fixes that).
  Verify: start the server, then
  `cargo run -p redlinedb-client --bin redlinedb-client-smoke -- 127.0.0.1:6033`.
- ✅ `db-shim` defaults to a Redline-only production dependency graph. SQLite behavior, namespace
  expansion, commit, and rollback are covered by the explicit `sqlite-parity` host-local lane.
  Live Redline parity remains an explicit
  service-backed smoke lane, not a hidden dependency of the required lane.

## Validate

Run the complete protected-review contract with
`bash scripts/ci-local.sh required`. Release identity and immutable-tag rules
are documented in [`docs/release.md`](docs/release.md).
