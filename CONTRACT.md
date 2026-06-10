# redline-web — API contract (source of truth)

This file is the **single source of truth** the Rust backend (`apps/api/`) and
the React frontend (`apps/web/`) are both built against. Field names are JSON
(the Rust side uses `#[serde(rename_all = "camelCase")]`). The TS types in
`apps/web/src/api/types.ts` must mirror these exactly.

## Engine model

The server talks to a SQLite-compatible database through a `Connector`:

- **sqlite-file** (primary): opens any SQLite database file with bundled SQLite
  via `rusqlite`. Works for a plain `.sqlite`/`.db` file **and** for a RedlineDB
  database file (RedlineDB is SQLite-shaped).
- **target-bin** (secondary): drives an external SQLite-compatible binary's CLI
  (e.g. a running `redline` / `redline-core` build) by spawning it with SQL on
  stdin — the same `--target-bin` pattern `redline-testing` uses. Used to
  "interact with a running redlineDB" without linking its crates.

`engine` in `ConnectionInfo` is `"redline"` when the target looks like RedlineDB
(detected via `select redline_version()` or a `--target-bin` that reports it),
else `"sqlite"`.

## DTOs (JSON shapes)

```
ConnectionInfo {
  mode: "sqlite-file" | "target-bin",
  engine: "sqlite" | "redline",
  path: string,
  readOnly: boolean,
  sqliteVersion: string,
  engineVersion: string | null,
  sizeBytes: number
}

SchemaObject { name: string, kind: "table"|"view"|"index"|"trigger",
               sql: string | null, rowCount: number | null }
SchemaResponse { objects: SchemaObject[] }

ColumnInfo { name: string, type: string, notNull: boolean, pk: boolean,
             defaultValue: string | null }
TableSchema { name: string, kind: string, columns: ColumnInfo[],
              rowCount: number | null, indexes: string[] }

CellValue = string | number | boolean | null   // JSON scalar per cell
TablePage { name: string, columns: string[], rows: CellValue[][],
            total: number | null, limit: number, offset: number }

QueryRequest { sql: string, maxRows: number | null }
QueryResult { columns: string[], rows: CellValue[][], rowCount: number,
              rowsAffected: number | null, elapsedMs: number, truncated: boolean }
ApiError { error: string }   // returned with HTTP 4xx/5xx

LatencyMs { p50: number, p95: number, p99: number, max: number }
DbStats { sizeBytes: number, pageCount: number, pageSize: number,
          freelistCount: number, walBytes: number }
TableSize { name: string, rowCount: number | null, bytes: number | null }
MetricsSnapshot {
  uptimeSecs: number, totalQueries: number, failedQueries: number,
  qps: number, latencyMs: LatencyMs, db: DbStats, tables: TableSize[],
  atUnixMs: number
}

SlowQuery { sql: string, elapsedMs: number, atUnixMs: number,
            rowCount: number | null, ok: boolean }
SlowQueriesResponse { queries: SlowQuery[] }

Health { status: "ok", version: string, engine: "sqlite" | "redline" }
```

## Endpoints

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/health` | — | `Health` |
| GET | `/api/connection` | — | `ConnectionInfo` |
| GET | `/api/schema` | — | `SchemaResponse` |
| GET | `/api/tables/:name/schema` | — | `TableSchema` |
| GET | `/api/tables/:name?limit&offset&orderBy&dir` | — | `TablePage` |
| POST | `/api/query` | `QueryRequest` | `QueryResult` (400 `ApiError` on SQL error) |
| GET | `/api/metrics` | — | `MetricsSnapshot` |
| GET | `/api/metrics/stream` | — | SSE of `MetricsSnapshot` (~2s cadence) |
| GET | `/api/slow-queries?limit` | — | `SlowQueriesResponse` |
| GET | `/metrics` | — | Prometheus text exposition |
| GET | `/*` | — | embedded SPA (`apps/web/dist`); unmatched non-API paths return `index.html` |

## Behaviour rules

- **Every** query through `POST /api/query` and table paging is recorded in the
  metrics registry (count, latency histogram, slow-query ring if `elapsedMs`
  exceeds the slow threshold, default 100ms).
- `--read-only` opens SQLite `OpenFlags::SQLITE_OPEN_READ_ONLY` and `POST /api/query`
  rejects statements that are not read-only (best-effort: first keyword not in
  SELECT/EXPLAIN/PRAGMA-read/WITH…SELECT) with `ApiError`.
- Results are capped at `maxRows` (default `--max-rows`, 1000); `truncated=true`
  when the cap is hit.
- Query timeout via `--query-timeout-ms` (default 15000) using SQLite's progress
  handler / interrupt.

## Config (clap + env)

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--db <PATH>` | `REDLINE_WEB_DB` | `:memory:` (seeded demo) | SQLite file to open |
| `--target-bin <PATH>` | `REDLINE_WEB_TARGET_BIN` | — | external redline/SQLite CLI |
| `--bind <ADDR>` | `REDLINE_WEB_BIND` | `127.0.0.1:7788` | listen address |
| `--read-only` | `REDLINE_WEB_READ_ONLY` | false | reject writes |
| `--max-rows <N>` | `REDLINE_WEB_MAX_ROWS` | 1000 | result cap |
| `--query-timeout-ms <N>` | `REDLINE_WEB_QUERY_TIMEOUT_MS` | 15000 | per-query timeout |
| `--slow-ms <N>` | `REDLINE_WEB_SLOW_MS` | 100 | slow-query threshold |

When `--db :memory:` with no file given, seed a small demo schema so the UI is
useful out of the box.
