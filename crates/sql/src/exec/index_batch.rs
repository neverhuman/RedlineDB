// Phase 11 W1-C/E batched cursor consumers extracted from
// `index_access.rs` to keep the latter under the 1000-LOC cap. The
// streaming range-scan, count-range, and covering-range execution
// helpers live here; the access-path matching and probe-shape glue
// stays in the parent module.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;

use redlinedb_kernel::catalog::{IndexDef, SortDir, TableDef};
use redlinedb_kernel::engine::{Engine, Txn};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::index::{
    CursorYield, IndexCursor, IndexRowRef, KeyRange, RawIndexCursor, SnapshotView,
};

use crate::error::Result;
use crate::value::SqlValue;

use super::index_access::{MAX_BATCH, open_handle, visible_in_relation};

/// Phase 11 W1-C: streaming range scan with batched cursor consumption,
/// per-heap-page grouped recheck, and optional early-stop after `limit`
/// visible rows.
///
/// Each `next_batch` call drains up to `MAX_BATCH` `IndexRowRef`s; the
/// batch is then sorted by heap `page_id` so consecutive rows on the
/// same page coalesce into one logical pass. We still issue a
/// per-row `load_table_row_by_rowid` (the heap layer does its own
/// per-page caching), but the per-batch ordering keeps the buffer-pool
/// pin pattern monotone.
///
/// Telemetry: bumps `Phase11Counters::heap_rechecks` ONCE per batch
/// (rather than per row).
pub(super) fn execute_index_range_scan_streaming(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
    limit: Option<usize>,
) -> Result<Vec<RowId>> {
    let Some(handle) = open_handle(engine, index) else {
        return Ok(Vec::new());
    };
    let counters = engine.phase11_counters();
    // Snapshot is taken once at the top so the cursor's borrow does
    // not collide with the per-batch heap recheck (which needs `&mut
    // tx`). The SQL layer's read paths are snapshot-isolation, so the
    // snapshot value never changes mid-statement; cloning it here is
    // semantically a no-op.
    let tx_status = engine.tx_status();
    let owner = Some(tx.id());
    let snapshot = tx.snapshot().clone();
    let range = KeyRange::half_open(start, end);
    let mut out: Vec<RowId> = match limit {
        Some(n) => Vec::with_capacity(n.min(MAX_BATCH)),
        None => Vec::new(),
    };
    let mut batch: Vec<IndexRowRef> = Vec::with_capacity(MAX_BATCH);
    {
        let view = SnapshotView::visible(tx_status, &snapshot, owner);
        let mut cursor = IndexCursor::open_with_counters(&handle, range, view, Some(&*counters))?;
        loop {
            if let Some(n) = limit
                && out.len() >= n
            {
                break;
            }
            batch.clear();
            match cursor.next_batch(&mut batch, MAX_BATCH)? {
                CursorYield::End => break,
                CursorYield::Batch(_) => {
                    counters.heap_rechecks.fetch_add(1, AtomicOrdering::Relaxed);
                    // Drop cursor's borrow of `snapshot` for the
                    // duration of the heap-recheck call by stepping
                    // out of the cursor scope is not possible here —
                    // instead we rely on the fact that
                    // `process_recheck_batch` only needs `&mut tx`,
                    // which is independent of `snapshot`.
                    process_recheck_batch(engine, tx, table, &batch, limit, &mut out)?;
                }
            }
        }
        cursor.close();
    }
    Ok(out)
}

/// Phase 11 W1-D: ordered cursor consumption for ORDER BY/LIMIT
/// shortcuts. Unlike the general streaming path, this keeps the
/// cursor's physical index order and never groups by heap page.
pub(super) fn execute_index_range_scan_ordered(
    engine: &Engine,
    tx: &mut Txn,
    _table: &Arc<TableDef>,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
    limit: usize,
) -> Result<Vec<RowId>> {
    let Some(handle) = open_handle(engine, index) else {
        return Ok(Vec::new());
    };
    let counters = engine.phase11_counters();
    counters
        .ordered_limit_path_hits
        .fetch_add(1, AtomicOrdering::Relaxed);
    let tx_status = engine.tx_status();
    let owner = Some(tx.id());
    let snapshot = tx.snapshot().clone();
    let range = KeyRange::half_open(start, end);
    let mut out: Vec<RowId> = Vec::with_capacity(limit.min(MAX_BATCH));
    let mut batch: Vec<IndexRowRef> = Vec::with_capacity(MAX_BATCH);
    {
        let view = SnapshotView::visible(tx_status, &snapshot, owner);
        let mut cursor =
            RawIndexCursor::open_with_counters(&handle, range, view, Some(&*counters))?;
        loop {
            let remaining = limit.saturating_sub(out.len());
            if remaining == 0 {
                break;
            }
            batch.clear();
            let batch_cap = remaining.clamp(1, MAX_BATCH);
            match cursor.next_rowid_batch(&mut batch, batch_cap)? {
                CursorYield::End => break,
                CursorYield::Batch(_) => {
                    for entry in &batch {
                        if out.len() >= limit {
                            break;
                        }
                        out.push(entry.row_id);
                    }
                }
            }
        }
        cursor.close();
    }
    counters
        .ordered_limit_rows_returned
        .fetch_add(out.len() as u64, AtomicOrdering::Relaxed);
    Ok(out)
}

/// Heap recheck for a batch of `IndexRowRef`s. Rows are grouped by
/// heap `page_id` so consecutive heap touches on the same page stay
/// adjacent — the buffer pool's per-page warm-cache wins this way
/// even when the index batch hopped pages.
///
/// `limit` short-circuits on the *visible* row count: if the caller is
/// after `LIMIT n` and we already have `n`, we stop the batch walk
/// immediately so the next outer iteration breaks.
fn process_recheck_batch(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    batch: &[IndexRowRef],
    limit: Option<usize>,
    out: &mut Vec<RowId>,
) -> Result<()> {
    let mut groups: HashMap<u64, Vec<IndexRowRef>> = HashMap::with_capacity(8);
    for entry in batch {
        groups
            .entry(entry.tuple.page_id.0)
            .or_default()
            .push(*entry);
    }
    let mut keys: Vec<u64> = groups.keys().copied().collect();
    keys.sort_unstable();
    for page_id in keys {
        let mut entries = groups.remove(&page_id).expect("page id present");
        entries.sort_by_key(|e| e.tuple.slot);
        for entry in entries {
            if let Some(n) = limit
                && out.len() >= n
            {
                return Ok(());
            }
            if visible_in_relation(engine, tx, table, entry.row_id)? {
                out.push(entry.row_id);
            }
        }
    }
    Ok(())
}

/// Phase 11 W1-E: count visible entries inside the supplied range
/// without any heap loads. The cursor's `SnapshotView::visible` filter
/// is the source of truth.
pub(super) fn execute_index_count_range(
    engine: &Engine,
    tx: &Txn,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
) -> Result<i64> {
    let Some(handle) = open_handle(engine, index) else {
        return Ok(0);
    };
    let counters = engine.phase11_counters();
    let snapshot = tx.snapshot().clone();
    let view = SnapshotView::visible(engine.tx_status(), &snapshot, Some(tx.id()));
    let range = KeyRange::half_open(start, end);
    let mut cursor = RawIndexCursor::open_with_counters(&handle, range, view, Some(&*counters))?;
    let count = cursor.count_remaining()? as i64;
    cursor.close();
    Ok(count)
}

/// Phase 11 W1-E: serve a covering range scan from the index leaf
/// chain. `out_columns` describes where each output column comes from.
/// Returns one row per visible entry.
pub(super) fn execute_index_covering_range(
    engine: &Engine,
    tx: &Txn,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
    out_columns: &[OutputColumnSource],
    limit: Option<usize>,
) -> Result<Vec<Vec<SqlValue>>> {
    let Some(handle) = open_handle(engine, index) else {
        return Ok(Vec::new());
    };
    let counters = engine.phase11_counters();
    let snapshot = tx.snapshot().clone();
    let view = SnapshotView::visible(engine.tx_status(), &snapshot, Some(tx.id()));
    let range = KeyRange::half_open(start, end);
    let mut cursor = RawIndexCursor::open_with_counters(&handle, range, view, Some(&*counters))?;
    let mut batch: Vec<(Vec<u8>, IndexRowRef)> = Vec::with_capacity(MAX_BATCH);
    let mut out: Vec<Vec<SqlValue>> = Vec::new();
    let dirs: Vec<SortDir> = index.keys.iter().map(|k| k.sort_dir).collect();
    'outer: loop {
        let remaining = match limit {
            Some(n) => n.saturating_sub(out.len()),
            None => usize::MAX,
        };
        if remaining == 0 {
            break;
        }
        batch.clear();
        let batch_cap = remaining.clamp(1, MAX_BATCH);
        match cursor.next_batch_with_keys(&mut batch, batch_cap)? {
            CursorYield::End => break,
            CursorYield::Batch(_) => {
                for (key_bytes, entry) in batch.drain(..) {
                    if let Some(n) = limit
                        && out.len() >= n
                    {
                        break 'outer;
                    }
                    let row = decode_covering_row(&key_bytes, &entry, &dirs, out_columns);
                    out.push(row);
                }
            }
        }
    }
    cursor.close();
    Ok(out)
}

/// Where a single output column comes from when serving a covering
/// scan. Plain column indexes only this wave — no expression /
/// partial / generated covers.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OutputColumnSource {
    /// Decode the i-th leaf-key part from the encoded `logical_key`.
    IndexColumn { ordinal: usize },
    /// Read the rowid alias straight off `IndexRowRef.row_id`.
    Rowid,
}

/// Decode the index leaf row into the requested `out_columns` shape.
fn decode_covering_row(
    key_bytes: &[u8],
    entry: &IndexRowRef,
    dirs: &[SortDir],
    out_columns: &[OutputColumnSource],
) -> Vec<SqlValue> {
    let mut row = Vec::with_capacity(out_columns.len());
    for col in out_columns {
        match col {
            OutputColumnSource::Rowid => {
                row.push(SqlValue::Integer(entry.row_id.0 as i64));
            }
            OutputColumnSource::IndexColumn { ordinal } => {
                let value =
                    decode_index_key_part(key_bytes, dirs, *ordinal).unwrap_or(SqlValue::Null);
                row.push(value);
            }
        }
    }
    row
}

/// Decode the encoded leaf key into a vector of `SqlValue`s. The
/// encoding lives in `redlinedb_kernel::catalog::key::encode_part`:
/// each part starts with a 1-byte type tag, holds its body, and is
/// terminated by `0xff`. `Desc` parts have every body byte (and the
/// tag) bit-inverted so we flip them back before decoding.
#[allow(dead_code)]
pub(crate) fn decode_index_key_parts(bytes: &[u8], dirs: &[SortDir]) -> Vec<SqlValue> {
    let mut out = Vec::with_capacity(dirs.len());
    let mut idx = 0;
    for &dir in dirs {
        let Some((part, next_idx)) = decode_part_at(bytes, idx, dir) else {
            out.push(SqlValue::Null);
            break;
        };
        out.push(part);
        idx = next_idx;
    }
    out
}

fn decode_index_key_part(bytes: &[u8], dirs: &[SortDir], target: usize) -> Option<SqlValue> {
    let mut idx = 0;
    for (ordinal, &dir) in dirs.iter().enumerate() {
        let (part, next_idx) = decode_part_at(bytes, idx, dir)?;
        if ordinal == target {
            return Some(part);
        }
        idx = next_idx;
    }
    None
}

fn decode_part_at(bytes: &[u8], idx: usize, dir: SortDir) -> Option<(SqlValue, usize)> {
    let raw_tag = *bytes.get(idx)?;
    let tag = maybe_uninvert(raw_tag, dir);
    let part_len = match tag {
        0x00 => 1,
        0x10 | 0x20 => 9,
        0x30 | 0x40 => textish_part_len(bytes, idx + 1, dir)?,
        _ => return None,
    };
    let end = idx.checked_add(part_len)?;
    if end > bytes.len() {
        return None;
    }
    let value = match tag {
        0x00 => SqlValue::Null,
        0x10 => decode_integer_part(&bytes[idx + 1..end], dir),
        0x20 => decode_real_part(&bytes[idx + 1..end], dir),
        0x30 | 0x40 => {
            let mut part = bytes[idx..end].to_vec();
            if dir == SortDir::Desc {
                for byte in part.iter_mut() {
                    *byte = !*byte;
                }
            }
            decode_part(&part)
        }
        _ => return None,
    };
    // `encode_index_key` appends an uninverted 0xff separator after
    // every encoded part. Tolerate a missing final separator by moving
    // to `end`; malformed keys decode as far as their tags allow.
    let next_idx = if bytes.get(end).copied() == Some(0xff) {
        end + 1
    } else {
        end
    };
    Some((value, next_idx))
}

fn textish_part_len(bytes: &[u8], body_start: usize, dir: SortDir) -> Option<usize> {
    let mut i = body_start;
    while i + 1 < bytes.len() {
        let first = maybe_uninvert(bytes[i], dir);
        let second = maybe_uninvert(bytes[i + 1], dir);
        if first == 0 && second == 0 {
            return Some(i + 2 - (body_start - 1));
        }
        if first == 0 && second == 0xff {
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

fn maybe_uninvert(byte: u8, dir: SortDir) -> u8 {
    if dir == SortDir::Desc { !byte } else { byte }
}

fn decode_part(bytes: &[u8]) -> SqlValue {
    if bytes.is_empty() {
        return SqlValue::Null;
    }
    match bytes[0] {
        0x00 => SqlValue::Null,
        0x10 => {
            if bytes.len() < 9 {
                return SqlValue::Null;
            }
            let arr: [u8; 8] = bytes[1..9].try_into().unwrap_or([0; 8]);
            let raw = u64::from_be_bytes(arr) ^ 0x8000_0000_0000_0000;
            SqlValue::Integer(raw as i64)
        }
        0x20 => {
            if bytes.len() < 9 {
                return SqlValue::Null;
            }
            let arr: [u8; 8] = bytes[1..9].try_into().unwrap_or([0; 8]);
            let sortable = u64::from_be_bytes(arr);
            let bits = if sortable & 0x8000_0000_0000_0000 != 0 {
                sortable ^ 0x8000_0000_0000_0000
            } else {
                !sortable
            };
            SqlValue::Real(f64::from_bits(bits))
        }
        0x30 => {
            let body = unescape_textish(&bytes[1..]);
            SqlValue::Text(std::sync::Arc::from(
                String::from_utf8_lossy(&body).into_owned(),
            ))
        }
        0x40 => {
            let body = unescape_textish(&bytes[1..]);
            SqlValue::Blob(std::sync::Arc::from(body.into_boxed_slice()))
        }
        _ => SqlValue::Null,
    }
}

/// Pull the leading 8 bytes off an index-key payload, undoing the
/// per-byte bit-flip if the column is sorted descending. Returns `None`
/// when there are fewer than 8 bytes available — callers map that to
/// `SqlValue::Null`.
fn take_sortable_u64(bytes: &[u8], dir: SortDir) -> Option<u64> {
    if bytes.len() < 8 {
        return None;
    }
    let mut arr = [0_u8; 8];
    if dir == SortDir::Desc {
        for (slot, byte) in arr.iter_mut().zip(bytes.iter().take(8)) {
            *slot = !*byte;
        }
    } else {
        arr.copy_from_slice(&bytes[..8]);
    }
    Some(u64::from_be_bytes(arr))
}

fn decode_integer_part(bytes: &[u8], dir: SortDir) -> SqlValue {
    let Some(sortable) = take_sortable_u64(bytes, dir) else {
        return SqlValue::Null;
    };
    let raw = sortable ^ 0x8000_0000_0000_0000;
    SqlValue::Integer(raw as i64)
}

fn decode_real_part(bytes: &[u8], dir: SortDir) -> SqlValue {
    let Some(sortable) = take_sortable_u64(bytes, dir) else {
        return SqlValue::Null;
    };
    let bits = if sortable & 0x8000_0000_0000_0000 != 0 {
        sortable ^ 0x8000_0000_0000_0000
    } else {
        !sortable
    };
    SqlValue::Real(f64::from_bits(bits))
}

/// Inverse of `encode_bytes` in `catalog::key`: strips the trailing
/// `0x00 0x00` terminator and unescapes `0x00 0xff` -> `0x00`. Best
/// effort; non-conforming bytes fall through unchanged.
fn unescape_textish(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == 0 && bytes[i + 1] == 0 {
            break;
        }
        if bytes[i] == 0 && bytes[i + 1] == 0xff {
            out.push(0);
            i += 2;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_integer_round_trips() {
        // Encode 42 with SortDir::Asc, then decode.
        let mut buf = Vec::new();
        let parts: [redlinedb_kernel::catalog::ValueRef<'_>; 1] =
            [redlinedb_kernel::catalog::ValueRef::Integer(42)];
        let dirs = [SortDir::Asc];
        let encoded = redlinedb_kernel::catalog::encode_index_key(&parts, &dirs, &mut buf);
        let decoded = decode_index_key_parts(&encoded.bytes, &dirs);
        assert_eq!(decoded.len(), 1);
        assert!(matches!(decoded[0], SqlValue::Integer(42)));
    }

    #[test]
    fn decode_two_part_integer_pair() {
        let mut buf = Vec::new();
        let parts = [
            redlinedb_kernel::catalog::ValueRef::Integer(7),
            redlinedb_kernel::catalog::ValueRef::Integer(-3),
        ];
        let dirs = [SortDir::Asc, SortDir::Asc];
        let encoded = redlinedb_kernel::catalog::encode_index_key(&parts, &dirs, &mut buf);
        let decoded = decode_index_key_parts(&encoded.bytes, &dirs);
        assert_eq!(decoded.len(), 2);
        assert!(matches!(decoded[0], SqlValue::Integer(7)));
        assert!(matches!(decoded[1], SqlValue::Integer(-3)));
    }

    #[test]
    fn decode_text_blob_and_desc_match_index_key_encoding() {
        let mut buf = Vec::new();
        let blob: &[u8] = b"\x00\xfftail";
        let parts = [
            redlinedb_kernel::catalog::ValueRef::Text("a\0\u{00ff}z"),
            redlinedb_kernel::catalog::ValueRef::Blob(blob),
            redlinedb_kernel::catalog::ValueRef::Integer(-42),
        ];
        let dirs = [SortDir::Asc, SortDir::Desc, SortDir::Desc];
        let encoded = redlinedb_kernel::catalog::encode_index_key(&parts, &dirs, &mut buf);
        let decoded = decode_index_key_parts(&encoded.bytes, &dirs);
        assert_eq!(decoded.len(), 3);
        assert_eq!(
            decoded[0],
            SqlValue::Text(std::sync::Arc::from("a\0\u{00ff}z"))
        );
        assert_eq!(decoded[1], SqlValue::Blob(std::sync::Arc::from(blob)));
        assert_eq!(decoded[2], SqlValue::Integer(-42));
    }

    #[test]
    fn output_column_source_is_copy() {
        let _a = OutputColumnSource::Rowid;
        let _b = _a;
        let _c = OutputColumnSource::IndexColumn { ordinal: 0 };
        let _d = _c;
    }
}
