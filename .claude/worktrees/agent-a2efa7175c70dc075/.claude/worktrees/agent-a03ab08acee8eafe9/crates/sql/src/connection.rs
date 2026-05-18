use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arc_swap::ArcSwap;
use parking_lot::RwLock;
use redlinedb_kernel::catalog::{SchemaSnapshot, StatsEpoch, StatsSnapshot, StatsStore};
use redlinedb_kernel::engine::page_heap::VacuumStats;
use redlinedb_kernel::engine::{
    CheckpointStats, CommitOutcome, Engine, EngineConfig, RecoveryTarget, StorageStatsSnapshot, Txn,
};
use redlinedb_kernel::error::Error as KernelError;
use redlinedb_kernel::txn::Isolation;

use crate::error::{Error, Result};
use crate::parser::parse_prepared_template;
use crate::parser::savepoint::{SavepointAction, try_parse_savepoint};
use crate::session::{BeginMode, JournalEntry, SavepointFrame, SessionState, UniqueLockTable};
use crate::statement::{PreparedTemplate, Statement, Step};

const USER_VERSION_FILE: &str = "user_version.redline";

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct StatementCacheKey {
    schema_epoch: u64,
    stats_epoch: u64,
    optimizer_hash: u64,
    sql: Arc<str>,
}

#[derive(Debug, Default)]
struct StatementCache {
    shards: Vec<RwLock<HashMap<StatementCacheKey, Arc<PreparedTemplate>>>>,
}

impl StatementCache {
    fn new() -> Self {
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

    fn get(&self, key: &StatementCacheKey) -> Option<Arc<PreparedTemplate>> {
        let shard = self.shard_index(key);
        self.shards[shard].read().get(key).cloned()
    }

    fn insert(&self, key: StatementCacheKey, template: Arc<PreparedTemplate>) {
        let shard = self.shard_index(&key);
        self.shards[shard].write().insert(key, template);
    }
}

#[derive(Debug, Clone)]
pub struct DbOptions {
    pub engine: EngineConfig,
    pub unique_lock_shards: usize,
    pub busy_timeout: Duration,
    pub optimizer: OptimizerConfig,
    pub query_memory: QueryMemoryConfig,
    pub stats: StatsConfig,
    pub temp_dir: Option<PathBuf>,
}

impl Default for DbOptions {
    fn default() -> Self {
        Self {
            engine: EngineConfig::default(),
            unique_lock_shards: 128,
            busy_timeout: Duration::from_secs(5),
            optimizer: OptimizerConfig::default(),
            query_memory: QueryMemoryConfig::default(),
            stats: StatsConfig::default(),
            temp_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptimizerConfig {
    pub enabled: bool,
    pub max_exact_join_tables: usize,
    pub max_join_alternatives: usize,
    pub enable_multi_index_or: bool,
    pub enable_multi_index_and: bool,
    pub enable_covering_index: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_exact_join_tables: 8,
            max_join_alternatives: 4,
            enable_multi_index_or: true,
            enable_multi_index_and: true,
            enable_covering_index: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryMemoryConfig {
    pub work_mem_bytes: usize,
    pub max_spill_bytes: usize,
    pub batch_rows: usize,
}

impl Default for QueryMemoryConfig {
    fn default() -> Self {
        Self {
            work_mem_bytes: 8 * 1024 * 1024,
            max_spill_bytes: 1024 * 1024 * 1024,
            batch_rows: 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsConfig {
    pub exact_analyze_row_threshold: usize,
    pub sample_rows: usize,
    pub mcv_capacity: usize,
    pub histogram_buckets: usize,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            exact_analyze_row_threshold: 100_000,
            sample_rows: 32_768,
            mcv_capacity: 100,
            histogram_buckets: 100,
        }
    }
}

#[derive(Debug)]
pub struct Database {
    path: Arc<PathBuf>,
    engine: Arc<Engine>,
    unique_locks: Arc<UniqueLockTable>,
    stmt_cache: StatementCache,
    optimizer_hash: u64,
    stats_store: StatsStore,
    stats: ArcSwap<StatsSnapshot>,
    stats_config: StatsConfig,
    query_memory: QueryMemoryConfig,
    temp_dir: Option<PathBuf>,
    optimizer: OptimizerConfig,
    user_version: Mutex<i64>,
}

#[derive(Debug)]
pub struct Connection {
    db: Arc<Database>,
    session: Mutex<SessionState>,
    local_cache: StatementCache,
}

impl Database {
    pub fn create(path: impl AsRef<Path>, opts: DbOptions) -> Result<Arc<Self>> {
        let base = path.as_ref();
        let engine = Engine::create(base, opts.engine)?;
        save_user_version(base, 0)?;
        let stats_store = StatsStore::new(base);
        let stats = stats_store
            .load()?
            .unwrap_or_else(|| Arc::new(StatsSnapshot::default()));
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
        let stats = stats_store
            .load()?
            .unwrap_or_else(|| Arc::new(StatsSnapshot::default()));
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
        let stats = stats_store
            .load()?
            .unwrap_or_else(|| Arc::new(StatsSnapshot::default()));
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

impl Connection {
    /// Prepare a single SQL statement, ignoring any trailing statements.
    ///
    /// Most callers want this for backward compatibility — the returned
    /// statement is the *first* statement in `sql`, and any remaining text is
    /// silently dropped. Use [`Connection::prepare_v2`] for the
    /// `sqlite3_prepare_v2`-style API that returns the unconsumed tail.
    pub fn prepare(self: &Arc<Self>, sql: &str) -> Result<Statement> {
        let (stmt, _tail) = self.prepare_v2(sql)?;
        match stmt {
            Some(stmt) => Ok(stmt),
            None => Err(Error::UnsupportedSql(
                "no statement in SQL input".to_owned(),
            )),
        }
    }

    /// Prepare the first statement in `sql` and return it together with the
    /// tail (the byte slice of `sql` that was not consumed). When `sql` is
    /// blank/comment-only the `Option<Statement>` is `None` and the tail is
    /// empty — this matches `sqlite3_prepare_v2(db, "  --x", _, &stmt, &tail)`
    /// which sets `stmt = NULL` and returns OK.
    ///
    /// SAVEPOINT / RELEASE / ROLLBACK TO are handled eagerly here: their
    /// side-effects fire during preparation and the returned statement is a
    /// fully-completed no-op (`step` immediately yields `Step::Done`).
    pub fn prepare_v2<'a>(self: &Arc<Self>, sql: &'a str) -> Result<(Option<Statement>, &'a str)> {
        let (head, tail) = crate::parser::split_first_statement(sql);
        if crate::parser::is_blank_sql(head) {
            // Either fully blank input, or a remaining comment-only tail.
            return Ok((None, tail));
        }
        if let Some(action) = try_parse_savepoint(head)? {
            self.apply_savepoint_action(&action)?;
            // Build a no-op completed statement so the FFI still returns a
            // valid handle that the caller can step() / finalize().
            let template = self.savepoint_marker_template(head);
            let stmt = Statement::new_completed(Arc::clone(self), template);
            return Ok((Some(stmt), tail));
        }
        let template = self.prepare_cached(head)?;
        Ok((Some(Statement::new(Arc::clone(self), template)), tail))
    }

    /// Execute every statement in `sql`. For multi-statement input, runs
    /// them in order; the result is the affected-rows count of the last
    /// non-readonly statement (or the row count of the last SELECT).
    pub fn execute(self: &Arc<Self>, sql: &str) -> Result<usize> {
        let mut rest = sql;
        let mut last: usize = 0;
        loop {
            let (stmt_opt, tail) = self.prepare_v2(rest)?;
            if let Some(mut stmt) = stmt_opt {
                let mut rows = 0usize;
                while let Step::Row = stmt.step()? {
                    rows += 1;
                }
                last = if stmt.is_readonly() {
                    rows
                } else {
                    stmt.affected_rows()
                };
            }
            if tail.is_empty() {
                break;
            }
            rest = tail;
        }
        Ok(last)
    }

    /// Build a "marker" template for savepoint statements. The side-effects
    /// fire during `prepare_v2`; the returned `Statement` is constructed
    /// with `runtime = Done` so it never invokes the executor. We tag the
    /// template's `sql` field with a sentinel prefix so any later `reset`/
    /// `step` cycle is also a no-op.
    fn savepoint_marker_template(self: &Arc<Self>, sql: &str) -> Arc<PreparedTemplate> {
        let tagged = format!("{}{}", crate::statement::SAVEPOINT_MARKER_SQL_PREFIX, sql);
        Arc::new(PreparedTemplate {
            sql: Arc::from(tagged.as_str()),
            schema_epoch: self.schema_epoch(),
            stats_epoch: self.stats_epoch().0,
            optimizer_hash: self.optimizer_hash(),
            param_layout: crate::statement::ParamLayout::default(),
            output_columns: Arc::from([]),
            readonly: true,
            // The marker template never reaches `execute_prepared`; we pick
            // an existing variant with a trivial, idempotent handler so the
            // exec.rs match stays exhaustive without requiring edits.
            kind: crate::statement::PreparedKind::Pragma(
                crate::statement::PragmaPlan::SetForeignKeys(false),
            ),
        })
    }

    /// Push, release, or rewind a savepoint. Implements SQLite's three
    /// commands; called from both the SQL prepare-time interceptor and the
    /// programmatic Rust APIs (`Connection::savepoint` etc.).
    pub(crate) fn apply_savepoint_action(self: &Arc<Self>, action: &SavepointAction) -> Result<()> {
        match action {
            SavepointAction::Savepoint(name) => self.savepoint(name),
            SavepointAction::Release(name) => self.release(name),
            SavepointAction::RollbackTo(name) => self.rollback_to(name),
        }
    }

    /// Open a SAVEPOINT named `name`. If no transaction is active, opens an
    /// implicit deferred transaction first (matching SQLite, where SAVEPOINT
    /// outside a tx works as if BEGIN had been called).
    pub fn savepoint(&self, name: &str) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        let implicit_tx = if session.tx.is_none() {
            let tx = self.db.engine.begin(Isolation::Snapshot)?;
            session.tx = Some(tx);
            session.failed = false;
            true
        } else {
            false
        };
        let frame = SavepointFrame {
            name: name.to_owned(),
            journal_len: session.journal.len(),
            changes: session.changes,
            total_changes: session.total_changes,
            last_insert_rowid: session.last_insert_rowid,
            implicit_tx,
        };
        session.savepoints.push(frame);
        Ok(())
    }

    /// RELEASE a SAVEPOINT: pop frames down to and including the most-recent
    /// frame whose name matches. The journal stays intact (the released
    /// frame's work merges into its parent / the outer transaction).
    pub fn release(&self, name: &str) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        let pos = session
            .savepoints
            .iter()
            .rposition(|frame| frame.name == name)
            .ok_or(Error::TransactionState("no such savepoint"))?;
        // Track whether the bottom-most popped frame was implicit. If yes
        // and the stack is now empty AND no nested releases shadowed the
        // implicit-tx flag, we commit the surrounding tx (the SAVEPOINT
        // outside-of-tx contract).
        let bottom_was_implicit = session
            .savepoints
            .get(pos)
            .map(|frame| frame.implicit_tx)
            .unwrap_or(false);
        session.savepoints.truncate(pos);
        let stack_now_empty = session.savepoints.is_empty();
        if stack_now_empty {
            // No more savepoints — clear the journal too. The DML up to
            // this point will commit (or rollback) at the outer tx
            // boundary; we don't need to keep replay material.
            session.journal.clear();
        }
        if stack_now_empty && bottom_was_implicit {
            // Drop the lock before going through commit() which re-locks.
            drop(session);
            self.commit()?;
        }
        Ok(())
    }

    /// ROLLBACK TO SAVEPOINT: rewind the active transaction to the state
    /// captured when `name` was created. Implementation: close the kernel
    /// tx, open a fresh one, replay the journal up to the savepoint's
    /// prefix length. The savepoint frame stays on the stack (per SQLite).
    pub fn rollback_to(self: &Arc<Self>, name: &str) -> Result<()> {
        // Phase 1: locate frame, snapshot the journal prefix, drop the
        // active tx. The replay runs in phase 2 with no session lock held.
        let (replay_entries, target_changes, target_total, target_last_rowid, target_journal_len) = {
            let mut session = self.session.lock().expect("session poisoned");
            let pos = session
                .savepoints
                .iter()
                .rposition(|frame| frame.name == name)
                .ok_or(Error::TransactionState("no such savepoint"))?;
            // Drop frames *above* `pos` (they're rewound away), keep the
            // matching frame on the stack — RELEASE is a separate op.
            session.savepoints.truncate(pos + 1);
            let frame = session.savepoints[pos].clone();
            let journal_prefix: Vec<JournalEntry> = session.journal[..frame.journal_len].to_vec();
            session.journal.truncate(frame.journal_len);
            if let Some(tx) = session.tx.take() {
                let _ = self.db.engine.rollback(tx);
            }
            session.kernel_unique_guards.clear();
            session.unique_guards.clear();
            session.failed = false;
            session.changes = 0;
            session.total_changes = 0;
            session.last_insert_rowid = None;
            (
                journal_prefix,
                frame.changes,
                frame.total_changes,
                frame.last_insert_rowid,
                frame.journal_len,
            )
        };

        // Phase 2: open a fresh tx and replay the journal prefix.
        {
            let mut session = self.session.lock().expect("session poisoned");
            let tx = self.db.engine.begin(Isolation::Snapshot)?;
            session.tx = Some(tx);
            session.replay_in_progress = true;
        }
        let replay_result = self.replay_journal(&replay_entries);
        {
            let mut session = self.session.lock().expect("session poisoned");
            session.replay_in_progress = false;
            session.changes = target_changes;
            session.total_changes = target_total;
            session.last_insert_rowid = target_last_rowid;
            session.journal.truncate(target_journal_len);
        }
        replay_result
    }

    fn replay_journal(self: &Arc<Self>, entries: &[JournalEntry]) -> Result<()> {
        for entry in entries {
            let template = self.prepare_cached(&entry.sql)?;
            let mut stmt = Statement::new(Arc::clone(self), template);
            if !entry.bindings.is_empty() {
                for (idx, slot) in entry.bindings.iter().enumerate().skip(1) {
                    if let Some(value) = slot {
                        stmt.bind_value(idx, value.clone())?;
                    }
                }
            }
            while let Step::Row = stmt.step()? {}
        }
        Ok(())
    }

    pub fn begin(&self, mode: BeginMode) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        if session.tx.is_some() {
            return Err(Error::TransactionState("transaction already active"));
        }
        let mut tx = self.db.engine.begin(match mode {
            BeginMode::Deferred | BeginMode::Immediate | BeginMode::Exclusive => {
                Isolation::Snapshot
            }
        })?;
        if matches!(mode, BeginMode::Immediate | BeginMode::Exclusive) {
            self.db.engine.reserve_begin_lock(&mut tx)?;
        }
        session.tx = Some(tx);
        session.failed = false;
        // A fresh tx can never replay — drop any leftover journal/savepoint
        // state from a prior rolled-back tx.
        session.clear_savepoints();
        Ok(())
    }

    /// Append a successful non-readonly statement to the per-tx replay
    /// journal. Skipped when:
    ///   * the journal is currently being replayed (avoid feeding itself),
    ///   * no transaction is active (no replay possible).
    ///
    /// We journal eagerly for *every* active tx — even before any
    /// `SAVEPOINT` lands on the stack — because a later SAVEPOINT records
    /// the journal length at its creation moment, and ROLLBACK TO that
    /// savepoint must be able to replay the prefix up to that index. If we
    /// only journaled while a savepoint frame existed, the prefix would be
    /// missing the pre-savepoint statements and ROLLBACK TO would lose
    /// rows that should still be visible.
    pub(crate) fn journal_statement(
        &self,
        sql: &str,
        bindings: Vec<Option<crate::value::SqlValue>>,
    ) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        if session.replay_in_progress || session.tx.is_none() {
            return Ok(());
        }
        session.journal.push(JournalEntry {
            sql: sql.to_owned(),
            bindings,
        });
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        if session.failed {
            return Err(Error::TransactionState(
                "transaction is failed and must roll back",
            ));
        }
        let tx = session
            .tx
            .take()
            .ok_or(Error::TransactionState("no active transaction"))?;
        match self.db.engine.commit(tx) {
            Ok(CommitOutcome::Committed(_)) => {
                session.kernel_unique_guards.clear();
                session.unique_guards.clear();
                session.clear_savepoints();
                Ok(())
            }
            Ok(CommitOutcome::MaybeCommitted) => {
                session.kernel_unique_guards.clear();
                session.unique_guards.clear();
                session.clear_savepoints();
                Err(Error::CommitMaybeCommitted)
            }
            Ok(CommitOutcome::RolledBack) => {
                session.kernel_unique_guards.clear();
                session.unique_guards.clear();
                session.clear_savepoints();
                Err(Error::TransactionState("transaction rolled back"))
            }
            Err(err) => {
                session.kernel_unique_guards.clear();
                session.unique_guards.clear();
                session.clear_savepoints();
                Err(err.into())
            }
        }
    }

    pub fn rollback(&self) -> Result<()> {
        let mut session = self.session.lock().expect("session poisoned");
        let tx = session
            .tx
            .take()
            .ok_or(Error::TransactionState("no active transaction"))?;
        let result = self.db.engine.rollback(tx);
        session.kernel_unique_guards.clear();
        session.unique_guards.clear();
        session.failed = false;
        session.clear_savepoints();
        result?;
        Ok(())
    }

    pub fn last_insert_rowid(&self) -> Option<i64> {
        self.session
            .lock()
            .expect("session poisoned")
            .last_insert_rowid
    }

    pub fn changes(&self) -> usize {
        self.session.lock().expect("session poisoned").changes
    }

    pub fn total_changes(&self) -> usize {
        self.session.lock().expect("session poisoned").total_changes
    }

    pub fn in_transaction(&self) -> bool {
        self.session.lock().expect("session poisoned").tx.is_some()
    }

    pub(crate) fn database_path(&self) -> &Path {
        self.db.path()
    }

    pub(crate) fn foreign_keys(&self) -> bool {
        self.session.lock().expect("session poisoned").foreign_keys
    }

    pub(crate) fn set_foreign_keys(&self, value: bool) {
        self.session.lock().expect("session poisoned").foreign_keys = value;
    }

    pub(crate) fn user_version(&self) -> i64 {
        self.db.user_version()
    }

    pub(crate) fn set_user_version(&self, value: i64) -> Result<()> {
        self.db.set_user_version(value)
    }

    pub(crate) fn schema_epoch(&self) -> redlinedb_kernel::catalog::SchemaEpoch {
        self.db.engine.schema_epoch()
    }

    pub(crate) fn stats_epoch(&self) -> StatsEpoch {
        self.db.stats_epoch()
    }

    pub(crate) fn optimizer_hash(&self) -> u64 {
        self.db.optimizer_hash()
    }

    pub(crate) fn stats_config(&self) -> &StatsConfig {
        self.db.stats_config()
    }

    pub(crate) fn integrity_check(&self) -> Result<Vec<String>> {
        self.db.integrity_check()
    }

    pub(crate) fn query_memory(&self) -> &QueryMemoryConfig {
        self.db.query_memory()
    }

    pub(crate) fn temp_dir(&self) -> Option<&Path> {
        self.db.temp_dir()
    }

    pub(crate) fn optimizer_config(&self) -> &OptimizerConfig {
        self.db.optimizer_config()
    }

    pub(crate) fn stats_snapshot(&self) -> Arc<StatsSnapshot> {
        self.db.stats_snapshot()
    }

    pub(crate) fn publish_stats(&self, snapshot: Arc<StatsSnapshot>) -> Result<()> {
        self.db.publish_stats(snapshot)
    }

    pub(crate) fn engine(&self) -> &Arc<Engine> {
        &self.db.engine
    }

    /// Read-only access to the underlying kernel engine. Exposed for SQL
    /// smoke tests and tooling that need to inspect physical indexes /
    /// catalog state directly. Production code paths must continue to use
    /// the SQL execution surface.
    #[doc(hidden)]
    pub fn engine_for_tests(&self) -> Arc<Engine> {
        Arc::clone(&self.db.engine)
    }

    pub(crate) fn unique_locks(&self) -> &Arc<UniqueLockTable> {
        &self.db.unique_locks
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        self.db.set_busy_timeout(timeout);
    }

    pub(crate) fn with_session<T>(
        &self,
        f: impl FnOnce(&mut SessionState) -> Result<T>,
    ) -> Result<T> {
        let mut session = self.session.lock().expect("session poisoned");
        f(&mut session)
    }

    pub(crate) fn prepare_cached(self: &Arc<Self>, sql: &str) -> Result<Arc<PreparedTemplate>> {
        let normalized = sql.trim();
        if crate::parser::is_pragma_sql(normalized) {
            let mut template = parse_prepared_template(self.as_ref(), sql)?;
            template.stats_epoch = self.stats_epoch().0;
            template.optimizer_hash = self.optimizer_hash();
            return Ok(Arc::new(template));
        }
        let key = StatementCacheKey {
            schema_epoch: self.schema_epoch().0,
            stats_epoch: self.stats_epoch().0,
            optimizer_hash: self.optimizer_hash(),
            sql: Arc::from(normalized),
        };

        if let Some(template) = self.local_cache.get(&key) {
            return Ok(template);
        }

        if let Some(template) = self.db.stmt_cache.get(&key) {
            self.local_cache.insert(key, Arc::clone(&template));
            return Ok(template);
        }

        let mut template = parse_prepared_template(self.as_ref(), sql)?;
        template.stats_epoch = self.stats_epoch().0;
        template.optimizer_hash = self.optimizer_hash();
        let template = Arc::new(template);
        self.db
            .stmt_cache
            .insert(key.clone(), Arc::clone(&template));
        self.local_cache.insert(key, Arc::clone(&template));
        Ok(template)
    }
}

fn hash_optimizer(optimizer: &OptimizerConfig, query_memory: &QueryMemoryConfig) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    optimizer.hash(&mut hasher);
    query_memory.hash(&mut hasher);
    hasher.finish()
}

fn load_user_version(base: &Path) -> Result<i64> {
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

fn save_user_version(base: &Path, value: i64) -> Result<()> {
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

#[allow(dead_code)]
fn _keep_txn_use(_: Txn) {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;

    fn new_db() -> (tempfile::TempDir, Arc<Database>, Arc<Connection>) {
        new_db_with_timeout(Duration::from_secs(5))
    }

    fn new_db_with_timeout(
        timeout: Duration,
    ) -> (tempfile::TempDir, Arc<Database>, Arc<Connection>) {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("sql-conn-test.db");
        let opts = DbOptions {
            busy_timeout: timeout,
            ..DbOptions::default()
        };
        let db = Database::create(&path, opts).expect("db");
        let conn = db.connect();
        (dir, db, conn)
    }

    #[test]
    fn execute_uses_active_transaction() {
        let (_dir, db, conn1) = new_db();
        let conn2 = db.connect();

        conn1
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
            .expect("create");
        conn1.begin(BeginMode::Deferred).expect("begin");
        conn1
            .execute("INSERT INTO t VALUES (1, 'one')")
            .expect("insert");

        let mut stmt = conn2
            .prepare("SELECT v FROM t WHERE id = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Done);

        conn1.commit().expect("commit");

        let mut stmt = conn2
            .prepare("SELECT v FROM t WHERE id = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("value"), "one");
    }

    #[test]
    fn prepare_reuses_cached_templates() {
        let (_dir, _db, conn) = new_db();

        let stmt1 = conn.prepare("SELECT 1").expect("prepare");
        let stmt2 = conn.prepare("SELECT 1").expect("prepare");

        assert!(Arc::ptr_eq(&stmt1.template, &stmt2.template));
    }

    #[test]
    fn begin_immediate_reserves_writer_slot() {
        let (_dir, db, conn1) = new_db_with_timeout(Duration::from_millis(25));
        let conn2 = db.connect();

        conn1
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
            .expect("create");
        conn1.begin(BeginMode::Immediate).expect("begin immediate");

        let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
        assert_eq!(
            err,
            Error::Kernel(redlinedb_kernel::error::Error::LockTimeout)
        );

        conn1.rollback().expect("rollback");
    }

    #[test]
    fn set_busy_timeout_updates_future_lock_waits() {
        let (_dir, db, conn1) = new_db_with_timeout(Duration::from_secs(5));
        let conn2 = db.connect();

        conn1
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
            .expect("create");
        conn1.begin(BeginMode::Immediate).expect("begin immediate");
        conn2.set_busy_timeout(Duration::from_millis(25));

        let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
        assert_eq!(
            err,
            Error::Kernel(redlinedb_kernel::error::Error::LockTimeout)
        );

        conn1.rollback().expect("rollback");
    }
}
