//! Database connectors.
//!
//! A [`Connector`] abstracts the database the server talks to. Two transports
//! are provided:
//!
//! * [`sqlite::SqliteConnector`] — opens a SQLite file (or `:memory:`) directly
//!   with bundled `rusqlite`. This is the primary, fully featured path.
//! * [`target_bin::TargetBinConnector`] — drives an external SQLite-compatible
//!   CLI by spawning it with SQL on stdin (the `--target-bin` pattern).
//!
//! Concrete connectors are held by the [`AnyConnector`] enum so the API layer
//! can store a single value behind `Arc` without async-`dyn` gymnastics.

pub mod sqlite;
mod sqlite_support;
pub mod target_bin;

use thiserror::Error;

use crate::model::{
    ConnectionInfo, DbStats, QueryResult, SchemaResponse, TablePage, TableSchema, TableSize,
};

pub use sqlite::SqliteConnector;
pub use target_bin::TargetBinConnector;

/// Options controlling a single page of [`Connector::table_page`].
#[derive(Debug, Clone)]
pub struct PageOptions {
    /// Maximum rows to return.
    pub limit: i64,
    /// Rows to skip.
    pub offset: i64,
    /// Optional column to order by (validated against real columns).
    pub order_by: Option<String>,
    /// Optional sort direction (`asc` / `desc`).
    pub dir: Option<String>,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            order_by: None,
            dir: None,
        }
    }
}

/// Errors a connector can surface. These map to HTTP statuses in the API layer.
#[derive(Debug, Error)]
pub enum ConnectorError {
    /// Underlying SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// I/O error (file access, subprocess pipes).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Requested object (table/view) does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Caller supplied an invalid argument (e.g. unknown order-by column).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Write attempted against a read-only connection.
    #[error("read-only: {0}")]
    ReadOnly(String),

    /// The query exceeded its configured timeout.
    #[error("query timed out")]
    Timeout,

    /// The connector does not support the requested operation.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A spawned target binary failed.
    #[error("target binary error: {0}")]
    Process(String),

    /// Any other failure.
    #[error("{0}")]
    Other(String),
}

impl ConnectorError {
    /// Coarse HTTP status category for this error (used by the API layer).
    pub fn http_status(&self) -> u16 {
        match self {
            ConnectorError::NotFound(_) => 404,
            ConnectorError::InvalidArgument(_) => 400,
            ConnectorError::ReadOnly(_) => 400,
            ConnectorError::Timeout => 408,
            ConnectorError::Unsupported(_) => 501,
            ConnectorError::Sqlite(_) => 400,
            _ => 500,
        }
    }
}

/// A database connector. All methods are `async` so the target-bin transport
/// can drive a subprocess; the SQLite transport simply runs synchronously.
#[allow(async_fn_in_trait)]
pub trait Connector {
    /// Describe the live connection.
    async fn connection_info(&self) -> Result<ConnectionInfo, ConnectorError>;
    /// List every schema object (tables, views, indexes, triggers).
    async fn schema(&self) -> Result<SchemaResponse, ConnectorError>;
    /// Full schema for a single table/view.
    async fn table_schema(&self, name: &str) -> Result<TableSchema, ConnectorError>;
    /// A paginated window over a table's rows.
    async fn table_page(&self, name: &str, opts: &PageOptions)
    -> Result<TablePage, ConnectorError>;
    /// Execute arbitrary SQL, capping output at `max_rows`.
    async fn query(&self, sql: &str, max_rows: i64) -> Result<QueryResult, ConnectorError>;
    /// On-disk storage statistics.
    async fn db_stats(&self) -> Result<DbStats, ConnectorError>;
    /// Approximate per-table sizes.
    async fn table_sizes(&self) -> Result<Vec<TableSize>, ConnectorError>;
}

/// Owning enum over the available connector transports.
///
/// Using an enum (rather than `dyn Connector`) keeps the trait free to use
/// `async fn` while still presenting a single concrete type to `AppState`.
#[derive(Debug)]
pub enum AnyConnector {
    /// Direct SQLite-file transport.
    Sqlite(SqliteConnector),
    /// External-binary transport.
    TargetBin(TargetBinConnector),
}

impl Connector for AnyConnector {
    async fn connection_info(&self) -> Result<ConnectionInfo, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.connection_info().await,
            AnyConnector::TargetBin(c) => c.connection_info().await,
        }
    }

    async fn schema(&self) -> Result<SchemaResponse, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.schema().await,
            AnyConnector::TargetBin(c) => c.schema().await,
        }
    }

    async fn table_schema(&self, name: &str) -> Result<TableSchema, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.table_schema(name).await,
            AnyConnector::TargetBin(c) => c.table_schema(name).await,
        }
    }

    async fn table_page(
        &self,
        name: &str,
        opts: &PageOptions,
    ) -> Result<TablePage, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.table_page(name, opts).await,
            AnyConnector::TargetBin(c) => c.table_page(name, opts).await,
        }
    }

    async fn query(&self, sql: &str, max_rows: i64) -> Result<QueryResult, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.query(sql, max_rows).await,
            AnyConnector::TargetBin(c) => c.query(sql, max_rows).await,
        }
    }

    async fn db_stats(&self) -> Result<DbStats, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.db_stats().await,
            AnyConnector::TargetBin(c) => c.db_stats().await,
        }
    }

    async fn table_sizes(&self) -> Result<Vec<TableSize>, ConnectorError> {
        match self {
            AnyConnector::Sqlite(c) => c.table_sizes().await,
            AnyConnector::TargetBin(c) => c.table_sizes().await,
        }
    }
}

/// Best-effort check that `sql` is a read-only statement.
///
/// Used by the read-only guard. The first significant keyword must be one of
/// `SELECT`, `EXPLAIN`, `PRAGMA`, `WITH`, or `VALUES`. This is intentionally
/// conservative and may reject some benign statements.
pub fn is_read_only_sql(sql: &str) -> bool {
    let trimmed = strip_leading_noise(sql);
    let first = trimmed
        .split(|c: char| c.is_whitespace() || c == '(')
        .find(|tok| !tok.is_empty())
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        first.as_str(),
        "SELECT" | "EXPLAIN" | "PRAGMA" | "WITH" | "VALUES"
    )
}

/// Strip leading whitespace and SQL line/block comments.
fn strip_leading_noise(sql: &str) -> &str {
    let mut s = sql.trim_start();
    loop {
        if let Some(rest) = s.strip_prefix("--") {
            // Line comment: skip to end of line.
            match rest.find('\n') {
                Some(idx) => s = rest[idx + 1..].trim_start(),
                None => return "",
            }
        } else if let Some(rest) = s.strip_prefix("/*") {
            // Block comment: skip to closing */.
            match rest.find("*/") {
                Some(idx) => s = rest[idx + 2..].trim_start(),
                None => return "",
            }
        } else {
            return s;
        }
    }
}

/// Quote an identifier for safe interpolation into SQL (`"` doubled).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_detection() {
        assert!(is_read_only_sql("select 1"));
        assert!(is_read_only_sql("  SELECT * FROM t"));
        assert!(is_read_only_sql("-- comment\nselect 1"));
        assert!(is_read_only_sql(
            "/* c */ WITH x AS (SELECT 1) SELECT * FROM x"
        ));
        assert!(is_read_only_sql("PRAGMA table_info(t)"));
        assert!(!is_read_only_sql("insert into t values (1)"));
        assert!(!is_read_only_sql("DELETE FROM t"));
        assert!(!is_read_only_sql("update t set a=1"));
        assert!(!is_read_only_sql("drop table t"));
    }

    #[test]
    fn ident_quoting() {
        assert_eq!(quote_ident("users"), "\"users\"");
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }
}
