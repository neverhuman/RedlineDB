use std::time::Duration;

use hdrhistogram::Histogram;

use crate::report::LatencySummary;

/// Disjoint failure classes recorded by the workload runner. Reviewer
/// Finding #7 called out that the bench harness was collapsing LOCKED
/// into BUSY, which hid contention vs. lock-wait pathology in the
/// telemetry. The classes are intentionally non-overlapping: the
/// workload classifier picks at most one bucket per error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Busy,
    Locked,
    Timeout,
    Other,
}

#[derive(Debug)]
pub struct Metrics {
    latency: Histogram<u64>,
    operations: u64,
    failures: u64,
    busy_errors: u64,
    locked_errors: u64,
    timeout_errors: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            latency: Histogram::new(3).expect("histogram"),
            operations: 0,
            failures: 0,
            busy_errors: 0,
            locked_errors: 0,
            timeout_errors: 0,
        }
    }

    pub fn record_success(&mut self, elapsed: Duration) {
        self.operations += 1;
        let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        let _ = self.latency.record(micros.max(1));
    }

    pub fn record_failure(&mut self, kind: FailureKind) {
        self.failures += 1;
        match kind {
            FailureKind::Busy => self.busy_errors += 1,
            FailureKind::Locked => self.locked_errors += 1,
            FailureKind::Timeout => self.timeout_errors += 1,
            FailureKind::Other => {}
        }
    }

    pub fn operations(&self) -> u64 {
        self.operations
    }

    pub fn failures(&self) -> u64 {
        self.failures
    }

    pub fn busy_errors(&self) -> u64 {
        self.busy_errors
    }

    pub fn locked_errors(&self) -> u64 {
        self.locked_errors
    }

    pub fn timeout_errors(&self) -> u64 {
        self.timeout_errors
    }

    pub fn latency(&self) -> LatencySummary {
        LatencySummary {
            p50_us: self.latency.value_at_quantile(0.50),
            p95_us: self.latency.value_at_quantile(0.95),
            p99_us: self.latency.value_at_quantile(0.99),
            p999_us: self.latency.value_at_quantile(0.999),
            max_us: self.latency.max(),
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.operations += other.operations;
        self.failures += other.failures;
        self.busy_errors += other.busy_errors;
        self.locked_errors += other.locked_errors;
        self.timeout_errors += other.timeout_errors;
        let _ = self.latency.add(&other.latency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_summary_tracks_percentiles() {
        let mut metrics = Metrics::new();
        metrics.record_success(Duration::from_micros(10));
        metrics.record_success(Duration::from_micros(20));
        metrics.record_success(Duration::from_micros(30));
        let latency = metrics.latency();
        assert!(latency.p50_us >= 10);
        assert!(latency.p999_us >= 30);
        assert_eq!(latency.max_us, 30);
    }

    #[test]
    fn metrics_split_busy_locked_timeout() {
        let mut metrics = Metrics::new();
        metrics.record_failure(FailureKind::Busy);
        metrics.record_failure(FailureKind::Busy);
        metrics.record_failure(FailureKind::Locked);
        metrics.record_failure(FailureKind::Timeout);
        metrics.record_failure(FailureKind::Timeout);
        metrics.record_failure(FailureKind::Timeout);
        metrics.record_failure(FailureKind::Other);

        assert_eq!(metrics.failures(), 7);
        assert_eq!(metrics.busy_errors(), 2);
        assert_eq!(metrics.locked_errors(), 1);
        assert_eq!(metrics.timeout_errors(), 3);
        // The "other" failure must not leak into any of the named buckets.
        assert_eq!(
            metrics.busy_errors() + metrics.locked_errors() + metrics.timeout_errors(),
            6
        );
    }

    #[test]
    fn metrics_merge_aggregates_each_counter_independently() {
        let mut a = Metrics::new();
        a.record_failure(FailureKind::Busy);
        a.record_failure(FailureKind::Locked);
        let mut b = Metrics::new();
        b.record_failure(FailureKind::Locked);
        b.record_failure(FailureKind::Timeout);
        a.merge(&b);
        assert_eq!(a.failures(), 4);
        assert_eq!(a.busy_errors(), 1);
        assert_eq!(a.locked_errors(), 2);
        assert_eq!(a.timeout_errors(), 1);
    }
}
