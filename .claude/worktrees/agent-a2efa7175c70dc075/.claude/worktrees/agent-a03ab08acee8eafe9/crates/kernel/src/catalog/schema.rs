use std::collections::HashMap;
use std::sync::Arc;

use crate::format::PageId;
use crate::format::RelId;

use super::affinity::Affinity;
use super::ids::{ColumnId, ConstraintId, IndexId, ObjectId, SchemaId, TableId};
use super::key::IndexKeyDef;
use super::value::OwnedValue;

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
    pub default_expr: Option<Arc<super::expr::CompiledExpr>>,
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
    pub origin: super::ddl::IndexOrigin,
    pub keys: Vec<IndexKeyDef>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct CheckDef {
    pub constraint_id: ConstraintId,
    pub name: Option<Box<str>>,
    pub expr: Arc<super::expr::CompiledExpr>,
}

#[derive(Debug, Clone)]
pub struct ConstraintDef {
    pub constraint_id: ConstraintId,
    pub table_id: TableId,
    pub name: Option<Box<str>>,
    pub kind: ConstraintKind,
    pub column_id: Option<ColumnId>,
    pub index_id: Option<IndexId>,
    pub expr: Option<Arc<super::expr::CompiledExpr>>,
    pub conflict_action: super::ddl::ConflictAction,
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
    pub rowid_alias_column: Option<u16>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    pub meta: CatalogMeta,
    pub namespaces: Vec<NamespaceDef>,
    pub tables: Vec<Arc<TableDef>>,
    pub indexes: Vec<Arc<IndexDef>>,
    by_table_id: HashMap<TableId, Arc<TableDef>>,
    by_index_id: HashMap<IndexId, Arc<IndexDef>>,
    by_table_name: HashMap<(SchemaId, Box<str>), Arc<TableDef>>,
    by_namespace_name: HashMap<Box<str>, SchemaId>,
    by_index_name: HashMap<(SchemaId, Box<str>), Arc<IndexDef>>,
}

impl SchemaSnapshot {
    pub fn empty(meta: CatalogMeta) -> Self {
        Self {
            meta,
            namespaces: Vec::new(),
            tables: Vec::new(),
            indexes: Vec::new(),
            by_table_id: HashMap::new(),
            by_index_id: HashMap::new(),
            by_table_name: HashMap::new(),
            by_namespace_name: HashMap::new(),
            by_index_name: HashMap::new(),
        }
    }

    pub fn lookup_table(&self, schema_id: SchemaId, name: &str) -> Option<Arc<TableDef>> {
        self.by_table_name
            .get(&(schema_id, name.to_ascii_lowercase().into_boxed_str()))
            .cloned()
    }

    pub fn table_by_id(&self, table_id: TableId) -> Option<Arc<TableDef>> {
        self.by_table_id.get(&table_id).cloned()
    }

    pub fn index_by_id(&self, index_id: IndexId) -> Option<Arc<IndexDef>> {
        self.by_index_id.get(&index_id).cloned()
    }

    pub fn lookup_namespace(&self, name: &str) -> Option<SchemaId> {
        self.by_namespace_name
            .get(&name.to_ascii_lowercase().into_boxed_str())
            .copied()
    }

    pub fn lookup_index(&self, schema_id: SchemaId, name: &str) -> Option<Arc<IndexDef>> {
        self.by_index_name
            .get(&(schema_id, name.to_ascii_lowercase().into_boxed_str()))
            .cloned()
    }

    pub fn sqlite_schema_rows(&self) -> Vec<SqliteSchemaRow> {
        let mut rows = Vec::new();
        for table in &self.tables {
            rows.push(SqliteSchemaRow {
                type_name: "table".into(),
                name: table.name.clone(),
                tbl_name: table.name.clone(),
                rootpage: 0,
                sql: table
                    .normalized_sql
                    .clone()
                    .unwrap_or_else(|| render_create_table(table).into_boxed_str()),
            });
            for index in &table.indexes {
                rows.push(SqliteSchemaRow {
                    type_name: "index".into(),
                    name: index.name.clone(),
                    tbl_name: table.name.clone(),
                    rootpage: index.relation_id.0,
                    sql: index
                        .normalized_sql
                        .clone()
                        .unwrap_or_else(|| render_create_index(table, index).into_boxed_str()),
                });
            }
        }
        rows
    }

    pub(crate) fn rebuild_indexes(&mut self) {
        self.by_table_id.clear();
        self.by_index_id.clear();
        self.by_table_name.clear();
        self.by_namespace_name.clear();
        self.by_index_name.clear();
        self.indexes.clear();
        for table in &self.tables {
            self.by_table_id.insert(table.table_id, Arc::clone(table));
            self.by_table_name
                .insert((table.schema_id, table.folded.clone()), Arc::clone(table));
            for index in &table.indexes {
                let index = Arc::new(index.clone());
                self.by_index_id.insert(index.index_id, Arc::clone(&index));
                self.by_index_name
                    .insert((table.schema_id, index.folded.clone()), Arc::clone(&index));
                self.indexes.push(index);
            }
        }
        for namespace in &self.namespaces {
            self.by_namespace_name
                .insert(namespace.folded.clone(), namespace.schema_id);
        }
    }
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

fn render_create_table(table: &TableDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE TABLE ");
    out.push_str(&table.name);
    out.push_str(" (");
    for (idx, column) in table.columns.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&column.name);
        if let Some(declared) = &column.declared_type {
            out.push(' ');
            out.push_str(declared);
        }
        if column.not_null {
            out.push_str(" NOT NULL");
        }
    }
    out.push(')');
    out
}

fn render_create_index(table: &TableDef, index: &IndexDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE ");
    if index.unique {
        out.push_str("UNIQUE ");
    }
    out.push_str("INDEX ");
    out.push_str(&index.name);
    out.push_str(" ON ");
    out.push_str(&table.name);
    out.push_str(" (...)");
    out
}
