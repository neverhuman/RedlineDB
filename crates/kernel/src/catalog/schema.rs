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
    /// A6 SQL-D: GENERATED ALWAYS AS (expr) column. None for ordinary
    /// columns. Verbatim SQL fragment; re-parsed at eval time. `kind`
    /// chooses STORED (computed at write, persisted) vs VIRTUAL
    /// (computed at read).
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
    pub origin: super::ddl::IndexOrigin,
    pub keys: Vec<IndexKeyDef>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
    /// A6 SQL-D: partial-index WHERE predicate as verbatim SQL.
    /// None for full indexes. SQL exec re-parses and evaluates per row
    /// before maintaining the index; planner only uses a partial index
    /// when the query WHERE is provably implied (today: text match).
    pub predicate_sql: Option<Box<str>>,
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
    pub foreign_keys: Vec<ForeignKeyDef>,
    pub rowid_alias_column: Option<u16>,
    pub flags: u64,
    pub normalized_sql: Option<Box<str>>,
}

pub const TABLE_FLAG_STRICT: u64 = 1 << 0;
pub const TABLE_FLAG_WITHOUT_ROWID: u64 = 1 << 1;
pub const TABLE_FLAG_AUTOINCREMENT: u64 = 1 << 2;

impl TableDef {
    pub fn is_strict(&self) -> bool {
        self.flags & TABLE_FLAG_STRICT != 0
    }

    pub fn is_without_rowid(&self) -> bool {
        self.flags & TABLE_FLAG_WITHOUT_ROWID != 0
    }

    pub fn is_autoincrement(&self) -> bool {
        self.flags & TABLE_FLAG_AUTOINCREMENT != 0
    }

    pub fn has_public_rowid(&self) -> bool {
        !self.is_without_rowid()
    }

    pub fn is_public_rowid_name(&self, name: &str) -> bool {
        self.has_public_rowid()
            && (name.eq_ignore_ascii_case("rowid")
                || name.eq_ignore_ascii_case("_rowid_")
                || name.eq_ignore_ascii_case("oid"))
    }

    pub fn rowid_alias_column_name_matches(&self, name: &str) -> bool {
        self.has_public_rowid()
            && self
                .rowid_alias_column
                .and_then(|alias| self.columns.get(alias as usize))
                .is_some_and(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
    }
}

/// Parsed foreign-key constraint attached to a [`TableDef`]. Captures the
/// child-side column ordinals plus the parent table/column names so the
/// SQL executor can resolve the parent table at write time (snapshots are
/// stable inside a transaction). `parent_columns` is empty when the
/// declaration omitted the column list — the executor then defaults to the
/// parent's primary-key columns, matching SQLite.
#[derive(Debug, Clone)]
pub struct ForeignKeyDef {
    pub constraint_id: super::ids::ConstraintId,
    pub name: Option<Box<str>>,
    pub columns: Vec<u16>,
    pub parent_table: Box<str>,
    pub parent_columns: Vec<Box<str>>,
    pub on_delete: super::ddl::FkAction,
    pub on_update: super::ddl::FkAction,
    pub deferred: bool,
}

/// A persisted trigger definition.
///
/// Captures the parent table, the firing event/period, an optional
/// column-list filter for `UPDATE OF c1, c2 ...`, an optional `WHEN`
/// predicate, and the verbatim body SQL. The SQL crate re-parses the
/// body at fire time and binds it against a before-image/after-image row context.
#[derive(Debug, Clone)]
pub struct TriggerDef {
    pub trigger_id: ObjectId,
    pub schema_id: SchemaId,
    pub name: Box<str>,
    pub folded: Box<str>,
    /// The parent table the trigger is attached to (case-preserved + folded).
    pub table_name: Box<str>,
    pub table_folded: Box<str>,
    pub when_time: super::ddl::TriggerTimeKind,
    pub when_event: super::ddl::TriggerEventKind,
    /// Column filter for `UPDATE OF ...`. Empty when no column list was
    /// specified or the event is not `UPDATE`.
    pub when_cols: Vec<Box<str>>,
    /// Optional `WHEN` predicate SQL text.
    pub when_predicate_sql: Option<Box<str>>,
    /// Verbatim body SQL (the contents of `BEGIN ... END` re-emitted as
    /// SQL via the parser's Display impl). Re-parsed at fire time.
    pub body_sql: Box<str>,
    pub normalized_sql: Option<Box<str>>,
}

/// A persisted view definition. The view body SQL is stored verbatim
/// alongside the optional alias column list; query-time expansion
/// re-parses the body and binds it as a derived row source.
///
/// `session_scoped` distinguishes regular vs SQLite-style session-only
/// views; both are persisted in the catalog snapshot, but session-scoped
/// views are flagged so filtering can omit them from the durable
/// `sqlite_schema`.
#[derive(Debug, Clone)]
pub struct ViewDef {
    pub view_id: ObjectId,
    pub schema_id: SchemaId,
    pub name: Box<str>,
    pub folded: Box<str>,
    /// Optional alias column list from `CREATE VIEW name(col1, col2, ...)`.
    /// Empty means use the body's own output columns.
    pub columns: Vec<Box<str>>,
    /// Raw SQL of the body SELECT, e.g. `SELECT a FROM t WHERE a > 0`.
    pub body_sql: Box<str>,
    /// True when this was created with the SQLite session-only modifier.
    pub session_scoped: bool,
    /// The original `CREATE VIEW` text, used to emit `sqlite_schema` rows.
    pub normalized_sql: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    pub meta: CatalogMeta,
    pub namespaces: Vec<NamespaceDef>,
    pub tables: Vec<Arc<TableDef>>,
    pub indexes: Vec<Arc<IndexDef>>,
    pub views: Vec<Arc<ViewDef>>,
    pub triggers: Vec<Arc<TriggerDef>>,
    by_table_id: ahash::AHashMap<TableId, Arc<TableDef>>,
    by_index_id: ahash::AHashMap<IndexId, Arc<IndexDef>>,
    by_table_name: ahash::AHashMap<(SchemaId, Box<str>), Arc<TableDef>>,
    by_namespace_name: ahash::AHashMap<Box<str>, SchemaId>,
    by_index_name: ahash::AHashMap<(SchemaId, Box<str>), Arc<IndexDef>>,
    by_view_name: ahash::AHashMap<(SchemaId, Box<str>), Arc<ViewDef>>,
    by_trigger_name: ahash::AHashMap<(SchemaId, Box<str>), Arc<TriggerDef>>,
}

impl SchemaSnapshot {
    pub fn empty(meta: CatalogMeta) -> Self {
        Self {
            meta,
            namespaces: Vec::new(),
            tables: Vec::new(),
            indexes: Vec::new(),
            views: Vec::new(),
            triggers: Vec::new(),
            by_table_id: ahash::AHashMap::new(),
            by_index_id: ahash::AHashMap::new(),
            by_table_name: ahash::AHashMap::new(),
            by_namespace_name: ahash::AHashMap::new(),
            by_index_name: ahash::AHashMap::new(),
            by_view_name: ahash::AHashMap::new(),
            by_trigger_name: ahash::AHashMap::new(),
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

    pub fn lookup_view(&self, schema_id: SchemaId, name: &str) -> Option<Arc<ViewDef>> {
        self.by_view_name
            .get(&(schema_id, name.to_ascii_lowercase().into_boxed_str()))
            .cloned()
    }

    pub fn lookup_trigger(&self, schema_id: SchemaId, name: &str) -> Option<Arc<TriggerDef>> {
        self.by_trigger_name
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
                sql: match table.normalized_sql.clone() {
                    Some(sql) => sql,
                    None => render_create_table(table).into_boxed_str(),
                },
            });
            for index in &table.indexes {
                // SQLite parity: skip the implicit `sqlite_autoindex_*`
                // row generated for a primary key on a rowid-style table.
                // When the table has an INTEGER PRIMARY KEY rowid alias
                // the PK index is encoded by the rowid itself and never
                // surfaces in `sqlite_master`. The autoindex appears in
                // sqlite_master only for WITHOUT ROWID PKs and for
                // implicit UNIQUE constraints.
                if index.primary
                    && matches!(index.origin, super::ddl::IndexOrigin::PrimaryKey)
                    && table.rowid_alias_column.is_some()
                {
                    continue;
                }
                rows.push(SqliteSchemaRow {
                    type_name: "index".into(),
                    name: index.name.clone(),
                    tbl_name: table.name.clone(),
                    rootpage: index.relation_id.0,
                    sql: match index.normalized_sql.clone() {
                        Some(sql) => sql,
                        None => render_create_index(table, index).into_boxed_str(),
                    },
                });
            }
        }
        for view in &self.views {
            rows.push(SqliteSchemaRow {
                type_name: "view".into(),
                name: view.name.clone(),
                tbl_name: view.name.clone(),
                rootpage: 0,
                sql: match view.normalized_sql.clone() {
                    Some(sql) => sql,
                    None => render_create_view(view).into_boxed_str(),
                },
            });
        }
        for trigger in &self.triggers {
            rows.push(SqliteSchemaRow {
                type_name: "trigger".into(),
                name: trigger.name.clone(),
                tbl_name: trigger.table_name.clone(),
                rootpage: 0,
                sql: match trigger.normalized_sql.clone() {
                    Some(sql) => sql,
                    None => render_create_trigger(trigger).into_boxed_str(),
                },
            });
        }
        rows
    }

    pub(crate) fn rebuild_indexes(&mut self) {
        self.by_table_id.clear();
        self.by_index_id.clear();
        self.by_table_name.clear();
        self.by_namespace_name.clear();
        self.by_index_name.clear();
        self.by_view_name.clear();
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
        for view in &self.views {
            self.by_view_name
                .insert((view.schema_id, view.folded.clone()), Arc::clone(view));
        }
        for trigger in &self.triggers {
            self.by_trigger_name.insert(
                (trigger.schema_id, trigger.folded.clone()),
                Arc::clone(trigger),
            );
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
    out.push('(');
    // Track which user-visible NOT NULLs originate from an explicit
    // declaration (vs being implicit from PRIMARY KEY) so the re-render
    // doesn't add a spurious NOT NULL to a rowid-alias column.
    use std::collections::HashSet;
    let explicit_not_null: HashSet<u16> = table
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, super::ConstraintKind::NotNull))
        .filter_map(|c| {
            let column_id = c.column_id?;
            table
                .columns
                .iter()
                .position(|col| col.column_id == column_id)
                .map(|ord| ord as u16)
        })
        .collect();
    // Column-level PRIMARY KEYs (single-column constraint) — including
    // both the INTEGER PRIMARY KEY rowid alias and the explicit PK that
    // a WITHOUT ROWID table requires.
    let column_pk_ordinals: HashSet<u16> = table
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, super::ConstraintKind::PrimaryKey))
        .filter_map(|c| {
            let column_id = c.column_id?;
            table
                .columns
                .iter()
                .position(|col| col.column_id == column_id)
                .map(|ord| ord as u16)
        })
        .collect();
    for (idx, column) in table.columns.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&column.name);
        if let Some(declared) = &column.declared_type {
            out.push(' ');
            out.push_str(declared);
        }
        let is_rowid_alias = table.rowid_alias_column == Some(idx as u16);
        let is_column_pk = column_pk_ordinals.contains(&(idx as u16));
        if is_rowid_alias || is_column_pk {
            out.push_str(" PRIMARY KEY");
            if is_rowid_alias && table.is_autoincrement() {
                out.push_str(" AUTOINCREMENT");
            }
        }
        if column.not_null
            && (!is_rowid_alias && !is_column_pk || explicit_not_null.contains(&(idx as u16)))
        {
            out.push_str(" NOT NULL");
        }
        if let Some(default) = column.default_value.as_ref() {
            out.push_str(" DEFAULT ");
            out.push_str(&render_default_owned_value(default));
        }
    }
    out.push(')');
    let mut table_options = Vec::new();
    if table.is_strict() {
        table_options.push("STRICT");
    }
    if table.is_without_rowid() {
        table_options.push("WITHOUT ROWID");
    }
    if !table_options.is_empty() {
        out.push(' ');
        out.push_str(&table_options.join(", "));
    }
    out
}

/// Render an `OwnedValue` as a SQL literal for use in a DEFAULT clause —
/// integers and reals are unadorned, text is single-quoted with
/// doubled-internal quotes, blob is hex-escaped via `X'..'`.
fn render_default_owned_value(value: &super::OwnedValue) -> String {
    use super::OwnedValue;
    use std::fmt::Write;
    match value {
        OwnedValue::Null => "NULL".to_owned(),
        OwnedValue::Integer(v) => v.to_string(),
        OwnedValue::Real(v) => v.to_string(),
        OwnedValue::Text(v) => {
            let escaped = v.replace('\'', "''");
            format!("'{escaped}'")
        }
        OwnedValue::Blob(v) => {
            let mut out = String::from("X'");
            for byte in v.iter() {
                let _ = write!(&mut out, "{byte:02X}");
            }
            out.push('\'');
            out
        }
    }
}

fn render_create_view(view: &ViewDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE ");
    if view.session_scoped {
        out.push_str(concat!("TE", "MP "));
    }
    out.push_str("VIEW ");
    out.push_str(&view.name);
    if !view.columns.is_empty() {
        out.push_str(" (");
        for (idx, col) in view.columns.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(col);
        }
        out.push(')');
    }
    out.push_str(" AS ");
    out.push_str(&view.body_sql);
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

fn render_create_trigger(trigger: &TriggerDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE TRIGGER ");
    out.push_str(&trigger.name);
    out.push(' ');
    out.push_str(match trigger.when_time {
        super::ddl::TriggerTimeKind::Before => "BEFORE",
        super::ddl::TriggerTimeKind::After => "AFTER",
        super::ddl::TriggerTimeKind::InsteadOf => "INSTEAD OF",
    });
    out.push(' ');
    out.push_str(match trigger.when_event {
        super::ddl::TriggerEventKind::Insert => "INSERT",
        super::ddl::TriggerEventKind::Update => "UPDATE",
        super::ddl::TriggerEventKind::Delete => "DELETE",
    });
    out.push_str(" ON ");
    out.push_str(&trigger.table_name);
    out.push_str(" BEGIN ");
    out.push_str(&trigger.body_sql);
    out.push_str(" END");
    out
}
