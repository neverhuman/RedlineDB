//! Phase 11 Wave 1b Worker B — telemetry test for the cursor-driven
//! advisory prefetch path.
//!
//! Builds a multi-leaf B-tree fixture (≥ 10 leaves), opens an
//! `IndexCursor` over a range that spans most of the chain, drains it
//! batch by batch, and asserts that
//! `Phase11Counters::prefetch_hits + prefetch_misses` is strictly
//! positive — i.e. the cursor's leaf-boundary callback is firing and
//! `BufferPool::prefetch` is bumping the right counters.
//!
//! Exact split between hits and misses is not asserted: it depends on
//! buffer-pool residency at hint time, which is sensitive to the
//! capacity of the pool relative to the leaf count and to the order
//! `find_leaf` walks the chain during cursor open. The Wave 1b
//! verification metric is "did we attempt to warm a cold leaf at
//! all" — i.e. a non-zero combined counter.

use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::format::{PageGeneration, PageId, RelId, TuplePtr};
use redlinedb_kernel::index::{
    BtreeIndex, CursorYield, IndexCursor, IndexDescriptor, IndexId, IndexRowRef, IndexUniqueness,
    KeyRange, SnapshotView,
};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

/// Sized so the resulting B-tree definitely splits into at least 10
/// leaves with the 512-byte page size used by `build_fixture`. The
/// equivalence test uses 5_000 keys; we drop to 4_000 here to keep the
/// run cheap while still guaranteeing >> 10 leaves on a 512-byte page.
const FIXTURE_KEYS: u64 = 4_000;
const FIXTURE_SEED: u64 = 11_002;

/// Tiny xorshift64* PRNG so the fixture is deterministic without
/// pulling `rand` into the kernel test surface — same shape as the
/// existing `index_cursor_equivalence` fixture.
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
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

/// Build a multi-leaf B-tree fixture. The buffer pool is sized
/// generously (8192 frames, matching the equivalence-test fixture)
/// so the bulk insertion never trips the "no unpinned frame
/// available for eviction" guard the buffer pool raises when capacity
/// is exhausted by a long pin chain. The cursor sweep still produces
/// non-zero prefetch counters because the prefetch hint fires once
/// per leaf-boundary crossing regardless of whether the target was
/// already resident — the hits/misses split is what shifts with
/// capacity, not the combined total.
fn build_fixture() -> (TempDir, Arc<BufferPool>, BtreeIndex) {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 8192).unwrap());
    let index = BtreeIndex::create(
        Arc::clone(&buffer),
        IndexDescriptor::new(IndexId(11_021), RelId(1), IndexUniqueness::NonUnique),
    )
    .unwrap();

    // Insert in a shuffled order so split paths are exercised. Same
    // pattern as `index_cursor_equivalence::build_fixture`.
    let mut order: Vec<u64> = (0..FIXTURE_KEYS).collect();
    let mut rng = XorShift64::new(FIXTURE_SEED ^ 0xA5A5_5A5A_5A5A_A5A5);
    for i in (1..order.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        order.swap(i, j);
    }
    for (slot, k) in order.iter().enumerate() {
        let key = format!("k{:08}", k);
        let row = IndexRowRef::new(TuplePtr::new_with_generation(
            PageId(20_000 + k),
            (slot % u16::MAX as usize) as u16,
            PageGeneration::ONE,
        ));
        index.insert(key.as_bytes(), row).unwrap();
    }
    (temp, buffer, index)
}

#[test]
fn cursor_prefetch_emits_hit_or_miss_counters() {
    let (_tmp, _buffer, index) = build_fixture();
    let counters = Phase11Counters::new();

    // Sweep the entire fixture in a single open so the cursor crosses
    // every leaf boundary. The unbounded `range_scan` would also work,
    // but the explicit `[k00000000, k99999999)` range matches the
    // shape of typical analytic scans the prefetch hint targets.
    let start_key = b"k00000000".to_vec();
    let end_key = b"k99999999".to_vec();
    let range = KeyRange::half_open(&start_key, &end_key);
    let view = SnapshotView::all();
    let mut cur = IndexCursor::open_with_counters(&index, range, view, Some(&counters)).unwrap();

    // Drain in moderate-sized batches so we get many cursor->buffer
    // round trips. Batch size 32 means a 4_000-row fixture pulls
    // ~125 batches.
    let mut total_rows: usize = 0;
    let mut out: Vec<IndexRowRef> = Vec::new();
    loop {
        out.clear();
        match cur.next_batch(&mut out, 32).unwrap() {
            CursorYield::Batch(n) => {
                total_rows += n;
            }
            CursorYield::End => break,
        }
    }
    let hints = cur.prefetch_hints_emitted();
    cur.close();

    // Sanity: we actually scanned the fixture.
    assert_eq!(
        total_rows as u64, FIXTURE_KEYS,
        "expected to drain all {} fixture keys, got {}",
        FIXTURE_KEYS, total_rows
    );

    let snap = counters.snapshot();
    let combined = snap.prefetch_hits + snap.prefetch_misses;
    assert!(
        combined > 0,
        "expected at least one prefetch_hit or prefetch_miss after a full \
         {}-key sweep; got hits={} misses={} hints_emitted={}",
        FIXTURE_KEYS,
        snap.prefetch_hits,
        snap.prefetch_misses,
        hints,
    );
    // The cursor's own diagnostic counter should match the combined
    // buffer-pool counter exactly: every emitted hint flows through
    // BufferPool::prefetch and ends up in either the hit bucket or
    // the miss bucket. If this ever drifts, it points at a bug in
    // either the cursor's emission site or the buffer pool's
    // counter wiring.
    assert_eq!(
        hints, combined,
        "prefetch_hints_emitted ({}) should equal hits ({}) + misses ({})",
        hints, snap.prefetch_hits, snap.prefetch_misses,
    );

    eprintln!(
        "cursor_prefetch_emits_hit_or_miss_counters: rows={} hints_emitted={} \
         hits={} misses={}",
        total_rows, hints, snap.prefetch_hits, snap.prefetch_misses,
    );
}

#[test]
fn buffer_prefetch_warms_cold_page_then_reports_hit() {
    // Direct test of `BufferPool::prefetch`: allocate a fresh page,
    // drop the guard so the page is unpinned but still resident,
    // call prefetch — the page is resident so we should see a hit.
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    use redlinedb_kernel::format::PageKind;
    let guard = buffer.allocate(PageKind::Heap, RelId(1)).unwrap();
    let page_id = guard.page_id();
    drop(guard);

    let counters = Phase11Counters::new();
    buffer.prefetch(page_id, &counters);
    let snap = counters.snapshot();
    assert_eq!(
        snap.prefetch_hits, 1,
        "prefetch on a resident page should bump hits by 1 (got hits={} misses={})",
        snap.prefetch_hits, snap.prefetch_misses,
    );
    assert_eq!(snap.prefetch_misses, 0);
}

#[test]
fn buffer_prefetch_on_cold_page_reports_miss_and_warms() {
    // Direct test of the cold-load path: write a page through the
    // page file but never pin it. The buffer pool's residency probe
    // should miss; the synchronous cold-load path then loads the
    // page so a follow-up prefetch hits.
    use redlinedb_kernel::format::PageKind;
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    // Generously sized so the cold-load attempt does not race
    // eviction; this test is about counter correctness, not pool
    // pressure.
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());

    // Allocate then immediately flush + drop so the page exists on
    // disk but is no longer resident in the buffer pool.
    let guard = buffer.allocate(PageKind::Heap, RelId(1)).unwrap();
    let page_id = guard.page_id();
    use redlinedb_kernel::format::Lsn;
    guard.mark_dirty(Lsn(1)).unwrap();
    drop(guard);
    // Flush so the page file has the bytes for the cold-load path
    // to read back, then evict to force the next pin/prefetch to
    // reload from disk. The buffer pool exposes `flush_page` and
    // `flush_all`; pair flush_all with eviction by calling pin and
    // letting capacity pressure evict — but at capacity 64 with
    // only one resident page nothing evicts on its own. Instead
    // flush + drop then probe a fresh page id that was never
    // touched: the residency probe must miss because the buffer
    // pool has never seen it.
    buffer.flush_all(Lsn(1)).unwrap();

    // Probe a never-loaded page: the residency probe will miss and
    // the cold-load path will attempt to read the page. Since the
    // page exists on disk (we just allocated it), the read should
    // succeed and the page becomes resident — but the counter
    // bump is "miss" because the page was not resident at probe
    // time, which is the W1-B verification metric.
    let counters = Phase11Counters::new();
    // Re-create a buffer pool against the same page file so the
    // resident set starts empty: this guarantees the next prefetch
    // probe is a real cold-miss.
    drop(buffer);
    let buffer2 = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    buffer2.prefetch(page_id, &counters);
    let snap = counters.snapshot();
    assert_eq!(
        snap.prefetch_misses, 1,
        "prefetch on a non-resident page should bump misses by 1 \
         (got hits={} misses={})",
        snap.prefetch_hits, snap.prefetch_misses,
    );
    assert_eq!(snap.prefetch_hits, 0);

    // WS-C4: the cold-load path is now async. Spin briefly for the
    // background worker to finish the cold load, then a second
    // prefetch should report a hit. This pins down the "warm side-
    // effect" half of the prefetch contract.
    let mut warmed = false;
    for _ in 0..200 {
        if buffer2.resident_pages() > 0 {
            warmed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        warmed,
        "prefetch worker should have warmed the cold page within 1s; resident_pages={}",
        buffer2.resident_pages(),
    );
    buffer2.prefetch(page_id, &counters);
    let snap = counters.snapshot();
    assert_eq!(
        snap.prefetch_hits, 1,
        "second prefetch on a now-resident page should hit (got hits={} misses={})",
        snap.prefetch_hits, snap.prefetch_misses,
    );
    assert_eq!(snap.prefetch_misses, 1);
}
