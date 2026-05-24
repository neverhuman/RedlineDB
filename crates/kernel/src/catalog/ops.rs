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
    CheckDef, ColumnDef, ConstraintDef, ConstraintKind, ForeignKeyDef, IndexDef, SchemaEpoch,
    SchemaSnapshot, TABLE_FLAG_STRICT, TABLE_FLAG_WITHOUT_ROWID, TableDef,
};
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
    let mut foreign_keys: Vec<ForeignKeyDef> = Vec::new();
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
        let mut default_expr = None;
        for constraint in &column.constraints {
            match constraint {
                ColumnConstraintSpec::PrimaryKey { sort_dir, conflict } => {
                    not_null = true;
                    if !spec.without_rowid
                        && rowid_alias_column.is_none()
                        && affinity == super::Affinity::Integer
                    {
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
                        predicate_sql: None,
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
                        predicate_sql: None,
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
                    if let ExprAst::Const(value) = expr {
                        default_value = Some(value.clone());
                    } else {
                        default_expr = Some(compile_expr(expr));
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
            generated: column.generated.clone(),
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
            TableConstraintSpec::ForeignKey {
                name,
                columns: fk_columns,
                parent_table,
                parent_columns,
                on_delete,
                on_update,
                deferred,
            } => {
                let constraint_id = ConstraintId(next_object_id.0);
                next_object_id.0 += 1;
                let mut ordinals = Vec::with_capacity(fk_columns.len());
                for col in fk_columns {
                    let folded = col.folded().to_owned().into_boxed_str();
                    match column_lookup.get(&folded) {
                        Some(idx) => ordinals.push(*idx),
                        None => return Err(Error::ObjectNotFound),
                    }
                }
                foreign_keys.push(ForeignKeyDef {
                    constraint_id,
                    name: name.as_ref().map(|n| n.original().into()),
                    columns: ordinals,
                    parent_table: parent_table.original().into(),
                    parent_columns: parent_columns.iter().map(|n| n.original().into()).collect(),
                    on_delete: *on_delete,
                    on_update: *on_update,
                    deferred: *deferred,
                });
            }
        }
    }

    let mut flags = 0;
    if spec.strict {
        flags |= TABLE_FLAG_STRICT;
    }
    if spec.without_rowid {
        flags |= TABLE_FLAG_WITHOUT_ROWID;
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
        foreign_keys,
        rowid_alias_column,
        flags,
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
        if spec.if_not_exists {
            return Ok(snapshot);
        }
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
        predicate_sql: spec.predicate_sql.map(|sql| sql.into_boxed_str()),
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
            let old_folded = table.folded.clone();
            table.name = table_name.name.original().into();
            table.folded = table_name.name.folded().into();
            // Force regeneration of the user-visible CREATE TABLE text
            // so `sqlite_master.sql` reflects the new name.
            table.normalized_sql = None;
            // SQLite-parity: when a table is renamed, dependent view
            // and trigger bodies reflect the new name (quoted with
            // double-quotes for safety) inside `sqlite_master.sql`.
            // PRAGMA legacy_alter_table=1 disables this rewrite, but
            // the kernel layer doesn't have access to that session
            // state — the surface layer can opt out by skipping the
            // rewrite call site.
            let new_original = table_name.name.original().to_owned();
            rewrite_dependent_view_table_refs(
                &mut snapshot,
                &old_folded,
                &new_original,
            );
            rewrite_dependent_trigger_table_refs(
                &mut snapshot,
                &old_folded,
                &new_original,
            );
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
            // sqlite_master must reflect the new column name.
            table.normalized_sql = None;
            // SQLite-parity: rewrite the column reference in dependent
            // view bodies and trigger bodies. We do a word-boundary
            // textual substitution (SQLite uses a full re-parse path,
            // but the surface tests only exercise simple identifier
            // references) so a column rename propagates into the
            // sqlite_master.sql text of any dependent object.
            let table_folded = table.folded.clone();
            let old_folded = old_name.folded().to_owned();
            let new_original = new_name.original().to_owned();
            rewrite_dependent_view_bodies(&mut snapshot, &table_folded, &old_folded, &new_original);
            rewrite_dependent_trigger_bodies(
                &mut snapshot,
                &table_folded,
                &old_folded,
                &new_original,
            );
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
                default_expr: None,
                generated: None,
            });
            // Force CREATE TABLE re-render so the new column appears in
            // sqlite_master.sql alongside the existing definitions.
            table.normalized_sql = None;
        }
        AlterTableOperationSpec::DropColumn {
            column_name,
            if_exists,
        } => {
            apply_drop_column(&mut table, column_name.folded(), if_exists)?;
        }
    }

    snapshot.tables[table_index] = Arc::new(table);
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

thread_local! {
    /// `PRAGMA legacy_alter_table` toggle, read by the dependent-object
    /// rewrite helpers. The SQL layer installs the active connection's
    /// value in a per-thread cell before invoking the kernel ALTER
    /// path. When the flag is set, the rewrite helpers leave dependent
    /// view / trigger bodies untouched.
    static LEGACY_ALTER_TABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// SAFETY: pure per-thread mutation, no aliasing concerns.
pub fn set_legacy_alter_table(value: bool) {
    LEGACY_ALTER_TABLE.with(|c| c.set(value));
}

/// Snapshot the current `legacy_alter_table` flag so callers can save +
/// restore around an `alter_table` invocation. Suffix avoids collision
/// with a future production-side accessor that might have a different
/// signature; for now it is the only read path.
pub fn legacy_alter_table_active_for_tests() -> bool {
    legacy_alter_table_active()
}

fn legacy_alter_table_active() -> bool {
    LEGACY_ALTER_TABLE.with(|c| c.get())
}

/// Rewrite occurrences of `old_col` (case-insensitive, identifier
/// boundary) to `new_col` inside every view body that mentions
/// `table_folded`. Used by `ALTER TABLE … RENAME COLUMN` to propagate
/// the rename into `sqlite_master.sql` for dependent views.
///
/// The scan only touches view bodies whose text contains the renamed
/// table name as a folded identifier, to avoid spuriously rewriting
/// columns in unrelated views that happen to use the same name.
fn rewrite_dependent_view_bodies(
    snapshot: &mut super::schema::SchemaSnapshot,
    table_folded: &str,
    old_col: &str,
    new_col: &str,
) {
    if legacy_alter_table_active() {
        return;
    }
    let mut new_views: Vec<std::sync::Arc<super::schema::ViewDef>> = Vec::new();
    for view in &snapshot.views {
        let mentions_table = ascii_word_match(view.body_sql.as_ref(), table_folded);
        if !mentions_table {
            new_views.push(std::sync::Arc::clone(view));
            continue;
        }
        let new_body = rewrite_identifier(view.body_sql.as_ref(), old_col, new_col);
        let new_normalized = view
            .normalized_sql
            .as_ref()
            .map(|sql| rewrite_identifier(sql.as_ref(), old_col, new_col));
        let mut rebuilt = (**view).clone();
        rebuilt.body_sql = new_body.into_boxed_str();
        rebuilt.normalized_sql = new_normalized.map(|sql| sql.into_boxed_str());
        new_views.push(std::sync::Arc::new(rebuilt));
    }
    snapshot.views = new_views;
}

/// Rewrite occurrences of `old_col` to `new_col` inside dependent
/// trigger bodies. Triggers carry an explicit `table_folded` field, so
/// we can scope the rewrite without scanning the body for the table
/// name first.
fn rewrite_dependent_trigger_bodies(
    snapshot: &mut super::schema::SchemaSnapshot,
    table_folded: &str,
    old_col: &str,
    new_col: &str,
) {
    if legacy_alter_table_active() {
        return;
    }
    let mut new_triggers: Vec<std::sync::Arc<super::schema::TriggerDef>> = Vec::new();
    for trigger in &snapshot.triggers {
        if trigger.table_folded.as_ref() != table_folded {
            new_triggers.push(std::sync::Arc::clone(trigger));
            continue;
        }
        let new_body = rewrite_identifier(trigger.body_sql.as_ref(), old_col, new_col);
        let new_normalized = trigger
            .normalized_sql
            .as_ref()
            .map(|sql| rewrite_identifier(sql.as_ref(), old_col, new_col));
        let mut rebuilt = (**trigger).clone();
        rebuilt.body_sql = new_body.into_boxed_str();
        rebuilt.normalized_sql = new_normalized.map(|sql| sql.into_boxed_str());
        new_triggers.push(std::sync::Arc::new(rebuilt));
    }
    snapshot.triggers = new_triggers;
}

/// Rewrite a table reference in dependent view bodies after RENAME
/// TABLE. The new name is emitted quoted with double-quotes — SQLite
/// uses that surface so callers reading `sqlite_master.sql` see the
/// canonical post-rename text.
fn rewrite_dependent_view_table_refs(
    snapshot: &mut super::schema::SchemaSnapshot,
    old_folded: &str,
    new_original: &str,
) {
    if legacy_alter_table_active() {
        return;
    }
    let quoted_new = format!("\"{new_original}\"");
    let mut new_views: Vec<std::sync::Arc<super::schema::ViewDef>> = Vec::new();
    for view in &snapshot.views {
        let mentions = ascii_word_match(view.body_sql.as_ref(), old_folded);
        if !mentions {
            new_views.push(std::sync::Arc::clone(view));
            continue;
        }
        let new_body = rewrite_identifier(view.body_sql.as_ref(), old_folded, &quoted_new);
        let new_normalized = view
            .normalized_sql
            .as_ref()
            .map(|sql| rewrite_identifier(sql.as_ref(), old_folded, &quoted_new));
        let mut rebuilt = (**view).clone();
        rebuilt.body_sql = new_body.into_boxed_str();
        rebuilt.normalized_sql = new_normalized.map(|sql| sql.into_boxed_str());
        new_views.push(std::sync::Arc::new(rebuilt));
    }
    snapshot.views = new_views;
}

/// Rewrite trigger table references after RENAME TABLE. Also updates
/// the trigger's own `table_name` / `table_folded` fields so the
/// trigger keeps firing on the renamed table.
fn rewrite_dependent_trigger_table_refs(
    snapshot: &mut super::schema::SchemaSnapshot,
    old_folded: &str,
    new_original: &str,
) {
    let quoted_new = format!("\"{new_original}\"");
    let new_folded = new_original.to_ascii_lowercase();
    // The trigger's own `table_name` binding always tracks the rename
    // — only the body-text rewrite (and quoted-name reflection in
    // sqlite_master) is gated by legacy_alter_table.
    let body_rewrite_enabled = !legacy_alter_table_active();
    let mut new_triggers: Vec<std::sync::Arc<super::schema::TriggerDef>> = Vec::new();
    for trigger in &snapshot.triggers {
        if trigger.table_folded.as_ref() != old_folded {
            new_triggers.push(std::sync::Arc::clone(trigger));
            continue;
        }
        let new_body = if body_rewrite_enabled {
            rewrite_identifier(trigger.body_sql.as_ref(), old_folded, &quoted_new)
        } else {
            trigger.body_sql.as_ref().to_owned()
        };
        let new_normalized = if body_rewrite_enabled {
            trigger
                .normalized_sql
                .as_ref()
                .map(|sql| rewrite_identifier(sql.as_ref(), old_folded, &quoted_new))
        } else {
            trigger.normalized_sql.as_ref().map(|sql| sql.as_ref().to_owned())
        };
        let mut rebuilt = (**trigger).clone();
        // The trigger's binding tracks the underlying table identity so
        // updates / inserts continue firing it (independent of
        // legacy_alter_table; only the user-visible text is gated).
        rebuilt.table_name = new_original.to_string().into_boxed_str();
        rebuilt.table_folded = new_folded.clone().into_boxed_str();
        rebuilt.body_sql = new_body.into_boxed_str();
        rebuilt.normalized_sql = new_normalized.map(|sql| sql.into_boxed_str());
        new_triggers.push(std::sync::Arc::new(rebuilt));
    }
    snapshot.triggers = new_triggers;
}

/// True iff `needle` appears in `haystack` as an ASCII-identifier-
/// boundary word, case-insensitively. Used by the rename rewriters so
/// references to a column / table are detected without matching a
/// substring inside an unrelated identifier.
fn ascii_word_match(haystack: &str, needle: &str) -> bool {
    let h_lower = haystack.to_ascii_lowercase();
    let n_lower = needle.to_ascii_lowercase();
    let bytes = h_lower.as_bytes();
    let mut start = 0usize;
    while let Some(idx) = h_lower[start..].find(&n_lower) {
        let pos = start + idx;
        let before_ok = pos == 0 || !is_ident_byte(bytes[pos - 1]);
        let after = pos + n_lower.len();
        let after_ok = after == bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = pos + 1;
    }
    false
}

/// Case-insensitive identifier-boundary replacement. Used by the
/// rename rewriters to update view / trigger bodies without touching
/// substrings inside unrelated identifiers or string literals.
fn rewrite_identifier(input: &str, needle: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let lower = input.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut cursor = 0usize;
    let mut search_from = 0usize;
    while let Some(idx) = lower[search_from..].find(&needle_lower) {
        let pos = search_from + idx;
        let before_ok = pos == 0 || !is_ident_byte(bytes[pos - 1]);
        let after = pos + needle_lower.len();
        let after_ok = after == bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            out.push_str(&input[cursor..pos]);
            out.push_str(replacement);
            cursor = after;
            search_from = after;
        } else {
            search_from = pos + 1;
        }
    }
    out.push_str(&input[cursor..]);
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn apply_drop_column(table: &mut TableDef, folded_name: &str, if_exists: bool) -> Result<()> {
    let Some(drop_ordinal) = table
        .columns
        .iter()
        .position(|column| column.folded.as_ref() == folded_name)
    else {
        return if if_exists {
            Ok(())
        } else {
            Err(Error::ObjectNotFound)
        };
    };
    if table.columns.len() == 1 {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN cannot drop the last column",
        ));
    }
    if table.rowid_alias_column == Some(drop_ordinal as u16) {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN cannot drop an INTEGER PRIMARY KEY rowid alias",
        ));
    }
    if table.columns[drop_ordinal].generated.is_some()
        || table
            .columns
            .iter()
            .any(|column| column.generated.is_some())
    {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN with generated columns is not supported",
        ));
    }

    reject_drop_column_references(table, drop_ordinal as u16)?;

    table.columns.remove(drop_ordinal);
    for (ordinal, column) in table.columns.iter_mut().enumerate() {
        column.ordinal = ordinal as u16;
    }
    renumber_drop_column_ordinals(table, drop_ordinal as u16);
    table.normalized_sql = None;
    Ok(())
}

fn reject_drop_column_references(table: &TableDef, drop_ordinal: u16) -> Result<()> {
    let dropped = &table.columns[drop_ordinal as usize];
    if table.constraints.iter().any(|constraint| {
        constraint.column_id == Some(dropped.column_id)
            || constraint
                .expr
                .as_ref()
                .is_some_and(|expr| expr.referenced_cols.iter().any(|col| *col >= drop_ordinal))
    }) {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN cannot drop a constrained column",
        ));
    }
    if table.checks.iter().any(|check| {
        check
            .expr
            .referenced_cols
            .iter()
            .any(|col| *col >= drop_ordinal)
    }) {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN with CHECK constraints is not supported",
        ));
    }
    for index in &table.indexes {
        if index.predicate_sql.is_some() {
            return Err(Error::UnsupportedDdl(
                "ALTER TABLE DROP COLUMN with partial indexes is not supported",
            ));
        }
        for key in &index.keys {
            match &key.source {
                IndexKeySource::Column { attnum } if *attnum == drop_ordinal => {
                    return Err(Error::UnsupportedDdl(
                        "ALTER TABLE DROP COLUMN cannot drop an indexed column",
                    ));
                }
                IndexKeySource::Expression {
                    referenced_cols, ..
                } if referenced_cols.iter().any(|col| *col >= drop_ordinal) => {
                    return Err(Error::UnsupportedDdl(
                        "ALTER TABLE DROP COLUMN with expression indexes is not supported",
                    ));
                }
                _ => {}
            }
        }
    }
    if table
        .foreign_keys
        .iter()
        .any(|fk| fk.columns.contains(&drop_ordinal))
    {
        return Err(Error::UnsupportedDdl(
            "ALTER TABLE DROP COLUMN cannot drop a foreign-key column",
        ));
    }
    Ok(())
}

fn renumber_drop_column_ordinals(table: &mut TableDef, drop_ordinal: u16) {
    if let Some(alias) = &mut table.rowid_alias_column
        && *alias > drop_ordinal
    {
        *alias -= 1;
    }
    for index in &mut table.indexes {
        for key in &mut index.keys {
            if key.ordinal > drop_ordinal {
                key.ordinal -= 1;
            }
            if let IndexKeySource::Column { attnum } = &mut key.source
                && *attnum > drop_ordinal
            {
                *attnum -= 1;
            }
        }
    }
    for fk in &mut table.foreign_keys {
        for column in &mut fk.columns {
            if *column > drop_ordinal {
                *column -= 1;
            }
        }
    }
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
        predicate_sql: None,
    };
    input.next_relation_id.0 += 1;
    let _ = input.conflict;
    Ok((index, constraint_id))
}

fn build_index_keys(table: &TableDef, columns: &[IndexColumnSpec]) -> Result<Vec<IndexKeyDef>> {
    let mut keys = Vec::with_capacity(columns.len());
    for (idx, column) in columns.iter().enumerate() {
        let (ordinal, source) = if let Some(expr_sql) = column.expr_sql.as_ref() {
            // A6 SQL-D: expression-source index key. Synthetic ordinal
            // is the position within the key list; kernel never indexes
            // expression keys by column ordinal.
            (
                idx as u16,
                IndexKeySource::Expression {
                    sql: expr_sql.clone().into_boxed_str(),
                    referenced_cols: column.expr_referenced_cols.clone(),
                },
            )
        } else {
            let ordinal = table
                .columns
                .iter()
                .find(|candidate| candidate.folded.as_ref() == column.name.folded())
                .ok_or(Error::ColumnNotFound)?
                .ordinal;
            (ordinal, IndexKeySource::Column { attnum: ordinal })
        };
        keys.push(IndexKeyDef {
            ordinal,
            source,
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
