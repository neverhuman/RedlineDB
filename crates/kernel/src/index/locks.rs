use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use crate::{Error, Result};

#[derive(Debug, Default)]
pub struct UniqueKeyLockTable {
    shards: Vec<Mutex<HashMap<Vec<u8>, UniqueKeyLockState>>>,
    cvars: Vec<Condvar>,
}

#[derive(Clone, Copy, Debug, Default)]
struct UniqueKeyLockState {
    owner: Option<u64>,
    depth: usize,
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
        let mut cvars = Vec::with_capacity(shards);
        for _ in 0..shards {
            locks.push(Mutex::new(HashMap::new()));
            cvars.push(Condvar::new());
        }
        Self {
            shards: locks,
            cvars,
        }
    }

    pub fn lock(self: &Arc<Self>, key: &[u8], owner: u64) -> Result<UniqueKeyGuard> {
        let shard = self.shard(key);
        let mut map = self.shards[shard]
            .lock()
            .map_err(|_| Error::CorruptPage("unique lock shard poisoned"))?;
        let key = key.to_vec();
        loop {
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
                Some(_) => {
                    map = self.cvars[shard]
                        .wait(map)
                        .map_err(|_| Error::CorruptPage("unique lock wait poisoned"))?;
                }
            }
        }
    }

    fn unlock(&self, shard: usize, key: Vec<u8>, owner: u64) {
        if let Ok(mut map) = self.shards[shard].lock() {
            if let Some(state) = map.get_mut(&key)
                && state.owner == Some(owner)
            {
                state.depth = state.depth.saturating_sub(1);
                if state.depth == 0 {
                    map.remove(&key);
                }
            }
            self.cvars[shard].notify_all();
        }
    }

    fn shard(&self, key: &[u8]) -> usize {
        let mut hash = 0_u64;
        for byte in key {
            hash = hash.wrapping_mul(131).wrapping_add(*byte as u64);
        }
        hash as usize % self.shards.len().max(1)
    }
}

impl Drop for UniqueKeyGuard {
    fn drop(&mut self) {
        self.table
            .unlock(self.shard, std::mem::take(&mut self.key), self.owner);
    }
}
