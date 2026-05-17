use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arc_swap::ArcSwap;
use redlinedb_kernel::catalog::{SchemaSnapshot, StatsEpoch, StatsSnapshot, StatsStore};
use redlinedb_kernel::engine::page_heap::VacuumStats;
use redlinedb_kernel::engine::{
    CheckpointStats, Engine, EngineConfig, RecoveryTarget, StorageStatsSnapshot,
};
use redlinedb_kernel::error::Error as KernelError;

use crate::error::Result;
use crate::session::{SessionState, UniqueLockTable};

use super::cache::StatementCache;
use super::options::{DbOptions, OptimizerConfig, QueryMemoryConfig, StatsConfig};
use super::session::Connection;

pub(super) const USER_VERSION_FILE: &str = "user_version.redline";

#[derive(Debug)]
pub struct Database {
    pub(super) path: Arc<PathBuf>,
    pub(super) engine: Arc<Engine>,
    pub(super) unique_locks: Arc<UniqueLockTable>,
    pub(super) stmt_cache: StatementCache,
    pub(super) optimizer_hash: u64,
    pub(super) stats_store: StatsStore,
    pub(super) stats: ArcSwap<StatsSnapshot>,
    pub(super) stats_config: StatsConfig,
    pub(super) query_memory: QueryMemoryConfig,
    pub(super) temp_dir: Option<PathBuf>,
    pub(super) optimizer: OptimizerConfig,
    pub(super) user_version: Mutex<i64>,
}

impl Database {
    pub fn create(path: impl AsRef<Path>, opts: DbOptions) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let engine = Engine::create(base, opts.engine)?;
        save_user_version(base, 0)?;
        let stats_store = StatsStore::new(base);
        let stats = match stats_store.load()? {
            Some(s) => s,
            None => Arc::new(StatsSnapshot::default()),
        };
        let optimizer_hash = hash_optimizer(&opts.optimizer, &opts.query_memory);
        Ok(Arc::new(Self {
            path: Arc::new(base.to_path_buf()),
            engine,
            unique_locks: UniqueLockTable::new(opts.unique_lock_shards, opts.busy_timeout),
            stmt_cache: StatementCache::new(),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(load_user_version(base)?),
        }))
    }

    pub fn open(path: impl AsRef<Path>, opts: DbOptions) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let engine = Engine::open(base, opts.engine)?;
        let user_version = load_user_version(base)?;
        let stats_store = StatsStore::new(base);
        let stats = match stats_store.load()? {
            Some(s) => s,
            None => Arc::new(StatsSnapshot::default()),
        };
        let optimizer_hash = hash_optimizer(&opts.optimizer, &opts.query_memory);
        Ok(Arc::new(Self {
            path: Arc::new(base.to_path_buf()),
            engine,
            unique_locks: UniqueLockTable::new(opts.unique_lock_shards, opts.busy_timeout),
            stmt_cache: StatementCache::new(),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(user_version),
        }))
    }

    pub fn open_with_recovery_target(
        path: impl AsRef<Path>,
        opts: DbOptions,
        target: RecoveryTarget,
    ) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let engine = Engine::open_with_recovery_target(base, opts.engine, target)?;
        let user_version = load_user_version(base)?;
        let stats_store = StatsStore::new(base);
        let stats = match stats_store.load()? {
            Some(s) => s,
            None => Arc::new(StatsSnapshot::default()),
        };
        let optimizer_hash = hash_optimizer(&opts.optimizer, &opts.query_memory);
        Ok(Arc::new(Self {
            path: Arc::new(base.to_path_buf()),
            engine,
            unique_locks: UniqueLockTable::new(opts.unique_lock_shards, opts.busy_timeout),
            stmt_cache: StatementCache::new(),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(user_version),
        }))
    }

    pub fn connect(self: &Arc<Self>) -> Arc<Connection> {
        Arc::new(Connection {
            db: Arc::clone(self),
            session: Mutex::new(SessionState::default()),
            local_cache: StatementCache::new(),
            attach_map: crate::exec::attach::AttachMap::new(),
        })
    }

    pub(crate) fn stats_epoch(&self) -> StatsEpoch {
        self.stats.load_full().epoch
    }

    pub(crate) fn stats_snapshot(&self) -> Arc<StatsSnapshot> {
        self.stats.load_full()
    }

    pub(crate) fn optimizer_hash(&self) -> u64 {
        self.optimizer_hash
    }

    pub(crate) fn stats_config(&self) -> &StatsConfig {
        &self.stats_config
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        self.engine.set_busy_timeout(timeout);
        self.unique_locks.set_timeout(timeout);
    }

    pub(crate) fn query_memory(&self) -> &QueryMemoryConfig {
        &self.query_memory
    }

    pub(crate) fn temp_dir(&self) -> Option<&Path> {
        self.temp_dir.as_deref()
    }

    pub(crate) fn optimizer_config(&self) -> &OptimizerConfig {
        &self.optimizer
    }

    pub(crate) fn publish_stats(&self, snapshot: Arc<StatsSnapshot>) -> Result<()> {
        self.stats_store.save(snapshot.as_ref())?;
        self.stats.store(snapshot);
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<CheckpointStats> {
        Ok(self.engine.checkpoint_with_stats()?)
    }

    pub fn vacuum(&self) -> Result<VacuumStats> {
        Ok(self.engine.vacuum()?)
    }

    pub fn stats(&self) -> Result<StorageStatsSnapshot> {
        Ok(self.engine.storage_stats()?)
    }

    pub fn integrity_check(&self) -> Result<Vec<String>> {
        Ok(self.engine.integrity_check()?)
    }

    pub fn tx_status_stats(&self) -> redlinedb_kernel::engine::TxStatusStats {
        self.engine.tx_status_stats()
    }

    pub fn schema_epoch(&self) -> redlinedb_kernel::catalog::SchemaEpoch {
        self.engine.schema_epoch()
    }

    pub fn schema_snapshot(&self) -> Arc<SchemaSnapshot> {
        self.engine.schema_snapshot()
    }

    pub fn engine_config(&self) -> EngineConfig {
        self.engine.config().clone()
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn user_version(&self) -> i64 {
        *self.user_version.lock().expect("user_version poisoned")
    }

    pub(crate) fn set_user_version(&self, value: i64) -> Result<()> {
        save_user_version(self.path.as_ref(), value)?;
        *self.user_version.lock().expect("user_version poisoned") = value;
        Ok(())
    }
}

pub(super) fn hash_optimizer(optimizer: &OptimizerConfig, query_memory: &QueryMemoryConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    optimizer.hash(&mut hasher);
    query_memory.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn load_user_version(base: &Path) -> Result<i64> {
    let path = base.join(USER_VERSION_FILE);
    if !path.exists() {
        return Ok(0);
    }
    let mut text = String::new();
    File::open(&path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .map_err(KernelError::Io)?;
    text.trim()
        .parse::<i64>()
        .map_err(|_| KernelError::InvalidRecord("invalid user_version sidecar").into())
}

pub(super) fn save_user_version(base: &Path, value: i64) -> Result<()> {
    fs::create_dir_all(base).map_err(KernelError::Io)?;
    let path = base.join(USER_VERSION_FILE);
    let temp_path = base.join(format!("{USER_VERSION_FILE}.tmp"));
    {
        let mut file = File::create(&temp_path).map_err(KernelError::Io)?;
        writeln!(file, "{value}").map_err(KernelError::Io)?;
        file.sync_all().map_err(KernelError::Io)?;
    }
    fs::rename(&temp_path, &path).map_err(KernelError::Io)?;
    Ok(())
}
