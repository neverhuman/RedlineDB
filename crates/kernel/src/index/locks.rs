use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Instant;

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
        match state.owner {
            None => {
                state.owner = Some(owner);
                state.depth = 1;
                return Ok(UniqueKeyGuard {
                    table: Arc::clone(self),
                    shard,
                    key,
                    owner,
                });
            }
            Some(current) if current == owner => {
                state.depth += 1;
                return Ok(UniqueKeyGuard {
                    table: Arc::clone(self),
                    shard,
                    key,
                    owner,
                });
            }
            Some(_) => {}
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
            match state.owner {
                None => {
                    // Pop from the head if we're the front of the FIFO.
                    if state
                        .waiters
                        .front()
                        .is_some_and(|cv| Arc::ptr_eq(cv, &my_cv))
                    {
                        state.waiters.pop_front();
                    } else if !state.waiters.iter().any(|cv| Arc::ptr_eq(cv, &my_cv)) {
                        // Spurious wake while we'd been removed; re-park.
                        state.waiters.push_back(Arc::clone(&my_cv));
                        continue;
                    }
                    state.owner = Some(owner);
                    state.depth = 1;
                    self.record_lock_wait_us(wait_started.elapsed());
                    return Ok(UniqueKeyGuard {
                        table: Arc::clone(self),
                        shard,
                        key,
                        owner,
                    });
                }
                Some(current) if current == owner => {
                    // Re-entrant grab snuck in — drop FIFO slot.
                    if let Some(pos) = state.waiters.iter().position(|cv| Arc::ptr_eq(cv, &my_cv)) {
                        state.waiters.remove(pos);
                    }
                    state.depth += 1;
                    self.record_lock_wait_us(wait_started.elapsed());
                    return Ok(UniqueKeyGuard {
                        table: Arc::clone(self),
                        shard,
                        key,
                        owner,
                    });
                }
                Some(_) => continue,
            }
        }
    }

    fn unlock(&self, shard: usize, key: Vec<u8>, owner: u64) {
        if let Ok(mut map) = self.shards[shard].lock() {
            let drop_entry = if let Some(state) = map.get_mut(&key) {
                if state.owner == Some(owner) {
                    state.depth = state.depth.saturating_sub(1);
                    if state.depth == 0 {
                        state.owner = None;
                        // Targeted handoff to the front of the FIFO.
                        if let Some(next) = state.waiters.front().cloned() {
                            next.notify_one();
                        }
                    }
                }
                state.owner.is_none() && state.waiters.is_empty()
            } else {
                false
            };
            if drop_entry {
                map.remove(&key);
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
