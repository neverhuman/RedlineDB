use std::sync::Arc;

use crate::format::PageId;
use crate::format::RelId;

use crate::catalog::affinity::Affinity;
use crate::catalog::ddl::{
    ConflictAction, FkAction, IndexOrigin, TriggerEventKind, TriggerTimeKind,
};
use crate::catalog::ids::{ColumnId, ConstraintId, IndexId, ObjectId, SchemaId, TableId};
use crate::catalog::key::IndexKeyDef;
use crate::catalog::value::OwnedValue;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SchemaEpoch(pub u64);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ClassKind {
    Table = 1,
    Index = 2,
    SystemTable = 3,
    SystemIndex = 4,
}

#[derive(Debug, Clone)]
pub struct CatalogMeta {
    pub format_version: u64,
    pub schema_epoch: SchemaEpoch,
    pub next_object_id: ObjectId,
    pub next_relation_id: RelId,
    pub database_uuid: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct NamespaceDef {
    pub schema_id: SchemaId,
    pub name: Box<str>,
    pub folded: Box<str>,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub column_id: ColumnId,
    pub ordinal: u16,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub declared_type: Option<Box<str>>,
    pub affinity: Affinity,
    pub not_null: bool,
    pub default_value: Option<OwnedValue>,
    pub default_expr: Option<Arc<crate::catalog::expr::CompiledExpr>>,
    pub generated: Option<GeneratedColumnSpec>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneratedColumnKind {
    Stored = 0,
    Virtual = 1,
}

#[derive(Debug, Clone)]
pub struct GeneratedColumnSpec {
    pub kind: GeneratedColumnKind,
    pub expr_sql: Box<str>,
}

#[derive(Debug, Clone)]
pub struct IndexDef {
    pub index_id: IndexId,
    pub table_id: TableId,
    pub relation_id: RelId,
    pub meta_page_id: Option<PageId>,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub unique: bool,
    pub primary: bool,
    pub origin: IndexOrigin,
    pub keys: Vec<IndexKeyDef>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
    pub predicate_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct CheckDef {
    pub constraint_id: ConstraintId,
    pub name: Option<Box<str>>,
    pub expr: Arc<crate::catalog::expr::CompiledExpr>,
}

#[derive(Debug, Clone)]
pub struct ConstraintDef {
    pub constraint_id: ConstraintId,
    pub table_id: TableId,
    pub name: Option<Box<str>>,
    pub kind: ConstraintKind,
    pub column_id: Option<ColumnId>,
    pub index_id: Option<IndexId>,
    pub expr: Option<Arc<crate::catalog::expr::CompiledExpr>>,
    pub conflict_action: ConflictAction,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ConstraintKind {
    PrimaryKey = 1,
    Unique = 2,
    NotNull = 3,
    Check = 4,
    Default = 5,
}

#[derive(Debug, Clone)]
pub struct TableDef {
    pub table_id: TableId,
    pub schema_id: SchemaId,
    pub relation_id: RelId,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub columns: Vec<ColumnDef>,
    pub indexes: Vec<IndexDef>,
    pub constraints: Vec<ConstraintDef>,
    pub checks: Vec<CheckDef>,
    pub foreign_keys: Vec<ForeignKeyDef>,
    pub rowid_alias_column: Option<u16>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyDef {
    pub constraint_id: crate::catalog::ids::ConstraintId,
    pub name: Option<Box<str>>,
    pub columns: Vec<u16>,
    pub parent_table: Box<str>,
    pub parent_columns: Vec<Box<str>>,
    pub on_delete: FkAction,
    pub on_update: FkAction,
    pub deferred: bool,
}

#[derive(Debug, Clone)]
pub struct TriggerDef {
    pub trigger_id: ObjectId,
    pub schema_id: SchemaId,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub table_name: Box<str>,
    pub table_folded: Box<str>,
    pub when_time: TriggerTimeKind,
    pub when_event: TriggerEventKind,
    pub when_cols: Vec<Box<str>>,
    pub when_predicate_sql: Option<Box<str>>,
    pub body_sql: Box<str>,
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct ViewDef {
    pub view_id: ObjectId,
    pub schema_id: SchemaId,
    pub name: Box<str>,
    pub folded: Box<str>,
    pub columns: Vec<Box<str>>,
    pub body_sql: Box<str>,
    pub session_scoped: bool,
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteSchemaRow {
    pub type_name: Box<str>,
    pub name: Box<str>,
    pub tbl_name: Box<str>,
    pub rootpage: u64,
    pub sql: Box<str>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("catalog corruption: {0}")]
    Corrupt(&'static str),
}
