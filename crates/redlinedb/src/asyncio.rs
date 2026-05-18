//! Built-in tokio async wrappers. Gated by the `tokio` feature.
//!
//! Bridges redlinedb's synchronous engine to async/await by running each
//! operation inside [`tokio::task::spawn_blocking`]. This lets consumers
//! avoid hand-rolling their own spawn_blocking + Arc<Mutex<Connection>>
//! scaffolding.
//!
//! ```no_run
//! # #[cfg(feature = "tokio")]
//! # async fn example() -> redlinedb::Result<()> {
//! use redlinedb::{AsyncDatabase, OpenOptions, Value};
//!
//! let db = AsyncDatabase::create("/tmp/example.redline").await?;
//! let conn = db.connect().await?;
//! conn.execute("CREATE TABLE t(id INTEGER)".into(), vec![]).await?;
//! conn.execute("INSERT INTO t VALUES (?)".into(), vec![Value::Integer(1)]).await?;
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::{Error, ErrorCode, Result};
use crate::handle::Database;
use crate::iter::FromRow;
use crate::options::{ExecuteSummary, OpenOptions};
use crate::value::Value;
use redlinedb_sql::BeginMode;

/// Async-friendly handle around [`Database`].
///
/// `Database` is `Send + Sync + Clone` (per redlinedb's public contracts);
/// this type only adds async fn entry points that spawn blocking work
/// onto a tokio runtime.
#[derive(Clone)]
pub struct AsyncDatabase {
    inner: Arc<Database>,
}

impl AsyncDatabase {
    pub async fn create(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let inner = spawn_blocking_err(move || Database::create(&path)).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let inner = spawn_blocking_err(move || Database::open(&path)).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub async fn open_with_options(path: impl Into<PathBuf>, options: OpenOptions) -> Result<Self> {
        let path = path.into();
        let inner = spawn_blocking_err(move || Database::open_with_options(&path, options)).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub async fn connect(&self) -> Result<AsyncConnection> {
        let db = Arc::clone(&self.inner);
        let conn = spawn_blocking_err(move || db.connect()).await?;
        Ok(AsyncConnection {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn into_inner(self) -> Arc<Database> {
        self.inner
    }
}

/// Async-friendly per-session connection. Internally stores the sync
/// [`Connection`] behind an `Arc<Mutex<_>>` so async tasks can drive it
/// concurrently (operations serialize per-connection, matching SQLite's
/// single-writer-per-handle semantics).
#[derive(Clone)]
pub struct AsyncConnection {
    inner: Arc<Mutex<Connection>>,
}

impl AsyncConnection {
    pub async fn execute(&self, sql: String, params: Vec<Value>) -> Result<ExecuteSummary> {
        let conn = Arc::clone(&self.inner);
        spawn_blocking_err(move || {
            let mut conn = conn.lock().map_err(|_| poison_error())?;
            conn.execute(&sql, params)
        })
        .await
    }

    pub async fn query_row<T>(&self, sql: String, params: Vec<Value>) -> Result<T>
    where
        T: FromRow + Send + 'static,
    {
        let conn = Arc::clone(&self.inner);
        spawn_blocking_err(move || {
            let mut conn = conn.lock().map_err(|_| poison_error())?;
            conn.query_row::<_, T>(&sql, params)
        })
        .await
    }

    pub async fn query_row_opt<T>(&self, sql: String, params: Vec<Value>) -> Result<Option<T>>
    where
        T: FromRow + Send + 'static,
    {
        let conn = Arc::clone(&self.inner);
        spawn_blocking_err(move || {
            let mut conn = conn.lock().map_err(|_| poison_error())?;
            conn.query_row_opt::<_, T>(&sql, params)
        })
        .await
    }

    /// Run a closure inside a transaction. The closure receives a `&mut
    /// Connection` and must return `Result<T>`. On success the transaction
    /// commits; on `Err`, on panic, or on any other early return the
    /// transaction rolls back.
    pub async fn transaction<T, F>(&self, mode: BeginMode, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let conn = Arc::clone(&self.inner);
        spawn_blocking_err(move || {
            let mut conn = conn.lock().map_err(|_| poison_error())?;
            conn.begin(mode)?;
            match f(&mut conn) {
                Ok(value) => {
                    conn.commit()?;
                    Ok(value)
                }
                Err(err) => {
                    let _ = conn.rollback();
                    Err(err)
                }
            }
        })
        .await
    }

    pub async fn set_busy_timeout(&self, timeout: Duration) -> Result<()> {
        let conn = Arc::clone(&self.inner);
        spawn_blocking_err(move || {
            let mut conn = conn.lock().map_err(|_| poison_error())?;
            conn.set_busy_timeout(timeout);
            Ok(())
        })
        .await
    }
}

fn poison_error() -> Error {
    Error::new(ErrorCode::Internal, "redlinedb connection mutex poisoned")
}

async fn spawn_blocking_err<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|join_err| Error::new(ErrorCode::Internal, join_err.to_string()))?
}
