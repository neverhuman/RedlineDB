use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use redlinedb_kernel::catalog::{StatsEpoch, StatsSnapshot};
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
        // Views (and other binders that execute sub-queries against the
        // live connection) embed the materialised rows into the template,
        // so the cached template is correct only for the schema/data
        // epoch captured at prepare time. We skip the shared db cache
        // when the template depends on the current connection's row
        // store; views are detected by checking whether the prepared
        // SELECT plan contains a `SelectSource::Cte` that did not come
        // from a CTE (i.e. is unbound in the current scope). For now we
        // simply cache locally to avoid cross-connection staleness.
        if !template_contains_view_materialisation(&template) {
            self.db
                .stmt_cache
                .insert(key.clone(), Arc::clone(&template));
        }
        self.local_cache.insert(key, Arc::clone(&template));
        Ok(template)
    }
}

/// True if the prepared template embeds view-materialised rows. Such
/// templates must not be shared across connections because the embedded
/// row set captures point-in-time data, not just schema. CTE templates
/// share the same `SelectSource::Cte` shape but are produced from query
/// SQL that contains `WITH`, so we conservatively skip caching any
/// template whose SQL is not a `WITH`-prefixed SELECT but still embeds
/// pre-materialised rows.
fn template_contains_view_materialisation(template: &PreparedTemplate) -> bool {
    use crate::statement::{PreparedKind, SelectSource};
    let plan = match &template.kind {
        PreparedKind::Select(plan) => plan,
        _ => return false,
    };
    fn source_has_view(src: &SelectSource) -> bool {
        match src {
            SelectSource::Cte { .. } => true,
            SelectSource::CompoundAll(branches) | SelectSource::CompoundSet { branches, .. } => {
                branches.iter().any(|p| source_has_view(&p.source))
            }
            _ => false,
        }
    }
    if !source_has_view(&plan.source) {
        return false;
    }
    let sql_trimmed = template.sql.trim_start().to_ascii_lowercase();
    // CTE prepares always start with `WITH`. Anything else that produced a
    // `SelectSource::Cte` came from a view or a TVF expansion — keep those
    // out of the cross-connection cache.
    !sql_trimmed.starts_with("with")
}

#[allow(dead_code)]
fn _keep_txn_use(_: Txn) {}
