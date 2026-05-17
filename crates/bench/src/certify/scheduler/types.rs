use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};

use crate::report::RunRecord;

use crate::config::RunSpec;

/// Lane BH P0 #1: cores reserved for the OS / parent harness when
/// the parallel certify scheduler decides how many children to
/// spawn concurrently. Empirically xbabe1's 128-core box loses too
/// much accuracy when the scheduler allocates the entire machine
/// to children, leaving no headroom for the parent's own bookkeeping
/// and OS jitter.
pub const RESERVED_CORES: usize = 4;
pub const MAX_PARALLEL_THREADS_ENV: &str = "REDLINEDB_BENCH_MAX_PARALLEL_THREADS";

/// Polling interval for the parallel scheduler's `try_wait` loop.
///
/// Small enough that finished children are reaped promptly; large
/// enough that the busy loop does not noticeably steal CPU from
/// running children.
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A scheduled benchmark child the parallel scheduler will dispatch.
///
/// `is_warmup == true` means the resulting `RunRecord` is discarded
/// after the child exits; we still need to allocate a slot in the
/// scheduler so cache/disk priming actually happens.
#[derive(Debug, Clone)]
pub struct Job {
    pub spec: RunSpec,
    pub rep_idx: usize,
    pub is_warmup: bool,
}

/// Lightweight summary returned by [`dispatch_parallel_with_spawner`].
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    /// Total number of children that reached `Ok(_)` exit status.
    pub reaped: usize,
    /// Peak sum of `threads` across all simultaneously in-flight
    /// children. Compared against `available` to confirm the
    /// scheduler is actually parallelizing.
    pub max_in_flight_threads: usize,
    /// Wall-clock time from the first dispatch to the last reap.
    pub elapsed: Duration,
}

/// In-flight child slot tracked by the scheduler.
pub(super) struct InFlight {
    pub child: Child,
    pub threads_used: usize,
    pub job: Job,
    pub out_path: PathBuf,
    pub run_dir: PathBuf,
    pub strace_path: Option<PathBuf>,
    pub wrap_with_strace: bool,
    /// Index in the original job queue, used to preserve a stable
    /// output order across reruns even when the scheduler
    /// completes children out of dispatch order.
    pub queue_index: usize,
}

/// The aggregated outcome of a finalized child plus warmup flag.
#[derive(Debug)]
pub struct ScheduledOutcome {
    pub record: RunRecord,
    pub strace_path: Option<PathBuf>,
    pub is_warmup: bool,
    pub queue_index: usize,
}
