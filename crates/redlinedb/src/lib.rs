mod backup;
mod error;
mod machine;
mod options;
mod params;
mod phase8;
mod registry;
mod value;

use std::cell::Cell;
use std::cmp::Ordering;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Duration;

pub use error::{Error, ErrorCode, Result};
pub use machine::{
    BinaryOp, ColumnRef, DeleteSpec, ExprSpec, InsertSpec, OrderSpec, QuerySpec, SchemaHandle,
    SelectSpec, TableRef, UnaryOp, UpdateSpec,
};
pub use options::{
    AnalyzeOptions, BackupOptions, BackupStats, BenchmarkStats, BufferStats, CheckpointBenchStats,
    CheckpointStats, CommitStats, ConnectionStats, DatabaseStats, Durability, ExecuteSummary,
    FunctionArity, FunctionFlags, MemoryOptions, OpenOptions, OptimizerOptions, QueryMemoryOptions,
    TxBenchStats, VacuumStats, WalBenchStats,
};
pub use params::Params;
pub use phase8::{
    ArchiveMode, ArchiveStats, PhysicalBackupOptions, PhysicalBackupStats, ReplicationSlot,
    ReplicationSlotStats, RestoreOptions, RestoreStats, RetentionHorizon, SlotKind, WalLevel,
};
pub use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId, WalSegmentNo};
pub use redlinedb_sql::RecoveryTarget;
pub use value::{Value, ValueRef};

pub use redlinedb_sql::BeginMode;

/// Thread-safe handle to a database image.
///
/// `Database` is cheap to clone, `Send + Sync`, and intended to be the
/// pooling boundary: open one `Database`, then hand out fresh
/// [`Connection`] values to worker threads as needed. All connections opened
/// from the same `Database` see the same state.
pub struct Database {
    inner: Arc<registry::DatabaseEntry>,
}

#[derive(Debug)]
pub struct OwnedStatement {
    inner: redlinedb_sql::Statement,
    interrupted: Arc<AtomicBool>,
    _marker: Rc<()>,
}

/// Per-session connection handle.
///
/// `Connection` is `Send` but not `Sync`. Move it between threads if you
/// need to, but do not share a single handle for concurrent mutation;
/// create one connection per thread or guard it with external locking.
pub struct Connection {
    inner: Arc<redlinedb_sql::Connection>,
    read_only: bool,
    busy_timeout: Duration,
    interrupted: Arc<AtomicBool>,
    _sync_marker: Cell<()>,
}

#[derive(Clone, Debug)]
pub struct Prepared {
    template: Arc<redlinedb_sql::PreparedTemplate>,
}

/// Borrowed prepared statement tied to one live [`Connection`].
///
/// `Statement` is intentionally not pooled or shared across threads. It
/// carries a mutable borrow of the parent connection, so keep it on the
/// thread that owns that connection and drop it before handing the
/// connection back to a pool.
pub struct Statement<'conn> {
    inner: OwnedStatement,
    _conn: std::marker::PhantomData<&'conn mut Connection>,
}

pub struct Row<'stmt> {
    stmt: &'stmt Statement<'stmt>,
}

pub enum Step<'a> {
    Row(Row<'a>),
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedStep {
    Row,
    Done,
}

pub struct Rows<'conn> {
    stmt: Statement<'conn>,
}

pub struct Transaction<'conn> {
    conn: &'conn mut Connection,
    committed: bool,
}

#[derive(Clone)]
pub struct InterruptHandle {
    flag: Arc<AtomicBool>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, OpenOptions::default())
    }

    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(
            path,
            OpenOptions {
                create: true,
                ..Default::default()
            },
        )
    }

    pub fn open_with_options(path: impl AsRef<Path>, options: OpenOptions) -> Result<Self> {
        let inner = registry::open_database(path, &options, options.create)?;
        Ok(Self { inner })
    }

    /// Create a private ephemeral database rooted under `options.temp_dir`
    /// when provided, otherwise under the system temp directory.
    ///
    /// Multiple connections opened from the returned [`Database`] share the
    /// same transient state. The backing directory disappears when the last
    /// `Database` owner drops.
    pub fn create_in_memory(options: OpenOptions) -> Result<Self> {
        let mut options = options;
        options.create = true;
        let inner = registry::create_in_memory_database(&options)?;
        Ok(Self { inner })
    }

    /// Create or reopen a process-local ephemeral database identified by
    /// `session_name`.
    ///
    /// Compatible calls with the same `session_name` reuse the same live
    /// session for as long as at least one [`Database`] handle is still
    /// alive. When the final owner drops, the owned temp root is removed.
    pub fn create_ephemeral(session_name: &str, options: OpenOptions) -> Result<Self> {
        let mut options = options;
        options.create = true;
        let inner = registry::create_ephemeral_database(session_name, &options)?;
        Ok(Self { inner })
    }

    pub fn connect(&self) -> Result<Connection> {
        let busy_timeout = *self
            .inner
            .busy_timeout
            .lock()
            .expect("busy timeout poisoned");
        Ok(Connection {
            inner: self.inner.db.connect(),
            read_only: self.inner.fingerprint.read_only,
            busy_timeout,
            interrupted: Arc::clone(&self.inner.interrupt),
            _sync_marker: Cell::new(()),
        })
    }

    pub fn set_busy_timeout(&self, timeout: Duration) {
        *self
            .inner
            .busy_timeout
            .lock()
            .expect("busy timeout poisoned") = timeout;
        self.inner.db.set_busy_timeout(timeout);
    }

    pub fn prepare(&self, sql: &str) -> Result<Prepared> {
        let mut conn = self.connect()?;
        let stmt = conn.prepare(sql)?;
        Ok(Prepared {
            template: stmt.template(),
        })
    }

    pub fn checkpoint(&self) -> Result<CheckpointStats> {
        let checkpoint = self.inner.db.checkpoint()?;
        let _ = phase8::update_retention(self);
        Ok(CheckpointStats {
            generation: checkpoint.control.generation,
            checkpoint_lsn: checkpoint.control.checkpoint_lsn.0,
            page_count: checkpoint.control.page_count,
            flushed_pages: checkpoint.flushed_pages,
            flush_batches: checkpoint.flush_batches,
        })
    }

    pub fn vacuum(&self) -> Result<VacuumStats> {
        let vacuum = self.inner.db.vacuum()?;
        Ok(VacuumStats {
            rows_scanned: vacuum.rows_scanned,
            chains_pruned: vacuum.chains_pruned,
            undo_links_removed: vacuum.undo_links_removed,
            dead_rows_removed: vacuum.dead_rows_removed,
            oldest_active_snapshot_csn: vacuum.oldest_active_snapshot_csn.0,
        })
    }

    pub fn stats(&self) -> Result<DatabaseStats> {
        let stats = self.inner.db.stats()?;
        Ok(DatabaseStats {
            schema_epoch: self.inner.db.schema_epoch().0,
            checkpoint_generation: stats.checkpoint.map(|control| control.generation),
            resident_heap_pages: stats.resident_heap_pages,
            wal_written_lsn: stats.wal_written_lsn.0,
            wal_durable_lsn: stats.wal_durable_lsn.0,
            vacuum_horizon_csn: stats.vacuum_horizon_csn.0,
            table_count: stats.tx.committed_states,
            column_count: stats.tx.active_transactions,
            index_count: stats.tx.active_snapshots,
        })
    }

    pub fn benchmark_stats(&self) -> Result<BenchmarkStats> {
        let stats = self.inner.db.stats()?;
        let retained_bytes = self
            .archive_stats()
            .map(|archive| archive.archived_bytes)
            .unwrap_or(0);
        Ok(BenchmarkStats {
            buffer: BufferStats {
                resident_pages: stats.buffer.resident_pages,
                reads: stats.buffer.reads,
                writes: stats.buffer.writes,
                evictions: stats.buffer.evictions,
                checkpoint_flushes: stats.buffer.checkpoint_flushes,
            },
            tx: TxBenchStats {
                next_tx: stats.tx.next_tx.0,
                next_csn: stats.tx.next_csn.0,
                published_csn: stats.tx.published_csn.0,
                active_transactions: stats.tx.active_transactions,
                active_snapshots: stats.tx.active_snapshots,
                committed_states: stats.tx.committed_states,
                pending_csns: stats.tx.pending_csns,
            },
            wal: WalBenchStats {
                written_lsn: stats.wal_written_lsn.0,
                durable_lsn: stats.wal_durable_lsn.0,
                retained_bytes,
                // Lane BH P1 #7: forward kernel WAL coordinator
                // syscall counters so the bench harness no longer
                // has to leave `process_metrics.fsync_count` etc.
                // as `None` on the macOS / no-strace paths.
                fsyncs_issued: stats.wal_sync_counters.fsyncs_issued,
                fdatasyncs_issued: stats.wal_sync_counters.fdatasyncs_issued,
                pwrites_issued: stats.wal_sync_counters.pwrites_issued,
                group_commits_issued: stats.wal_sync_counters.group_commits_issued,
                group_commit_batch_bytes_sum: stats.wal_sync_counters.group_commit_batch_bytes_sum,
                group_commit_batch_record_count_sum: stats
                    .wal_sync_counters
                    .group_commit_batch_record_count_sum,
                group_commit_batch_p50: stats.wal_sync_counters.batch_record_count_percentile(0.50),
                group_commit_batch_p95: stats.wal_sync_counters.batch_record_count_percentile(0.95),
                group_commit_batch_p99: stats.wal_sync_counters.batch_record_count_percentile(0.99),
                group_commit_batch_max: stats.wal_sync_counters.batch_record_count_max(),
            },
            checkpoint: CheckpointBenchStats {
                generation: stats.checkpoint.map(|control| control.generation),
                vacuum_horizon_csn: stats.vacuum_horizon_csn.0,
            },
            // Phase 11 Wave 0: forward the structural counter
            // snapshot. All fields are zero today; emission lands in
            // subsequent waves. `Some(_)` so manifests carry the
            // typed shape (Wave 1 dashboards can rely on a stable
            // schema) while `#[serde(skip_serializing_if =
            // "Option::is_none")]` keeps the door open for engines
            // that cannot supply the counters at all.
            phase11_counters: Some(stats.phase11_counters),
        })
    }

    pub fn backup_to_path(
        &self,
        dst: impl AsRef<Path>,
        options: BackupOptions,
    ) -> Result<BackupStats> {
        backup::backup_to_path(self, dst, options)
    }

    pub fn backup_physical_to_path(
        &self,
        dst: impl AsRef<Path>,
        options: PhysicalBackupOptions,
    ) -> Result<PhysicalBackupStats> {
        phase8::backup_physical_to_path(self, dst, options)
    }

    pub fn restore_from_backup(
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
        options: RestoreOptions,
    ) -> Result<RestoreStats> {
        phase8::restore_from_backup(src, dst, options)
    }

    pub fn create_physical_slot(&self, name: &str) -> Result<ReplicationSlot> {
        phase8::create_physical_slot(self, name, true)
    }

    pub fn create_logical_slot(&self, name: &str) -> Result<ReplicationSlot> {
        phase8::create_logical_slot(self, name, true)
    }

    pub fn drop_replication_slot(&self, name: &str) -> Result<()> {
        phase8::drop_replication_slot(self, name)
    }

    pub fn archive_stats(&self) -> Result<ArchiveStats> {
        phase8::archive_stats(self)
    }

    pub fn replication_slots(&self) -> Result<Vec<ReplicationSlotStats>> {
        phase8::replication_slots(self)
    }

    pub fn retention_horizon(&self) -> Result<RetentionHorizon> {
        phase8::retention_horizon(self)
    }

    pub fn database_id(&self) -> Result<DbId> {
        phase8::current_database_id(self)
    }

    pub fn interrupt_all(&self) {
        self.inner.interrupt.store(true, AtomicOrdering::Relaxed);
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Prepared {
    pub fn sql(&self) -> &str {
        self.template.sql.as_ref()
    }

    pub fn parameter_count(&self) -> usize {
        self.template.param_layout.count()
    }

    pub fn column_count(&self) -> usize {
        self.template.output_columns.len()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.template.output_columns[index].as_str()
    }

    pub fn is_readonly(&self) -> bool {
        self.template.readonly
    }
}

impl Connection {
    pub fn prepare<'c>(&'c mut self, sql: &str) -> Result<Statement<'c>> {
        Ok(Statement {
            inner: self.prepare_owned(sql)?,
            _conn: std::marker::PhantomData,
        })
    }

    pub fn prepare_owned(&mut self, sql: &str) -> Result<OwnedStatement> {
        self.check_interrupt()?;
        let stmt = self.inner.prepare(sql)?;
        if self.read_only && !stmt.is_readonly() {
            return Err(Error::new(ErrorCode::ReadOnly, "connection is read-only"));
        }
        Ok(OwnedStatement {
            inner: stmt,
            interrupted: Arc::clone(&self.interrupted),
            _marker: Rc::new(()),
        })
    }

    pub fn prepare_cached<'c>(&'c mut self, sql: &str) -> Result<Statement<'c>> {
        self.prepare(sql)
    }

    pub fn query<'c, P: Params>(&'c mut self, sql: &str, params: P) -> Result<Rows<'c>> {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        Ok(Rows { stmt })
    }

    pub fn execute<P: Params>(&mut self, sql: &str, params: P) -> Result<ExecuteSummary> {
        let mut stmt = self.prepare(sql)?;
        stmt.bind_all(params)?;
        let mut rows = 0_u64;
        while let Step::Row(_) = stmt.step()? {
            rows += 1;
        }
        Ok(ExecuteSummary {
            rows_affected: stmt.affected_rows() as u64,
            rows_returned: rows,
        })
    }

    pub fn begin(&mut self, mode: BeginMode) -> Result<()> {
        self.check_interrupt()?;
        self.inner.begin(mode)?;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<CommitStats> {
        self.check_interrupt()?;
        self.inner.commit()?;
        Ok(CommitStats {
            changes: self.inner.changes() as u64,
        })
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.inner.rollback()?;
        Ok(())
    }

    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        self.begin(BeginMode::Deferred)?;
        let mut tx = Transaction {
            conn: self,
            committed: false,
        };
        let result = f(&mut tx);
        if result.is_ok() && !tx.committed {
            tx.commit()?;
        } else if result.is_err() && !tx.committed {
            let _ = tx.rollback();
        }
        result
    }

    pub fn set_busy_timeout(&mut self, timeout: Duration) {
        self.busy_timeout = timeout;
        self.inner.set_busy_timeout(timeout);
    }

    pub fn interrupt_handle(&self) -> InterruptHandle {
        InterruptHandle {
            flag: Arc::clone(&self.interrupted),
        }
    }

    pub fn changes(&self) -> u64 {
        self.inner.changes() as u64
    }

    pub fn last_insert_rowid(&self) -> Option<i64> {
        self.inner.last_insert_rowid()
    }

    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            changes: self.changes(),
            last_insert_rowid: self.last_insert_rowid(),
            busy_timeout_ms: self.busy_timeout.as_millis() as u64,
            interrupted: self.interrupted.load(AtomicOrdering::Relaxed),
        }
    }

    pub fn create_scalar_function<F>(
        &mut self,
        _name: &str,
        _arity: FunctionArity,
        _flags: FunctionFlags,
        _f: F,
    ) -> Result<()>
    where
        F: Send + Sync + 'static + Fn(&[ValueRef<'_>]) -> Result<Value>,
    {
        Err(Error::unsupported(
            "scalar function hooks are not implemented yet",
        ))
    }

    pub fn create_collation<F>(&mut self, _name: &str, _cmp: F) -> Result<()>
    where
        F: Send + Sync + 'static + Fn(&str, &str) -> Ordering,
    {
        Err(Error::unsupported(
            "collation hooks are not implemented yet",
        ))
    }

    fn check_interrupt(&self) -> Result<()> {
        if self.interrupted.load(AtomicOrdering::Relaxed) {
            return Err(Error::new(ErrorCode::Interrupt, "interrupted"));
        }
        Ok(())
    }
}

impl<'conn> Statement<'conn> {
    pub fn bind_all<P: Params>(&mut self, params: P) -> Result<()> {
        params.bind_into(self)
    }

    pub fn bind_null(&mut self, index: usize) -> Result<()> {
        self.inner.bind_null(index)
    }

    pub fn bind_i64(&mut self, index: usize, value: i64) -> Result<()> {
        self.inner.bind_i64(index, value)
    }

    pub fn bind_f64(&mut self, index: usize, value: f64) -> Result<()> {
        self.inner.bind_f64(index, value)
    }

    pub fn bind_text(&mut self, index: usize, value: impl Into<Arc<str>>) -> Result<()> {
        self.inner.bind_text(index, value)
    }

    pub fn bind_blob(&mut self, index: usize, value: impl Into<Arc<[u8]>>) -> Result<()> {
        self.inner.bind_blob(index, value)
    }

    pub fn bind_value(&mut self, index: usize, value: Value) -> Result<()> {
        match value {
            Value::Null => self.bind_null(index),
            Value::Integer(value) => self.bind_i64(index, value),
            Value::Real(value) => self.bind_f64(index, value),
            Value::Text(value) => self.bind_text(index, value),
            Value::Blob(value) => self.bind_blob(index, value),
        }
    }

    pub fn bind_named(&mut self, name: &str, value: Value) -> Result<()> {
        self.inner.bind_named(name, value)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.inner.reset()?;
        Ok(())
    }

    pub fn clear_bindings(&mut self) {
        self.inner.clear_bindings();
    }

    pub fn step(&mut self) -> Result<Step<'_>> {
        match self.inner.step()? {
            OwnedStep::Row => Ok(Step::Row(Row { stmt: self })),
            OwnedStep::Done => Ok(Step::Done),
        }
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.is_readonly()
    }

    pub fn affected_rows(&self) -> usize {
        self.inner.affected_rows()
    }

    pub fn parameter_count(&self) -> usize {
        self.inner.parameter_count()
    }

    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.inner.parameter_index(name)
    }

    pub fn column_count(&self) -> usize {
        self.inner.column_count()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.inner.column_name(index)
    }

    pub fn template(&self) -> Arc<redlinedb_sql::PreparedTemplate> {
        self.inner.template()
    }
}

impl OwnedStatement {
    pub fn bind_null(&mut self, index: usize) -> Result<()> {
        Ok(self.inner.bind_null(index)?)
    }

    pub fn bind_i64(&mut self, index: usize, value: i64) -> Result<()> {
        Ok(self.inner.bind_i64(index, value)?)
    }

    pub fn bind_f64(&mut self, index: usize, value: f64) -> Result<()> {
        Ok(self.inner.bind_f64(index, value)?)
    }

    pub fn bind_text(&mut self, index: usize, value: impl Into<Arc<str>>) -> Result<()> {
        Ok(self.inner.bind_text(index, value)?)
    }

    pub fn bind_blob(&mut self, index: usize, value: impl Into<Arc<[u8]>>) -> Result<()> {
        Ok(self.inner.bind_blob(index, value)?)
    }

    pub fn bind_value(&mut self, index: usize, value: Value) -> Result<()> {
        match value {
            Value::Null => self.bind_null(index),
            Value::Integer(value) => self.bind_i64(index, value),
            Value::Real(value) => self.bind_f64(index, value),
            Value::Text(value) => self.bind_text(index, value),
            Value::Blob(value) => self.bind_blob(index, value),
        }
    }

    pub fn bind_named(&mut self, name: &str, value: Value) -> Result<()> {
        self.inner.bind_named(name, value.into())?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.inner.reset()?;
        Ok(())
    }

    pub fn clear_bindings(&mut self) {
        self.inner.clear_bindings();
    }

    pub fn step(&mut self) -> Result<OwnedStep> {
        if self.interrupted.load(AtomicOrdering::Relaxed) {
            return Err(Error::new(ErrorCode::Interrupt, "interrupted"));
        }
        Ok(match self.inner.step()? {
            redlinedb_sql::Step::Row => OwnedStep::Row,
            redlinedb_sql::Step::Done => OwnedStep::Done,
        })
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.is_readonly()
    }

    pub fn affected_rows(&self) -> usize {
        self.inner.affected_rows()
    }

    pub fn parameter_count(&self) -> usize {
        self.inner.parameter_count()
    }

    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.inner.parameter_index(name)
    }

    pub fn column_count(&self) -> usize {
        self.inner.column_count()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.inner.column_name(index)
    }

    pub fn column_ref(&self, index: usize) -> Result<ValueRef<'_>> {
        Ok(match self.inner.column_value(index)? {
            redlinedb_sql::SqlValue::Null => ValueRef::Null,
            redlinedb_sql::SqlValue::Integer(value) => ValueRef::Integer(*value),
            redlinedb_sql::SqlValue::Real(value) => ValueRef::Real(*value),
            redlinedb_sql::SqlValue::Text(value) => ValueRef::Text(value.as_ref()),
            redlinedb_sql::SqlValue::Blob(value) => ValueRef::Blob(value.as_ref()),
        })
    }

    pub fn column_i64(&self, index: usize) -> Result<i64> {
        Ok(self.inner.column_i64(index)?)
    }

    pub fn column_f64(&self, index: usize) -> Result<f64> {
        Ok(self.inner.column_f64(index)?)
    }

    pub fn column_text(&self, index: usize) -> Result<&str> {
        Ok(self.inner.column_text(index)?)
    }

    pub fn column_blob(&self, index: usize) -> Result<&[u8]> {
        Ok(self.inner.column_blob(index)?)
    }

    pub fn template(&self) -> Arc<redlinedb_sql::PreparedTemplate> {
        self.inner.template()
    }
}

impl<'stmt> Row<'stmt> {
    pub fn get<T: FromValue>(&self, index: usize) -> Result<T> {
        T::from_statement(self.stmt, index)
    }

    pub fn get_ref(&self, index: usize) -> Result<ValueRef<'_>> {
        self.stmt.inner.column_ref(index)
    }
}

impl<'conn> Rows<'conn> {
    pub fn step(&mut self) -> Result<Step<'_>> {
        self.stmt.step()
    }

    pub fn statement(&mut self) -> &mut Statement<'conn> {
        &mut self.stmt
    }
}

impl<'conn> Transaction<'conn> {
    pub fn execute<P: Params>(&mut self, sql: &str, params: P) -> Result<ExecuteSummary> {
        self.conn.execute(sql, params)
    }

    pub fn prepare<'a>(&'a mut self, sql: &str) -> Result<Statement<'a>> {
        self.conn.prepare(sql)
    }

    pub fn commit(&mut self) -> Result<()> {
        if !self.committed {
            let result = self.conn.commit();
            self.committed = true;
            result?;
        }
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.committed {
            self.conn.rollback()?;
            self.committed = true;
        }
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.rollback();
        }
    }
}

impl InterruptHandle {
    pub fn interrupt(&self) {
        self.flag.store(true, AtomicOrdering::Relaxed);
    }
}

pub trait FromValue: Sized {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self>;
}

impl FromValue for i64 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        stmt.inner.column_i64(index)
    }
}

impl FromValue for f64 {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        stmt.inner.column_f64(index)
    }
}

impl FromValue for String {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        Ok(stmt.inner.column_text(index)?.to_owned())
    }
}

impl FromValue for Value {
    fn from_statement(stmt: &Statement<'_>, index: usize) -> Result<Self> {
        Ok(match stmt.inner.column_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => Value::Integer(value),
            ValueRef::Real(value) => Value::Real(value),
            ValueRef::Text(value) => Value::Text(Arc::from(value)),
            ValueRef::Blob(value) => Value::Blob(Arc::from(value)),
        })
    }
}

fn sql_options(options: &OpenOptions) -> redlinedb_sql::DbOptions {
    let mut db = redlinedb_sql::DbOptions::default();
    let page_size = db.engine.page_size.max(1);
    let buffer_pages = (options.memory.cache_bytes / page_size).max(16);
    db.engine.buffer_pool_pages = buffer_pages;
    db.engine.busy_timeout = options.busy_timeout;
    db.engine.commit_durability = match options.durability {
        Durability::Strict => redlinedb_kernel::engine::CommitDurability::Strict,
        Durability::Normal => redlinedb_kernel::engine::CommitDurability::Normal,
        Durability::UnsafeDev => redlinedb_kernel::engine::CommitDurability::UnsafeDev,
    };
    db.optimizer.enabled = options.optimizer.enabled;
    db.optimizer.max_exact_join_tables = options.optimizer.max_exact_join_tables;
    db.optimizer.max_join_alternatives = options.optimizer.max_join_alternatives;
    db.optimizer.enable_multi_index_or = options.optimizer.enable_multi_index_or;
    db.optimizer.enable_multi_index_and = options.optimizer.enable_multi_index_and;
    db.optimizer.enable_covering_index = options.optimizer.enable_covering_index;
    db.query_memory.work_mem_bytes = options.query_memory.work_mem_bytes;
    db.query_memory.max_spill_bytes = options.query_memory.max_spill_bytes;
    db.query_memory.batch_rows = options.query_memory.batch_rows;
    db.temp_dir = options.temp_dir.clone();
    db.stats.exact_analyze_row_threshold = options.stats.exact_analyze_row_threshold;
    db.stats.sample_rows = options.stats.sample_rows;
    db.stats.mcv_capacity = options.stats.mcv_capacity;
    db.stats.histogram_buckets = options.stats.histogram_buckets;
    db
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn public_handle_thread_contracts_compile() {
        assert_send::<Database>();
        assert_sync::<Database>();
        assert_send::<Connection>();
    }

    #[test]
    fn owned_and_borrowed_statements_return_the_same_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("owned.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            (),
        )
        .expect("create");
        conn.execute(
            "INSERT INTO items(id, name) VALUES (?, ?)",
            params![1_i64, "Ada"],
        )
        .expect("insert");

        {
            let mut borrowed = conn
                .prepare("SELECT name FROM items WHERE id = ?")
                .expect("borrowed");
            borrowed.bind_all(params![1_i64]).expect("bind");
            match borrowed.step().expect("step") {
                Step::Row(row) => {
                    assert_eq!(row.get_ref(0).expect("ref"), ValueRef::Text("Ada"));
                }
                Step::Done => panic!("expected row"),
            }
            assert!(matches!(borrowed.step().expect("done"), Step::Done));
        }

        let mut owned = conn
            .prepare_owned("SELECT name FROM items WHERE id = ?")
            .expect("owned");
        owned.bind_value(1, Value::Integer(1)).expect("bind");
        match owned.step().expect("step") {
            OwnedStep::Row => {
                assert_eq!(owned.column_ref(0).expect("ref"), ValueRef::Text("Ada"));
            }
            OwnedStep::Done => panic!("expected row"),
        }
        assert!(matches!(owned.step().expect("done"), OwnedStep::Done));
    }

    #[test]
    fn benchmark_stats_surface_current_storage_counters() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("stats.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        conn.execute(
            "CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
            (),
        )
        .expect("create");
        conn.execute(
            "INSERT INTO kv(k, tenant, v, version) VALUES (?, ?, ?, ?)",
            params![1_i64, 1_i64, vec![1_u8, 2, 3], 1_i64],
        )
        .expect("insert");

        let stats = db.benchmark_stats().expect("benchmark stats");
        assert!(stats.buffer.resident_pages > 0);
        assert!(stats.wal.written_lsn >= stats.wal.durable_lsn);
        assert!(stats.tx.next_tx >= 1);
    }

    #[test]
    fn connection_set_busy_timeout_applies_to_future_lock_conflicts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("timeout.redline")).expect("db");
        let mut conn1 = db.connect().expect("conn1");
        let mut conn2 = db.connect().expect("conn2");

        conn1
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)", ())
            .expect("create");
        conn1.begin(BeginMode::Immediate).expect("begin immediate");
        conn2.set_busy_timeout(Duration::from_millis(25));

        let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
        assert_eq!(err.code(), ErrorCode::Busy);

        conn1.rollback().expect("rollback");
    }
}
