use super::expr::ExprAst;
use super::ids::{ColumnId, ConstraintId, IndexId, SchemaId, TableId};
use super::names::{DbName, QualifiedName};
use super::value::OwnedValue;
use crate::format::RelId;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConflictAction {
    Abort,
    Ignore,
    Replace,
}

/// Referential action attached to a foreign-key declaration's `ON DELETE`
/// or `ON UPDATE` clause. Mirrors `sqlparser::ast::ReferentialAction` and
/// SQLite's documented semantics; `NoAction` is SQLite's default when no
/// clause is supplied.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FkAction {
    NoAction,
    Restrict,
    SetNull,
    SetDefault,
    Cascade,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IndexOrigin {
    User,
    PrimaryKey,
    UniqueConstraint,
}

#[derive(Debug, Clone)]
pub struct CreateTableSpec {
    pub schema: Option<DbName>,
    pub name: DbName,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnSpec>,
    pub constraints: Vec<TableConstraintSpec>,
    pub strict: bool,
    pub without_rowid: bool,
    pub normalized_sql: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DropTableSpec {
    pub name: QualifiedName,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct CreateIndexSpec {
    pub schema: Option<DbName>,
    pub name: DbName,
    pub table: QualifiedName,
    pub unique: bool,
    pub columns: Vec<IndexColumnSpec>,
    pub origin: IndexOrigin,
    pub normalized_sql: Option<String>,
    /// A6 SQL-D: optional WHERE predicate for partial indexes, as
    /// verbatim SQL.
    pub predicate_sql: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DropIndexSpec {
    pub name: QualifiedName,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct AlterTableSpec {
    pub name: QualifiedName,
    pub if_exists: bool,
    pub operation: AlterTableOperationSpec,
}

/// Create a persisted view. `body_sql` is the verbatim SELECT text that
/// the binder re-parses at expansion time; `columns`, when non-empty,
/// renames the body's output columns.
#[derive(Debug, Clone)]
pub struct CreateViewSpec {
    pub schema: Option<DbName>,
    pub name: DbName,
    pub if_not_exists: bool,
    /// SQLite session-scoped view modifier.
    pub session_scoped: bool,
    pub columns: Vec<DbName>,
    pub body_sql: String,
    pub normalized_sql: Option<String>,
}

/// Drop a view by qualified name.
#[derive(Debug, Clone)]
pub struct DropViewSpec {
    pub name: QualifiedName,
    pub if_exists: bool,
}

/// Firing period for a trigger — BEFORE or AFTER row mutation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TriggerTimeKind {
    Before,
    After,
}

/// Firing event for a trigger — INSERT, UPDATE, or DELETE on the parent
/// table. `INSTEAD OF` is intentionally absent at this layer; it is
/// deferred to a followup task.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TriggerEventKind {
    Insert,
    Update,
    Delete,
}

/// Spec for `CREATE TRIGGER`. The body SQL is stored verbatim and
/// re-parsed at fire time.
#[derive(Debug, Clone)]
pub struct CreateTriggerSpec {
    pub schema: Option<DbName>,
    pub name: DbName,
    pub if_not_exists: bool,
    pub table: DbName,
    pub when_time: TriggerTimeKind,
    pub when_event: TriggerEventKind,
    /// Column filter for `UPDATE OF c1, c2 ...`. Empty when no filter
    /// was specified or the event is not UPDATE.
    pub when_cols: Vec<DbName>,
    /// Optional `WHEN` predicate SQL text (re-parsed at fire time).
    pub when_predicate_sql: Option<String>,
    /// Verbatim body SQL, re-emitted from the `BEGIN ... END` block via
    /// the parser's Display impl.
    pub body_sql: String,
    pub normalized_sql: Option<String>,
}

/// Drop a trigger by qualified name.
#[derive(Debug, Clone)]
pub struct DropTriggerSpec {
    pub name: QualifiedName,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub enum AlterTableOperationSpec {
    RenameTable {
        table_name: QualifiedName,
    },
    RenameColumn {
        old_name: DbName,
        new_name: DbName,
    },
    AddColumn {
        column: ColumnSpec,
        if_not_exists: bool,
    },
    /// Lane SQL-D phase 10: parsed-only. Execution layer should reject with
    /// a `not yet implemented` error when the target column exists.
    DropColumn {
        column_name: DbName,
        if_exists: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub name: DbName,
    pub declared_type: Option<String>,
    pub constraints: Vec<ColumnConstraintSpec>,
    pub collation: Option<String>,
    pub default_value: Option<OwnedValue>,
    /// A6 SQL-D: GENERATED ALWAYS AS (expr) [STORED|VIRTUAL]. None for
    /// ordinary columns.
    pub generated: Option<super::schema::GeneratedColumnSpec>,
}

#[derive(Debug, Clone)]
pub enum ColumnConstraintSpec {
    PrimaryKey {
        sort_dir: super::key::SortDir,
        conflict: ConflictAction,
    },
    Unique {
        conflict: ConflictAction,
    },
    NotNull {
        conflict: ConflictAction,
    },
    Default {
        expr: ExprAst,
        normalized_sql: String,
    },
    Check {
        expr: ExprAst,
        normalized_sql: String,
    },
}

#[derive(Debug, Clone)]
pub enum TableConstraintSpec {
    PrimaryKey {
        name: Option<DbName>,
        columns: Vec<DbName>,
        conflict: ConflictAction,
    },
    Unique {
        name: Option<DbName>,
        columns: Vec<DbName>,
        conflict: ConflictAction,
    },
    Check {
        name: Option<DbName>,
        expr: ExprAst,
        normalized_sql: String,
    },
    /// Declared FOREIGN KEY. Stored verbatim so the executor can verify
    /// referenced rows exist on INSERT/UPDATE and apply ON DELETE/UPDATE
    /// actions when the parent row is mutated.
    ForeignKey {
        name: Option<DbName>,
        /// Child-table columns participating in the FK.
        columns: Vec<DbName>,
        /// Parent table name (optionally qualified with schema).
        parent_table: DbName,
        /// Parent-table columns. Empty when omitted — caller fills with
        /// the parent's PK at validation time.
        parent_columns: Vec<DbName>,
        on_delete: FkAction,
        on_update: FkAction,
        deferred: bool,
    },
}

#[derive(Debug, Clone)]
pub struct IndexColumnSpec {
    pub name: DbName,
    pub sort_dir: super::key::SortDir,
    pub collation: Option<String>,
    /// A6 SQL-D: when the index column is an expression rather than a
    /// bare column name, the verbatim SQL fragment and the set of
    /// column ordinals it references. `None` for ordinary column keys.
    pub expr_sql: Option<String>,
    pub expr_referenced_cols: Vec<u16>,
}

#[allow(dead_code)]
pub(crate) fn _keep_type_use(_: (ColumnId, ConstraintId, IndexId, RelId, SchemaId, TableId)) {}
