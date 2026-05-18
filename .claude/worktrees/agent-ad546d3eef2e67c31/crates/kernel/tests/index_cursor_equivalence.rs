//! Phase 11 Wave 1a Worker A — equivalence test for [`IndexCursor`].
//!
//! Builds a multi-leaf B-tree fixture with 5_000 deterministic keys, then
//! generates 100 deterministic random key ranges (seed 11) and asserts
//! that:
//!
//! 1. Concatenating all `next_batch` results from `IndexCursor` produces
//!    a `Vec<IndexRowRef>` byte-for-byte identical to the legacy
//!    `range_scan_filter_legacy` output (exposed via the test-only
//!    `range_scan_legacy_for_test` shim).
//! 2. The cursor wrapper-driven `range_scan` (which is *also* the legacy
//!    public API) returns the same vector. This pins both the cursor
//!    implementation and the wrapper rewrite.
//! 3. Telemetry: `Phase11Counters::cursor_batches_emitted` is bumped at
//!    least once per non-empty range when the cursor is opened with
//!    counters wired in. Confirms the emission site is reachable.

use std::ops::Bound;
use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::format::{PageGeneration, PageId, RelId, TuplePtr};
use redlinedb_kernel::index::{
    BtreeIndex, CursorYield, IndexCursor, IndexDescriptor, IndexId, IndexRowRef, IndexUniqueness,
    KeyRange, RawIndexCursor, SnapshotView,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

const FIXTURE_KEYS: u64 = 5_000;
const FIXTURE_SEED: u64 = 11;
const RANGE_SAMPLES: usize = 100;

/// xorshift64* — small, deterministic, dependency-free PRNG so the
/// fixture and range samples are reproducible without pulling `rand`
/// into the kernel test surface.
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            // xorshift cannot start from zero; map a zero seed to one.
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn build_fixture() -> (TempDir, Arc<BufferPool>, BtreeIndex) {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    // Generous buffer pool: the fixture has thousands of keys spilling
    // across many leaves; sizing this so the cursor never has to evict
    // keeps the test focused on cursor correctness rather than buffer-
    // pool race conditions.
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 8192).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(11_001), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    // Insert keys in a deterministic non-sequential order so we exercise
    // splits and right-link traversal. Keys themselves are zero-padded
    // ASCII so the bit-for-bit comparison is meaningful.
    let mut order: Vec<u64> = (0..FIXTURE_KEYS).collect();
    let mut rng = XorShift64::new(FIXTURE_SEED ^ 0xA5A5_5A5A_5A5A_A5A5);
    for i in (1..order.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        order.swap(i, j);
    }
    for (slot, k) in order.iter().enumerate() {
        let key = format!("k{:08}", k);
        let row = IndexRowRef::new(TuplePtr::new_with_generation(
            PageId(10_000 + k),
            (slot % u16::MAX as usize) as u16,
            PageGeneration::ONE,
        ));
        index.insert(key.as_bytes(), row).unwrap();
    }
    (temp, buffer, index)
}

/// Materialise every row the cursor would yield for `range`, with the
/// given snapshot view, by repeatedly calling `next_batch`. Returns the
/// concatenated rows plus the final `cursor_batches_emitted` counter
/// snapshot. `batch_size` cycles through a few sizes so we exercise
/// both single-leaf and multi-leaf batch boundaries.
fn collect_via_cursor(
    index: &BtreeIndex,
    range: KeyRange<'_>,
    counters: &Phase11Counters,
    batch_size: usize,
) -> (Vec<IndexRowRef>, u64) {
    let mut out = Vec::new();
    let view = SnapshotView::all();
    let mut cur = IndexCursor::open_with_counters(index, range, view, Some(counters)).unwrap();
    while let CursorYield::Batch(_) = cur.next_batch(&mut out, batch_size).unwrap() {}
    cur.close();
    (out, counters.snapshot().cursor_batches_emitted)
}

fn count_via_raw_cursor(index: &BtreeIndex, range: KeyRange<'_>) -> i64 {
    let view = SnapshotView::all();
    let mut cur = RawIndexCursor::open(index, range, view).unwrap();
    let mut count = 0_i64;
    while let CursorYield::Batch(n) = cur.next_count_batch(64).unwrap() {
        count += n as i64;
    }
    cur.close();
    count
}

fn count_via_raw_cursor_single_pass(index: &BtreeIndex, range: KeyRange<'_>) -> i64 {
    let view = SnapshotView::all();
    let mut cur = RawIndexCursor::open(index, range, view).unwrap();
    let count = cur.count_remaining().unwrap() as i64;
    cur.close();
    count
}

#[test]
fn cursor_matches_legacy_range_scan_bitwise() {
    let (_tmp, _buffer, index) = build_fixture();
    let counters = Phase11Counters::new();
    let mut total_batches: u64 = 0;
    let mut nonempty_ranges: u64 = 0;

    let mut rng = XorShift64::new(FIXTURE_SEED);
    for sample in 0..RANGE_SAMPLES {
        // Pick two random keys in the fixture's domain. Cycle the
        // batch size through 1, 7, 64, 1024 so we cover sub-leaf,
        // mid-leaf, leaf-spanning, and very-large batch sizes.
        let mut a = rng.next_u64() % FIXTURE_KEYS;
        let mut b = rng.next_u64() % FIXTURE_KEYS;
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let start_key = format!("k{:08}", a);
        let end_key = format!("k{:08}", b);

        let batch_size = match sample % 4 {
            0 => 1,
            1 => 7,
            2 => 64,
            _ => 1024,
        };
        let range = KeyRange::half_open(start_key.as_bytes(), end_key.as_bytes());

        // Legacy reference output (verbatim copy of pre-cursor
        // `range_scan_filter` body, exposed via a doc-hidden shim).
        let legacy = index
            .range_scan_legacy_for_test(start_key.as_bytes(), end_key.as_bytes())
            .unwrap();

        // Cursor output, with telemetry sink wired in.
        let prev_batches = counters.snapshot().cursor_batches_emitted;
        let (cursor_out, batches_after) = collect_via_cursor(&index, range, &counters, batch_size);
        let pushed_batches = batches_after - prev_batches;
        total_batches += pushed_batches;
        if !cursor_out.is_empty() {
            nonempty_ranges += 1;
            assert!(
                pushed_batches >= 1,
                "non-empty range {:?}..{:?} yielded {} rows but bumped {} batches",
                start_key,
                end_key,
                cursor_out.len(),
                pushed_batches,
            );
        }

        assert_eq!(
            legacy, cursor_out,
            "cursor output diverged from legacy at sample {} (batch={}) range={}..{}",
            sample, batch_size, start_key, end_key,
        );

        // Also pin the public wrapper: `range_scan` is itself a
        // cursor-driven wrapper now, and must agree.
        let via_wrapper = index
            .range_scan(start_key.as_bytes(), end_key.as_bytes())
            .unwrap();
        assert_eq!(
            via_wrapper, cursor_out,
            "wrapper diverged at sample {}",
            sample
        );
    }

    // The deliverable asks us to confirm the emission site is reachable
    // — at least one batch must have been emitted across the 100
    // samples. In practice we expect dozens, since most of the random
    // ranges yield non-empty results.
    assert!(
        total_batches > 0,
        "expected cursor_batches_emitted > 0 across {} samples; got {}",
        RANGE_SAMPLES,
        total_batches
    );
    assert!(
        nonempty_ranges > 0,
        "fixture sanity: at least one of the {} random ranges should be non-empty",
        RANGE_SAMPLES
    );
    eprintln!(
        "cursor_batches_emitted across {} samples: {} (nonempty ranges: {})",
        RANGE_SAMPLES, total_batches, nonempty_ranges
    );
}

#[test]
fn cursor_unbounded_end_walks_full_chain() {
    // Worker A regression: an `Unbounded` upper bound must keep walking
    // the leaf chain to its tail — the legacy code explicitly supported
    // this (callers passed `[0xff; 32]` to fake it). Drop the fake and
    // use `Bound::Unbounded` directly; the cursor must visit the same
    // rows as the legacy `range_scan_legacy_for_test` with a sentinel.
    let (_tmp, _buffer, index) = build_fixture();
    let counters = Phase11Counters::new();
    let start = b"k00000000".to_vec();
    let sentinel_end = vec![0xff; 32];
    let legacy = index
        .range_scan_legacy_for_test(&start, &sentinel_end)
        .unwrap();
    let range = KeyRange {
        start: Bound::Included(&start),
        end: Bound::Unbounded,
    };
    let (cursor_out, _batches) = collect_via_cursor(&index, range, &counters, 256);
    assert_eq!(legacy, cursor_out);
}

#[test]
fn raw_count_matches_legacy_with_duplicates_and_tombstones() {
    let (_tmp, _buffer, index) = build_fixture();

    let live_row = IndexRowRef::new(TuplePtr::new_with_generation(
        PageId(99_001),
        0,
        PageGeneration::ONE,
    ));
    let dup_row = IndexRowRef::new(TuplePtr::new_with_generation(
        PageId(99_002),
        1,
        PageGeneration::ONE,
    ));
    let tombstone_row = IndexRowRef::new(TuplePtr::new_with_generation(
        PageId(99_003),
        2,
        PageGeneration::ONE,
    ));
    index.insert(b"tenant-01", live_row).unwrap();
    index.insert(b"tenant-01", dup_row).unwrap();
    index.insert(b"tenant-02", tombstone_row).unwrap();
    index.delete_mark(b"tenant-02", tombstone_row).unwrap();

    let start = b"tenant-00";
    let end = b"tenant-09";
    let legacy = index.range_scan_legacy_for_test(start, end).unwrap();
    let raw_count = count_via_raw_cursor(&index, KeyRange::half_open(start, end));
    let raw_count_single_pass =
        count_via_raw_cursor_single_pass(&index, KeyRange::half_open(start, end));

    assert_eq!(
        raw_count,
        legacy.len() as i64,
        "raw count diverged from legacy"
    );
    assert_eq!(
        raw_count_single_pass,
        legacy.len() as i64,
        "single-pass raw count diverged from legacy"
    );
    assert_eq!(
        raw_count, 2,
        "duplicate live keys should count; tombstones should not"
    );
}
