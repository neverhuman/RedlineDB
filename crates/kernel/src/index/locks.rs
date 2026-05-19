use std::collections::hash_map::Entry as HashEntry;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Instant;

use crate::engine::lock_fifo;
use crate::telemetry::{Phase11Counters, phase11_bucket_index};
use crate::{Error, Result};

#[derive(Debug, Default)]
pub struct UniqueKeyLockTable {
    shards: Vec<Mutex<HashMap<Vec<u8>, UniqueKeyLockState>>>,
    /// Optional Phase 11 telemetry sink. Installed post-construction
    /// via `set_phase11_counters` so the public `new` signature stays
    /// stable.
    phase11: RwLock<Option<Arc<Phase11Counters>>>,
}

#[derive(Debug, Default)]
struct UniqueKeyLockState {
    owner: Option<u64>,
    depth: usize,
    /// FIFO of parked waiters, each holding its own `Condvar` so the
    /// lock holder can wake exactly the next-in-line on release rather
    /// than firing a shard-wide `notify_all` and letting every waiter
    /// re-contend.
    waiters: VecDeque<Arc<Condvar>>,
}

/// Owns its lock table via `Arc` so the guard can be stored across SQL-side
/// transaction state (e.g. inside `SessionState`) without lifetime gymnastics.
/// The guard's `Drop` releases the per-key reservation for `owner`.
#[derive(Debug)]
pub struct UniqueKeyGuard {
    table: Arc<UniqueKeyLockTable>,
    shard: usize,
    key: Vec<u8>,
    owner: u64,
}

impl UniqueKeyLockTable {
    pub fn new(shards: usize) -> Self {
        let shards = shards.max(1);
        let mut locks = Vec::with_capacity(shards);
        for _ in 0..shards {
            locks.push(Mutex::new(HashMap::new()));
        }
        Self {
            shards: locks,
            phase11: RwLock::new(None),
        }
    }

    /// Install a Phase 11 telemetry sink. Existing call sites continue
    /// to work without ever invoking this; only owners that want
    /// `lock_wait_us_buckets` data wire it in.
    pub fn set_phase11_counters(&self, counters: Arc<Phase11Counters>) {
        *self
            .phase11
            .write()
            .expect("unique lock phase11 sink poisoned") = Some(counters);
    }

    pub fn lock(self: &Arc<Self>, key: &[u8], owner: u64) -> Result<UniqueKeyGuard> {
        let shard = self.shard(key);
        let mut map = self.shards[shard]
            .lock()
            .map_err(|_| Error::CorruptPage("unique lock shard poisoned"))?;
        let key = key.to_vec();

        // Fast path: free or re-entrant. No FIFO touch.
        let state = map.entry(key.clone()).or_default();
        if state.owner.is_none() {
            return Ok(self.grant_guard(state, shard, key, owner, 1));
        }
        if state.owner == Some(owner) {
            let depth = state.depth + 1;
            return Ok(self.grant_guard(state, shard, key, owner, depth));
        }

        // Slow path: enqueue ourselves and park on our own Condvar so
        // the holder's release wakes only the next-in-line.
        let my_cv = Arc::new(Condvar::new());
        state.waiters.push_back(Arc::clone(&my_cv));
        let wait_started = Instant::now();
        loop {
            map = my_cv
                .wait(map)
                .map_err(|_| Error::CorruptPage("unique lock wait poisoned"))?;
            let state = map.entry(key.clone()).or_default();
            let grant = lock_fifo::wake_decision(&mut state.waiters, state.owner, owner, &my_cv);
            if let Some(guard) =
                self.complete_wake(grant, state, shard, key.clone(), owner, wait_started)
            {
                return Ok(guard);
            }
        }
    }

    fn unlock(&self, shard: usize, key: Vec<u8>, owner: u64) {
        if let Ok(mut map) = self.shards[shard].lock() {
            match map.entry(key) {
                HashEntry::Occupied(mut entry) => {
                    let drop_entry = self.release_lock_state(entry.get_mut(), owner);
                    if drop_entry {
                        entry.remove();
                    }
                }
                HashEntry::Vacant(_) => {}
            }
        }
    }

    fn record_lock_wait_us(&self, elapsed: std::time::Duration) {
        if let Ok(slot) = self.phase11.read()
            && let Some(counters) = slot.as_ref()
        {
            let micros = elapsed.as_micros().min(u64::MAX as u128) as u64;
            let bucket = phase11_bucket_index(micros);
            counters.lock_wait_us_buckets[bucket].fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    fn shard(&self, key: &[u8]) -> usize {
        poly_hash_u64(key) as usize % self.shards.len().max(1)
    }

    fn build_guard(self: &Arc<Self>, shard: usize, key: Vec<u8>, owner: u64) -> UniqueKeyGuard {
        UniqueKeyGuard {
            table: Arc::clone(self),
            shard,
            key,
            owner,
        }
    }

    fn grant_guard(
        self: &Arc<Self>,
        state: &mut UniqueKeyLockState,
        shard: usize,
        key: Vec<u8>,
        owner: u64,
        depth: usize,
    ) -> UniqueKeyGuard {
        state.owner = Some(owner);
        state.depth = depth;
        self.build_guard(shard, key, owner)
    }

    fn complete_wake(
        self: &Arc<Self>,
        grant: lock_fifo::WakeDecision,
        state: &mut UniqueKeyLockState,
        shard: usize,
        key: Vec<u8>,
        owner: u64,
        wait_started: Instant,
    ) -> Option<UniqueKeyGuard> {
        match grant {
            lock_fifo::WakeDecision::AcquireFree => {
                self.record_lock_wait_us(wait_started.elapsed());
                Some(self.grant_guard(state, shard, key, owner, 1))
            }
            lock_fifo::WakeDecision::AcquireReentrant => {
                let depth = state.depth + 1;
                self.record_lock_wait_us(wait_started.elapsed());
                Some(self.grant_guard(state, shard, key, owner, depth))
            }
            lock_fifo::WakeDecision::Continue => None,
        }
    }

    fn release_lock_state(&self, state: &mut UniqueKeyLockState, owner: u64) -> bool {
        if state.owner != Some(owner) {
            return false;
        }
        match state.depth {
            0 => false,
            1 => {
                state.depth = 0;
                lock_fifo::release_owner(&mut state.owner, owner, &state.waiters)
            }
            depth => {
                state.depth = depth - 1;
                false
            }
        }
    }
}

// shared with sql via redlinedb-kernel::index::locks
/// Polynomial rolling hash with multiplier 131. Public so the SQL session
/// shard router (`crates/sql/src/session.rs`) can keep its sharding key
/// consistent with the kernel-side unique-key lock table; SQL imports this
/// to avoid carrying its own copy of the body.
pub fn poly_hash_u64(key: &[u8]) -> u64 {
    let mut hash = 0_u64;
    for byte in key {
        hash = hash.wrapping_mul(131).wrapping_add(*byte as u64);
    }
    hash
}

impl Drop for UniqueKeyGuard {
    fn drop(&mut self) {
        self.table
            .unlock(self.shard, std::mem::take(&mut self.key), self.owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;

    /// Four threads contend on the same unique key. Acquisition order
    /// must match enqueue order: per-key FIFO + targeted handoff.
    #[test]
    fn unique_key_fifo_handoff_preserves_enqueue_order() {
        let table = Arc::new(UniqueKeyLockTable::new(4));
        let key = b"row-key";

        // Holder takes the lock so the four contenders all park in
        // the FIFO.
        let holder = table.lock(key, 1).unwrap();

        let counter = Arc::new(AtomicU64::new(0));
        let acq_seq: Arc<[AtomicU64; 4]> = Arc::new([
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
        ]);

        let mut handles = Vec::new();
        for i in 0..4_u64 {
            let table = Arc::clone(&table);
            let counter = Arc::clone(&counter);
            let seq = Arc::clone(&acq_seq);
            handles.push(thread::spawn(move || {
                // Stagger so FIFO enqueue order is deterministic.
                thread::sleep(Duration::from_millis(50 * (i + 1)));
                let g = table.lock(key, 100 + i).unwrap();
                let pos = counter.fetch_add(1, Ordering::SeqCst);
                seq[i as usize].store(pos, Ordering::SeqCst);
                drop(g);
            }));
        }

        thread::sleep(Duration::from_millis(300));
        drop(holder);

        for h in handles {
            h.join().unwrap();
        }

        for i in 0..4 {
            assert_eq!(
                acq_seq[i].load(Ordering::SeqCst),
                i as u64,
                "thread {i} acquired out of FIFO order"
            );
        }
    }
}
