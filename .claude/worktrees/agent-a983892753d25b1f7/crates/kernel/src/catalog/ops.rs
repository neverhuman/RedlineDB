use std::collections::HashMap;
use std::sync::Arc;

use super::affinity::derive_affinity;
use super::ddl::{
    AlterTableOperationSpec, AlterTableSpec, ColumnConstraintSpec, ConflictAction, CreateIndexSpec,
    CreateTableSpec, DropIndexSpec, DropTableSpec, IndexColumnSpec, IndexOrigin,
    TableConstraintSpec,
};
use super::expr::{ExprAst, compile_expr};
use super::ids::{ColumnId, ConstraintId, IndexId, ObjectId, SchemaId, TableId};
use super::key::{IndexKeyDef, IndexKeySource, NullOrder};
use super::names::{DbName, QualifiedName};
use super::schema::{
    CheckDef, ColumnDef, ConstraintDef, ConstraintKind, IndexDef, SchemaEpoch, SchemaSnapshot,
    TableDef,
};
use super::value::OwnedValue;
use crate::format::{PageId, RelId};
use crate::{Error, Result};

pub fn resolve_schema_id(snapshot: &SchemaSnapshot, schema: Option<&DbName>) -> Result<SchemaId> {
    match schema {
        Some(name) if name.folded() != "main" => {
            Err(Error::UnsupportedDdl("only the main schema is supported"))
        }
        _ => snapshot
            .lookup_namespace("main")
            .ok_or(Error::CatalogCorrupt("main schema missing")),
    }
}

pub fn lookup_table(snapshot: &SchemaSnapshot, name: &QualifiedName) -> Result<Arc<TableDef>> {
    let schema_id = resolve_schema_id(snapshot, Some(&name.schema))?;
    snapshot
        .lookup_table(schema_id, name.name.folded())
        .ok_or(Error::ObjectNotFound)
}

pub fn lookup_index(snapshot: &SchemaSnapshot, name: &QualifiedName) -> Result<Arc<IndexDef>> {
    let schema_id = resolve_schema_id(snapshot, Some(&name.schema))?;
    snapshot
        .lookup_index(schema_id, name.name.folded())
        .ok_or(Error::ObjectNotFound)
}

pub fn apply_create_table(
    mut snapshot: SchemaSnapshot,
    spec: CreateTableSpec,
) -> Result<SchemaSnapshot> {
    if spec.without_rowid {
        return Err(Error::UnsupportedDdl("WITHOUT ROWID is not supported yet"));
    }

    let schema_id = resolve_schema_id(&snapshot, spec.schema.as_ref())?;
    if snapshot
        .lookup_table(schema_id, spec.name.folded())
        .is_some()
    {
        if spec.if_not_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectExists);
    }

    let mut next_object_id = snapshot.meta.next_object_id;
    let mut next_relation_id = snapshot.meta.next_relation_id;
    let table_id = TableId(next_object_id.0);
    next_object_id.0 += 1;
    let relation_id = next_relation_id;
    next_relation_id.0 += 1;

    let mut used_names = HashMap::new();
    let mut columns = Vec::with_capacity(spec.columns.len());
    let mut column_lookup = HashMap::new();
    let mut constraints = Vec::new();
    let mut checks = Vec::new();
    let mut indexes = Vec::new();
    let mut rowid_alias_column = None;

    for (ordinal, column) in spec.columns.iter().enumerate() {
        let folded = column.name.folded().to_owned().into_boxed_str();
        if used_names.insert(folded.clone(), ()).is_some() {
            return Err(Error::ObjectExists);
        }
        let column_id = ColumnId(next_object_id.0);
        next_object_id.0 += 1;
        column_lookup.insert(folded.clone(), ordinal as u16);
        let affinity = derive_affinity(column.declared_type.as_deref());
        let mut not_null = false;
        let mut default_value = column.default_value.clone();
        let mut default_expr = default_const_expr(default_value.as_ref());
        for constraint in &column.constraints {
            match constraint {
                ColumnConstraintSpec::PrimaryKey { sort_dir, conflict } => {
                    not_null = true;
                    if rowid_alias_column.is_none() && affinity == super::Affinity::Integer {
                        rowid_alias_column = Some(ordinal as u16);
                    }
                    let constraint_id = ConstraintId(next_object_id.0);
                    next_object_id.0 += 1;
                    let index_id = IndexId(next_object_id.0);
                    next_object_id.0 += 1;
                    let index = IndexDef {
                        index_id,
                        table_id,
                        relation_id: next_relation_id,
                        meta_page_id: None,
                        name: format!("sqlite_autoindex_{}_{}", spec.name.original(), ordinal + 1)
                            .into_boxed_str(),
                        folded: format!("sqlite_autoindex_{}_{}", spec.name.folded(), ordinal + 1)
                            .into_boxed_str(),
                        unique: true,
                        primary: true,
                        origin: IndexOrigin::PrimaryKey,
                        keys: vec![IndexKeyDef {
                            ordinal: ordinal as u16,
                            source: IndexKeySource::Column {
                                attnum: ordinal as u16,
                            },
                            sort_dir: *sort_dir,
                            null_order: NullOrder::First,
                        }],
                        flags: 0,
                        normalized_sql: None,
                    };
                    next_relation_id.0 += 1;
                    indexes.push(index.clone());
                    constraints.push(ConstraintDef {
                        constraint_id,
                        table_id,
                        name: None,
                        kind: ConstraintKind::PrimaryKey,
                        column_id: Some(column_id),
                        index_id: Some(index_id),
                        expr: None,
                        conflict_action: *conflict,
                    });
                }
                ColumnConstraintSpec::Unique { conflict } => {
                    let constraint_id = ConstraintId(next_object_id.0);
                    next_object_id.0 += 1;
                    let index_id = IndexId(next_object_id.0);
                    next_object_id.0 += 1;
                    let index = IndexDef {
                        index_id,
                        table_id,
                        relation_id: next_relation_id,
                        meta_page_id: None,
                        name: format!("sqlite_autoindex_{}_{}", spec.name.original(), ordinal + 1)
                            .into_boxed_str(),
                        folded: format!("sqlite_autoindex_{}_{}", spec.name.folded(), ordinal + 1)
                            .into_boxed_str(),
                        unique: true,
                        primary: false,
                        origin: IndexOrigin::UniqueConstraint,
                        keys: vec![IndexKeyDef {
                            ordinal: ordinal as u16,
                            source: IndexKeySource::Column {
                                attnum: ordinal as u16,
                            },
                            sort_dir: super::SortDir::Asc,
                            null_order: NullOrder::First,
                        }],
                        flags: 0,
                        normalized_sql: None,
                    };
                    next_relation_id.0 += 1;
                    indexes.push(index.clone());
                    constraints.push(ConstraintDef {
                        constraint_id,
                        table_id,
                        name: None,
                        kind: ConstraintKind::Unique,
                        column_id: Some(column_id),
                        index_id: Some(index_id),
                        expr: None,
                        conflict_action: *conflict,
                    });
                }
                ColumnConstraintSpec::NotNull { conflict } => {
                    not_null = true;
                    constraints.push(ConstraintDef {
                        constraint_id: ConstraintId(next_object_id.0),
                        table_id,
                        name: None,
                        kind: ConstraintKind::NotNull,
                        column_id: Some(column_id),
                        index_id: None,
                        expr: None,
                        conflict_action: *conflict,
                    });
                    next_object_id.0 += 1;
                }
                ColumnConstraintSpec::Default {
                    expr,
                    normalized_sql: _,
                } => {
                    default_expr = Some(compile_expr(expr));
                    if let ExprAst::Const(value) = expr {
                        default_value = Some(value.clone());
                    }
                }
                ColumnConstraintSpec::Check { expr, .. } => {
                    let constraint_id = ConstraintId(next_object_id.0);
                    next_object_id.0 += 1;
                    checks.push(CheckDef {
                        constraint_id,
                        name: None,
                        expr: compile_expr(expr),
                    });
                }
            }
        }

        columns.push(ColumnDef {
            column_id,
            ordinal: ordinal as u16,
            name: column.name.original().into(),
            folded,
            declared_type: column
                .declared_type
                .clone()
                .map(|value| value.into_boxed_str()),
            affinity,
            not_null,
            default_value,
            default_expr,
        });
    }

    for constraint in &spec.constraints {
        match constraint {
            TableConstraintSpec::PrimaryKey {
                name,
                columns: pk_columns,
                conflict,
            } => {
                let (index, constraint_def) =
                    build_table_constraint_index(TableConstraintIndexInput {
                        table_name: &spec.name,
                        table_id,
                        next_object_id: &mut next_object_id,
                        next_relation_id: &mut next_relation_id,
                        origin: IndexOrigin::PrimaryKey,
                        unique: true,
                        columns: pk_columns,
                        conflict,
                        column_lookup: &column_lookup,
                    })?;
                constraints.push(ConstraintDef {
                    constraint_id: constraint_def,
                    table_id,
                    name: name.as_ref().map(|n| n.original().into()),
                    kind: ConstraintKind::PrimaryKey,
                    column_id: None,
                    index_id: Some(index.index_id),
                    expr: None,
                    conflict_action: *conflict,
                });
                indexes.push(index);
            }
            TableConstraintSpec::Unique {
                name,
                columns: unique_columns,
                conflict,
            } => {
                let (index, constraint_def) =
                    build_table_constraint_index(TableConstraintIndexInput {
                        table_name: &spec.name,
                        table_id,
                        next_object_id: &mut next_object_id,
                        next_relation_id: &mut next_relation_id,
                        origin: IndexOrigin::UniqueConstraint,
                        unique: true,
                        columns: unique_columns,
                        conflict,
                        column_lookup: &column_lookup,
                    })?;
                constraints.push(ConstraintDef {
                    constraint_id: constraint_def,
                    table_id,
                    name: name.as_ref().map(|n| n.original().into()),
                    kind: ConstraintKind::Unique,
                    column_id: None,
                    index_id: Some(index.index_id),
                    expr: None,
                    conflict_action: *conflict,
                });
                indexes.push(index);
            }
            TableConstraintSpec::Check {
                name,
                expr,
                normalized_sql: _,
            } => {
                let constraint_id = ConstraintId(next_object_id.0);
                next_object_id.0 += 1;
                checks.push(CheckDef {
                    constraint_id,
                    name: name.as_ref().map(|n| n.original().into()),
                    expr: compile_expr(expr),
                });
            }
        }
    }

    let table = Arc::new(TableDef {
        table_id,
        schema_id,
        relation_id,
        name: spec.name.original().into(),
        folded: spec.name.folded().into(),
        columns,
        indexes,
        constraints,
        checks,
        rowid_alias_column,
        flags: u64::from(spec.strict),
        normalized_sql: spec.normalized_sql.map(|sql| sql.into_boxed_str()),
    });

    snapshot.tables.push(table);
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.next_relation_id = next_relation_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

pub fn apply_drop_table(
    mut snapshot: SchemaSnapshot,
    spec: DropTableSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let folded = spec.name.name.folded().to_owned();
    let len_before = snapshot.tables.len();
    snapshot.tables.retain(|table| {
        !(table.schema_id == schema_id && table.folded.as_ref() == folded.as_str())
    });
    if snapshot.tables.len() == len_before {
        if spec.if_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

pub fn apply_create_index(
    mut snapshot: SchemaSnapshot,
    spec: CreateIndexSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, spec.schema.as_ref())?;
    if snapshot
        .lookup_index(schema_id, spec.name.folded())
        .is_some()
    {
        return Err(Error::ObjectExists);
    }
    let table = lookup_table(&snapshot, &spec.table)?;
    let mut next_object_id = snapshot.meta.next_object_id;
    let mut next_relation_id = snapshot.meta.next_relation_id;
    let index_id = IndexId(next_object_id.0);
    next_object_id.0 += 1;
    let relation_id = next_relation_id;
    next_relation_id.0 += 1;

    let keys = build_index_keys(&table, &spec.columns)?;
    let index = IndexDef {
        index_id,
        table_id: table.table_id,
        relation_id,
        meta_page_id: None,
        name: spec.name.original().into(),
        folded: spec.name.folded().into(),
        unique: spec.unique,
        primary: matches!(spec.origin, IndexOrigin::PrimaryKey),
        origin: spec.origin,
        keys,
        flags: 0,
        normalized_sql: spec.normalized_sql.map(|sql| sql.into_boxed_str()),
    };

    let mut updated = false;
    for slot in &mut snapshot.tables {
        if slot.table_id == table.table_id {
            let mut table = (**slot).clone();
            table.indexes.push(index.clone());
            *slot = Arc::new(table);
            updated = true;
            break;
        }
    }
    if !updated {
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.next_relation_id = next_relation_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

/// Update the `meta_page_id` for an index inside the snapshot. Lane A wires
/// this together with `apply_create_index` so the catalog snapshot durably
/// records the freshly-allocated B-tree meta page. Returns an error if the
/// index id is not present in the snapshot. Bumps the schema epoch so the
/// catalog snapshot is treated as fresh by recovery and consumers.
pub fn apply_set_index_meta_page_id(
    mut snapshot: SchemaSnapshot,
    index_id: IndexId,
    meta_page_id: PageId,
) -> Result<SchemaSnapshot> {
    let mut updated = false;
    for slot in &mut snapshot.tables {
        if slot.indexes.iter().any(|idx| idx.index_id == index_id) {
            let mut table = (**slot).clone();
            for idx in &mut table.indexes {
                if idx.index_id == index_id {
                    idx.meta_page_id = Some(meta_page_id);
                    updated = true;
                    break;
                }
            }
            *slot = Arc::new(table);
            break;
        }
    }
    if !updated {
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

pub fn apply_drop_index(
    mut snapshot: SchemaSnapshot,
    spec: DropIndexSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let Some(index) = snapshot.lookup_index(schema_id, spec.name.name.folded()) else {
        return if spec.if_exists {
            Ok(snapshot)
        } else {
            Err(Error::ObjectNotFound)
        };
    };
    if index.origin != IndexOrigin::User {
        return Err(Error::UnsupportedDdl(
            "constraint-backed indexes cannot be dropped",
        ));
    }
    let mut updated = false;
    for slot in &mut snapshot.tables {
        if slot.table_id == index.table_id {
            let mut table = (**slot).clone();
            table
                .indexes
                .retain(|candidate| candidate.index_id != index.index_id);
            *slot = Arc::new(table);
            updated = true;
            break;
        }
    }
    if !updated {
        if spec.if_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

pub fn apply_alter_table(
    mut snapshot: SchemaSnapshot,
    spec: AlterTableSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let Some(table_index) = snapshot.tables.iter().position(|table| {
        table.schema_id == schema_id && table.folded.as_ref() == spec.name.name.folded()
    }) else {
        return if spec.if_exists {
            Ok(snapshot)
        } else {
            Err(Error::ObjectNotFound)
        };
    };

    let mut table = (*snapshot.tables[table_index]).clone();
    let mut next_object_id = snapshot.meta.next_object_id;

    match spec.operation {
        AlterTableOperationSpec::RenameTable { table_name } => {
            let target_schema = resolve_schema_id(&snapshot, Some(&table_name.schema))?;
            if target_schema != schema_id {
                return Err(Error::UnsupportedDdl(
                    "ALTER TABLE rename across schemas is not supported",
                ));
            }
            if snapshot
                .lookup_table(target_schema, table_name.name.folded())
                .is_some()
            {
                return Err(Error::ObjectExists);
            }
            table.name = table_name.name.original().into();
            table.folded = table_name.name.folded().into();
        }
        AlterTableOperationSpec::RenameColumn { old_name, new_name } => {
            if table
                .columns
                .iter()
                .any(|column| column.folded.as_ref() == new_name.folded())
            {
                return Err(Error::ObjectExists);
            }
            let column = table
                .columns
                .iter_mut()
                .find(|column| column.folded.as_ref() == old_name.folded())
                .ok_or(Error::ObjectNotFound)?;
            column.name = new_name.original().into();
            column.folded = new_name.folded().into();
        }
        AlterTableOperationSpec::AddColumn {
            column,
            if_not_exists,
        } => {
            if table
                .columns
                .iter()
                .any(|existing| existing.folded.as_ref() == column.name.folded())
            {
                if if_not_exists {
                    return Ok(snapshot);
                }
                return Err(Error::ObjectExists);
            }
            if column.constraints.iter().any(|constraint| {
                !matches!(
                    constraint,
                    ColumnConstraintSpec::NotNull { .. } | ColumnConstraintSpec::Default { .. }
                )
            }) {
                return Err(Error::UnsupportedDdl(
                    "ALTER TABLE ADD COLUMN supports NOT NULL and DEFAULT only",
                ));
            }
            let not_null = column
                .constraints
                .iter()
                .any(|constraint| matches!(constraint, ColumnConstraintSpec::NotNull { .. }));
            if not_null && column.default_value.is_none() {
                return Err(Error::UnsupportedDdl(
                    "ALTER TABLE ADD COLUMN NOT NULL requires a default",
                ));
            }
            let ordinal = table.columns.len() as u16;
            let column_id = ColumnId(next_object_id.0);
            next_object_id.0 += 1;
            let declared_type = column.declared_type.clone();
            table.columns.push(ColumnDef {
                column_id,
                ordinal,
                name: column.name.original().into(),
                folded: column.name.folded().into(),
                declared_type: declared_type.clone().map(|value| value.into_boxed_str()),
                affinity: derive_affinity(declared_type.as_deref()),
                not_null,
                default_value: column.default_value.clone(),
                default_expr: default_const_expr(column.default_value.as_ref()),
            });
        }
        AlterTableOperationSpec::DropColumn { .. } => {
            // Lane SQL-D phase 10: ALTER TABLE ... DROP COLUMN reaches the
            // catalog only via parser-only paths today. The SQL crate is
            // expected to reject before it gets here; if the operation is
            // ever applied we surface an UnsupportedDdl rather than corrupt
            // the schema by silently dropping data we cannot rewrite.
            return Err(Error::UnsupportedDdl(
                "ALTER TABLE DROP COLUMN is parsed-only; execution not yet implemented",
            ));
        }
    }

    snapshot.tables[table_index] = Arc::new(table);
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

struct TableConstraintIndexInput<'a> {
    table_name: &'a DbName,
    table_id: TableId,
    next_object_id: &'a mut ObjectId,
    next_relation_id: &'a mut RelId,
    origin: IndexOrigin,
    unique: bool,
    columns: &'a [DbName],
    conflict: &'a ConflictAction,
    column_lookup: &'a HashMap<Box<str>, u16>,
}

fn build_table_constraint_index(
    input: TableConstraintIndexInput<'_>,
) -> Result<(IndexDef, ConstraintId)> {
    let constraint_id = ConstraintId(input.next_object_id.0);
    input.next_object_id.0 += 1;
    let index_id = IndexId(input.next_object_id.0);
    input.next_object_id.0 += 1;
    let keys = build_index_keys_from_names(input.columns, input.column_lookup)?;
    let index = IndexDef {
        index_id,
        table_id: input.table_id,
        relation_id: *input.next_relation_id,
        meta_page_id: None,
        name: format!(
            "sqlite_autoindex_{}_{}",
            input.table_name.folded(),
            index_id.0
        )
        .into_boxed_str(),
        folded: format!(
            "sqlite_autoindex_{}_{}",
            input.table_name.folded(),
            index_id.0
        )
        .into_boxed_str(),
        unique: input.unique,
        primary: matches!(input.origin, IndexOrigin::PrimaryKey),
        origin: input.origin,
        keys,
        flags: 0,
        normalized_sql: None,
    };
    input.next_relation_id.0 += 1;
    let _ = input.conflict;
    Ok((index, constraint_id))
}

fn build_index_keys(table: &TableDef, columns: &[IndexColumnSpec]) -> Result<Vec<IndexKeyDef>> {
    let mut keys = Vec::with_capacity(columns.len());
    for column in columns {
        let ordinal = table
            .columns
            .iter()
            .find(|candidate| candidate.folded.as_ref() == column.name.folded())
            .ok_or(Error::ColumnNotFound)?
            .ordinal;
        keys.push(IndexKeyDef {
            ordinal,
            source: IndexKeySource::Column { attnum: ordinal },
            sort_dir: column.sort_dir,
            null_order: NullOrder::First,
        });
    }
    Ok(keys)
}

fn build_index_keys_from_names(
    columns: &[DbName],
    column_lookup: &HashMap<Box<str>, u16>,
) -> Result<Vec<IndexKeyDef>> {
    let mut keys = Vec::with_capacity(columns.len());
    for column in columns {
        let ordinal = *column_lookup
            .get(column.folded())
            .ok_or(Error::ColumnNotFound)?;
        keys.push(IndexKeyDef {
            ordinal,
            source: IndexKeySource::Column { attnum: ordinal },
            sort_dir: super::SortDir::Asc,
            null_order: NullOrder::First,
        });
    }
    Ok(keys)
}

fn default_const_expr(value: Option<&OwnedValue>) -> Option<Arc<super::expr::CompiledExpr>> {
    value.map(|value| compile_expr(&ExprAst::Const(value.clone())))
}
