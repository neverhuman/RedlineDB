use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::error::Error as KernelError;
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::index::UniqueKeyGuard as KernelUniqueKeyGuard;

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
    /// 1-based bindings (slot 0 unused) matching `Statement::bindings`. Empty
    /// for statements driven through `Connection::execute(sql)` directly.
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

#[derive(Debug, Default)]
pub struct SessionState {
    pub tx: Option<Txn>,
    pub failed: bool,
    pub changes: usize,
    pub total_changes: usize,
    pub foreign_keys: bool,
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
}

impl SessionState {
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.tx = None;
        self.failed = false;
        self.changes = 0;
        self.total_changes = 0;
        self.foreign_keys = false;
        self.last_insert_rowid = None;
        self.unique_guards.clear();
        self.kernel_unique_guards.clear();
        self.journal.clear();
        self.savepoints.clear();
        self.replay_in_progress = false;
    }

    /// Reset journal + savepoint stack at a transaction boundary.
    pub fn clear_savepoints(&mut self) {
        self.journal.clear();
        self.savepoints.clear();
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

#[allow(dead_code)]
pub(crate) fn _keep_rowid_use(_: RowId) {}
