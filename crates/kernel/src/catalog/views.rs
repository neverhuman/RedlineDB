//! View catalog operations.
//!
//! Views are persisted in the catalog snapshot alongside tables. Each
//! view stores the verbatim body SQL (the SELECT text after `AS`) plus
//! an optional alias-column list. Query-time expansion is handled by the
//! SQL crate, which re-parses the body and binds it as a derived row
//! source — the kernel only owns persistence, lookups, and uniqueness.
//!
//! Lane A5-views: introduced with catalog format_version 4. Older
//! catalogs decode with an empty `views` Vec.

use std::sync::Arc;

use super::ddl::{CreateViewSpec, DropViewSpec};
use super::ids::ObjectId;
use super::ops::resolve_schema_id;
use super::schema::{SchemaEpoch, SchemaSnapshot, ViewDef};
use crate::{Error, Result};

/// Apply a `CREATE VIEW` spec to the snapshot. Returns the next snapshot
/// with the view appended and the schema-epoch bumped.
///
/// Mirrors SQLite semantics: a view name collides with both other views
/// and with tables in the same schema. `IF NOT EXISTS` makes the
/// operation a no-op when a same-named view already exists.
pub fn apply_create_view(
    mut snapshot: SchemaSnapshot,
    spec: CreateViewSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, spec.schema.as_ref())?;
    // Reject name collisions with existing tables / indexes / views.
    if snapshot
        .lookup_table(schema_id, spec.name.folded())
        .is_some()
    {
        if spec.if_not_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectExists);
    }
    if snapshot
        .lookup_view(schema_id, spec.name.folded())
        .is_some()
    {
        if spec.if_not_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectExists);
    }
    if snapshot
        .lookup_index(schema_id, spec.name.folded())
        .is_some()
    {
        return Err(Error::ObjectExists);
    }

    let mut next_object_id = snapshot.meta.next_object_id;
    let view_id = ObjectId(next_object_id.0);
    next_object_id.0 += 1;

    let view = Arc::new(ViewDef {
        view_id,
        schema_id,
        name: spec.name.original().into(),
        folded: spec.name.folded().into(),
        columns: spec
            .columns
            .into_iter()
            .map(|n| n.original().to_owned().into_boxed_str())
            .collect(),
        body_sql: spec.body_sql.into_boxed_str(),
        session_scoped: spec.session_scoped,
        normalized_sql: spec.normalized_sql.map(|sql| sql.into_boxed_str()),
    });
    snapshot.views.push(view);
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

/// Apply a `DROP VIEW` spec to the snapshot. Returns `ObjectNotFound`
/// when the name is unknown unless `IF EXISTS` is set.
pub fn apply_drop_view(mut snapshot: SchemaSnapshot, spec: DropViewSpec) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let folded = spec.name.name.folded().to_owned();
    let len_before = snapshot.views.len();
    snapshot
        .views
        .retain(|view| !(view.schema_id == schema_id && view.folded.as_ref() == folded.as_str()));
    if snapshot.views.len() == len_before {
        if spec.if_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

/// Lookup a view by qualified name. Returns the same `ObjectNotFound`
/// error as `lookup_table` so callers can treat the views/tables namespace
/// uniformly.
pub fn lookup_view(
    snapshot: &SchemaSnapshot,
    name: &super::names::QualifiedName,
) -> Result<Arc<ViewDef>> {
    let schema_id = resolve_schema_id(snapshot, Some(&name.schema))?;
    snapshot
        .lookup_view(schema_id, name.name.folded())
        .ok_or(Error::ObjectNotFound)
}
