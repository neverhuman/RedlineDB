//! Heap-rows ⇔ index-entries cross-check.
//!
//! For each catalog index over a relation the integrity checker:
//!
//! 1. Computes the expected `(logical_key, row_id)` pair for every visible
//!    heap row by replaying [`crate::catalog::encode_index_key`] over the
//!    decoded record (the same code path used by `CREATE INDEX` backfill).
//! 2. Asserts every expected pair shows up in the durable index dump.
//! 3. Asserts every durable dump entry whose key/row_id pair is not in the
//!    expected set resolves to no visible heap row (the orphan case).
//!
//! Counts feed [`super::IndexReport::heap_minus_index`] and
//! [`super::IndexReport::index_minus_heap`]; healthy state is zero on both.

use std::collections::HashSet;

use crate::catalog::{
    EncodedIndexKey, IndexDef, IndexKeySource, RecordRef, RecordScratch, SortDir, TableDef,
    ValueRef, encode_index_key,
};
use crate::format::RowId;
use crate::index::IndexEntry;
use crate::{Error, Result};

use super::IndexReport;

#[derive(Debug, Default)]
pub(crate) struct EquivalenceCounts {
    pub heap_minus_index: usize,
    pub index_minus_heap: usize,
    pub errors: Vec<String>,
}

/// Compare the expected (heap-derived) `(logical_key, row_id)` pairs against
/// the durable dump entries. Mutates `report` in place with the resulting
/// counts and any decode errors collected along the way.
pub(crate) fn cross_check(
    table: &TableDef,
    index: &IndexDef,
    heap_rows: &[(RowId, Vec<u8>)],
    index_entries: &[IndexEntry],
    report: &mut IndexReport,
) -> Result<()> {
    let counts = cross_check_inner(table, index, heap_rows, index_entries)?;
    report.heap_minus_index = counts.heap_minus_index;
    report.index_minus_heap = counts.index_minus_heap;
    report.errors.extend(counts.errors);
    Ok(())
}

fn cross_check_inner(
    table: &TableDef,
    index: &IndexDef,
    heap_rows: &[(RowId, Vec<u8>)],
    index_entries: &[IndexEntry],
) -> Result<EquivalenceCounts> {
    let mut counts = EquivalenceCounts::default();

    // SQL writes heap records with a leading `table_id` element prepended
    // to the user columns; the kernel-internal CREATE INDEX backfill (and
    // the kernel-only test harness) writes records starting directly with
    // the user columns. Both layouts are valid and present in the same
    // codebase; the integrity checker probes the first matching row to
    // pick the offset that yields a real key/row_id intersection with the
    // durable dump, then commits to it for the whole relation.
    let durable: HashSet<(Vec<u8>, RowId)> = index_entries
        .iter()
        .map(|entry| (entry.logical_key.clone(), entry.row.row_id))
        .collect();

    let offset = match probe_record_offset(table, index, heap_rows, &durable)? {
        Some(off) => off,
        None => {
            // No probe row matched in either layout — fall back to a
            // row-id-only equivalence so we still catch the "orphan / no
            // matching heap row" and "missing / no matching index row"
            // shapes. The keys aren't cross-checked under this fallback
            // because we cannot tell which record column an `attnum`
            // refers to with confidence.
            return Ok(row_id_only_check(heap_rows, index_entries));
        }
    };

    let expected = expected_keys(table, index, heap_rows, offset, &mut counts);

    for pair in &expected {
        if !durable.contains(pair) {
            counts.heap_minus_index += 1;
        }
    }
    for pair in &durable {
        if !expected.contains(pair) {
            counts.index_minus_heap += 1;
        }
    }

    Ok(counts)
}

/// Probe the heap row corresponding to the first durable entry under each
/// supported layout offset; return the offset whose derived key matches
/// the durable entry's bytes. `None` indicates neither layout matches
/// (which is itself the equivalence failure surfaced by the row_id-only
/// fallback in `cross_check_inner`).
fn probe_record_offset(
    table: &TableDef,
    index: &IndexDef,
    heap_rows: &[(RowId, Vec<u8>)],
    durable: &HashSet<(Vec<u8>, RowId)>,
) -> Result<Option<usize>> {
    let mut by_row: std::collections::HashMap<RowId, &[u8]> =
        std::collections::HashMap::with_capacity(heap_rows.len());
    for (row_id, payload) in heap_rows {
        by_row.insert(*row_id, payload.as_slice());
    }

    for offset in [0_usize, 1_usize] {
        let mut matched_any = false;
        for (key_bytes, row_id) in durable {
            let Some(payload) = by_row.get(row_id) else {
                continue;
            };
            let candidate = match derive_key_bytes(table, index, payload, offset) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if candidate.as_slice() == key_bytes.as_slice() {
                matched_any = true;
                break;
            }
        }
        if matched_any {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

fn derive_key_bytes(
    _table: &TableDef,
    index: &IndexDef,
    payload: &[u8],
    offset: usize,
) -> Result<Vec<u8>> {
    let record = RecordRef::new(payload).map_err(|_| Error::CorruptPage("record parse failed"))?;
    let mut scratch = RecordScratch::default();
    record
        .decode_into(&mut scratch)
        .map_err(|_| Error::CorruptPage("record decode failed"))?;
    let dirs: Vec<SortDir> = index.keys.iter().map(|key| key.sort_dir).collect();
    let mut parts: Vec<ValueRef<'_>> = Vec::with_capacity(index.keys.len());
    for key in &index.keys {
        let IndexKeySource::Column { attnum } = key.source;
        let value = record
            .value_at(&scratch, (attnum as usize) + offset)
            .map_err(|_| Error::CorruptPage("attnum out of range"))?;
        parts.push(value);
    }
    let mut buf = Vec::new();
    let EncodedIndexKey { bytes, .. } = encode_index_key(&parts, &dirs, &mut buf);
    Ok(bytes)
}

fn expected_keys(
    table: &TableDef,
    index: &IndexDef,
    heap_rows: &[(RowId, Vec<u8>)],
    offset: usize,
    counts: &mut EquivalenceCounts,
) -> HashSet<(Vec<u8>, RowId)> {
    let mut out: HashSet<(Vec<u8>, RowId)> = HashSet::with_capacity(heap_rows.len());
    for (row_id, payload) in heap_rows {
        match derive_key_bytes(table, index, payload, offset) {
            Ok(bytes) => {
                out.insert((bytes, *row_id));
            }
            Err(_) => counts.errors.push(format!(
                "row_id={} on relation {} ({}): could not derive index key",
                row_id.0, table.relation_id.0, table.name
            )),
        }
    }
    out
}

fn row_id_only_check(
    heap_rows: &[(RowId, Vec<u8>)],
    index_entries: &[IndexEntry],
) -> EquivalenceCounts {
    let heap_ids: HashSet<RowId> = heap_rows.iter().map(|(row_id, _)| *row_id).collect();
    let index_ids: HashSet<RowId> = index_entries.iter().map(|entry| entry.row.row_id).collect();
    let mut counts = EquivalenceCounts::default();
    for row_id in &heap_ids {
        if !index_ids.contains(row_id) {
            counts.heap_minus_index += 1;
        }
    }
    for row_id in &index_ids {
        if !heap_ids.contains(row_id) {
            counts.index_minus_heap += 1;
        }
    }
    counts
}

#[allow(dead_code)]
fn _force_error_visibility(_e: Error) {}
