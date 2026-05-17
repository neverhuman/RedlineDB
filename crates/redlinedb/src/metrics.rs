//! Per-operation metrics hook.
//!
//! Consumers implement [`Metrics`] and pass `Arc<dyn Metrics>` to
//! [`crate::pool::PoolBuilder::metrics`]. The trait fires on every
//! query / execute / commit / rollback / pool acquire. The default
//! is [`NoopMetrics`] which compiles down to zero overhead.
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use std::time::Duration;
//! # use redlinedb::{Database, Pool};
//! use redlinedb::metrics::{Metrics, MetricResult};
//!
//! struct StdoutMetrics;
//! impl Metrics for StdoutMetrics {
//!     fn on_query(&self, sql: &str, duration: Duration, result: MetricResult) {
//!         println!("query {result:?} took {duration:?}: {sql}");
//!     }
//! }
//!
//! # fn example() -> redlinedb::Result<()> {
//! let db = Database::create("/tmp/m.redline")?;
//! let pool = Pool::builder(db)
//!     .max_connections(4)
//!     .metrics(Arc::new(StdoutMetrics))
//!     .build()?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

/// Result classification surfaced to [`Metrics`] callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricResult {
    Ok,
    Err,
}

/// Sink for redlinedb runtime metrics. Default impls do nothing so users
/// implement only what they want.
pub trait Metrics: Send + Sync + 'static {
    fn on_query(&self, _sql: &str, _duration: Duration, _result: MetricResult) {}
    fn on_execute(
        &self,
        _sql: &str,
        _duration: Duration,
        _rows_affected: u64,
        _result: MetricResult,
    ) {
    }
    fn on_commit(&self, _duration: Duration, _result: MetricResult) {}
    fn on_rollback(&self, _duration: Duration, _result: MetricResult) {}
    fn on_pool_acquire(&self, _wait: Duration, _result: MetricResult) {}
}

/// Default no-op sink. Selected when no `metrics` is configured.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMetrics;

impl Metrics for NoopMetrics {}
