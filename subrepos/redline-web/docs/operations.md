# Operations

## Running

```bash
bash scripts/setup.sh                 # install deps, build web + binary
./target/release/redline-web --db ./demo.sqlite --bind 127.0.0.1:7788
```

Flags (see `--help` and `CONTRACT.md#config`): `--db`, `--target-bin`, `--bind`,
`--read-only`, `--max-rows`, `--query-timeout-ms`, `--slow-ms`. Each maps to a
`REDLINE_WEB_*` env var.

## Observability

- `GET /api/metrics` and `GET /api/metrics/stream` (SSE) expose uptime, qps,
  latency p50/p95/p99, db/WAL sizes, per-table sizes, and a slow-query ring.
- `GET /metrics` serves a Prometheus text exposition.
- Tracing is wired via `tracing-subscriber` (`RUST_LOG` controls verbosity).

## Repair receipts

Every database-boundary failure logs a typed `RepairHint`
(`apps/api/src/repair.rs`): `purpose`, `reason`, `common_fixes`, `docs_url`, and
a one-line `repair_hint` naming where to rerun proof. A `--read-only` write
rejection, for example, points the next agent at
`cargo test -p redline-web-server --test property`.

### Timeouts

A query that exceeds `--query-timeout-ms` is interrupted and returns HTTP 408
with the `Timeout` repair hint. Narrow the query/add an index, or raise the
flag for legitimately slow work.

## Backup and restore

redline-web holds **no durable server-side state** — it serves whatever SQLite
database you point it at (`--db`/`--target-bin`). Backup and restore are
therefore the database's own procedure:

- **Backup:** snapshot the SQLite file (with the server stopped, or via
  `sqlite3 <db> ".backup '<db>.bak'"` / `VACUUM INTO` for a live copy). Include
  any `-wal`/`-shm` sidecar files when copying a live database.
- **Restore:** stop the server, replace the file with the snapshot, restart.
- **Read-only safety:** run with `--read-only` to serve a backup/audit copy with
  writes rejected at the boundary.

## Kill switch

Set `REDLINE_COST_KILL_SWITCH=1` to assert the zero-spend stop condition in
automation (see `agent/cost-budget.toml`).
