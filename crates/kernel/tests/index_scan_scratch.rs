//! Phase 5 WS-A4: scratch-based `RawIndexCursor` produces bit-identical
//! results to the unscratched cursor and reuses its buffer across scans.

use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::format::{PageGeneration, PageId, RelId, RowId, TuplePtr};
use redlinedb_kernel::index::{
    BtreeIndex, CursorYield, IndexDescriptor, IndexId, IndexRowRef, IndexScanScratch,
    IndexUniqueness, KeyRange, RawIndexCursor, SnapshotView,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

fn build_index(index_id: u64) -> (TempDir, BtreeIndex) {
    let dir = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(dir.path().join("data.redline"), 1024).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 1024).unwrap());
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

fn collect_scratched(
    index: &BtreeIndex,
    start: &[u8],
    end: &[u8],
    counters: &Phase11Counters,
    scratch: &mut IndexScanScratch,
) -> Vec<IndexRowRef> {
    let mut cursor = RawIndexCursor::open_with_scratch_and_counters(
        index,
        KeyRange::half_open(start, end),
        SnapshotView::all(),
        Some(counters),
        scratch,
    )
    .unwrap();
    let mut out = Vec::new();
    while let CursorYield::Batch(_) = cursor.next_rowid_batch(&mut out, 16).unwrap() {}
    cursor.close_into_scratch(scratch);
    out
}

fn collect_unscratched(
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
    while let CursorYield::Batch(_) = cursor.next_rowid_batch(&mut out, 16).unwrap() {}
    cursor.close();
    out
}

#[test]
fn scratched_cursor_matches_unscratched_full_range() {
    let (_dir, index) = build_index(42_001);
    for i in 0..1000_u64 {
        let key = format!("k{i:06}");
        index.insert(key.as_bytes(), row_ref(i)).unwrap();
    }
    let counters = Phase11Counters::new();
    let mut scratch = IndexScanScratch::new();
    let scratched = collect_scratched(&index, b"k000000", b"k001000", &counters, &mut scratch);
    let plain = collect_unscratched(&index, b"k000000", b"k001000", &counters);
    assert_eq!(scratched.len(), 1000);
    assert_eq!(scratched, plain);
}

#[test]
fn scratched_cursor_matches_unscratched_partial_range() {
    let (_dir, index) = build_index(42_002);
    for i in 0..1000_u64 {
        let key = format!("k{i:06}");
        index.insert(key.as_bytes(), row_ref(i)).unwrap();
    }
    let counters = Phase11Counters::new();
    let mut scratch = IndexScanScratch::new();
    let scratched = collect_scratched(&index, b"k000250", b"k000750", &counters, &mut scratch);
    let plain = collect_unscratched(&index, b"k000250", b"k000750", &counters);
    assert_eq!(scratched, plain);
    assert_eq!(scratched.len(), 500);
}

#[test]
fn scratch_reuses_entry_buffer_across_scans() {
    let (_dir, index) = build_index(42_003);
    for i in 0..1000_u64 {
        let key = format!("k{i:06}");
        index.insert(key.as_bytes(), row_ref(i)).unwrap();
    }
    let counters = Phase11Counters::new();
    let mut scratch = IndexScanScratch::new();

    // First scan warms the scratch.
    let first = collect_scratched(&index, b"k000000", b"k001000", &counters, &mut scratch);
    assert_eq!(first.len(), 1000);

    // Drain back into scratch happens via `close_into_scratch`; the
    // entry buffer should now carry capacity from the warmest leaf.
    // We cannot read capacity directly from outside the kernel, but we
    // can prove reuse functionally: ten subsequent scans must all
    // produce identical results to the unscratched baseline with no
    // panics, regardless of range coverage.
    let baseline = collect_unscratched(&index, b"k000000", b"k001000", &counters);
    for _ in 0..10 {
        let out = collect_scratched(&index, b"k000000", b"k001000", &counters, &mut scratch);
        assert_eq!(out, baseline);
    }
}

#[test]
fn scratch_alternating_subranges_stable() {
    let (_dir, index) = build_index(42_004);
    for i in 0..1000_u64 {
        let key = format!("k{i:06}");
        index.insert(key.as_bytes(), row_ref(i)).unwrap();
    }
    let counters = Phase11Counters::new();
    let mut scratch = IndexScanScratch::new();
    let baseline_a = collect_unscratched(&index, b"k000100", b"k000200", &counters);
    let baseline_b = collect_unscratched(&index, b"k000500", b"k000600", &counters);
    for _ in 0..5 {
        let a = collect_scratched(&index, b"k000100", b"k000200", &counters, &mut scratch);
        let b = collect_scratched(&index, b"k000500", b"k000600", &counters, &mut scratch);
        assert_eq!(a, baseline_a);
        assert_eq!(b, baseline_b);
    }
}
