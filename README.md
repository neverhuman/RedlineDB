# redline-central

[![Jankurai](https://img.shields.io/badge/Jankurai-governed-blue)](agent/repo-score.md)

Agent entrypoint: [`AGENTS.md`](AGENTS.md).

The **centralized RedlineDB for the whole system**: one shared `redlinedb-server` that every project
connects to through a backend-neutral contract and validated per-project table prefixes. The
production composition selects `backend-redline`; SQLite and Postgres are isolated oracle features.
Switching providers changes one dependency feature and `DB_DSN`, not application database code.

## Layout
- `crates/redlinedb-client` — sync remote client for `redlinedb-server` (framed TCP), rusqlite-shaped.
- `crates/db-shim` — owned values/errors/capabilities/statements plus isolated provider adapters.
- `docker/` — `Dockerfile` + `docker-compose.yml` for the central server.
- `.env.example` — neutral `DB_DSN` / `DB_NAMESPACE` template a consumer copies.
- `db/backend-contract.toml` — machine-readable feature and operation-corpus boundary.

## Quick start

```sh
cargo test --locked --workspace --all-targets
DB_DSN=:memory: DB_NAMESPACE=demo \
  cargo run --locked -p db-shim --no-default-features --features oracle-sqlite --bin db-shim-parity
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
- ✅ `db-shim` has exact isolated Redline, SQLite, and Postgres dependency graphs. The same governed
  `db-shim.used-operations/v1` corpus covers values, validated namespacing, parameter binding,
  query ordering, and rollback. Host-local required runs genuine SQLite. Armed release CI requires
  genuine Redline and Postgres services and fails closed when their DSNs are absent.
- ⚠️ The corpus is the compatibility claim. Arbitrary SQL-dialect equivalence is not claimed.

## Validate

Run the complete protected-review contract with
`bash scripts/ci-local.sh required`. Release identity and immutable-tag rules
are documented in [`docs/release.md`](docs/release.md).
