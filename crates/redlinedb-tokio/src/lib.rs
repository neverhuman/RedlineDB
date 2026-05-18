//! Tokio async adapter for RedlineDB.
//!
//! RedlineDB's core API ([`redlinedb::Database`], [`redlinedb::Connection`]) is
//! intentionally synchronous and thread-bound. That's the right shape for an
//! embedded MVCC engine, but async tokio crates have to wrap every call in
//! `tokio::task::spawn_blocking` and pool their own connections to keep the
//! ergonomics tolerable.
//!
//! This crate is that wrapper. The public surface deliberately mirrors a thin
//! subset of `sqlx::Pool<Any>` so callers migrating from sqlx can do a
//! mechanical rename.
//!
//! # Example
//!
//! ```no_run
//! use redlinedb_tokio::{Pool, params};
//!
//! # async fn run() -> redlinedb_tokio::Result<()> {
//! let pool = Pool::open_in_memory().await?;
//! pool.execute(
//!     "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
//!     params![],
//! ).await?;
//! pool.execute(
//!     "INSERT INTO items(id, name) VALUES (?, ?)",
//!     params![1_i64, "Ada"],
//! ).await?;
//! let row = pool
//!     .fetch_one("SELECT name FROM items WHERE id = ?", params![1_i64])
//!     .await?;
//! assert_eq!(row.try_get_text(0)?, "Ada");
//! # Ok(())
//! # }
//! ```
//!
//! # Concurrency model
//!
//! Internally, `Pool` holds one cheaply-cloned [`redlinedb::Database`] and a
//! [`tokio::sync::Semaphore`] that bounds the number of concurrent
//! `spawn_blocking` tasks. Every operation acquires a permit, opens a fresh
//! `Connection` from the shared `Database`, runs the sync call on a blocking
//! thread, and returns the materialized result.
//!
//! Rows returned from `fetch_*` are materialized as owned [`AsyncRow`] values
//! that survive `.await` boundaries — the borrowed `redlinedb::Row` cannot
//! cross threads because it carries a lifetime to the parent connection.

#![warn(missing_docs)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

pub use redlinedb::{
    BeginMode, Connection, Database, Error, ErrorCode, ExecuteSummary, OpenOptions, Result, Row,
    Statement, Step, Value, ValueRef, params,
};

/// Default maximum concurrent blocking tasks (mirrors sqlx default).
const DEFAULT_MAX_CONNECTIONS: usize = 10;

/// Default busy-timeout applied to each acquired connection.
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// AsyncRow — owned row that survives .await boundaries
// ---------------------------------------------------------------------------

/// Owned row materialized from a borrowed [`redlinedb::Row`].
///
/// `Value` is `Clone + Send + Sync` so the row can safely cross `.await`
/// points and be sent across tasks. Column-name lookup is supported when the
/// originating query exposes its column metadata.
#[derive(Debug, Clone)]
pub struct AsyncRow {
    columns: Vec<Value>,
    names: Arc<[String]>,
}

impl AsyncRow {
    /// Number of columns in the row.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// True iff [`AsyncRow::len`] is zero (no columns).
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Borrow the raw owned [`Value`] at `index`.
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.columns.get(index)
    }

    /// Move the owned [`Value`] at `index` out of the row.
    pub fn take(mut self, index: usize) -> Option<Value> {
        if index >= self.columns.len() {
            return None;
        }
        Some(self.columns.swap_remove(index))
    }

    /// Column name at `index` if metadata was available.
    pub fn column_name(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    /// Iterate over the column names.
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Try to read an `i64` from `index`. Errors if the column is missing or
    /// not an integer.
    pub fn try_get_i64(&self, index: usize) -> Result<i64> {
        match self.get_required(index)? {
            Value::Integer(v) => Ok(*v),
            other => Err(mismatch(index, "Integer", other)),
        }
    }

    /// Try to read an `f64` from `index`. Errors if the column is missing or
    /// not a real.
    pub fn try_get_f64(&self, index: usize) -> Result<f64> {
        match self.get_required(index)? {
            Value::Real(v) => Ok(*v),
            other => Err(mismatch(index, "Real", other)),
        }
    }

    /// Try to read a borrowed text slice from `index`. Errors if the column is
    /// missing or not text.
    pub fn try_get_text(&self, index: usize) -> Result<&str> {
        match self.get_required(index)? {
            Value::Text(v) => Ok(v.as_ref()),
            other => Err(mismatch(index, "Text", other)),
        }
    }

    /// Try to read a borrowed blob slice from `index`. Errors if the column is
    /// missing or not a blob.
    pub fn try_get_blob(&self, index: usize) -> Result<&[u8]> {
        match self.get_required(index)? {
            Value::Blob(v) => Ok(v.as_ref()),
            other => Err(mismatch(index, "Blob", other)),
        }
    }

    /// Try to read an optional `i64`. Returns `Ok(None)` for SQL NULL.
    pub fn try_get_optional_i64(&self, index: usize) -> Result<Option<i64>> {
        match self.get_required(index)? {
            Value::Null => Ok(None),
            Value::Integer(v) => Ok(Some(*v)),
            other => Err(mismatch(index, "Integer or Null", other)),
        }
    }

    /// Try to read an optional text slice. Returns `Ok(None)` for SQL NULL.
    pub fn try_get_optional_text(&self, index: usize) -> Result<Option<&str>> {
        match self.get_required(index)? {
            Value::Null => Ok(None),
            Value::Text(v) => Ok(Some(v.as_ref())),
            other => Err(mismatch(index, "Text or Null", other)),
        }
    }

    fn get_required(&self, index: usize) -> Result<&Value> {
        match self.columns.get(index) {
            Some(value) => Ok(value),
            None => Err(column_out_of_bounds(index, self.columns.len())),
        }
    }
}

fn column_out_of_bounds(index: usize, len: usize) -> Error {
    Error::new(
        ErrorCode::Range,
        format!("column index {index} out of bounds (row has {len} columns)"),
    )
}

fn mismatch(index: usize, expected: &str, got: &Value) -> Error {
    let kind = match got {
        Value::Null => "Null",
        Value::Integer(_) => "Integer",
        Value::Real(_) => "Real",
        Value::Text(_) => "Text",
        Value::Blob(_) => "Blob",
    };
    Error::new(
        ErrorCode::Mismatch,
        format!("column {index} is {kind}, expected {expected}"),
    )
}

// ---------------------------------------------------------------------------
// Pool — async wrapper over a thread-bounded set of blocking tasks
// ---------------------------------------------------------------------------

/// Async pool that runs every operation on a tokio blocking thread.
///
/// `Pool` is cheap to `Clone`; clones share the underlying [`Database`] and
/// semaphore. Drop the last clone to release the database.
#[derive(Clone)]
pub struct Pool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    db: Database,
    semaphore: Arc<Semaphore>,
    busy_timeout: Duration,
    max_connections: usize,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("max_permits", &self.inner.semaphore.available_permits())
            .field("busy_timeout", &self.inner.busy_timeout)
            .finish()
    }
}

impl Pool {
    /// Open a database file at `path`, creating it if it does not exist.
    ///
    /// Uses default pool settings ([`DEFAULT_MAX_CONNECTIONS`] permits and
    /// [`DEFAULT_BUSY_TIMEOUT`]).
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let db = tokio::task::spawn_blocking(move || Database::create(path))
            .await
            .map_err(join_err)??;
        Self::from_database(db)
    }

    /// Open an ephemeral in-memory database. Lifetime is tied to the returned
    /// `Pool` (and any clones).
    pub async fn open_in_memory() -> Result<Self> {
        let db = tokio::task::spawn_blocking(|| Database::create_in_memory(OpenOptions::default()))
            .await
            .map_err(join_err)??;
        Self::from_database(db)
    }

    /// Build a `Pool` from an already-opened [`Database`] using default
    /// settings.
    pub fn from_database(db: Database) -> Result<Self> {
        PoolBuilder::default().database(db).build()
    }

    /// Begin configuring a new pool. Call [`PoolBuilder::build`] to finalize.
    pub fn builder() -> PoolBuilder {
        PoolBuilder::default()
    }

    /// Borrow the underlying [`Database`] (for admin operations not exposed by
    /// `Pool` like `checkpoint`, `vacuum`, `backup`).
    pub fn database(&self) -> &Database {
        &self.inner.db
    }

    /// Maximum concurrent blocking tasks. Set via [`PoolBuilder::max_connections`].
    pub fn max_connections(&self) -> usize {
        self.inner.max_connections
    }

    /// Execute a statement, returning the number of rows affected / returned.
    pub async fn execute(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<ExecuteSummary> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || {
            let mut conn = db.connect()?;
            conn.set_busy_timeout(busy);
            conn.execute(&sql, params)
        })
        .await
    }

    /// Run `sql` and materialize all returned rows.
    pub async fn fetch_all(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<Vec<AsyncRow>> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || materialize_rows(&db, &sql, params, busy, None))
            .await
    }

    /// Run `sql` and materialize the first row. Errors with `ErrorCode::NotFound`
    /// when no row is returned.
    pub async fn fetch_one(&self, sql: impl Into<String>, params: Vec<Value>) -> Result<AsyncRow> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        let rows = self
            .run_blocking(move || materialize_rows(&db, &sql, params, busy, Some(1)))
            .await?;
        match rows.into_iter().next() {
            Some(row) => Ok(row),
            None => Err(Error::new(
                ErrorCode::NotFound,
                "fetch_one: query returned no rows",
            )),
        }
    }

    /// Run `sql` and materialize the first row if any. Returns `Ok(None)` for
    /// empty result sets.
    pub async fn fetch_optional(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<Option<AsyncRow>> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        let rows = self
            .run_blocking(move || materialize_rows(&db, &sql, params, busy, Some(1)))
            .await?;
        Ok(rows.into_iter().next())
    }

    /// Run a closure that needs a `Connection` for multi-step work (e.g.
    /// transactions where you want explicit BEGIN / COMMIT / ROLLBACK).
    ///
    /// The closure runs on a blocking thread and may not `.await`. Returned
    /// values must be `Send + 'static` so they can travel back to the caller.
    pub async fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut redlinedb::Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || {
            let mut conn = db.connect()?;
            conn.set_busy_timeout(busy);
            f(&mut conn)
        })
        .await
    }

    /// Run a closure inside an automatic transaction. Commits on `Ok`,
    /// rolls back on `Err` (or panic).
    pub async fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut redlinedb::Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        use redlinedb::BeginMode;
        self.with_connection(move |conn| {
            conn.begin(BeginMode::Deferred)?;
            match f(conn) {
                Ok(value) => {
                    conn.commit()?;
                    Ok(value)
                }
                Err(e) => {
                    let _ = conn.rollback();
                    Err(e)
                }
            }
        })
        .await
    }

    async fn run_blocking<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = Arc::clone(&self.inner.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| Error::new(ErrorCode::Internal, "pool semaphore closed"))?;
        tokio::task::spawn_blocking(move || {
            let result = f();
            drop(permit);
            result
        })
        .await
        .map_err(join_err)?
    }
}

fn materialize_rows(
    db: &Database,
    sql: &str,
    params: Vec<Value>,
    busy: Duration,
    limit: Option<usize>,
) -> Result<Vec<AsyncRow>> {
    use redlinedb::Step;
    let mut conn = db.connect()?;
    conn.set_busy_timeout(busy);
    let mut stmt = conn.prepare(sql)?;
    stmt.bind_all(params)?;
    let column_count = stmt.column_count();
    let mut names: Vec<String> = Vec::with_capacity(column_count);
    for i in 0..column_count {
        names.push(stmt.column_name(i).to_owned());
    }
    let names_arc: Arc<[String]> = names.into();
    let mut out = Vec::new();
    let cap = limit.unwrap_or(usize::MAX);
    while let Step::Row(row) = stmt.step()? {
        if out.len() >= cap {
            break;
        }
        let mut columns: Vec<Value> = Vec::with_capacity(column_count);
        for i in 0..column_count {
            columns.push(row.get::<Value>(i)?);
        }
        out.push(AsyncRow {
            columns,
            names: Arc::clone(&names_arc),
        });
    }
    Ok(out)
}

fn join_err(err: tokio::task::JoinError) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("redlinedb-tokio blocking task panicked: {err}"),
    )
}

// ---------------------------------------------------------------------------
// PoolBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`Pool`].
pub struct PoolBuilder {
    db: Option<Database>,
    max_connections: usize,
    busy_timeout: Duration,
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self {
            db: None,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }
}

impl PoolBuilder {
    /// Use the provided [`Database`] handle. Required.
    pub fn database(mut self, db: Database) -> Self {
        self.db = Some(db);
        self
    }

    /// Maximum number of concurrent blocking tasks. Defaults to
    /// [`DEFAULT_MAX_CONNECTIONS`]. Setting to zero panics.
    pub fn max_connections(mut self, max: usize) -> Self {
        assert!(max > 0, "max_connections must be > 0");
        self.max_connections = max;
        self
    }

    /// Per-connection busy timeout. Defaults to [`DEFAULT_BUSY_TIMEOUT`].
    pub fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// Construct the [`Pool`].
    pub fn build(self) -> Result<Pool> {
        let db = match self.db {
            Some(db) => db,
            None => {
                return Err(Error::new(
                    ErrorCode::Misuse,
                    "PoolBuilder requires a Database",
                ));
            }
        };
        Ok(Pool {
            inner: Arc::new(PoolInner {
                db,
                semaphore: Arc::new(Semaphore::new(self.max_connections)),
                busy_timeout: self.busy_timeout,
                max_connections: self.max_connections,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn pool_is_send_sync() {
        assert_send::<Pool>();
        assert_sync::<Pool>();
    }

    #[test]
    fn async_row_is_send_sync() {
        assert_send::<AsyncRow>();
        assert_sync::<AsyncRow>();
    }
}
