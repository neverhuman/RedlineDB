use std::sync::Arc;

use redlinedb_kernel::catalog::{
    AlterTableSpec, CreateIndexSpec, CreateTableSpec, CreateTriggerSpec, CreateViewSpec,
    DropIndexSpec, DropTableSpec, DropTriggerSpec, DropViewSpec, SchemaEpoch, SqliteSchemaRow,
    TableDef,
};
use redlinedb_kernel::engine::Txn;
use redlinedb_kernel::format::RowId;
use sqlparser::ast::{Expr, OrderByExpr, SelectItem};

use crate::batch::{ExecContext, MaterializeNode, QueryMemoryBroker, RowBatch};
use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::exec::execute_prepared;
use crate::session::BeginMode;
use crate::value::SqlValue;

#[derive(Debug, Clone, Default)]
pub struct ParamLayout {
    pub(crate) slots: Vec<Option<String>>,
    pub(crate) named: std::collections::HashMap<String, usize>,
}

impl ParamLayout {
    pub fn push_anonymous(&mut self) -> usize {
        self.slots.push(None);
        self.slots.len()
    }

    pub fn push_numbered(&mut self, index: usize) -> usize {
        if index == 0 {
            return 0;
        }
        if self.slots.len() < index {
            self.slots.resize(index, None);
        }
        index
    }

    pub fn push_named(&mut self, name: String) -> usize {
        if let Some(&slot) = self.named.get(&name) {
            return slot;
        }
        self.slots.push(Some(name.clone()));
        let slot = self.slots.len();
        self.named.insert(name, slot);
        slot
    }

    pub fn slot_for_name(&self, name: &str) -> Option<usize> {
        if let Some(rest) = name.strip_prefix('?')
            && let Ok(slot) = rest.parse::<usize>()
        {
            return Some(slot);
        }
        if let Some(slot) = self.named.get(name).copied() {
            return Some(slot);
        }
        if let Some(slot) = self.named.get(&format!(":{name}")).copied() {
            return Some(slot);
        }
        if let Some(slot) = self.named.get(&format!("@{name}")).copied() {
            return Some(slot);
        }
        self.named.get(&format!("${name}")).copied()
    }

    pub fn count(&self) -> usize {
        self.slots.len()
    }
}

#[derive(Debug, Clone)]
pub struct PreparedTemplate {
    pub sql: Arc<str>,
    pub schema_epoch: SchemaEpoch,
    pub stats_epoch: u64,
    pub optimizer_hash: u64,
    pub param_layout: ParamLayout,
    pub output_columns: Arc<[String]>,
    pub readonly: bool,
    pub kind: PreparedKind,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PreparedKind {
    Begin(BeginMode),
    Commit,
    Rollback,
    Pragma(PragmaPlan),
    Reindex,
    Vacuum,
    VacuumInto {
        path: Arc<str>,
    },
    CreateTable(CreateTableSpec),
    CreateTempTable(CreateTableSpec),
    CreateTableAsSelect(CreateTableAsSelectSpec),
    CreateIndex(CreateIndexSpec),
    CreateView(CreateViewSpec),
    CreateTrigger(CreateTriggerSpec),
    DropTable(DropTableSpec),
    DropIndex(DropIndexSpec),
    DropView(DropViewSpec),
    DropTrigger(DropTriggerSpec),
    AlterTable(AlterTableSpec),
    Analyze(AnalyzePlan),
    Explain(ExplainPlan),
    Select(SelectPlan),
    Insert(InsertPlan),
    InsertView(InsertViewPlan),
    Update(UpdatePlan),
    Delete(DeletePlan),
    /// ATTACH DATABASE 'path' AS alias / DETACH DATABASE alias — minimal
    /// alias-map maintenance executed by [`crate::exec::attach::AttachPlan`].
    Attach(crate::exec::attach::AttachPlan),
    CrossDbSql(CrossDbSqlPlan),
    CreateVirtualTable(CreateVirtualTablePlan),
    /// Track K — SQL:2003 `MERGE INTO target USING source ON ... WHEN ...`
    /// dispatches to per-clause UPDATE / DELETE / INSERT actions against
    /// the target table.
    Merge(MergePlan),
}

/// Sentinel SQL prefix used to tag `PreparedTemplate`s built for
/// SAVEPOINT/RELEASE/ROLLBACK TO commands. The savepoint side-effects fire
/// during `Connection::prepare_v2`, so the resulting statement is
/// constructed with `runtime = Done` and never reaches the executor; the
/// prefix lets `Statement::step` short-circuit even if the caller resets and
/// re-steps it.
pub(crate) const SAVEPOINT_MARKER_SQL_PREFIX: &str = "\u{0}__redline_savepoint_marker__:";

/// True if `template` was produced by `Connection::prepare_v2` for a
/// savepoint command. We tag it via a SQL prefix because `PreparedKind` is a
/// closed enum that we cannot extend (lane SQL-A owns `exec.rs`).
pub(crate) fn is_savepoint_marker_template(template: &PreparedTemplate) -> bool {
    template.sql.starts_with(SAVEPOINT_MARKER_SQL_PREFIX)
}

#[derive(Debug, Clone)]
pub struct AnalyzePlan {
    pub table: Option<Arc<TableDef>>,
}

#[derive(Debug, Clone)]
pub struct CreateTableAsSelectSpec {
    pub table: CreateTableSpec,
    pub select: Option<SelectPlan>,
}

#[derive(Debug, Clone)]
pub struct CrossDbSqlPlan {
    pub alias: Arc<str>,
    pub sql: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct CreateVirtualTablePlan {
    pub name: Arc<str>,
    pub module: Arc<str>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainFormat {
    QueryPlan,
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct ExplainPlan {
    pub format: ExplainFormat,
    pub analyze: bool,
    pub inner: Arc<PreparedTemplate>,
}

#[derive(Debug, Clone)]
pub enum PragmaPlan {
    SetForeignKeys(bool),
    SetUserVersion(i64),
    SetRecursiveTriggers(bool),
    SetJournalMode(JournalMode),
    SetSynchronous(SynchronousLevel),
    SetTempStore(TempStoreMode),
    SetCacheSize(i64),
    SetQueryOnly(bool),
    SetCaseSensitiveLike(bool),
    WalCheckpoint,
    SetAnalysisLimit(i64),
    SetApplicationId(i64),
    SetAutoVacuum(i64),
    SetAutomaticIndex(bool),
    SetBusyTimeout(i64),
    SetCacheSpill(i64),
    SetCheckpointFullfsync(bool),
    SetDeferForeignKeys(bool),
    SetFullfsync(bool),
    SetHardHeapLimit(i64),
    SetIgnoreCheckConstraints(bool),
    SetLegacyAlterTable(bool),
    SetLockingMode(LockingMode),
    SetMaxPageCount(i64),
    SetMmapSize(i64),
    SetReverseUnorderedSelects(bool),
    SetSecureDelete(bool),
    SetSoftHeapLimit(i64),
    SetThreads(i64),
    SetTrustedSchema(bool),
    SetWritableSchema(bool),
}

/// SQLite-compatible `PRAGMA journal_mode` values. RedlineDB stores the
/// requested mode and exposes a truthful `wal` response for the WAL-style
/// journal it already uses internally; `truncate` and `persist` remain
/// rejected because their on-disk semantics are not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    Delete,
    Memory,
    Wal,
    Off,
}

impl JournalMode {
    pub fn as_str(self) -> &'static str {
        match self {
            JournalMode::Delete => "delete",
            JournalMode::Memory => "memory",
            JournalMode::Wal => "wal",
            JournalMode::Off => "off",
        }
    }
}

/// SQLite-compatible `PRAGMA synchronous` values. Stored on the session;
/// the underlying engine's fsync policy is workspace-wide so the value is
/// recall-only (documented in `docs/sqlite-parity.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronousLevel {
    Off = 0,
    Normal = 1,
    Full = 2,
    Extra = 3,
}

/// SQLite-compatible `PRAGMA temp_store` values. RedlineDB honours the
/// `MEMORY` selection by routing spill artifacts to in-memory buffers; the
/// `FILE` selection requires a caller-supplied temp root (see
/// `Database::create_in_memory`). `DEFAULT` means "engine default".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempStoreMode {
    Default = 0,
    File = 1,
    Memory = 2,
}

/// SQLite-compatible `PRAGMA locking_mode` values. RedlineDB does not
/// implement a literal file-locking surface — concurrency is handled by
/// the kernel transaction layer — but we accept and recall the value so
/// callers probing it (ORMs, migration tooling) see the SQLite-expected
/// strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockingMode {
    Normal,
    Exclusive,
}

impl LockingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            LockingMode::Normal => "normal",
            LockingMode::Exclusive => "exclusive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAlgorithm {
    Abort,
    Rollback,
    Fail,
    Ignore,
    Replace,
}

#[derive(Debug, Clone)]
pub enum InsertConflict {
    Sqlite(ConflictAlgorithm),
    Upsert(Box<UpsertPlan>),
}

#[derive(Debug, Clone)]
pub enum DmlValue {
    Expr(Expr),
    Default,
}

#[derive(Debug, Clone)]
pub struct UpsertPlan {
    pub target: Option<UpsertTarget>,
    pub action: UpsertAction,
}

#[derive(Debug, Clone)]
pub enum UpsertTarget {
    Columns(Vec<usize>),
    Constraint(Arc<str>),
}

#[derive(Debug, Clone)]
pub enum UpsertAction {
    DoNothing,
    DoUpdate(Box<UpsertUpdatePlan>),
}

#[derive(Debug, Clone)]
pub struct UpsertUpdatePlan {
    pub assignments: Vec<(usize, DmlValue)>,
    pub selection: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum SelectSource {
    Table(Arc<TableDef>),
    Tables(Vec<BoundTable>),
    Joined(JoinSource),
    CompoundAll(Vec<SelectPlan>),
    /// `UNION` (distinct), `INTERSECT`, `EXCEPT` — implemented in
    /// `crate::exec::set_ops`. Each branch is materialised, then combined
    /// according to [`CompoundSetOp`].
    CompoundSet {
        op: CompoundSetOp,
        branches: Vec<SelectPlan>,
    },
    SqliteSchema,
    SqliteTempSchema,
    StaticRows {
        rows: Arc<[Vec<crate::value::SqlValue>]>,
    },
    /// A CTE / named-subquery reference: pre-materialized rows whose
    /// column names are tracked so projections can resolve identifiers
    /// like `cte.col`. Produced by `crate::exec::cte`.
    Cte {
        name: Arc<str>,
        alias: Option<Arc<str>>,
        columns: Arc<[String]>,
        rows: Arc<[Vec<crate::value::SqlValue>]>,
    },
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundSetOp {
    /// `UNION` (distinct): dedup both sides, concatenate, dedup again.
    UnionDistinct,
    /// `INTERSECT`: rows in both sides (deduped).
    Intersect,
    /// `EXCEPT`: rows in left-not-in-right (deduped).
    Except,
}

#[derive(Debug, Clone)]
pub struct BoundTable {
    pub table: Arc<TableDef>,
    pub alias: Option<Arc<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone)]
pub struct JoinStep {
    pub right: BoundTable,
    pub kind: JoinKind,
    pub selection: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct JoinSource {
    pub base: BoundTable,
    pub joins: Vec<JoinStep>,
}

#[derive(Debug, Clone)]
pub struct SelectPlan {
    pub source: SelectSource,
    pub distinct: bool,
    /// Track K — Postgres `SELECT DISTINCT ON (exprs) ...` keeps the first
    /// row per distinct combination of `exprs`, where "first" is decided by
    /// any outer `ORDER BY`. Empty when no DISTINCT ON is requested. Holds
    /// at most one entry per logical "ON" expression.
    pub distinct_on: Vec<Expr>,
    pub projection: Vec<SelectItem>,
    pub selection: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByExpr>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct InsertPlan {
    pub table: Arc<TableDef>,
    pub columns: Vec<usize>,
    pub rows: Vec<Vec<DmlValue>>,
    pub source_select: Option<Box<SelectPlan>>,
    pub default_values: bool,
    pub returning: Option<Vec<SelectItem>>,
    pub conflict: Option<InsertConflict>,
}

#[derive(Debug, Clone)]
pub struct InsertViewPlan {
    pub view_name: Arc<str>,
    pub columns: Arc<[String]>,
    pub rows: Vec<Vec<DmlValue>>,
}

#[derive(Debug, Clone)]
pub struct UpdatePlan {
    pub table: Arc<TableDef>,
    pub assignments: Vec<(usize, DmlValue)>,
    pub selection: Option<Expr>,
    pub returning: Option<Vec<SelectItem>>,
}

#[derive(Debug, Clone)]
pub struct DeletePlan {
    pub table: Arc<TableDef>,
    pub selection: Option<Expr>,
    pub returning: Option<Vec<SelectItem>>,
}

/// Track K — Lowered plan for SQL:2003 `MERGE INTO target USING source ON ...`.
/// At execute time the dispatcher iterates source rows, looks for matching
/// target rows under the ON predicate, and applies the first WHEN-clause
/// whose AND-predicate holds.
#[derive(Debug, Clone)]
pub struct MergePlan {
    pub target: Arc<TableDef>,
    pub target_alias: Option<Arc<str>>,
    pub source: Arc<TableDef>,
    pub source_alias: Option<Arc<str>>,
    pub on: Expr,
    pub clauses: Vec<MergeClausePlan>,
}

/// Track K — A single `WHEN [NOT] MATCHED [AND pred] THEN <action>` clause
/// in a [`MergePlan`].
#[derive(Debug, Clone)]
pub enum MergeClausePlan {
    /// `WHEN MATCHED [AND pred] THEN UPDATE SET col = expr, ...`
    MatchedUpdate {
        predicate: Option<Expr>,
        assignments: Vec<(usize, DmlValue)>,
    },
    /// `WHEN MATCHED [AND pred] THEN DELETE`
    MatchedDelete { predicate: Option<Expr> },
    /// `WHEN NOT MATCHED [AND pred] THEN INSERT (col, ...) VALUES (expr, ...)`
    /// — `columns` lists target column ordinals; `values` lists exprs in the
    /// same order. Implicit columns (no list) expand to all non-generated
    /// columns at bind time.
    NotMatchedInsert {
        predicate: Option<Expr>,
        columns: Vec<usize>,
        values: Vec<DmlValue>,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Default)]
pub(crate) enum RuntimeState {
    #[default]
    Idle,
    Select(SelectRuntime),
    Done,
}

#[derive(Debug)]
pub(crate) struct ExecutionResult {
    pub(crate) runtime: RuntimeState,
    pub(crate) affected_rows: usize,
}

#[derive(Debug)]
pub(crate) struct SelectRuntime {
    pub(crate) tx: SelectRuntimeTx,
    pub(crate) restore_tx: bool,
    pub(crate) source: SelectRuntimeSource,
    pub(crate) selection: Option<Expr>,
    pub(crate) projection: Vec<SelectItem>,
    pub(crate) limit: usize,
    pub(crate) offset: usize,
    pub(crate) seen: usize,
    pub(crate) yielded: usize,
    pub(crate) memory: QueryMemoryBroker,
}

#[derive(Debug)]
pub(crate) enum SelectRuntimeTx {
    Owned(Txn),
    Borrowed(*mut Txn),
    Empty,
}

impl SelectRuntimeTx {
    pub(crate) fn as_mut(&mut self) -> Option<&mut Txn> {
        match self {
            Self::Owned(tx) => Some(tx),
            Self::Borrowed(ptr) => {
                // `ptr` is a `*mut Txn` borrowed from `Connection::txn` and is
                // guaranteed to outlive this `SelectRuntime` because the
                // runtime is dropped before the connection's transaction slot
                // is cleared (enforced by `Statement::reset` / `Drop for
                // Statement` calling `finalize_runtime`). Exclusive access is
                // held: the outer `with_current_connection` re-entrancy guard
                // prevents any other code path from acquiring `&mut Txn` while
                // this borrow is live.
                // SAFETY: ptr outlives runtime; re-entrancy guard enforces exclusivity.
                let tx = unsafe { ptr.as_mut() };
                debug_assert!(tx.is_some(), "borrowed transaction pointer must be valid");
                tx
            }
            Self::Empty => None,
        }
    }

    pub(crate) fn take_owned(&mut self) -> Option<Txn> {
        match std::mem::replace(self, Self::Empty) {
            Self::Owned(tx) => Some(tx),
            Self::Borrowed(ptr) => {
                *self = Self::Borrowed(ptr);
                None
            }
            Self::Empty => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SelectRuntimeSource {
    Table {
        table: Arc<TableDef>,
        rowids: Vec<RowId>,
        cursor: usize,
    },
    SqliteSchema {
        rows: Vec<SqliteSchemaRow>,
        cursor: usize,
    },
    StaticRows {
        rows: Arc<[Vec<crate::value::SqlValue>]>,
        cursor: usize,
    },
    Batched {
        node: MaterializeNode,
        ctx: ExecContext,
        batch: RowBatch,
        cursor: usize,
    },
    Empty,
}

#[derive(Debug)]
pub struct Statement {
    pub(crate) conn: Arc<Connection>,
    pub(crate) template: Arc<PreparedTemplate>,
    bindings: Vec<Option<SqlValue>>,
    runtime: RuntimeState,
    current_row: Option<Vec<SqlValue>>,
    affected_rows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Row,
    Done,
}

impl Statement {
    pub(crate) fn new(conn: Arc<Connection>, template: Arc<PreparedTemplate>) -> Self {
        let mut bindings = Vec::with_capacity(template.param_layout.count() + 1);
        bindings.resize(template.param_layout.count() + 1, None);
        Self {
            conn,
            template,
            bindings,
            runtime: RuntimeState::Idle,
            current_row: None,
            affected_rows: 0,
        }
    }

    /// Build a `Statement` whose execution has already completed. Used by
    /// `Connection::prepare_v2` for SAVEPOINT/RELEASE/ROLLBACK TO commands —
    /// the savepoint stack is updated synchronously during prepare, so the
    /// returned statement is a no-op marker that returns `Step::Done` and
    /// reports zero affected rows / no columns.
    pub(crate) fn new_completed(conn: Arc<Connection>, template: Arc<PreparedTemplate>) -> Self {
        let mut bindings = Vec::with_capacity(template.param_layout.count() + 1);
        bindings.resize(template.param_layout.count() + 1, None);
        Self {
            conn,
            template,
            bindings,
            runtime: RuntimeState::Done,
            current_row: None,
            affected_rows: 0,
        }
    }

    /// Internal binding helper used by replay — accepts a raw `SqlValue`
    /// without re-wrapping. Public binders go through the typed setters.
    pub(crate) fn bind_value(&mut self, index: usize, value: SqlValue) -> Result<()> {
        self.set_binding(index, value)
    }

    pub fn bind_null(&mut self, index: usize) -> Result<()> {
        self.set_binding(index, SqlValue::Null)
    }

    pub fn bind_i64(&mut self, index: usize, value: i64) -> Result<()> {
        self.set_binding(index, SqlValue::Integer(value))
    }

    pub fn bind_f64(&mut self, index: usize, value: f64) -> Result<()> {
        self.set_binding(index, SqlValue::Real(value))
    }

    pub fn bind_text(&mut self, index: usize, value: impl Into<Arc<str>>) -> Result<()> {
        self.set_binding(index, SqlValue::Text(value.into()))
    }

    pub fn bind_blob(&mut self, index: usize, value: impl Into<Arc<[u8]>>) -> Result<()> {
        self.set_binding(index, SqlValue::Blob(value.into()))
    }

    pub fn bind_named(&mut self, name: &str, value: SqlValue) -> Result<()> {
        let slot = self
            .template
            .param_layout
            .slot_for_name(name)
            .ok_or(Error::ParameterOutOfRange(0))?;
        self.set_binding(slot, value)
    }

    pub fn reset(&mut self) -> Result<()> {
        crate::exec::finalize_runtime(self.conn.as_ref(), &mut self.runtime)?;
        self.runtime = RuntimeState::Idle;
        self.current_row = None;
        self.affected_rows = 0;
        Ok(())
    }

    pub fn step(&mut self) -> Result<Step> {
        // Hoist `&Connection` out so the closure body retains exclusive
        // access to `self` for runtime mutation.
        let conn_ptr: *const Connection = self.conn.as_ref();
        // `conn_ptr` is derived from `self.conn.as_ref()`, where `self.conn`
        // is an `Arc<Connection>` owned by `self`. The `Arc` keeps the
        // `Connection` allocation alive for the duration of this method (and
        // longer, since this borrow is bounded by the closure passed to
        // `with_current_connection`). No `&mut Connection` exists anywhere
        // because `Connection` only exposes `&self` methods, so aliasing is
        // sound. The pointer is non-null because it comes from a reference.
        // SAFETY: Arc keeps Connection alive; only &self methods exposed.
        let conn: &Connection = unsafe { &*conn_ptr };
        crate::exec::with_current_connection(conn, || {
            if matches!(self.runtime, RuntimeState::Idle) {
                // Short-circuit the savepoint marker: its side-effects fired
                // at prepare-time. Detected via SQL prefix because the
                // PreparedKind enum is closed (we cannot add a dedicated
                // variant without modifying exec.rs).
                if is_savepoint_marker_template(&self.template) {
                    self.runtime = RuntimeState::Done;
                    return Ok(Step::Done);
                }
                if self.template.schema_epoch != self.conn.schema_epoch()
                    || self.template.stats_epoch != self.conn.stats_epoch().0
                    || self.template.optimizer_hash != self.conn.optimizer_hash()
                {
                    let new_template = self.conn.prepare_cached(self.template.sql.as_ref())?;
                    let mut new_bindings =
                        Vec::with_capacity(new_template.param_layout.count() + 1);
                    new_bindings.resize(new_template.param_layout.count() + 1, None);
                    for (idx, value) in self.bindings.iter().cloned().enumerate() {
                        if idx < new_bindings.len() {
                            new_bindings[idx] = value;
                        }
                    }
                    self.template = new_template;
                    self.bindings = new_bindings;
                }
                let result = execute_prepared(conn, &self.template, &self.bindings)?;
                self.affected_rows = result.affected_rows;
                self.runtime = result.runtime;
                // Journal this statement's SQL+bindings if the savepoint
                // stack is non-empty and this is a non-readonly mutation.
                // We deliberately journal AFTER `execute_prepared` succeeded
                // so a failing statement (e.g. constraint violation) does
                // not pollute the replay log. Pure SELECT/PRAGMA reads
                // skip — they can be re-issued by the caller post-rewind
                // and don't affect tx state.
                self.maybe_journal();
            }
            match &mut self.runtime {
                RuntimeState::Select(runtime) => {
                    let done = crate::exec::step_select_runtime(
                        conn,
                        runtime,
                        &self.bindings,
                        &mut self.current_row,
                    )?;
                    if done {
                        self.runtime = RuntimeState::Done;
                        Ok(Step::Done)
                    } else {
                        Ok(Step::Row)
                    }
                }
                RuntimeState::Done => Ok(Step::Done),
                RuntimeState::Idle => {
                    unreachable!("runtime state should be initialized before match")
                }
            }
        })
    }

    fn maybe_journal(&self) {
        if self.template.readonly {
            return;
        }
        // Skip kernel transaction-control statements — they are tracked by
        // the savepoint stack itself, not the journal.
        if matches!(
            self.template.kind,
            PreparedKind::Begin(_) | PreparedKind::Commit | PreparedKind::Rollback
        ) {
            return;
        }
        if is_savepoint_marker_template(&self.template) {
            return;
        }
        let _ = self
            .conn
            .journal_statement(self.template.sql.as_ref(), self.bindings.clone());
    }

    pub fn column_count(&self) -> usize {
        self.template.output_columns.len()
    }

    pub fn column_name(&self, index: usize) -> &str {
        self.template.output_columns[index].as_str()
    }

    pub fn column_value(&self, index: usize) -> Result<&SqlValue> {
        match self.current_row.as_ref().and_then(|row| row.get(index)) {
            Some(v) => Ok(v),
            None => Err(Error::Bind("no current row".to_owned())),
        }
    }

    pub fn column_i64(&self, index: usize) -> Result<i64> {
        match self.column_value(index)? {
            SqlValue::Integer(v) => Ok(*v),
            SqlValue::Real(v) => Ok(*v as i64),
            _ => Err(Error::DatatypeMismatch),
        }
    }

    pub fn column_f64(&self, index: usize) -> Result<f64> {
        match self.column_value(index)? {
            SqlValue::Integer(v) => Ok(*v as f64),
            SqlValue::Real(v) => Ok(*v),
            _ => Err(Error::DatatypeMismatch),
        }
    }

    pub fn column_text(&self, index: usize) -> Result<&str> {
        match self.column_value(index)? {
            SqlValue::Text(v) => Ok(v.as_ref()),
            _ => Err(Error::DatatypeMismatch),
        }
    }

    pub fn column_blob(&self, index: usize) -> Result<&[u8]> {
        match self.column_value(index)? {
            SqlValue::Blob(v) => Ok(v.as_ref()),
            _ => Err(Error::DatatypeMismatch),
        }
    }

    pub fn parameter_count(&self) -> usize {
        self.template.param_layout.count()
    }

    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.template.param_layout.slot_for_name(name)
    }

    pub fn clear_bindings(&mut self) {
        for slot in &mut self.bindings {
            *slot = None;
        }
    }

    pub fn is_readonly(&self) -> bool {
        self.template.readonly
    }

    pub fn is_busy(&self) -> bool {
        !matches!(self.runtime, RuntimeState::Idle | RuntimeState::Done)
    }

    pub fn sql(&self) -> &str {
        let raw = self.template.sql.as_ref();
        raw.strip_prefix(SAVEPOINT_MARKER_SQL_PREFIX).unwrap_or(raw)
    }

    pub fn affected_rows(&self) -> usize {
        self.affected_rows
    }

    pub fn template(&self) -> Arc<PreparedTemplate> {
        Arc::clone(&self.template)
    }

    fn set_binding(&mut self, index: usize, value: SqlValue) -> Result<()> {
        if index == 0 || index >= self.bindings.len() {
            return Err(Error::ParameterOutOfRange(index));
        }
        self.bindings[index] = Some(value);
        Ok(())
    }
}

impl Drop for Statement {
    fn drop(&mut self) {
        let _ = crate::exec::finalize_runtime(self.conn.as_ref(), &mut self.runtime);
    }
}
