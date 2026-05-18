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
    /// Phase-11 SQL-D A6: optional `CREATE INDEX ... WHERE <predicate>`
    /// fragment, stored as verbatim SQL so the executor can re-parse.
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
    /// Phase-11 SQL-D A6: GENERATED ALWAYS AS (expr) [STORED|VIRTUAL].
    pub generated: Option<GeneratedColumnDef>,
}

#[derive(Debug, Clone)]
pub struct GeneratedColumnDef {
    pub kind: super::schema::GeneratedColumnKind,
    pub expr_sql: String,
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
}

#[derive(Debug, Clone)]
pub struct IndexColumnSpec {
    pub name: DbName,
    pub sort_dir: super::key::SortDir,
    pub collation: Option<String>,
    /// Phase-11 SQL-D A6: optional expression source for this index key.
    /// When set, `name` is the synthetic key column label
    /// (`__expr_<n>`) and the expression is evaluated against the row
    /// values to produce the actual key bytes. `referenced_cols` lets
    /// the SQL update path know whether any input column changed.
    pub expr_sql: Option<String>,
    pub expr_referenced_cols: Vec<u16>,
}

#[allow(dead_code)]
pub(crate) fn _keep_type_use(_: (ColumnId, ConstraintId, IndexId, RelId, SchemaId, TableId)) {}
