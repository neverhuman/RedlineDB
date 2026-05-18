//! Trigger catalog operations.
//!
//! Triggers are persisted alongside views in the same catalog snapshot
//! (format_version 5). Each trigger captures its parent table, event,
//! period (BEFORE/AFTER), optional column-list filter (for `UPDATE OF`),
//! optional `WHEN` predicate text, and verbatim body SQL text — the SQL
//! crate re-parses the body at fire time and binds it against the
//! current row context.
//!
//! `INSTEAD OF` triggers on views are intentionally deferred to a
//! followup task; this module accepts the syntax but is wired only for
//! `BEFORE`/`AFTER` triggers on regular tables.

use std::sync::Arc;

use super::ddl::{CreateTriggerSpec, DropTriggerSpec, TriggerEventKind, TriggerTimeKind};
use super::ids::ObjectId;
use super::ops::resolve_schema_id;
use super::schema::{SchemaEpoch, SchemaSnapshot, TriggerDef};
use crate::{Error, Result};

/// Apply a `CREATE TRIGGER` spec to the snapshot.
pub fn apply_create_trigger(
    mut snapshot: SchemaSnapshot,
    spec: CreateTriggerSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, spec.schema.as_ref())?;
    if snapshot
        .lookup_trigger(schema_id, spec.name.folded())
        .is_some()
    {
        if spec.if_not_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectExists);
    }
    // Triggers must reference an existing base table (we defer INSTEAD
    // OF on views to a followup task).
    let _table = snapshot
        .lookup_table(schema_id, spec.table.folded())
        .ok_or(Error::ObjectNotFound)?;

    let mut next_object_id = snapshot.meta.next_object_id;
    let trigger_id = ObjectId(next_object_id.0);
    next_object_id.0 += 1;

    let trigger = Arc::new(TriggerDef {
        trigger_id,
        schema_id,
        name: spec.name.original().into(),
        folded: spec.name.folded().into(),
        table_name: spec.table.original().into(),
        table_folded: spec.table.folded().into(),
        when_time: spec.when_time,
        when_event: spec.when_event,
        when_cols: spec
            .when_cols
            .into_iter()
            .map(|n| n.original().to_owned().into_boxed_str())
            .collect(),
        when_predicate_sql: spec.when_predicate_sql.map(|s| s.into_boxed_str()),
        body_sql: spec.body_sql.into_boxed_str(),
        normalized_sql: spec.normalized_sql.map(|s| s.into_boxed_str()),
    });
    snapshot.triggers.push(trigger);
    snapshot.meta.next_object_id = next_object_id;
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

/// Apply a `DROP TRIGGER` spec to the snapshot.
pub fn apply_drop_trigger(
    mut snapshot: SchemaSnapshot,
    spec: DropTriggerSpec,
) -> Result<SchemaSnapshot> {
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let folded = spec.name.name.folded().to_owned();
    let len_before = snapshot.triggers.len();
    snapshot.triggers.retain(|trigger| {
        !(trigger.schema_id == schema_id && trigger.folded.as_ref() == folded.as_str())
    });
    if snapshot.triggers.len() == len_before {
        if spec.if_exists {
            return Ok(snapshot);
        }
        return Err(Error::ObjectNotFound);
    }
    snapshot.meta.schema_epoch = SchemaEpoch(snapshot.meta.schema_epoch.0.saturating_add(1));
    snapshot.rebuild_indexes();
    Ok(snapshot)
}

/// Return every trigger attached to `table` that matches `(event, time)`,
/// ordered by `trigger_id` (creation order — SQLite's documented
/// firing-order guarantee).
pub fn triggers_for(
    snapshot: &SchemaSnapshot,
    schema_id: super::SchemaId,
    table_folded: &str,
    event: TriggerEventKind,
    time: TriggerTimeKind,
) -> Vec<Arc<TriggerDef>> {
    let mut out: Vec<Arc<TriggerDef>> = snapshot
        .triggers
        .iter()
        .filter(|t| {
            t.schema_id == schema_id
                && t.table_folded.as_ref() == table_folded
                && t.when_event == event
                && t.when_time == time
        })
        .cloned()
        .collect();
    out.sort_by_key(|t| t.trigger_id.0);
    out
}
