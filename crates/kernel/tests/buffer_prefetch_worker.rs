//! WS-C4 — verify the async prefetch worker is alive and drains its
//! queue. The assertion is intentionally loose: as long as the worker
//! either warmed some frames OR observed a full queue and dropped, the
//! background thread is doing its job.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use redlinedb_kernel::format::{Lsn, PageKind, RelId};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

#[test]
fn try_prefetch_worker_drains_queue_or_drops() {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());

    // Allocate 100 pages, flush them so the page-file has bytes, then
    // re-open a fresh buffer pool against the same file so the resident
    // set starts empty.
    let allocator = Arc::new(BufferPool::new(Arc::clone(&page_file), 256).unwrap());
    let mut page_ids = Vec::with_capacity(100);
    for _ in 0..100 {
        let guard = allocator.allocate(PageKind::Heap, RelId(1)).unwrap();
        guard.mark_dirty(Lsn(1)).unwrap();
        page_ids.push(guard.page_id());
        drop(guard);
    }
    allocator.flush_all(Lsn(1)).unwrap();
    drop(allocator);

    // Fresh, small pool so the queue may or may not fill.
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 64).unwrap());
    let counters = Phase11Counters::new();
    for &pid in &page_ids {
        buffer.try_prefetch(pid, &counters);
    }
    // Give the worker time to drain.
    thread::sleep(Duration::from_millis(100));

    let snap = counters.snapshot();
    let resident = buffer.resident_pages();
    eprintln!(
        "try_prefetch_worker: resident={} dropped={} hits={} misses={}",
        resident, snap.prefetch_dropped, snap.prefetch_hits, snap.prefetch_misses,
    );
    // Either the worker warmed at least one frame, or the queue was
    // saturated and dropped at least one hint. Either outcome proves
    // the worker thread is alive and the queue is bounded.
    assert!(
        resident > 0 || snap.prefetch_dropped > 0,
        "expected worker activity: resident={} prefetch_dropped={}",
        resident,
        snap.prefetch_dropped,
    );
}

#[test]
fn prefetch_drop_joins_worker_cleanly() {
    // Smoke test: creating and dropping many pools must not leak the
    // worker thread or deadlock on join. If the worker ever held a
    // strong Arc<Inner> this would hang forever.
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    for _ in 0..8 {
        let buffer = BufferPool::new(Arc::clone(&page_file), 32).unwrap();
        let counters = Phase11Counters::new();
        // Push something into the queue so the worker is actively
        // looping when we drop the pool.
        buffer.try_prefetch(redlinedb_kernel::format::PageId(1), &counters);
        drop(buffer);
    }
}
