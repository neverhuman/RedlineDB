use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::ids::{IndexId, SchemaId, TableId};

#[path = "schema/render.rs"]
mod render;
#[path = "schema/types.rs"]
mod types;

pub use types::*;

#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    pub meta: CatalogMeta,
    pub namespaces: Vec<NamespaceDef>,
    pub tables: Vec<Arc<TableDef>>,
    pub indexes: Vec<Arc<IndexDef>>,
    pub views: Vec<Arc<ViewDef>>,
    pub triggers: Vec<Arc<TriggerDef>>,
    by_table_id: HashMap<TableId, Arc<TableDef>>,
    by_index_id: HashMap<IndexId, Arc<IndexDef>>,
    by_table_name: HashMap<(SchemaId, Box<str>), Arc<TableDef>>,
    by_namespace_name: HashMap<Box<str>, SchemaId>,
    by_index_name: HashMap<(SchemaId, Box<str>), Arc<IndexDef>>,
    by_view_name: HashMap<(SchemaId, Box<str>), Arc<ViewDef>>,
    by_trigger_name: HashMap<(SchemaId, Box<str>), Arc<TriggerDef>>,
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
            by_table_id: HashMap::new(),
            by_index_id: HashMap::new(),
            by_table_name: HashMap::new(),
            by_namespace_name: HashMap::new(),
            by_index_name: HashMap::new(),
            by_view_name: HashMap::new(),
            by_trigger_name: HashMap::new(),
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
                    None => render::render_create_table(table).into_boxed_str(),
                },
            });
            for index in &table.indexes {
                rows.push(SqliteSchemaRow {
                    type_name: "index".into(),
                    name: index.name.clone(),
                    tbl_name: table.name.clone(),
                    rootpage: index.relation_id.0,
                    sql: match index.normalized_sql.clone() {
                        Some(sql) => sql,
                        None => render::render_create_index(table, index).into_boxed_str(),
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
                    None => render::render_create_view(view).into_boxed_str(),
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
                    None => render::render_create_trigger(trigger).into_boxed_str(),
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
