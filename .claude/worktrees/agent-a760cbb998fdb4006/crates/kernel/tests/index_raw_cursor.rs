use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::engine::ConcurrentTxStatus;
use redlinedb_kernel::format::{PageGeneration, PageId, RelId, RowId, TuplePtr};
use redlinedb_kernel::index::{
    BtreeIndex, CursorYield, IndexDescriptor, IndexId, IndexRowRef, IndexUniqueness, KeyRange,
    RawIndexCursor, RawPointCursor, SnapshotView,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

fn build_index(index_id: u64) -> (TempDir, BtreeIndex) {
    let dir = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(dir.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 512).unwrap());
    let index = BtreeIndex::create(
        buffer,
        IndexDescriptor::new(IndexId(index_id), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();
    (dir, index)
}

fn row_ref(id: u64) -> IndexRowRef {
    IndexRowRef::with_row_id(
        RowId(id),
        TuplePtr::new_with_generation(PageId(10_000 + id), id as u16, PageGeneration::ONE),
    )
}

fn collect_raw_point(
    index: &BtreeIndex,
    key: &[u8],
    view: SnapshotView<'_>,
    counters: &Phase11Counters,
) -> Vec<IndexRowRef> {
    let mut cursor = RawPointCursor::open_with_counters(index, key, view, Some(counters)).unwrap();
    let mut out = Vec::new();
    while let CursorYield::Batch(_) = cursor.next_rowid_batch(&mut out, 2).unwrap() {}
    cursor.close();
    out
}

fn collect_raw_range(
    index: &BtreeIndex,
    start: &[u8],
    end: &[u8],
    counters: &Phase11Counters,
) -> Vec<IndexRowRef> {
    let mut cursor = RawIndexCursor::open_with_counters(
        index,
        KeyRange::half_open(start, end),
        SnapshotView::all(),
        Some(counters),
    )
    .unwrap();
    let mut out = Vec::new();
    while let CursorYield::Batch(_) = cursor.next_rowid_batch(&mut out, 3).unwrap() {}
    cursor.close();
    out
}

#[test]
fn raw_point_cursor_matches_visible_lookup() {
    let (_dir, index) = build_index(21_001);
    let txs = ConcurrentTxStatus::new();

    let committed = row_ref(1);
    let hidden_insert = row_ref(2);
    let deleted = row_ref(3);

    let insert_tx = txs.begin();
    index.insert_tx(insert_tx, b"acct-7", committed).unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(insert_tx, csn);

    let hidden_tx = txs.begin();
    index
        .insert_tx(hidden_tx, b"acct-7", hidden_insert)
        .unwrap();

    let deleted_insert = txs.begin();
    index.insert_tx(deleted_insert, b"acct-7", deleted).unwrap();
    let deleted_insert_csn = txs.reserve_csn();
    txs.publish_commit(deleted_insert, deleted_insert_csn);
    let delete_tx = txs.begin();
    index.delete_mark_tx(delete_tx, b"acct-7", deleted).unwrap();
    let delete_csn = txs.reserve_csn();
    txs.publish_commit(delete_tx, delete_csn);

    let snapshot = txs.snapshot();
    let reference = index
        .point_lookup_visible(&txs, &snapshot, None, b"acct-7")
        .unwrap();
    let counters = Phase11Counters::new();
    let raw = collect_raw_point(
        &index,
        b"acct-7",
        SnapshotView::visible(&txs, &snapshot, None),
        &counters,
    );

    assert_eq!(raw, reference);
    assert_eq!(raw, vec![committed]);
    assert!(counters.snapshot().index_raw_point_batches > 0);
}

#[test]
fn raw_range_cursor_matches_public_range_with_duplicates_and_tombstones() {
    let (_dir, index) = build_index(21_002);
    for i in 0..80_u64 {
        let key = format!("k{i:03}");
        index.insert(key.as_bytes(), row_ref(100 + i)).unwrap();
    }
    let dup_a = row_ref(500);
    let dup_b = row_ref(501);
    let removed = row_ref(502);
    index.insert(b"k042", dup_a).unwrap();
    index.insert(b"k042", dup_b).unwrap();
    index.insert(b"k043", removed).unwrap();
    index.delete_mark(b"k043", removed).unwrap();

    let counters = Phase11Counters::new();
    let raw = collect_raw_range(&index, b"k040", b"k045", &counters);
    let reference = index.range_scan(b"k040", b"k045").unwrap();

    assert_eq!(raw, reference);
    assert!(raw.contains(&dup_a));
    assert!(raw.contains(&dup_b));
    assert!(!raw.contains(&removed));
    assert!(counters.snapshot().index_raw_range_batches > 0);
}
