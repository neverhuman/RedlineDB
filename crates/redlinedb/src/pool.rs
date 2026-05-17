//! Connection pool. Available always; async acquisition is gated by the
//! `tokio` feature.
//!
//! ```no_run
//! # fn example() -> redlinedb::Result<()> {
//! use redlinedb::{Database, Pool};
//!
//! let db = Database::create("/tmp/example.redline")?;
//! let pool = Pool::builder(db).max_connections(8).build()?;
//!
//! {
//!     let mut conn = pool.get()?;
//!     conn.execute("CREATE TABLE t(id INTEGER)", ())?;
//! } // connection returned to the pool on drop
//! # Ok(())
//! # }
//! ```

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::connection::Connection;
use crate::error::{Error, ErrorCode, Result};
use crate::handle::Database;
use crate::metrics::{MetricResult, Metrics, NoopMetrics};

const DEFAULT_MAX_CONNECTIONS: usize = 16;

/// Configurable builder for [`Pool`].
pub struct PoolBuilder {
    db: Database,
    max_connections: usize,
    busy_timeout: Option<Duration>,
    metrics: Arc<dyn Metrics>,
}

impl PoolBuilder {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            busy_timeout: None,
            metrics: Arc::new(NoopMetrics),
        }
    }

    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max.max(1);
        self
    }

    #[must_use]
    pub fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = Some(timeout);
        self
    }

    /// Attach a [`Metrics`] sink. Defaults to [`NoopMetrics`] (zero
    /// overhead). All pool acquires fire `on_pool_acquire`; future
    /// connection-level events will share the same sink.
    #[must_use]
    pub fn metrics(mut self, metrics: Arc<dyn Metrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn build(self) -> Result<Pool> {
        Ok(Pool {
            inner: Arc::new(PoolInner {
                db: self.db,
                state: Mutex::new(PoolState {
                    idle: Vec::with_capacity(self.max_connections),
                    in_use: 0,
                }),
                cond: Condvar::new(),
                max_connections: self.max_connections,
                busy_timeout: self.busy_timeout,
                metrics: self.metrics,
            }),
        })
    }
}

/// Shared, cloneable connection pool. Hands out per-call [`PooledConnection`]
/// guards that return the connection on `Drop`.
#[derive(Clone)]
pub struct Pool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    db: Database,
    state: Mutex<PoolState>,
    cond: Condvar,
    max_connections: usize,
    busy_timeout: Option<Duration>,
    metrics: Arc<dyn Metrics>,
}

struct PoolState {
    idle: Vec<Connection>,
    in_use: usize,
}

impl Pool {
    pub fn builder(db: Database) -> PoolBuilder {
        PoolBuilder::new(db)
    }

    /// Acquire a connection from the pool. Blocks the calling thread when
    /// the pool is at capacity. Returns a guard that returns the connection
    /// to the pool on drop.
    pub fn get(&self) -> Result<PooledConnection> {
        let start = Instant::now();
        let result = self.get_inner();
        let metric_result = match &result {
            Ok(_) => MetricResult::Ok,
            Err(_) => MetricResult::Err,
        };
        self.inner
            .metrics
            .on_pool_acquire(start.elapsed(), metric_result);
        result
    }

    fn get_inner(&self) -> Result<PooledConnection> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| poison_error("pool state"))?;
        loop {
            if let Some(conn) = state.idle.pop() {
                state.in_use += 1;
                return Ok(PooledConnection {
                    pool: Arc::clone(&self.inner),
                    conn: Some(conn),
                });
            }
            if state.in_use < self.inner.max_connections {
                state.in_use += 1;
                drop(state);
                return match self.inner.db.connect() {
                    Ok(mut conn) => {
                        if let Some(timeout) = self.inner.busy_timeout {
                            conn.set_busy_timeout(timeout);
                        }
                        Ok(PooledConnection {
                            pool: Arc::clone(&self.inner),
                            conn: Some(conn),
                        })
                    }
                    Err(err) => {
                        if let Ok(mut state) = self.inner.state.lock() {
                            state.in_use -= 1;
                            self.inner.cond.notify_one();
                        }
                        Err(err)
                    }
                };
            }
            state = self
                .inner
                .cond
                .wait(state)
                .map_err(|_| poison_error("pool condvar"))?;
        }
    }

    /// Number of connections currently checked out (in use).
    pub fn in_use(&self) -> usize {
        self.inner
            .state
            .lock()
            .map(|s| s.in_use)
            .unwrap_or_default()
    }

    /// Number of connections currently idle in the pool.
    pub fn idle(&self) -> usize {
        self.inner
            .state
            .lock()
            .map(|s| s.idle.len())
            .unwrap_or_default()
    }

    /// Maximum total connections the pool may hand out.
    pub fn max_connections(&self) -> usize {
        self.inner.max_connections
    }
}

/// RAII guard returned by [`Pool::get`] / [`Pool::acquire`]. Returns the
/// inner connection to the pool when dropped.
pub struct PooledConnection {
    pool: Arc<PoolInner>,
    conn: Option<Connection>,
}

impl Deref for PooledConnection {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("connection always present until drop")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect("connection always present until drop")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if let Ok(mut state) = self.pool.state.lock() {
                state.idle.push(conn);
                state.in_use = state.in_use.saturating_sub(1);
                self.pool.cond.notify_one();
            }
        }
    }
}

fn poison_error(what: &'static str) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("redlinedb pool {what} mutex poisoned"),
    )
}

/// Async-friendly variant of [`Pool::get`] that performs acquisition via
/// `tokio::task::spawn_blocking`. The returned [`PooledConnection`] is
/// the same type as the sync `get()` and returns to the pool on drop.
#[cfg(feature = "tokio")]
impl Pool {
    pub async fn acquire(&self) -> Result<PooledConnection> {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || pool.get())
            .await
            .map_err(|join_err| Error::new(ErrorCode::Internal, join_err.to_string()))?
    }
}
