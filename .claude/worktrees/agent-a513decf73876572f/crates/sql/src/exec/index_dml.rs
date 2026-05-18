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
// - Pre-Lane-A indexes (no `meta_page_id`) fall back to the legacy O(N)
//   scan; this preserves correctness while we ship.
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

/// Build the encoded index key bytes for `index` from a row's column values.
///
/// Mirrors the kernel-side encoding used by Lane A's CREATE INDEX backfill,
/// so SQL DML and DDL agree byte-for-byte on key shape.
pub(crate) fn build_index_key(index: &IndexDef, values: &[SqlValue]) -> BuiltIndexKey {
    let mut dirs: Vec<SortDir> = Vec::with_capacity(index.keys.len());
    let mut owned_refs: Vec<&SqlValue> = Vec::with_capacity(index.keys.len());
    for key in &index.keys {
        let IndexKeySource::Column { attnum } = key.source;
        owned_refs.push(values.get(attnum as usize).unwrap_or(&SqlValue::Null));
        dirs.push(key.sort_dir);
    }
    let value_refs: Vec<_> = owned_refs.iter().map(|v| v.as_ref()).collect();
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

/// Returns the live `BtreeIndex` handle for `index` if Lane A allocated one.
/// Pre-Lane-A indexes (no `meta_page_id`) return `None`; callers should fall
/// back to the legacy heap-scan path in that case.
pub(crate) fn open_index_handle(engine: &Engine, index: &IndexDef) -> Option<Arc<BtreeIndex>> {
    index.meta_page_id?;
    engine.index_handle(index.index_id)
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
        let Some(handle) = open_index_handle(engine, index) else {
            continue;
        };
        let key = build_index_key(index, values);
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
        let Some(handle) = open_index_handle(engine, index) else {
            continue;
        };
        let key = build_index_key(index, old_values);
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

/// Reflect an UPDATE: delete-mark old entries whose key or rowid changed,
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
        let Some(handle) = open_index_handle(engine, index) else {
            continue;
        };
        let old_key = build_index_key(index, old_values);
        let new_key = build_index_key(index, new_values);
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
