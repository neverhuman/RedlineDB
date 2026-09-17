//! Thread-safe metrics registry.
//!
//! Every query that flows through `POST /api/query` and table paging is
//! recorded here. The registry tracks:
//!
//! * total / failed query counters,
//! * a bounded ring of recent latencies, summarised as p50/p95/p99/max,
//! * a slow-query ring buffer (queries slower than the configured threshold),
//! * uptime (via [`Instant`]) and a rolling-window QPS estimate.
//!
//! All mutable state lives behind a single [`parking_lot::Mutex`]; the public
//! methods are cheap and never block on I/O.

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

use crate::model::{DbStats, LatencyMs, MetricsSnapshot, SlowQuery, TableSize};

/// Maximum number of latency samples retained for percentile estimation.
const LATENCY_SAMPLE_CAP: usize = 4096;
/// Maximum number of slow queries retained in the ring buffer.
const SLOW_RING_CAP: usize = 200;
/// Rolling window (seconds) over which QPS is estimated.
const QPS_WINDOW_SECS: f64 = 60.0;

/// Wall-clock milliseconds since the Unix epoch.
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug)]
struct Inner {
    total: u64,
    failed: u64,
    /// Recent latencies in milliseconds (most-recent-wins ring).
    latencies: VecDeque<f64>,
    /// Slow queries newest-last.
    slow: VecDeque<SlowQuery>,
    /// Timestamps of recent queries, for the rolling QPS window.
    recent: VecDeque<Instant>,
}

/// Process-wide metrics registry. Cheap to [`clone`](Clone) the `Arc` around it.
#[derive(Debug)]
pub struct MetricsRegistry {
    start: Instant,
    slow_threshold_ms: f64,
    inner: Mutex<Inner>,
}

impl MetricsRegistry {
    /// Create a registry; queries slower than `slow_threshold_ms` are ringed.
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self {
            start: Instant::now(),
            slow_threshold_ms: slow_threshold_ms as f64,
            inner: Mutex::new(Inner {
                total: 0,
                failed: 0,
                latencies: VecDeque::with_capacity(LATENCY_SAMPLE_CAP),
                slow: VecDeque::with_capacity(SLOW_RING_CAP),
                recent: VecDeque::new(),
            }),
        }
    }

    /// Record one executed query.
    ///
    /// `elapsed` is the measured wall-clock duration, `ok` whether it
    /// succeeded, and `row_count` the number of rows produced/affected (if
    /// known). Queries slower than the configured threshold are pushed onto the
    /// slow-query ring.
    pub fn record(&self, sql: &str, elapsed: &Duration, ok: bool, row_count: Option<i64>) {
        let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
        let now = Instant::now();
        let mut inner = self.inner.lock();

        inner.total += 1;
        if !ok {
            inner.failed += 1;
        }

        if inner.latencies.len() == LATENCY_SAMPLE_CAP {
            inner.latencies.pop_front();
        }
        inner.latencies.push_back(elapsed_ms);

        inner.recent.push_back(now);
        let cutoff = now
            .checked_sub(Duration::from_secs_f64(QPS_WINDOW_SECS))
            .unwrap_or(now);
        while let Some(front) = inner.recent.front() {
            if *front < cutoff {
                inner.recent.pop_front();
            } else {
                break;
            }
        }

        if elapsed_ms >= self.slow_threshold_ms {
            if inner.slow.len() == SLOW_RING_CAP {
                inner.slow.pop_front();
            }
            inner.slow.push_back(SlowQuery {
                sql: sql.to_string(),
                elapsed_ms,
                at_unix_ms: now_unix_ms(),
                row_count,
                ok,
            });
        }
    }

    /// Number of seconds the registry (process) has been running.
    pub fn uptime_secs(&self) -> u64 {
        self.start.elapsed().as_secs()
    }

    /// Snapshot of the slow-query ring, newest first, capped at `limit`.
    pub fn slow_queries(&self, limit: usize) -> Vec<SlowQuery> {
        let inner = self.inner.lock();
        inner.slow.iter().rev().take(limit).cloned().collect()
    }

    /// Current latency percentiles over the retained sample.
    fn latency_locked(inner: &Inner) -> LatencyMs {
        if inner.latencies.is_empty() {
            return LatencyMs::default();
        }
        let mut sorted: Vec<f64> = inner.latencies.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        LatencyMs {
            p50: percentile(&sorted, 50.0),
            p95: percentile(&sorted, 95.0),
            p99: percentile(&sorted, 99.0),
            max: *sorted.last().unwrap_or(&0.0),
        }
    }

    /// Rolling-window QPS estimate.
    fn qps_locked(&self, inner: &Inner) -> f64 {
        let count = inner.recent.len() as f64;
        let elapsed = self.start.elapsed().as_secs_f64();
        let window = QPS_WINDOW_SECS.min(elapsed).max(1.0);
        count / window
    }

    /// Build a full [`MetricsSnapshot`] from current counters plus the
    /// caller-supplied database statistics and table sizes.
    pub fn snapshot(&self, db: DbStats, tables: Vec<TableSize>) -> MetricsSnapshot {
        let inner = self.inner.lock();
        MetricsSnapshot {
            uptime_secs: self.start.elapsed().as_secs(),
            total_queries: inner.total,
            failed_queries: inner.failed,
            qps: self.qps_locked(&inner),
            latency_ms: Self::latency_locked(&inner),
            db,
            tables,
            at_unix_ms: now_unix_ms(),
        }
    }

    /// Prometheus text-exposition of the core counters and gauges.
    pub fn prometheus_text(&self) -> String {
        let inner = self.inner.lock();
        let latency = Self::latency_locked(&inner);
        let qps = self.qps_locked(&inner);
        let uptime = self.start.elapsed().as_secs();
        drop(inner);

        let mut out = String::new();
        let mut line = |s: String| out.push_str(&s);

        line("# HELP redline_web_queries_total Total queries executed.\n".to_string());
        line("# TYPE redline_web_queries_total counter\n".to_string());
        {
            let inner = self.inner.lock();
            line(format!("redline_web_queries_total {}\n", inner.total));
            line("# HELP redline_web_queries_failed_total Failed queries.\n".to_string());
            line("# TYPE redline_web_queries_failed_total counter\n".to_string());
            line(format!(
                "redline_web_queries_failed_total {}\n",
                inner.failed
            ));
        }

        line("# HELP redline_web_uptime_seconds Server uptime in seconds.\n".to_string());
        line("# TYPE redline_web_uptime_seconds gauge\n".to_string());
        line(format!("redline_web_uptime_seconds {uptime}\n"));

        line("# HELP redline_web_qps Queries per second over the rolling window.\n".to_string());
        line("# TYPE redline_web_qps gauge\n".to_string());
        line(format!("redline_web_qps {qps}\n"));

        line("# HELP redline_web_latency_ms Query latency percentiles (ms).\n".to_string());
        line("# TYPE redline_web_latency_ms gauge\n".to_string());
        line(format!(
            "redline_web_latency_ms{{quantile=\"0.5\"}} {}\n",
            latency.p50
        ));
        line(format!(
            "redline_web_latency_ms{{quantile=\"0.95\"}} {}\n",
            latency.p95
        ));
        line(format!(
            "redline_web_latency_ms{{quantile=\"0.99\"}} {}\n",
            latency.p99
        ));
        line(format!(
            "redline_web_latency_ms{{quantile=\"1.0\"}} {}\n",
            latency.max
        ));

        out
    }
}

/// Nearest-rank percentile over an ascending-sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    let rank = ((p / 100.0) * n as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(n - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_nearest_rank() {
        let data: Vec<f64> = (1..=100).map(|v| v as f64).collect();
        assert_eq!(percentile(&data, 50.0), 50.0);
        assert_eq!(percentile(&data, 95.0), 95.0);
        assert_eq!(percentile(&data, 99.0), 99.0);
        assert_eq!(percentile(&data, 100.0), 100.0);
    }

    #[test]
    fn records_and_counts() {
        let reg = MetricsRegistry::new(100);
        reg.record("select 1", &Duration::from_millis(5), true, Some(1));
        reg.record("boom", &Duration::from_millis(250), false, None);
        let snap = reg.snapshot(DbStats::default(), vec![]);
        assert_eq!(snap.total_queries, 2);
        assert_eq!(snap.failed_queries, 1);
        // The 250ms query exceeds the 100ms threshold.
        assert_eq!(reg.slow_queries(10).len(), 1);
        assert!(snap.latency_ms.max >= 250.0);
    }
}
