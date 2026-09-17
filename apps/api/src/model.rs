//! Data-transfer objects for the redline-web HTTP API.
//!
//! Every struct here mirrors a JSON shape defined in `CONTRACT.md`. The wire
//! format is camelCase (`#[serde(rename_all = "camelCase")]`); the TypeScript
//! types in `web/src/api/types.ts` must stay in lock-step with these.
//!
//! A single table cell is an arbitrary JSON scalar (`string | number | boolean
//! | null`), represented as [`serde_json::Value`] and aliased as [`CellValue`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single table cell: any JSON scalar (`string | number | boolean | null`).
pub type CellValue = Value;

/// The connector transport in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionMode {
    /// Opens a SQLite database file directly via bundled `rusqlite`.
    SqliteFile,
    /// Drives an external SQLite-compatible CLI over stdin/stdout.
    TargetBin,
}

/// Which engine the connected database reports as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Plain SQLite.
    Sqlite,
    /// RedlineDB (SQLite-shaped) detected via a version probe.
    Redline,
}

impl Engine {
    /// Lowercase wire string for this engine.
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Sqlite => "sqlite",
            Engine::Redline => "redline",
        }
    }
}

/// Describes the live connection backing the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub mode: ConnectionMode,
    pub engine: Engine,
    pub path: String,
    pub read_only: bool,
    pub sqlite_version: String,
    pub engine_version: Option<String>,
    pub size_bytes: u64,
}

/// The kind of a schema object as reported by `sqlite_master`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaObjectKind {
    Table,
    View,
    Index,
    Trigger,
}

/// One row of the database schema (table/view/index/trigger).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    pub name: String,
    pub kind: SchemaObjectKind,
    pub sql: Option<String>,
    pub row_count: Option<i64>,
}

/// Response for `GET /api/schema`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaResponse {
    pub objects: Vec<SchemaObject>,
}

/// Column metadata from `pragma table_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub not_null: bool,
    pub pk: bool,
    pub default_value: Option<String>,
}

/// Full schema of a single table/view, for `GET /api/tables/:name/schema`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub name: String,
    pub kind: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<i64>,
    pub indexes: Vec<String>,
}

/// A paginated window over a table, for `GET /api/tables/:name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub total: Option<i64>,
    pub limit: i64,
    pub offset: i64,
}

/// Request body for `POST /api/query`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRequest {
    pub sql: String,
    #[serde(default)]
    pub max_rows: Option<i64>,
}

/// Result of an executed SQL statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub row_count: i64,
    pub rows_affected: Option<i64>,
    pub elapsed_ms: f64,
    pub truncated: bool,
}

/// Error envelope returned with any non-2xx status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub error: String,
}

impl ApiError {
    /// Build an [`ApiError`] from anything stringifiable.
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

/// Latency percentiles in milliseconds.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyMs {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

/// On-disk storage statistics derived from SQLite pragmas.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStats {
    pub size_bytes: u64,
    pub page_count: i64,
    pub page_size: i64,
    pub freelist_count: i64,
    pub wal_bytes: u64,
}

/// Approximate size of one table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSize {
    pub name: String,
    pub row_count: Option<i64>,
    pub bytes: Option<i64>,
}

/// Point-in-time metrics snapshot, for `GET /api/metrics` and the SSE stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub uptime_secs: u64,
    pub total_queries: u64,
    pub failed_queries: u64,
    pub qps: f64,
    pub latency_ms: LatencyMs,
    pub db: DbStats,
    pub tables: Vec<TableSize>,
    pub at_unix_ms: u64,
}

/// One entry of the slow-query ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowQuery {
    pub sql: String,
    pub elapsed_ms: f64,
    pub at_unix_ms: u64,
    pub row_count: Option<i64>,
    pub ok: bool,
}

/// Response for `GET /api/slow-queries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowQueriesResponse {
    pub queries: Vec<SlowQuery>,
}

/// Response for `GET /api/health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: String,
    pub version: String,
    pub engine: Engine,
}
