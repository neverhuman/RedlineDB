use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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
}

impl StatementCache {
    pub(super) fn new() -> Self {
        let mut shards = Vec::with_capacity(64);
        for _ in 0..64 {
            shards.push(RwLock::new(HashMap::new()));
        }
        Self { shards }
    }

    fn shard_index(&self, key: &StatementCacheKey) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len().max(1)
    }

    pub(super) fn get(&self, key: &StatementCacheKey) -> Option<Arc<PreparedTemplate>> {
        let shard = self.shard_index(key);
        self.shards[shard].read().get(key).cloned()
    }

    pub(super) fn insert(&self, key: StatementCacheKey, template: Arc<PreparedTemplate>) {
        let shard = self.shard_index(&key);
        self.shards[shard].write().insert(key, template);
    }
}
