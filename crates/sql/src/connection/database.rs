use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use arc_swap::ArcSwap;
use redlinedb_kernel::catalog::{SchemaSnapshot, StatsEpoch, StatsSnapshot, StatsStore};
use redlinedb_kernel::engine::page_heap::VacuumStats;
use redlinedb_kernel::engine::{
    CheckpointStats, CommitDurability, Engine, EngineConfig, RecoveryTarget, StorageStatsSnapshot,
};
use redlinedb_kernel::error::Error as KernelError;

use crate::error::{Error, Result};
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
    metadata_sync_policy: MetadataSyncPolicy,
    _ephemeral_root: Option<Arc<EphemeralRoot>>,
    /// True for `:memory:` / private-memory databases. SQLite-parity
    /// surfaces (PRAGMA journal_mode, database_list path) consult this
    /// flag rather than the raw on-disk path, which is a tmpfs sidecar.
    private_memory: bool,
}

impl Database {
    pub fn create(path: impl AsRef<Path>, opts: DbOptions) -> Result<Arc<Self>> {
        Self::create_with_ephemeral_root(path.as_ref(), opts, None)
    }

    /// Create a private process-local ephemeral database.
    ///
    /// The backing root is owned by the returned handle and is removed when
    /// the final [`Arc<Database>`] owner drops. Connections opened from the
    /// returned database share the same transient state.
    pub fn create_in_memory(opts: DbOptions) -> Result<Arc<Self>> {
        let opts = volatile_db_options(opts);
        let id = EPHEMERAL_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let root = EphemeralRoot::new_private(id, &opts)?;
        let path = root.path().to_path_buf();
        Self::create_private_in_memory_with_root(&path, opts, Some(Arc::new(root)))
    }

    #[doc(hidden)]
    pub fn create_private_in_memory_at(
        path: impl AsRef<Path>,
        opts: DbOptions,
    ) -> Result<Arc<Self>> {
        Self::create_private_in_memory_with_root(path.as_ref(), volatile_db_options(opts), None)
    }

    /// Create or reuse a named process-local ephemeral database.
    ///
    /// Calls with the same `session_name` return the same live database while
    /// at least one owner is alive. A reuse with incompatible options fails
    /// instead of silently attaching a connection to a differently configured
    /// engine.
    pub fn create_ephemeral(session_name: &str, opts: DbOptions) -> Result<Arc<Self>> {
        if session_name.trim().is_empty() {
            return Err(Error::Config(
                "ephemeral session name must not be empty".to_string(),
            ));
        }
        let opts = volatile_db_options(opts);

        let mut registry = ephemeral_registry()
            .lock()
            .expect("ephemeral registry poisoned");
        if let Some(existing) = registry
            .sessions
            .get(session_name)
            .and_then(EphemeralSession::upgrade)
        {
            if existing.options != opts {
                return Err(Error::Config(format!(
                    "ephemeral session {session_name:?} already exists with incompatible options"
                )));
            }
            return Ok(existing.db);
        }

        let root = EphemeralRoot::new_named(session_name, &opts)?;
        let path = root.path().to_path_buf();
        let db = Self::create_with_ephemeral_root(&path, opts.clone(), Some(Arc::new(root)))?;
        registry.sessions.insert(
            session_name.to_string(),
            EphemeralSession {
                options: opts,
                db: Arc::downgrade(&db),
            },
        );
        Ok(db)
    }

    fn create_with_ephemeral_root(
        path: &Path,
        opts: DbOptions,
        ephemeral_root: Option<Arc<EphemeralRoot>>,
    ) -> Result<Arc<Self>> {
        Self::create_with_mode(path, opts, ephemeral_root, false)
    }

    fn create_private_in_memory_with_root(
        path: &Path,
        opts: DbOptions,
        ephemeral_root: Option<Arc<EphemeralRoot>>,
    ) -> Result<Arc<Self>> {
        Self::create_with_mode(path, opts, ephemeral_root, true)
    }

    fn create_with_mode(
        path: &Path,
        opts: DbOptions,
        ephemeral_root: Option<Arc<EphemeralRoot>>,
        private_memory: bool,
    ) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let metadata_sync_policy = if private_memory {
            MetadataSyncPolicy::Disabled
        } else {
            MetadataSyncPolicy::from_commit_durability(opts.engine.commit_durability)
        };
        let engine = if private_memory {
            Engine::create_volatile(base, opts.engine)?
        } else {
            Engine::create(base, opts.engine)?
        };
        save_user_version(base, 0, metadata_sync_policy)?;
        let stats_store = StatsStore::new(base);
        let stats = if private_memory {
            Arc::new(StatsSnapshot::default())
        } else {
            match stats_store.load()? {
                Some(s) => s,
                None => Arc::new(StatsSnapshot::default()),
            }
        };
        let optimizer_hash = hash_optimizer(&opts.optimizer, &opts.query_memory);
        Ok(Arc::new(Self {
            path: Arc::new(base.to_path_buf()),
            engine,
            unique_locks: UniqueLockTable::new(opts.unique_lock_shards, opts.busy_timeout),
            stmt_cache: StatementCache::with_capacity(opts.statement_cache_capacity),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(if private_memory {
                0
            } else {
                load_user_version(base)?
            }),
            metadata_sync_policy,
            _ephemeral_root: ephemeral_root,
            private_memory,
        }))
    }

    pub fn open(path: impl AsRef<Path>, opts: DbOptions) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let metadata_sync_policy =
            MetadataSyncPolicy::from_commit_durability(opts.engine.commit_durability);
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
            stmt_cache: StatementCache::with_capacity(opts.statement_cache_capacity),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(user_version),
            metadata_sync_policy,
            _ephemeral_root: None,
            private_memory: false,
        }))
    }

    pub fn open_with_recovery_target(
        path: impl AsRef<Path>,
        opts: DbOptions,
        target: RecoveryTarget,
    ) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let metadata_sync_policy =
            MetadataSyncPolicy::from_commit_durability(opts.engine.commit_durability);
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
            stmt_cache: StatementCache::with_capacity(opts.statement_cache_capacity),
            optimizer_hash,
            stats_store,
            stats: ArcSwap::from(stats),
            stats_config: opts.stats,
            query_memory: opts.query_memory,
            temp_dir: opts.temp_dir.clone(),
            optimizer: opts.optimizer,
            user_version: Mutex::new(user_version),
            metadata_sync_policy,
            _ephemeral_root: None,
            private_memory: false,
        }))
    }

    pub fn connect(self: &Arc<Self>) -> Arc<Connection> {
        Arc::new(Connection {
            db: Arc::clone(self),
            session: Mutex::new(SessionState::default()),
            local_cache: StatementCache::with_capacity(self.stmt_cache.capacity()),
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
        if self.metadata_sync_policy.writes_metadata() {
            self.stats_store.save(snapshot.as_ref())?;
        }
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

    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    /// True when this database is backed by a private/in-memory engine.
    /// The `redlinedb` registry routes `:memory:` to
    /// `create_private_in_memory_at` (which sets `private_memory = true`
    /// on construction) — the ephemeral root may live on the registry
    /// rather than on this struct, so we flag the private-memory mode
    /// explicitly rather than relying on `_ephemeral_root`.
    pub(crate) fn is_in_memory(&self) -> bool {
        self.private_memory
    }

    pub(crate) fn user_version(&self) -> i64 {
        *self.user_version.lock().expect("user_version poisoned")
    }

    pub(crate) fn set_user_version(&self, value: i64) -> Result<()> {
        save_user_version(self.path.as_ref(), value, self.metadata_sync_policy)?;
        *self.user_version.lock().expect("user_version poisoned") = value;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataSyncPolicy {
    Durable,
    Volatile,
    Disabled,
}

impl MetadataSyncPolicy {
    fn from_commit_durability(commit_durability: CommitDurability) -> Self {
        match commit_durability {
            CommitDurability::Strict | CommitDurability::Normal => Self::Durable,
            CommitDurability::UnsafeDev => Self::Volatile,
        }
    }

    fn syncs_metadata(self) -> bool {
        matches!(self, Self::Durable)
    }

    fn writes_metadata(self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

fn volatile_db_options(mut opts: DbOptions) -> DbOptions {
    opts.engine.commit_durability = CommitDurability::UnsafeDev;
    if opts.temp_dir.is_none() {
        opts.temp_dir = Some(standard_volatile_root());
    }
    opts
}

const SHARED_MEMORY_EPHEMERAL_ROOT: &str = "/dev/shm/redlinedb-ephemeral";

static VOLATILE_ROOT_CACHE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn standard_volatile_root() -> PathBuf {
    // Phase 1.4: cache + lighten the probe. Mirrors the same change in
    // crates/redlinedb/src/registry.rs. Together these eliminate the
    // 8-12 syscalls each in-memory open previously incurred when the
    // two duplicate probes both ran a create+write+unlink dance.
    VOLATILE_ROOT_CACHE
        .get_or_init(|| volatile_root_from_candidate(Path::new(SHARED_MEMORY_EPHEMERAL_ROOT)))
        .clone()
}

fn volatile_root_from_candidate(candidate: &Path) -> PathBuf {
    if ensure_writable_volatile_root(candidate) {
        candidate.to_path_buf()
    } else {
        std::env::temp_dir()
    }
}

fn ensure_writable_volatile_root(root: &Path) -> bool {
    fs::create_dir_all(root).is_ok()
}

#[derive(Debug)]
struct EphemeralRoot {
    dir: tempfile::TempDir,
}

impl EphemeralRoot {
    fn new_private(id: u64, opts: &DbOptions) -> Result<Self> {
        Self::new(
            format!("redlinedb-memory-{}-{id}-", std::process::id()),
            opts,
        )
    }

    fn new_named(session_name: &str, opts: &DbOptions) -> Result<Self> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        session_name.hash(&mut hasher);
        let digest = hasher.finish();
        Self::new(
            format!("redlinedb-ephemeral-{}-{digest:016x}-", std::process::id()),
            opts,
        )
    }

    fn new(prefix: String, opts: &DbOptions) -> Result<Self> {
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix);
        let dir = match opts.temp_dir.as_deref() {
            Some(root) => {
                fs::create_dir_all(root).map_err(KernelError::Io)?;
                builder.tempdir_in(root).map_err(KernelError::Io)?
            }
            None => builder.tempdir().map_err(KernelError::Io)?,
        };
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

#[derive(Default)]
struct EphemeralRegistry {
    sessions: HashMap<String, EphemeralSession>,
}

struct EphemeralSession {
    options: DbOptions,
    db: Weak<Database>,
}

struct EphemeralRegistryCell {
    registry: Mutex<EphemeralRegistry>,
}

impl EphemeralRegistryCell {
    fn new() -> Self {
        Self {
            registry: Mutex::new(EphemeralRegistry::default()),
        }
    }

    fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, EphemeralRegistry>> {
        self.registry.lock()
    }
}

struct LiveEphemeralSession {
    options: DbOptions,
    db: Arc<Database>,
}

impl EphemeralSession {
    fn upgrade(&self) -> Option<LiveEphemeralSession> {
        self.db.upgrade().map(|db| LiveEphemeralSession {
            options: self.options.clone(),
            db,
        })
    }
}

static EPHEMERAL_REGISTRY: OnceLock<EphemeralRegistryCell> = OnceLock::new();
static EPHEMERAL_COUNTER: AtomicU64 = AtomicU64::new(1);

fn ephemeral_registry() -> &'static EphemeralRegistryCell {
    EPHEMERAL_REGISTRY.get_or_init(EphemeralRegistryCell::new)
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

fn save_user_version(base: &Path, value: i64, sync_policy: MetadataSyncPolicy) -> Result<()> {
    if !sync_policy.writes_metadata() {
        return Ok(());
    }
    fs::create_dir_all(base).map_err(KernelError::Io)?;
    let path = base.join(USER_VERSION_FILE);
    let temp_path = base.join(format!("{USER_VERSION_FILE}.tmp"));
    {
        let mut file = File::create(&temp_path).map_err(KernelError::Io)?;
        writeln!(file, "{value}").map_err(KernelError::Io)?;
        if sync_policy.syncs_metadata() {
            file.sync_all().map_err(KernelError::Io)?;
        }
    }
    fs::rename(&temp_path, &path).map_err(KernelError::Io)?;
    Ok(())
}

#[cfg(test)]
mod volatile_root_tests {
    use super::*;

    #[test]
    fn explicit_volatile_root_remains_in_options() {
        let root = tempfile::tempdir().expect("scratch dir");
        let opts = DbOptions {
            temp_dir: Some(root.path().to_path_buf()),
            ..DbOptions::default()
        };

        assert_eq!(
            volatile_db_options(opts).temp_dir.as_deref(),
            Some(root.path())
        );
    }

    #[test]
    fn unusable_shared_memory_candidate_uses_process_scratch() {
        let root = tempfile::tempdir().expect("scratch dir");
        let file_path = root.path().join("not-a-directory");
        fs::write(&file_path, b"x").expect("probe file");

        assert_eq!(
            volatile_root_from_candidate(&file_path),
            std::env::temp_dir()
        );
    }
}
