// API DTOs — these mirror CONTRACT.md exactly (JSON is camelCase via serde
// rename_all = "camelCase" on the Rust side). Do not drift from the contract.

export type ConnectionMode = "sqlite-file" | "target-bin";
export type Engine = "sqlite" | "redline";

export interface ConnectionInfo {
  mode: ConnectionMode;
  engine: Engine;
  path: string;
  readOnly: boolean;
  sqliteVersion: string;
  engineVersion: string | null;
  sizeBytes: number;
}

export type SchemaObjectKind = "table" | "view" | "index" | "trigger";

export interface SchemaObject {
  name: string;
  kind: SchemaObjectKind;
  sql: string | null;
  rowCount: number | null;
}

export interface SchemaResponse {
  objects: SchemaObject[];
}

export interface ColumnInfo {
  name: string;
  type: string;
  notNull: boolean;
  pk: boolean;
  defaultValue: string | null;
}

export interface TableSchema {
  name: string;
  kind: string;
  columns: ColumnInfo[];
  rowCount: number | null;
  indexes: string[];
}

// JSON scalar per cell.
export type CellValue = string | number | boolean | null;

export interface TablePage {
  name: string;
  columns: string[];
  rows: CellValue[][];
  total: number | null;
  limit: number;
  offset: number;
}

export interface QueryRequest {
  sql: string;
  maxRows: number | null;
}

export interface QueryResult {
  columns: string[];
  rows: CellValue[][];
  rowCount: number;
  rowsAffected: number | null;
  elapsedMs: number;
  truncated: boolean;
}

export interface ApiErrorBody {
  error: string;
}

export interface LatencyMs {
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

export interface DbStats {
  sizeBytes: number;
  pageCount: number;
  pageSize: number;
  freelistCount: number;
  walBytes: number;
}

export interface TableSize {
  name: string;
  rowCount: number | null;
  bytes: number | null;
}

export interface MetricsSnapshot {
  uptimeSecs: number;
  totalQueries: number;
  failedQueries: number;
  qps: number;
  latencyMs: LatencyMs;
  db: DbStats;
  tables: TableSize[];
  atUnixMs: number;
}

export interface SlowQuery {
  sql: string;
  elapsedMs: number;
  atUnixMs: number;
  rowCount: number | null;
  ok: boolean;
}

export interface SlowQueriesResponse {
  queries: SlowQuery[];
}

export interface Health {
  status: "ok";
  version: string;
  engine: Engine;
}

// Options accepted by GET /api/tables/:name
export interface TablePageOptions {
  limit?: number;
  offset?: number;
  orderBy?: string;
  dir?: "asc" | "desc";
}
