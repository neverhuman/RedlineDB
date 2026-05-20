use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::RwLock;

use crate::statement::PreparedTemplate;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(super) struct StatementCacheKey {
    pub(super) schema_epoch: u64,
    pub(super) stats_epoch: u64,
    pub(super) optimizer_hash: u64,
    pub(super) sql: Arc<str>,
}

#[derive(Debug, Default)]
pub(super) struct StatementCache {
    shards: Vec<RwLock<HashMap<StatementCacheKey, Arc<PreparedTemplate>>>>,
    capacity: usize,
    entries: AtomicUsize,
}

impl StatementCache {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_capacity(128)
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        let shard_count = if capacity == 0 {
            1
        } else {
            capacity.min(64).max(1)
        };
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(HashMap::new()));
        }
        Self {
            shards,
            capacity,
            entries: AtomicUsize::new(0),
        }
    }

    pub(super) fn capacity(&self) -> usize {
        self.capacity
    }

    fn shard_index(&self, key: &StatementCacheKey) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len().max(1)
    }

    pub(super) fn get(&self, key: &StatementCacheKey) -> Option<Arc<PreparedTemplate>> {
        if self.capacity == 0 {
            return None;
        }
        let shard = self.shard_index(key);
        self.shards[shard].read().get(key).cloned()
    }

    pub(super) fn insert(&self, key: StatementCacheKey, template: Arc<PreparedTemplate>) {
        if self.capacity == 0 {
            return;
        }
        let shard = self.shard_index(&key);
        let mut shard = self.shards[shard].write();
        if !shard.contains_key(&key) && self.entries.load(Ordering::Relaxed) >= self.capacity {
            self.entries.fetch_sub(shard.len(), Ordering::Relaxed);
            shard.clear();
        }
        if shard.insert(key, template).is_none() {
            self.entries.fetch_add(1, Ordering::Relaxed);
        }
    }
}
