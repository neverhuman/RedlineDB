//! WS-C6: cross-lane WAL flush coalescer.
//!
//! Dedicated worker thread that batches flush requests across
//! every WAL lane and issues one `flush_until` per lane per batch.
//! When N waiters all request a flush concurrently, the worker
//! collapses them into a single per-lane fsync covering the union
//! of pending LSNs.
//!
//! # Feature gate
//!
//! Wired up only when the `wal_cross_lane_coalescer` feature is
//! enabled. Without it, [`WalLaneCoordinator`] keeps the historical
//! per-lane direct-flush behaviour byte-for-byte.
//!
//! # Correctness
//!
//! * Lane append ordering is untouched: the coalescer batches
//!   FLUSHES, never APPENDS. A flush request only resolves once the
//!   underlying lane reports `durable_lsn >= target_lsn`.
//! * Shutdown drains every queued request and performs the final
//!   per-lane flush before the worker exits.
//! * Worker panic / poison: callers fall back to invoking the lane's
//!   direct `flush_until` so durability is never lost.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;

use crate::format::Lsn;
use crate::wal::combiner::cross_lane_union;
use crate::wal::manager::WalCoordinator;
use crate::{Error, Result};

/// Default batching window. 100us is conservative: short enough
/// that single-writer latency is not visibly worsened, long enough
/// that bursts from many lanes can join one batch.
pub const DEFAULT_MAX_BATCH_US: u32 = 100;

/// Bounded queue capacity. Overflowing pushes fall back to the
/// direct per-lane flush path — the coalescer is best-effort, never
/// a durability gate.
const QUEUE_CAPACITY: usize = 4096;

/// Per-request handle. Each waiter owns its own slot so the worker
/// can publish `done` independently. Plain `(Mutex, Condvar)` keeps
/// the dependency surface within `std`.
#[derive(Debug)]
struct LaneFlushSlot {
    state: std::sync::Mutex<LaneFlushSlotState>,
    cvar: std::sync::Condvar,
}

#[derive(Debug, Default)]
struct LaneFlushSlotState {
    done: bool,
    failed: bool,
}

#[derive(Debug)]
struct LaneFlushReq {
    lane_idx: usize,
    target_lsn: Lsn,
    slot: Arc<LaneFlushSlot>,
}

struct CoalescerShared {
    pending: ArrayQueue<LaneFlushReq>,
    shutdown: AtomicBool,
    panicked: AtomicBool,
    wake: std::sync::Mutex<()>,
    wake_cv: std::sync::Condvar,
    lanes: Vec<Arc<WalCoordinator>>,
    max_batch_us: u32,
}

/// Cross-lane WAL flush coalescer. Created at WAL init when the
/// `wal_cross_lane_coalescer` feature is enabled.
pub struct WalCoalescer {
    shared: Arc<CoalescerShared>,
    worker: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for WalCoalescer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalCoalescer")
            .field("lanes", &self.shared.lanes.len())
            .field("max_batch_us", &self.shared.max_batch_us)
            .field("panicked", &self.shared.panicked.load(Ordering::Relaxed))
            .finish()
    }
}

impl WalCoalescer {
    pub fn new(lanes: Vec<Arc<WalCoordinator>>, max_batch_us: u32) -> Self {
        let shared = Arc::new(CoalescerShared {
            pending: ArrayQueue::new(QUEUE_CAPACITY),
            shutdown: AtomicBool::new(false),
            panicked: AtomicBool::new(false),
            wake: std::sync::Mutex::new(()),
            wake_cv: std::sync::Condvar::new(),
            lanes,
            max_batch_us,
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("redlinedb-wal-coalescer".to_owned())
            .spawn(move || coalescer_worker(worker_shared))
            .expect("spawn wal coalescer worker");
        Self {
            shared,
            worker: std::sync::Mutex::new(Some(worker)),
        }
    }

    pub fn lane_count(&self) -> usize {
        self.shared.lanes.len()
    }

    pub fn is_panicked(&self) -> bool {
        self.shared.panicked.load(Ordering::Acquire)
    }

    /// Enqueue a flush request and block until the worker reports
    /// durability. Falls back to direct per-lane flush whenever the
    /// worker is unavailable. Durability is preserved either way.
    pub fn flush_until(&self, lane_idx: usize, target_lsn: Lsn) -> Result<Lsn> {
        let coord = self
            .shared
            .lanes
            .get(lane_idx)
            .ok_or(Error::CorruptWal("lane index out of range"))?;

        if self.shared.panicked.load(Ordering::Acquire)
            || self.shared.shutdown.load(Ordering::Acquire)
        {
            return coord.flush_until(target_lsn);
        }

        let slot = Arc::new(LaneFlushSlot {
            state: std::sync::Mutex::new(LaneFlushSlotState::default()),
            cvar: std::sync::Condvar::new(),
        });
        let req = LaneFlushReq {
            lane_idx,
            target_lsn,
            slot: Arc::clone(&slot),
        };
        if self.shared.pending.push(req).is_err() {
            return coord.flush_until(target_lsn);
        }
        {
            let _guard = match self.shared.wake.lock() {
                Ok(g) => g,
                Err(_) => return coord.flush_until(target_lsn),
            };
            self.shared.wake_cv.notify_one();
        }

        let mut state = match slot.state.lock() {
            Ok(s) => s,
            Err(_) => return coord.flush_until(target_lsn),
        };
        loop {
            if state.done {
                return coord.durable_lsn();
            }
            if state.failed || self.shared.panicked.load(Ordering::Acquire) {
                return coord.flush_until(target_lsn);
            }
            let (next, _) = slot
                .cvar
                .wait_timeout(state, Duration::from_millis(50))
                .map_err(|_| Error::CorruptWal("coalescer slot wait poisoned"))?;
            state = next;
        }
    }

    pub fn shutdown(&self) {
        self.shared.shutdown.store(true, Ordering::Release);
        if let Ok(_g) = self.shared.wake.lock() {
            self.shared.wake_cv.notify_all();
        }
        if let Ok(mut slot) = self.worker.lock()
            && let Some(handle) = slot.take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for WalCoalescer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn coalescer_worker(shared: Arc<CoalescerShared>) {
    let guard = PanicGuard {
        shared: Arc::clone(&shared),
    };
    coalescer_worker_inner(&shared);
    drop(guard);
}

fn coalescer_worker_inner(shared: &Arc<CoalescerShared>) {
    let lane_count = shared.lanes.len();
    let mut per_lane_target: Vec<u64> = vec![0; lane_count];
    let mut batch: Vec<LaneFlushReq> = Vec::with_capacity(QUEUE_CAPACITY);

    loop {
        batch.clear();

        let mut drained = false;
        while let Some(req) = shared.pending.pop() {
            drained = true;
            batch.push(req);
        }

        if !drained {
            if shared.shutdown.load(Ordering::Acquire) {
                return;
            }
            let wait = Duration::from_micros(shared.max_batch_us.max(1) as u64);
            let guard = match shared.wake.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let (_g, _) = match shared.wake_cv.wait_timeout(guard, wait) {
                Ok(v) => v,
                Err(_) => return,
            };
            continue;
        }

        // Extend the batching window briefly so late arrivals join
        // this train.
        if shared.max_batch_us > 0 {
            thread::sleep(Duration::from_micros(shared.max_batch_us as u64));
            while let Some(req) = shared.pending.pop() {
                batch.push(req);
            }
        }

        // Compute the per-lane union via the pure helper in
        // wal::combiner so the aggregation step stays testable.
        let _touched = cross_lane_union(
            batch.iter().map(|r| (r.lane_idx, r.target_lsn.0)),
            &mut per_lane_target,
        );

        let mut lane_failures: Vec<bool> = vec![false; lane_count];
        for idx in 0..lane_count {
            if per_lane_target[idx] == 0 {
                continue;
            }
            let target = Lsn(per_lane_target[idx]);
            if shared.lanes[idx].flush_until(target).is_err() {
                lane_failures[idx] = true;
            }
        }

        for req in batch.drain(..) {
            let failed = req.lane_idx >= lane_count || lane_failures[req.lane_idx];
            if let Ok(mut state) = req.slot.state.lock() {
                if failed {
                    state.failed = true;
                } else {
                    state.done = true;
                }
                req.slot.cvar.notify_all();
            }
        }
    }
}

/// On thread unwind, mark the coalescer panicked and notify every
/// remaining waiter so they fall back to direct flush.
struct PanicGuard {
    shared: Arc<CoalescerShared>,
}

impl Drop for PanicGuard {
    fn drop(&mut self) {
        if thread::panicking() {
            self.shared.panicked.store(true, Ordering::Release);
            while let Some(req) = self.shared.pending.pop() {
                if let Ok(mut state) = req.slot.state.lock() {
                    state.failed = true;
                    req.slot.cvar.notify_all();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::wal::manager::{WalConfig, WalCoordinator};

    fn lane_coord(dir: &std::path::Path) -> Arc<WalCoordinator> {
        Arc::new(WalCoordinator::create(dir, WalConfig::default()).unwrap())
    }

    #[test]
    fn flush_until_runs_through_worker() {
        let tmp = TempDir::new().unwrap();
        let coord = lane_coord(tmp.path());
        let coalescer = WalCoalescer::new(vec![coord], DEFAULT_MAX_BATCH_US);
        let durable = coalescer.flush_until(0, Lsn::ZERO).unwrap();
        assert_eq!(durable, Lsn::ZERO);
    }

    #[test]
    fn out_of_range_lane_errors() {
        let tmp = TempDir::new().unwrap();
        let coord = lane_coord(tmp.path());
        let coalescer = WalCoalescer::new(vec![coord], DEFAULT_MAX_BATCH_US);
        assert!(coalescer.flush_until(7, Lsn::ZERO).is_err());
    }
}
