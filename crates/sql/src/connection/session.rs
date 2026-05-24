use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use redlinedb_kernel::catalog::{SchemaEpoch, SchemaSnapshot, StatsEpoch, StatsSnapshot};
use redlinedb_kernel::engine::{CommitOutcome, Engine, Txn};
use redlinedb_kernel::txn::Isolation;

use crate::error::{Error, Result};
use crate::parser::parse_prepared_template;
use crate::parser::savepoint::{SavepointAction, try_parse_savepoint};
use crate::session::{BeginMode, JournalEntry, SavepointFrame, SessionState, UniqueLockTable};
use crate::statement::{PreparedTemplate, Statement, Step};

use super::cache::{StatementCache, StatementCacheKey};
use super::database::Database;
use super::options::{OptimizerConfig, QueryMemoryConfig, StatsConfig};

#[derive(Debug)]
pub struct Connection {
    pub(super) db: Arc<Database>,
    pub(super) session: Mutex<SessionState>,
    pub(super) local_cache: StatementCache,
    /// Per-connection ATTACH/DETACH alias map. Populated by
    /// `crate::exec::attach::apply_attach_plan` when the executor runs
    /// `PreparedKind::Attach`.
    pub(super) attach_map: crate::exec::attach::AttachMap,
}

impl Connection {
    /// Prepare a single SQL statement, ignoring any trailing statements.
    ///
    /// Most callers want this for backward compatibility — the returned
    /// statement is the *first* statement in `sql`, and any remaining text is
    /// silently dropped. Use [`Connection::prepare_v2`] for the
    /// `sqlite3_prepare_v2`-style API that returns the unconsumed tail.
    pub fn prepare(self: &Arc<Self>, sql: &str) -> Result<Statement> {
        match catch_unwind(AssertUnwindSafe(|| self.prepare_v2(sql))) {
            Ok(result) => {
                let (stmt, _tail) = result?;
                match stmt {
                    Some(stmt) => Ok(stmt),
                    None => Err(Error::UnsupportedSql(
                        "no statement in SQL input".to_owned(),
                    )),
                }
            }
            Err(payload) => Err(Error::Parse(format!(
                "sql parser panic: {}",
                crate::parser::panic_payload_to_string(payload)
            ))),
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
        match catch_unwind(AssertUnwindSafe(|| {
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
        })) {
            Ok(result) => result,
            Err(payload) => Err(Error::Parse(format!(
                "sql parser panic: {}",
                crate::parser::panic_payload_to_string(payload)
            ))),
        }
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
        // A6 SQLite parity: drain DEFERRABLE INITIALLY DEFERRED FK
        // checks before handing off to the kernel commit. A failure here
        // aborts the commit and rolls back the tx, matching SQLite's
        // statement-time deferred enforcement.
        if let Some(mut tx) = session.tx.take() {
            let drain_result =
                crate::exec::fk::drain_deferred_fk_checks(self, &mut session, &mut tx);
            if let Err(err) = drain_result {
                let _ = self.db.engine.rollback(tx);
                session.kernel_unique_guards.clear();
                session.unique_guards.clear();
                session.failed = false;
                session.clear_savepoints();
                return Err(err);
            }
            session.tx = Some(tx);
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
                // SQLite parity: `PRAGMA defer_foreign_keys` is a
                // single-transaction flag — it auto-clears at COMMIT
                // (and at ROLLBACK below) so the next tx starts with
                // FK enforcement back to the default.
                session.defer_foreign_keys = false;
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
        // A6 SQLite parity: ROLLBACK discards every pending deferred FK
        // check; the rolled-back rows never made it to the durable state.
        crate::exec::fk::clear_deferred_fk_checks(&mut session);
        // SQLite parity: `PRAGMA defer_foreign_keys` is auto-cleared
        // at the next transaction boundary.
        session.defer_foreign_keys = false;
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

    /// True for `:memory:` / ephemeral databases — used by SQLite-parity
    /// PRAGMA shapes (e.g. `journal_mode` always reports `memory` here).
    pub(crate) fn is_in_memory(&self) -> bool {
        self.db.is_in_memory()
    }

    pub(crate) fn foreign_keys(&self) -> bool {
        self.session.lock().expect("session poisoned").foreign_keys
    }

    pub(crate) fn set_foreign_keys(&self, value: bool) {
        self.session.lock().expect("session poisoned").foreign_keys = value;
    }

    pub(crate) fn recursive_triggers(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .recursive_triggers
    }

    pub(crate) fn set_recursive_triggers(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .recursive_triggers = value;
    }

    pub(crate) fn journal_mode(&self) -> crate::statement::JournalMode {
        self.session.lock().expect("session poisoned").journal_mode
    }

    pub(crate) fn set_journal_mode(&self, value: crate::statement::JournalMode) {
        self.session.lock().expect("session poisoned").journal_mode = value;
    }

    pub(crate) fn synchronous(&self) -> crate::statement::SynchronousLevel {
        self.session.lock().expect("session poisoned").synchronous
    }

    pub(crate) fn set_synchronous(&self, value: crate::statement::SynchronousLevel) {
        self.session.lock().expect("session poisoned").synchronous = value;
    }

    pub(crate) fn temp_store(&self) -> crate::statement::TempStoreMode {
        self.session.lock().expect("session poisoned").temp_store
    }

    pub(crate) fn set_temp_store(&self, value: crate::statement::TempStoreMode) {
        self.session.lock().expect("session poisoned").temp_store = value;
    }

    pub(crate) fn cache_size(&self) -> i64 {
        self.session.lock().expect("session poisoned").cache_size
    }

    pub(crate) fn set_cache_size(&self, value: i64) {
        self.session.lock().expect("session poisoned").cache_size = value;
    }

    pub(crate) fn query_only(&self) -> bool {
        self.session.lock().expect("session poisoned").query_only
    }

    pub(crate) fn set_query_only(&self, value: bool) {
        self.session.lock().expect("session poisoned").query_only = value;
    }

    pub(crate) fn case_sensitive_like(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .case_sensitive_like
    }

    pub(crate) fn set_case_sensitive_like(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .case_sensitive_like = value;
    }

    pub(crate) fn defer_foreign_keys(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .defer_foreign_keys
    }

    pub(crate) fn set_defer_foreign_keys(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .defer_foreign_keys = value;
    }

    pub(crate) fn ignore_check_constraints(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .ignore_check_constraints
    }

    pub(crate) fn set_ignore_check_constraints(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .ignore_check_constraints = value;
    }

    pub(crate) fn trusted_schema(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .trusted_schema
    }

    pub(crate) fn set_trusted_schema(&self, value: bool) {
        self.session.lock().expect("session poisoned").trusted_schema = value;
    }

    pub(crate) fn secure_delete(&self) -> bool {
        self.session.lock().expect("session poisoned").secure_delete
    }

    pub(crate) fn set_secure_delete(&self, value: bool) {
        self.session.lock().expect("session poisoned").secure_delete = value;
    }

    pub(crate) fn locking_mode(&self) -> crate::statement::LockingMode {
        self.session.lock().expect("session poisoned").locking_mode
    }

    pub(crate) fn set_locking_mode(&self, value: crate::statement::LockingMode) {
        self.session.lock().expect("session poisoned").locking_mode = value;
    }

    pub(crate) fn busy_timeout_ms(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .busy_timeout_ms
    }

    pub(crate) fn set_busy_timeout_ms(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .busy_timeout_ms = value;
    }

    pub(crate) fn application_id(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .application_id
    }

    pub(crate) fn set_application_id(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .application_id = value;
    }

    pub(crate) fn max_page_count(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .max_page_count
    }

    pub(crate) fn set_max_page_count(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .max_page_count = value;
    }

    pub(crate) fn cache_spill(&self) -> i64 {
        self.session.lock().expect("session poisoned").cache_spill
    }

    pub(crate) fn set_cache_spill(&self, value: i64) {
        self.session.lock().expect("session poisoned").cache_spill = value;
    }

    pub(crate) fn fullfsync(&self) -> bool {
        self.session.lock().expect("session poisoned").fullfsync
    }

    pub(crate) fn set_fullfsync(&self, value: bool) {
        self.session.lock().expect("session poisoned").fullfsync = value;
    }

    pub(crate) fn checkpoint_fullfsync(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .checkpoint_fullfsync
    }

    pub(crate) fn set_checkpoint_fullfsync(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .checkpoint_fullfsync = value;
    }

    pub(crate) fn auto_vacuum(&self) -> i64 {
        self.session.lock().expect("session poisoned").auto_vacuum
    }

    pub(crate) fn set_auto_vacuum(&self, value: i64) {
        self.session.lock().expect("session poisoned").auto_vacuum = value;
    }

    pub(crate) fn threads(&self) -> i64 {
        self.session.lock().expect("session poisoned").threads
    }

    pub(crate) fn set_threads(&self, value: i64) {
        self.session.lock().expect("session poisoned").threads = value;
    }

    /// PRAGMA mmap_size getter — currently unused at the surface because
    /// SQLite reports an empty result set when mmap is zero (the
    /// :memory:-default case); kept for API symmetry with the setter.
    #[allow(dead_code)]
    pub(crate) fn mmap_size(&self) -> i64 {
        self.session.lock().expect("session poisoned").mmap_size
    }

    pub(crate) fn set_mmap_size(&self, value: i64) {
        self.session.lock().expect("session poisoned").mmap_size = value;
    }

    pub(crate) fn soft_heap_limit(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .soft_heap_limit
    }

    pub(crate) fn set_soft_heap_limit(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .soft_heap_limit = value;
    }

    pub(crate) fn hard_heap_limit(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .hard_heap_limit
    }

    pub(crate) fn set_hard_heap_limit(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .hard_heap_limit = value;
    }

    pub(crate) fn analysis_limit(&self) -> i64 {
        self.session
            .lock()
            .expect("session poisoned")
            .analysis_limit
    }

    pub(crate) fn set_analysis_limit(&self, value: i64) {
        self.session
            .lock()
            .expect("session poisoned")
            .analysis_limit = value;
    }

    pub(crate) fn automatic_index(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .automatic_index
    }

    pub(crate) fn set_automatic_index(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .automatic_index = value;
    }

    pub(crate) fn reverse_unordered_selects(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .reverse_unordered_selects
    }

    pub(crate) fn set_reverse_unordered_selects(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .reverse_unordered_selects = value;
    }

    pub(crate) fn writable_schema(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .writable_schema
    }

    pub(crate) fn set_writable_schema(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .writable_schema = value;
    }

    pub(crate) fn legacy_alter_table(&self) -> bool {
        self.session
            .lock()
            .expect("session poisoned")
            .legacy_alter_table
    }

    pub(crate) fn set_legacy_alter_table(&self, value: bool) {
        self.session
            .lock()
            .expect("session poisoned")
            .legacy_alter_table = value;
    }

    pub(crate) fn user_version(&self) -> i64 {
        self.db.user_version()
    }

    pub(crate) fn set_user_version(&self, value: i64) -> Result<()> {
        self.db.set_user_version(value)
    }

    pub(crate) fn schema_snapshot(&self) -> Arc<SchemaSnapshot> {
        if let Some(snapshot) = crate::exec::current_tx_schema_snapshot(self) {
            return snapshot;
        }
        let session = self.session.lock().expect("session poisoned");
        if let Some(tx) = session.tx.as_ref() {
            return self.db.engine.schema_snapshot_for_tx(tx);
        }
        drop(session);
        self.db.schema_snapshot()
    }

    pub(crate) fn schema_epoch(&self) -> SchemaEpoch {
        self.schema_snapshot().meta.schema_epoch
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

    pub(crate) fn vacuum(&self) -> Result<()> {
        self.db.vacuum().map(|_| ())
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

    pub(crate) fn attach_map(&self) -> &crate::exec::attach::AttachMap {
        &self.attach_map
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
        // Set the per-thread current-connection slot so binders that need
        // to execute sub-queries during prepare (notably view-body
        // materialisation in `crate::exec::view`) can locate the
        // connection. The slot is cleared on closure exit, restoring any
        // previous value installed by an outer `Statement::step`.
        crate::exec::with_current_connection(self.as_ref(), || self.prepare_cached_inner(sql))
    }

    fn prepare_cached_inner(self: &Arc<Self>, sql: &str) -> Result<Arc<PreparedTemplate>> {
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
        // Some binders execute subqueries during prepare and embed the
        // resulting rows in the prepared template (views, CTE row stores,
        // table-valued functions). That template is valid for the returned
        // statement, but it must not be reused by either cache: base-table
        // DML does not advance the schema/stats cache key.
        if template_embeds_materialised_rows(&template) {
            return Ok(template);
        }
        self.db
            .stmt_cache
            .insert(key.clone(), Arc::clone(&template));
        self.local_cache.insert(key, Arc::clone(&template));
        Ok(template)
    }
}

/// True if the prepared template embeds rows materialised during binding.
/// Such templates are valid for the statement produced by this prepare
/// call, but must not be reused by the local or shared statement caches
/// because ordinary base-table DML does not change the cache key.
fn template_embeds_materialised_rows(template: &PreparedTemplate) -> bool {
    use crate::statement::{BoundTable, JoinSource, PreparedKind, SelectPlan, SelectSource};
    let plan = match &template.kind {
        PreparedKind::Select(plan) => plan,
        _ => return false,
    };

    fn table_embeds_materialised_rows(table: &BoundTable) -> bool {
        crate::exec::view::is_view_table_def(&table.table)
    }

    fn join_embeds_materialised_rows(join: &JoinSource) -> bool {
        table_embeds_materialised_rows(&join.base)
            || join
                .joins
                .iter()
                .any(|step| table_embeds_materialised_rows(&step.right))
    }

    fn plan_embeds_materialised_rows(plan: &SelectPlan) -> bool {
        source_embeds_materialised_rows(&plan.source)
    }

    fn source_embeds_materialised_rows(src: &SelectSource) -> bool {
        match src {
            SelectSource::Cte { .. } => true,
            SelectSource::Tables(tables) => tables.iter().any(table_embeds_materialised_rows),
            SelectSource::Joined(join) => join_embeds_materialised_rows(join),
            SelectSource::CompoundAll(branches) | SelectSource::CompoundSet { branches, .. } => {
                branches.iter().any(plan_embeds_materialised_rows)
            }
            _ => false,
        }
    }
    source_embeds_materialised_rows(&plan.source)
}

#[allow(dead_code)]
fn _keep_txn_use(_: Txn) {}
