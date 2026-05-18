use crate::format::{RelId, RowId, TxId};
use crate::telemetry::{Phase11Counters, phase11_bucket_index};
use crate::{Error, Result};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RowKey {
    pub(crate) rel_id: RelId,
    pub(crate) row_id: RowId,
}

/// Per-row lock state plus a FIFO queue of parked waiters. Each waiter
/// owns its own `Condvar` so the holder can wake exactly the next-in-line
/// waiter on `unlock` (`notify_one`) instead of waking every parked
/// thread (`notify_all`) and letting them re-contend on the shard mutex.
#[derive(Debug, Default)]
struct RowLockState {
    owner: Option<TxId>,
    /// Front of `waiters` is the next-in-line. Each waiter parks on its
    /// own `Condvar` so we can target exactly one wake-up on release.
    waiters: std::collections::VecDeque<Arc<Condvar>>,
}

#[derive(Debug, Default)]
struct LockShard {
    rows: Mutex<HashMap<RowKey, RowLockState>>,
}

#[derive(Debug)]
pub struct RowLockManager {
    shards: Vec<LockShard>,
    timeout: RwLock<Duration>,
    /// Optional Phase 11 telemetry sink. Populated post-construction by
    /// `set_phase11_counters` so the public `new` signature stays stable.
    phase11: RwLock<Option<Arc<Phase11Counters>>>,
}

impl RowLockManager {
    pub fn new(shard_count: usize, timeout: Duration) -> Self {
        let shard_count = shard_count.max(1);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(LockShard::default());
        }
        Self {
            shards,
            timeout: RwLock::new(timeout),
            phase11: RwLock::new(None),
        }
    }

    pub fn set_timeout(&self, timeout: Duration) {
        *self.timeout.write().expect("row lock timeout poisoned") = timeout;
    }

    /// Optional: install a Phase 11 telemetry sink. Existing call sites
    /// continue to work without ever invoking this; only the engine
    /// owner pipes its `Arc<Phase11Counters>` here so contended-acquire
    /// waits land in `lock_wait_us_buckets`.
    pub fn set_phase11_counters(&self, counters: Arc<Phase11Counters>) {
        *self
            .phase11
            .write()
            .expect("row lock phase11 sink poisoned") = Some(counters);
    }

    pub fn lock(&self, rel_id: RelId, row_id: RowId, tx_id: TxId) -> Result<()> {
        let key = RowKey { rel_id, row_id };
        let shard = self.shard(key);
        let timeout = *self.timeout.read().expect("row lock timeout poisoned");
        let deadline = Instant::now() + timeout;
        let mut rows = shard
            .rows
            .lock()
            .map_err(|_| Error::CorruptPage("row lock shard poisoned"))?;

        // Fast path: lock free or already held by us. Touches no FIFO.
        let state = rows.entry(key).or_default();
        match state.owner {
            None => {
                state.owner = Some(tx_id);
                return Ok(());
            }
            Some(owner) if owner == tx_id => return Ok(()),
            Some(_) => {}
        }

        // Slow path: enqueue ourselves onto the per-row FIFO and park
        // on our own Condvar. On release the owner pops the front of
        // the queue and wakes that one waiter.
        let my_cv = Arc::new(Condvar::new());
        state.waiters.push_back(Arc::clone(&my_cv));
        let wait_started = Instant::now();

        loop {
            let now = Instant::now();
            if now >= deadline {
                self.dequeue_waiter(&mut rows, key, &my_cv);
                self.record_lock_wait_us(wait_started.elapsed());
                return Err(Error::LockTimeout);
            }
            let wait = deadline.saturating_duration_since(now);
            let (next_rows, timed_out) = my_cv
                .wait_timeout(rows, wait)
                .map(|(g, t)| (g, t.timed_out()))
                .map_err(|_| Error::CorruptPage("row lock wait poisoned"))?;
            rows = next_rows;
            if timed_out {
                self.dequeue_waiter(&mut rows, key, &my_cv);
                self.record_lock_wait_us(wait_started.elapsed());
                return Err(Error::LockTimeout);
            }
            // Re-check ownership. Since unlock only wakes the front
            // waiter, getting a wake-up almost always means the lock is
            // ours — but `wait_timeout` is allowed to spuriously wake,
            // so we still loop on a contention check.
            let state = rows.entry(key).or_default();
            match state.owner {
                None => {
                    // Confirm the front-of-FIFO targeting: pop ourselves
                    // off the head if present, then take ownership.
                    if state
                        .waiters
                        .front()
                        .is_some_and(|cv| Arc::ptr_eq(cv, &my_cv))
                    {
                        state.waiters.pop_front();
                    } else {
                        // Spurious wake; ensure we're still parked.
                        if !state.waiters.iter().any(|cv| Arc::ptr_eq(cv, &my_cv)) {
                            state.waiters.push_back(Arc::clone(&my_cv));
                        }
                    }
                    state.owner = Some(tx_id);
                    self.record_lock_wait_us(wait_started.elapsed());
                    return Ok(());
                }
                Some(owner) if owner == tx_id => {
                    // Re-entrant acquire that snuck in — drop our slot.
                    self.dequeue_waiter_inner(state, &my_cv);
                    self.record_lock_wait_us(wait_started.elapsed());
                    return Ok(());
                }
                Some(_) => continue,
            }
        }
    }

    pub fn unlock(&self, rel_id: RelId, row_id: RowId, tx_id: TxId) {
        let key = RowKey { rel_id, row_id };
        let shard = self.shard(key);
        if let Ok(mut rows) = shard.rows.lock() {
            let drop_entry = if let Some(state) = rows.get_mut(&key) {
                if state.owner == Some(tx_id) {
                    state.owner = None;
                    // Targeted handoff: wake exactly the next-in-line
                    // waiter (if any). No-op if FIFO is empty.
                    if let Some(next) = state.waiters.front().cloned() {
                        next.notify_one();
                    }
                }
                state.owner.is_none() && state.waiters.is_empty()
            } else {
                false
            };
            if drop_entry {
                rows.remove(&key);
            }
        }
    }

    fn dequeue_waiter(
        &self,
        rows: &mut HashMap<RowKey, RowLockState>,
        key: RowKey,
        cv: &Arc<Condvar>,
    ) {
        if let Some(state) = rows.get_mut(&key) {
            self.dequeue_waiter_inner(state, cv);
            // If we removed the front waiter and the lock is currently
            // free, notify the new front so the FIFO keeps draining.
            if state.owner.is_none()
                && let Some(next) = state.waiters.front().cloned()
            {
                next.notify_one();
            }
            if state.owner.is_none() && state.waiters.is_empty() {
                rows.remove(&key);
            }
        }
    }

    fn dequeue_waiter_inner(&self, state: &mut RowLockState, cv: &Arc<Condvar>) {
        if let Some(pos) = state.waiters.iter().position(|c| Arc::ptr_eq(c, cv)) {
            state.waiters.remove(pos);
        }
    }

    fn record_lock_wait_us(&self, elapsed: Duration) {
        if let Ok(slot) = self.phase11.read()
            && let Some(counters) = slot.as_ref()
        {
            let micros = elapsed.as_micros().min(u64::MAX as u128) as u64;
            let bucket = phase11_bucket_index(micros);
            counters.lock_wait_us_buckets[bucket].fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    fn shard(&self, key: RowKey) -> &LockShard {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        &self.shards[hasher.finish() as usize % self.shards.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{RelId, RowId, TxId};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;

    /// Four threads contend on the same row. The acquisition order must
    /// match the FIFO enqueue order — i.e. each waiter parks on its own
    /// Condvar and the holder hands off to the front of the queue, not
    /// to whichever runtime-scheduler-favoured thread grabs the shard
    /// mutex first after a `notify_all`.
    #[test]
    fn fifo_handoff_preserves_enqueue_order() {
        let mgr = Arc::new(RowLockManager::new(4, Duration::from_secs(5)));
        let rel_id = RelId(7);
        let row_id = RowId(42);

        // Holder takes the lock first so the four contenders all park.
        let holder_tx = TxId(1);
        mgr.lock(rel_id, row_id, holder_tx).unwrap();

        let counter = Arc::new(AtomicU64::new(0));
        let acq_seq: Arc<[AtomicU64; 4]> = Arc::new([
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
        ]);

        let mut handles = Vec::new();
        for i in 0..4_u64 {
            // Stagger starts so the FIFO enqueue order is deterministic
            // even though the OS scheduler is free to reorder threads
            // before they reach the parked-on-Condvar state.
            let mgr = Arc::clone(&mgr);
            let counter = Arc::clone(&counter);
            let seq = Arc::clone(&acq_seq);
            handles.push(thread::spawn(move || {
                // Pause proportional to thread index so each thread
                // enters the FIFO at a distinct, ordered moment.
                thread::sleep(Duration::from_millis(50 * (i + 1)));
                mgr.lock(rel_id, row_id, TxId(100 + i)).unwrap();
                let pos = counter.fetch_add(1, Ordering::SeqCst);
                seq[i as usize].store(pos, Ordering::SeqCst);
                mgr.unlock(rel_id, row_id, TxId(100 + i));
            }));
        }

        // Give every waiter time to enqueue before we release the
        // holder's lock. 50ms * 4 + slack covers the staggered starts.
        thread::sleep(Duration::from_millis(300));
        mgr.unlock(rel_id, row_id, holder_tx);

        for h in handles {
            h.join().unwrap();
        }

        // Acquisition position must equal enqueue position 0..3.
        for i in 0..4 {
            assert_eq!(
                acq_seq[i].load(Ordering::SeqCst),
                i as u64,
                "thread {i} acquired out of FIFO order: {:?}",
                acq_seq
                    .iter()
                    .map(|s| s.load(Ordering::SeqCst))
                    .collect::<Vec<_>>(),
            );
        }
    }

    #[test]
    fn unlock_with_no_waiters_is_a_noop() {
        let mgr = RowLockManager::new(2, Duration::from_secs(1));
        let rel_id = RelId(1);
        let row_id = RowId(1);
        // No prior lock: unlock must not panic and must be a no-op.
        mgr.unlock(rel_id, row_id, TxId(99));
        // Now lock and unlock once with no waiters; subsequent lock
        // by a different tx must succeed immediately.
        mgr.lock(rel_id, row_id, TxId(1)).unwrap();
        mgr.unlock(rel_id, row_id, TxId(1));
        mgr.lock(rel_id, row_id, TxId(2)).unwrap();
        mgr.unlock(rel_id, row_id, TxId(2));
    }
}
