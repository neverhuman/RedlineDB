use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::error::Error as KernelError;
use redlinedb_kernel::index::UniqueKeyGuard as KernelUniqueKeyGuard;
use redlinedb_kernel::index::poly_hash_u64;

use crate::value::SqlValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginMode {
    Deferred,
    Immediate,
    Exclusive,
}

/// One entry in the per-transaction redo journal.
///
/// We capture the SQL text plus a 1-based-indexed binding vector so that on
/// `ROLLBACK TO sp` we can drop the kernel transaction wholesale and replay
/// the journal up to the savepoint's prefix length. Entries are only
/// recorded for non-readonly statements that succeeded (i.e. were applied to
/// the kernel tx) — pure SELECT / PRAGMA reads don't perturb the persistent
/// state and would only bloat replay.
#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub sql: String,
    /// 1-based bindings (slot 0 reserved) matching `Statement::bindings`.
    /// Empty for statements driven through `Connection::execute(sql)`
    /// directly.
    pub bindings: Vec<Option<SqlValue>>,
}

/// A savepoint frame captures the journal length and counter snapshot at the
/// moment `SAVEPOINT name` ran. Multiple frames may share a name (SQLite
/// allows shadowing); `RELEASE`/`ROLLBACK TO` pick the most recent match.
#[derive(Debug, Clone)]
pub struct SavepointFrame {
    pub name: String,
    pub journal_len: usize,
    pub changes: usize,
    pub total_changes: usize,
    pub last_insert_rowid: Option<i64>,
    /// True if this savepoint implicitly opened the surrounding transaction.
    /// When the last frame popped was implicit, RELEASE that frame commits
    /// the underlying tx (matching SQLite's autocommit-style semantics).
    pub implicit_tx: bool,
}

/// A deferred FK check buffered until COMMIT. We capture just enough state
/// to recompute the parent-row lookup at commit time: the child table and
/// row id (so we can re-read the latest child values) plus the FK index
/// inside `table.foreign_keys`. Resolving the parent table is done from the
/// schema snapshot at commit time, which keeps replay/rollback semantics
/// straightforward.
#[derive(Debug, Clone)]
pub struct DeferredFkCheck {
    pub child_table_id: u64,
    pub child_rowid: u64,
    pub fk_index: usize,
}

#[derive(Debug)]
pub struct SessionState {
    pub tx: Option<Txn>,
    pub failed: bool,
    pub changes: usize,
    pub total_changes: usize,
    pub foreign_keys: bool,
    /// Mirrors SQLite's `PRAGMA recursive_triggers`. SQLite defaults this
    /// to ON, so trigger bodies that fire DML matching another trigger
    /// recurse up to the depth cap. When OFF, the executor skips firing
    /// any trigger from within an existing trigger body.
    pub recursive_triggers: bool,
    /// Mirrors SQLite's `PRAGMA journal_mode`. Stored as a typed value;
    /// RedlineDB accepts `memory`, `off`, `delete`, and `wal` (which
    /// round-trips as the WAL-style mode already used internally).
    pub journal_mode: crate::statement::JournalMode,
    /// Mirrors SQLite's `PRAGMA synchronous`. The engine's fsync policy is
    /// workspace-wide so this field is recall-only — set to satisfy
    /// callers (ORMs) that probe the value at connection open.
    pub synchronous: crate::statement::SynchronousLevel,
    /// Mirrors SQLite's `PRAGMA temp_store`. The executor's spill helpers
    /// consult this when picking between in-memory and on-disk spill.
    pub temp_store: crate::statement::TempStoreMode,
    /// Mirrors SQLite's `PRAGMA cache_size`. Negative values mean "KiB",
    /// positive values mean "pages"; stored as written and exposed back to
    /// the caller. The underlying page cache budget is engine-managed.
    pub cache_size: i64,
    /// Mirrors SQLite's `PRAGMA query_only`. When set, the executor
    /// rejects any non-read statement before dispatching it.
    pub query_only: bool,
    /// Mirrors SQLite's `PRAGMA case_sensitive_like`.
    pub case_sensitive_like: bool,
    /// Mirrors SQLite's `PRAGMA defer_foreign_keys`. When ON, all FK
    /// constraints behave as if DEFERRABLE INITIALLY DEFERRED until the
    /// next COMMIT. Defaults OFF and is auto-cleared at each COMMIT.
    pub defer_foreign_keys: bool,
    /// Mirrors SQLite's `PRAGMA ignore_check_constraints`. When ON, the
    /// executor skips CHECK clause evaluation. Defaults OFF.
    pub ignore_check_constraints: bool,
    /// Mirrors SQLite's `PRAGMA trusted_schema`. Recall-only.
    pub trusted_schema: bool,
    /// Mirrors SQLite's `PRAGMA secure_delete`. Recall-only — RedlineDB
    /// always overwrites freed pages with zeros, matching the SQLite
    /// compile-time default.
    pub secure_delete: bool,
    /// Mirrors SQLite's `PRAGMA locking_mode`. Recall-only; RedlineDB's
    /// concurrency model already provides per-connection isolation.
    pub locking_mode: crate::statement::LockingMode,
    /// Mirrors SQLite's `PRAGMA busy_timeout`. Stored in ms; recall-only
    /// because RedlineDB uses per-database lock timeouts.
    pub busy_timeout_ms: i64,
    /// Mirrors SQLite's `PRAGMA application_id`. Recall-only header value.
    pub application_id: i64,
    /// Mirrors SQLite's `PRAGMA max_page_count`. Recall-only soft cap.
    pub max_page_count: i64,
    /// Mirrors SQLite's `PRAGMA cache_spill`. Recall-only.
    pub cache_spill: i64,
    /// Mirrors SQLite's `PRAGMA fullfsync`. Recall-only boolean (0/1).
    pub fullfsync: bool,
    /// Mirrors SQLite's `PRAGMA checkpoint_fullfsync`. Recall-only.
    pub checkpoint_fullfsync: bool,
    /// Mirrors SQLite's `PRAGMA auto_vacuum` mode. Recall-only since
    /// RedlineDB's storage engine does not implement page-level
    /// vacuuming.
    pub auto_vacuum: i64,
    /// Mirrors SQLite's `PRAGMA threads`. Recall-only worker thread cap.
    pub threads: i64,
    /// Mirrors SQLite's `PRAGMA mmap_size`. Recall-only — RedlineDB does
    /// not use mmap for storage.
    pub mmap_size: i64,
    /// Mirrors SQLite's `PRAGMA soft_heap_limit`. Recall-only.
    pub soft_heap_limit: i64,
    /// Mirrors SQLite's `PRAGMA hard_heap_limit`. Recall-only.
    pub hard_heap_limit: i64,
    /// Mirrors SQLite's `PRAGMA analysis_limit`. Recall-only — controls
    /// ANALYZE per-index row scan cap in upstream sqlite3.
    pub analysis_limit: i64,
    /// Mirrors SQLite's `PRAGMA automatic_index`. Recall-only.
    pub automatic_index: bool,
    /// Mirrors SQLite's `PRAGMA reverse_unordered_selects`. Recall-only.
    pub reverse_unordered_selects: bool,
    /// Mirrors SQLite's `PRAGMA writable_schema`. Recall-only — RedlineDB
    /// does not currently permit direct schema-table mutation.
    pub writable_schema: bool,
    /// Mirrors SQLite's `PRAGMA legacy_alter_table`. Recall-only.
    pub legacy_alter_table: bool,
    /// RedlineDB-specific. When ON, the CLI `.import` dot-command takes a
    /// batched-VALUES fast path that submits N rows per executor call
    /// (instead of one row per prepared.step). No engine-side effect — the
    /// flag is consulted by the CLI layer.
    pub redline_bulk_import: bool,
    /// Names created through CREATE TEMP TABLE in this connection. The
    /// current lightweight implementation stores their rows in the same
    /// engine but exposes them through sqlite_temp_schema.
    pub temp_tables: Vec<String>,
    /// SQLite AUTOINCREMENT state keyed by folded table name. Backed by the
    /// `sqlite_sequence` pseudo-table on the SQL surface.
    pub sqlite_sequences: std::collections::BTreeMap<String, i64>,
    /// Snapshot of `sqlite_sequences` captured when the current write
    /// transaction began. Used to restore AUTOINCREMENT state on rollback
    /// and savepoint rewind.
    pub sqlite_sequences_tx_snapshot: Option<std::collections::BTreeMap<String, i64>>,
    /// AUTOINCREMENT tables mutated in the current transaction/statement.
    /// Commit publishing merges only these keys so an older transaction
    /// cannot overwrite a sibling connection's unrelated sequence entries.
    pub sqlite_sequences_dirty: std::collections::BTreeSet<String>,
    pub last_insert_rowid: Option<i64>,
    pub unique_guards: Vec<UniqueKeyGuard>,
    /// Kernel-level unique-key reservations held until end-of-transaction.
    /// We must keep these alive across the heap insert AND the SQL-side
    /// commit/rollback to close the probe-then-insert race; dropping them
    /// inside the SQL `collect_unique_conflicts` helper reopened the race
    /// (two writers both saw "no duplicate" and both committed).
    pub kernel_unique_guards: Vec<KernelUniqueKeyGuard>,
    /// Per-tx replay journal — see `JournalEntry`. Cleared on commit/rollback.
    pub journal: Vec<JournalEntry>,
    /// Savepoint stack. Cleared on commit/rollback.
    pub savepoints: Vec<SavepointFrame>,
    /// True while the journal is being replayed; suppresses re-recording so
    /// replay does not feed itself.
    pub replay_in_progress: bool,
    /// Pending FK checks for `DEFERRABLE INITIALLY DEFERRED` constraints.
    /// Drained at COMMIT; if any entry still violates referential integrity
    /// the commit is aborted with a `ConstraintViolation` mirroring SQLite.
    pub deferred_fk_checks: Vec<DeferredFkCheck>,
    /// Track J — Postgres-style `CREATE SCHEMA <name>` namespaces registered
    /// on this connection. SQLite has no schema layer; we keep the name set
    /// so `<schema>.<table>` qualifier checks resolve and so the
    /// `pg_namespace` shim returns the same names back to introspection
    /// tools. Stored lowercase-folded for case-insensitive lookup.
    pub pg_schemas: std::collections::BTreeSet<String>,
    /// Track J — Postgres-style sequences keyed by folded sequence name.
    /// Drives nextval/currval/setval.
    pub pg_sequences: std::collections::BTreeMap<String, SequenceState>,
    /// Track J — `SET TRANSACTION ISOLATION LEVEL` recall value. Default
    /// is `ReadCommitted` (Postgres' default).
    pub transaction_isolation: crate::statement::TransactionIsolationLevel,
}

/// Track J — runtime state for a PostgreSQL-style sequence.
///
/// `last_value` is the most-recently generated value (Postgres' `currval`
/// semantics); `None` means `nextval` has not yet been called. `start` and
/// `increment` carry over the `CREATE SEQUENCE` options so a
/// `setval(..., false)` followed by `nextval` advances by the configured step.
#[derive(Clone, Debug)]
pub struct SequenceState {
    pub start: i64,
    pub increment: i64,
    pub last_value: Option<i64>,
}

impl SequenceState {
    pub fn new(start: i64, increment: i64) -> Self {
        Self {
            start,
            increment,
            last_value: None,
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tx: None,
            failed: false,
            changes: 0,
            total_changes: 0,
            foreign_keys: false,
            recursive_triggers: false,
            journal_mode: crate::statement::JournalMode::Memory,
            // SQLite's default is FULL (2). The previous Normal default
            // produced 1 against sqlite3 v3.53.1's 2 for PRAGMA synchronous.
            synchronous: crate::statement::SynchronousLevel::Full,
            temp_store: crate::statement::TempStoreMode::Default,
            cache_size: -2000,
            query_only: false,
            case_sensitive_like: false,
            defer_foreign_keys: false,
            ignore_check_constraints: false,
            trusted_schema: false,
            // SQLite's default is OFF (`0`) on a :memory: database; the
            // SECURE_DELETE compile flag flips it to ON, but the
            // reference v3.53.1 build the parity suite probes against
            // does not set that flag. Mirror its surface.
            secure_delete: false,
            locking_mode: crate::statement::LockingMode::Normal,
            busy_timeout_ms: 0,
            application_id: 0,
            // sqlite3 default for an empty :memory: db is 0xfffffffe (2^32 - 2)
            // — exposed as 4294967294. Stored as the same i64.
            max_page_count: 4_294_967_294,
            cache_spill: 483,
            fullfsync: false,
            checkpoint_fullfsync: false,
            auto_vacuum: 0,
            threads: 0,
            mmap_size: 0,
            soft_heap_limit: 0,
            hard_heap_limit: 0,
            analysis_limit: 0,
            automatic_index: true,
            reverse_unordered_selects: false,
            writable_schema: false,
            legacy_alter_table: false,
            redline_bulk_import: false,
            temp_tables: Vec::new(),
            sqlite_sequences: std::collections::BTreeMap::new(),
            sqlite_sequences_tx_snapshot: None,
            sqlite_sequences_dirty: std::collections::BTreeSet::new(),
            last_insert_rowid: None,
            unique_guards: Vec::new(),
            kernel_unique_guards: Vec::new(),
            journal: Vec::new(),
            savepoints: Vec::new(),
            replay_in_progress: false,
            deferred_fk_checks: Vec::new(),
            pg_schemas: {
                // Track J — seed the Postgres-shipped namespaces.
                let mut s = std::collections::BTreeSet::new();
                s.insert("public".to_owned());
                s.insert("pg_catalog".to_owned());
                s
            },
            pg_sequences: std::collections::BTreeMap::new(),
            transaction_isolation: crate::statement::TransactionIsolationLevel::ReadCommitted,
        }
    }
}

impl SessionState {
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.tx = None;
        self.failed = false;
        self.changes = 0;
        self.total_changes = 0;
        self.foreign_keys = false;
        self.recursive_triggers = false;
        self.journal_mode = crate::statement::JournalMode::Memory;
        self.synchronous = crate::statement::SynchronousLevel::Full;
        self.temp_store = crate::statement::TempStoreMode::Default;
        self.cache_size = -2000;
        self.query_only = false;
        self.case_sensitive_like = false;
        self.defer_foreign_keys = false;
        self.ignore_check_constraints = false;
        self.trusted_schema = false;
        self.secure_delete = false;
        self.locking_mode = crate::statement::LockingMode::Normal;
        self.busy_timeout_ms = 0;
        self.application_id = 0;
        self.max_page_count = 4_294_967_294;
        self.cache_spill = 483;
        self.fullfsync = false;
        self.checkpoint_fullfsync = false;
        self.auto_vacuum = 0;
        self.threads = 0;
        self.mmap_size = 0;
        self.soft_heap_limit = 0;
        self.hard_heap_limit = 0;
        self.analysis_limit = 0;
        self.automatic_index = true;
        self.reverse_unordered_selects = false;
        self.writable_schema = false;
        self.legacy_alter_table = false;
        self.redline_bulk_import = false;
        self.temp_tables.clear();
        self.sqlite_sequences.clear();
        self.sqlite_sequences_tx_snapshot = None;
        self.sqlite_sequences_dirty.clear();
        self.last_insert_rowid = None;
        self.unique_guards.clear();
        self.kernel_unique_guards.clear();
        self.journal.clear();
        self.savepoints.clear();
        self.replay_in_progress = false;
        self.deferred_fk_checks.clear();
        self.pg_schemas.clear();
        self.pg_schemas.insert("public".to_owned());
        self.pg_schemas.insert("pg_catalog".to_owned());
        self.pg_sequences.clear();
        self.sqlite_sequences.clear();
        self.sqlite_sequences_tx_snapshot = None;
        self.sqlite_sequences_dirty.clear();
        self.transaction_isolation = crate::statement::TransactionIsolationLevel::ReadCommitted;
    }

    /// Reset journal + savepoint stack at a transaction boundary.
    pub fn clear_savepoints(&mut self) {
        self.journal.clear();
        self.savepoints.clear();
        self.deferred_fk_checks.clear();
    }
}

#[derive(Debug, Default)]
pub struct UniqueLockTable {
    shards: Vec<Mutex<HashMap<Vec<u8>, UniqueLockState>>>,
    cvars: Vec<Condvar>,
    timeout: RwLock<Duration>,
}

#[derive(Clone, Copy, Debug, Default)]
struct UniqueLockState {
    owner: u64,
    depth: usize,
}

#[derive(Debug)]
pub struct UniqueKeyGuard {
    table: Arc<UniqueLockTable>,
    shard: usize,
    key: Vec<u8>,
    owner: u64,
}

impl UniqueLockTable {
    pub fn new(shards: usize, timeout: Duration) -> Arc<Self> {
        let shards = shards.max(1);
        let mut tables = Vec::with_capacity(shards);
        let mut cvars = Vec::with_capacity(shards);
        for _ in 0..shards {
            tables.push(Mutex::new(HashMap::new()));
            cvars.push(Condvar::new());
        }
        Arc::new(Self {
            shards: tables,
            cvars,
            timeout: RwLock::new(timeout),
        })
    }

    pub fn set_timeout(&self, timeout: Duration) {
        *self.timeout.write().expect("unique lock timeout poisoned") = timeout;
    }

    pub fn lock(
        self: &Arc<Self>,
        key: Vec<u8>,
        owner: u64,
    ) -> crate::error::Result<UniqueKeyGuard> {
        let shard = self.shard(&key);
        let mut map = self.shards[shard].lock().expect("unique lock poisoned");
        let timeout = *self.timeout.read().expect("unique lock timeout poisoned");
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let state = map.entry(key.clone()).or_default();
            if state.owner == 0 || state.owner == owner {
                state.owner = owner;
                state.depth += 1;
                return Ok(UniqueKeyGuard {
                    table: Arc::clone(self),
                    shard,
                    key,
                    owner,
                });
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(crate::error::Error::Kernel(KernelError::LockTimeout));
            }
            let wait = deadline.saturating_duration_since(now);
            let (next_map, timeout) = self.cvars[shard]
                .wait_timeout(map, wait)
                .expect("unique lock poisoned");
            map = next_map;
            if timeout.timed_out() {
                return Err(crate::error::Error::Kernel(KernelError::LockTimeout));
            }
        }
    }

    fn unlock(&self, shard: usize, key: Vec<u8>, owner: u64) {
        if let Ok(mut map) = self.shards[shard].lock() {
            if let Some(state) = map.get_mut(&key)
                && state.owner == owner
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
        poly_hash_u64(key) as usize % self.shards.len().max(1)
    }
}

impl Drop for UniqueKeyGuard {
    fn drop(&mut self) {
        self.table
            .unlock(self.shard, std::mem::take(&mut self.key), self.owner);
    }
}
