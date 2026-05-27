// Lane B physical-index DML maintenance.
//
// This module owns the SQL-side bridge to the kernel's physical B-tree
// indexes. Lane A built `Engine::index_handle(index_id)` and made every new
// index ship a `meta_page_id`; here we use those handles to keep indexes in
// step with INSERT/UPDATE/DELETE on the heap.
//
// The high-level rules implemented here:
// - SQLite NULL parity: skip the unique-conflict check for indexes whose key
//   contains any NULL component. Duplicates of NULL are allowed.
// - Acquire the kernel-side `UniqueKeyLockTable` guard before probing /
//   inserting so concurrent writers serialize on the same key.
// - Maintain every index (unique or not) on every successful DML mutation.
// - Pre-Lane-A indexes (no `meta_page_id`) fall back to the heap-scan
//   path used before physical indexes existed; this preserves
//   correctness while we ship.
use std::sync::Arc;

use redlinedb_kernel::catalog::{
    EncodedIndexKey, IndexDef, IndexKeySource, SortDir, TableDef, encode_index_key,
};
use redlinedb_kernel::engine::{Engine, Txn};
use redlinedb_kernel::format::{PageGeneration, PageId, RowId, TuplePtr};
use redlinedb_kernel::index::{BtreeIndex, IndexRowRef, UniqueKeyGuard};

use crate::error::Result;
use crate::value::SqlValue;

/// The result of building an index key for one row's values. `contains_null`
/// reports whether any leading-key part was NULL, which lets callers honor
/// SQLite's NULL-in-unique-key parity rule (NULL parts disable the unique
/// conflict check) without re-walking `index.keys`.
pub(crate) struct BuiltIndexKey {
    pub bytes: Vec<u8>,
    #[allow(dead_code)] // reserved for upcoming Lane C planner integration
    pub contains_null: bool,
}

pub(crate) struct BuiltIndexKeyWithValues {
    pub key: BuiltIndexKey,
    pub values: Vec<SqlValue>,
}

pub(crate) fn has_expression_key(index: &IndexDef) -> bool {
    index
        .keys
        .iter()
        .any(|key| matches!(&key.source, IndexKeySource::Expression { .. }))
}

/// Build the encoded index key bytes for `index` from a row's column values.
///
/// Mirrors the kernel-side encoding used by Lane A's CREATE INDEX backfill,
/// so SQL DML and DDL agree byte-for-byte on key shape.
pub(crate) fn build_index_key(
    table: &TableDef,
    index: &IndexDef,
    values: &[SqlValue],
) -> Result<BuiltIndexKey> {
    if has_expression_key(index) {
        return Ok(build_index_key_with_values(table, index, values)?.key);
    }
    let mut dirs: Vec<SortDir> = Vec::with_capacity(index.keys.len());
    let mut value_refs = Vec::with_capacity(index.keys.len());
    let null_value = SqlValue::Null;
    for key in &index.keys {
        let IndexKeySource::Column { attnum } = key.source else {
            unreachable!("expression keys are handled by build_index_key_with_values");
        };
        value_refs.push(values.get(attnum as usize).unwrap_or(&null_value).as_ref());
        dirs.push(key.sort_dir);
    }
    Ok(encode_built_index_key(&value_refs, &dirs))
}

pub(crate) fn build_index_key_with_values(
    table: &TableDef,
    index: &IndexDef,
    values: &[SqlValue],
) -> Result<BuiltIndexKeyWithValues> {
    let mut dirs: Vec<SortDir> = Vec::with_capacity(index.keys.len());
    let mut key_values: Vec<SqlValue> = Vec::with_capacity(index.keys.len());
    for key in &index.keys {
        let value = match &key.source {
            IndexKeySource::Column { attnum } => values
                .get(*attnum as usize)
                .cloned()
                .unwrap_or(SqlValue::Null),
            IndexKeySource::Expression { sql, .. } => {
                crate::exec::index_predicate::eval_index_value_expr(table, sql, values)?
            }
        };
        key_values.push(value);
        dirs.push(key.sort_dir);
    }
    let value_refs: Vec<_> = key_values.iter().map(|v| v.as_ref()).collect();
    let key = encode_built_index_key(&value_refs, &dirs);
    Ok(BuiltIndexKeyWithValues {
        key,
        values: key_values,
    })
}

fn encode_built_index_key(
    value_refs: &[redlinedb_kernel::catalog::ValueRef<'_>],
    dirs: &[SortDir],
) -> BuiltIndexKey {
    let mut buf = Vec::new();
    let EncodedIndexKey {
        bytes,
        contains_null,
    } = encode_index_key(&value_refs, &dirs, &mut buf);
    BuiltIndexKey {
        bytes,
        contains_null,
    }
}

/// Returns the index handle visible inside `tx`, including a handle created
/// by CREATE INDEX in the same transaction but not published at COMMIT yet.
pub(crate) fn open_index_handle_for_tx(
    engine: &Engine,
    tx: &Txn,
    index: &IndexDef,
) -> Option<Arc<BtreeIndex>> {
    index.meta_page_id?;
    engine.index_handle_for_tx(tx, index.index_id)
}

/// Build an `IndexRowRef` that the BtreeIndex stores alongside the logical
/// key. We keep the `TuplePtr` synthetic (page 0, slot 0, generation ONE);
/// SQL Lane B does not yet need the heap tuple back-pointer, only the
/// `RowId` for visibility checks.
pub(crate) fn synthetic_row_ref(rowid: RowId) -> IndexRowRef {
    IndexRowRef::with_row_id(
        rowid,
        TuplePtr::new_with_generation(PageId(0), 0, PageGeneration::ONE),
    )
}

/// Probe `handle` for a unique-key duplicate of `key`, ignoring `skip_rowid`
/// (used by UPDATE so a row's own existing entry is not a self-conflict).
///
/// The kernel `UniqueKeyGuard` is acquired before the lookup so concurrent
/// writers serialize against this key; callers MUST keep the guard alive
/// until either the heap+index inserts complete (so the durable index entry
/// becomes the conflict witness) or the SQL transaction commits/rolls back.
/// Dropping the guard between probe and insert reopens the original race —
/// two writers both saw "no duplicate" and both committed two rows for the
/// same UNIQUE key. SQLite NULL parity is the caller's responsibility — this
/// routine only runs when the key has no NULL parts.
pub(crate) fn probe_unique_for_conflict(
    engine: &Engine,
    handle: &BtreeIndex,
    tx: &Txn,
    skip_rowid: Option<RowId>,
    key: &BuiltIndexKey,
) -> Result<(UniqueKeyGuard, Option<RowId>)> {
    let guard = handle.lock_unique_key(tx.id().0, &key.bytes)?;
    let latest = engine.tx_status().snapshot();
    let rows =
        handle.point_lookup_visible(engine.tx_status(), &latest, Some(tx.id()), &key.bytes)?;
    for row in rows {
        if skip_rowid == Some(row.row_id) {
            continue;
        }
        return Ok((guard, Some(row.row_id)));
    }
    Ok((guard, None))
}

/// Insert `values`'s index entries for every index on `table`. Run AFTER
/// the heap insert so a heap-side failure aborts cleanly; the kernel rolls
/// both back via WAL replay if a crash hits between the heap insert and
/// these index inserts (recovery atomicity).
///
pub(crate) fn maintain_indexes_on_insert(
    engine: &Engine,
    tx: &Txn,
    table: &TableDef,
    values: &[SqlValue],
    rowid: RowId,
) -> Result<()> {
    for index in &table.indexes {
        let Some(handle) = open_index_handle_for_tx(engine, tx, index) else {
            continue;
        };
        // A6 SQL-D: partial indexes only contain rows whose WHERE
        // predicate evaluates to true. Skip the insert when the row
        // doesn't match; the heap still has it, the planner falls back
        // to a table scan for queries that don't imply the predicate.
        if let Some(pred_sql) = index.predicate_sql.as_deref()
            && !crate::exec::index_predicate::eval_index_predicate(table, pred_sql, values)?
        {
            continue;
        }
        let key = build_index_key(table, index, values)?;
        let row_ref = synthetic_row_ref(rowid);
        // SQLite NULL parity for unique indexes: NULL key parts are not
        // duplicates, so we still insert them but never block on conflict.
        handle.insert_tx(tx.id(), &key.bytes, row_ref)?;
    }
    Ok(())
}

/// Delete-mark every index entry corresponding to `old_values` at `rowid`.
/// Used by DELETE and by UPDATE when the key or rowid changes.
pub(crate) fn maintain_indexes_on_delete(
    engine: &Engine,
    tx: &Txn,
    table: &TableDef,
    old_values: &[SqlValue],
    rowid: RowId,
) -> Result<()> {
    for index in &table.indexes {
        let Some(handle) = open_index_handle_for_tx(engine, tx, index) else {
            continue;
        };
        // A6 SQL-D: don't delete-mark partial-index keys for rows that
        // were never inserted (predicate was false at insert time).
        if let Some(pred_sql) = index.predicate_sql.as_deref()
            && !crate::exec::index_predicate::eval_index_predicate(table, pred_sql, old_values)?
        {
            continue;
        }
        let key = build_index_key(table, index, old_values)?;
        let row_ref = synthetic_row_ref(rowid);
        handle.delete_mark_tx_visible(
            engine.tx_status(),
            tx.snapshot(),
            Some(tx.id()),
            tx.id(),
            &key.bytes,
            row_ref,
        )?;
    }
    Ok(())
}

/// Reflect an UPDATE: delete-mark prior entries whose key or rowid changed,
/// then insert the new entries. Indexes whose key column set is untouched
/// AND whose rowid is unchanged are left alone (no churn).
#[allow(clippy::too_many_arguments)]
pub(crate) fn maintain_indexes_on_update(
    engine: &Engine,
    tx: &Txn,
    table: &TableDef,
    old_values: &[SqlValue],
    new_values: &[SqlValue],
    old_rowid: RowId,
    new_rowid: RowId,
) -> Result<()> {
    for index in &table.indexes {
        let Some(handle) = open_index_handle_for_tx(engine, tx, index) else {
            continue;
        };
        let old_key = build_index_key(table, index, old_values)?;
        let new_key = build_index_key(table, index, new_values)?;
        // The IndexRowRef carries the rowid into the physical entry, so a
        // rowid move always triggers a delete+insert even when key bytes
        // are byte-equal.
        if old_key.bytes == new_key.bytes && old_rowid == new_rowid {
            continue;
        }
        let old_row = synthetic_row_ref(old_rowid);
        let new_row = synthetic_row_ref(new_rowid);
        handle.delete_mark_tx_visible(
            engine.tx_status(),
            tx.snapshot(),
            Some(tx.id()),
            tx.id(),
            &old_key.bytes,
            old_row,
        )?;
        handle.insert_tx(tx.id(), &new_key.bytes, new_row)?;
    }
    Ok(())
}
