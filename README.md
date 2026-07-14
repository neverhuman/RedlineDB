# redline-central

The **centralized RedlineDB for the whole system**: one shared `redlinedb-server` (in Docker) that
every project connects to, with a switchable SQLite/RedlineDB abstraction and per-project table-name
prefixes. Lives beside `redline-core` under `redline-split/`; the server binary stays in redline-core,
this subrepo dockerizes it and provides the remote client + the switchable shim.

## Layout
- `crates/redlinedb-client` — sync remote client for `redlinedb-server` (framed TCP), rusqlite-shaped.
- `crates/db-shim` — *(next)* switchable `sqlite|redline` abstraction (+ `{ns}` prefixing, `.env`).
- `docker/` — `Dockerfile` + `docker-compose.yml` for the central server.
- `.env.example` — `DB_BACKEND` / `DB_DSN` / `DB_NAMESPACE` template a consumer copies.

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
- ⏳ `db-shim` (switchable sqlite|redline + `{ns}` prefix), central migration tooling — next.
