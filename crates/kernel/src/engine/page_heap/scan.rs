//! WS-C3 round 2: page-range parallel heap scan for `PageBackedHeap`.
//!
//! Round-1 (R1-D) shipped `ConcurrentHeap::parallel_scan` on the lane-vector
//! dev heap. The production SQL path reads from `PageBackedHeap`, which is
//! page-paginated and buffer-pool-backed, so the dev-heap API never reached
//! production callers. This module ports the worker-partitioning shape to
//! `PageBackedHeap`: each worker takes a disjoint slice of the requested
//! [`std::ops::Range<PageId>`], pins pages through the shared buffer pool
//! (which is shard-safe per `crates/kernel/src/storage/buffer.rs`), decodes
//! their tuples, applies snapshot visibility, and sends `HeapScanRow`
//! records back to the caller on a bounded `mpsc::sync_channel`.
//!
//! The `rayon` dep is intentionally NOT pulled into kernel; the SQL-side
//! gate is expected to wrap the call with `pool.install(|| ...)` so the
//! `std::thread::scope` workers run inside the rayon pool's context for
//! NUMA / affinity purposes. Callers that want strict pool ownership can
//! pass `worker_count = pool.current_num_threads()`.
//!
//! Result ordering across pages is **not** stable across runs; rows from
//! the same page are emitted in slot order. Callers that need a
//! deterministic order must sort the returned vector.

use std::sync::mpsc;
use std::thread;

use crate::engine::page_heap::{ConcurrentVisibility, PageBackedHeap};
use crate::engine::tx::ConcurrentTxStatus;
use crate::format::{PageId, PageKind, RelId, RowId, TupleVersion, TxId};
use crate::txn::{Snapshot, TupleVisibility};
use crate::{Error, Result};

/// One row materialised by [`PageBackedHeap::parallel_scan_page_range`].
/// Mirrors `ConcurrentHeap::parallel_scan`'s shape but carries the
/// `RelId` so callers can dispatch by relation when the heap holds
/// multiple relations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeapScanRow {
    pub rel_id: RelId,
    pub row_id: RowId,
    pub payload: Vec<u8>,
}

/// Diagnostic counters used by the WS-C3 R2 SQL-side gate to surface
/// "how many pages did this scan visit" + "how many fell through visibility"
/// telemetry without snapshotting the buffer-pool stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParallelScanDiagnostics {
    pub pages_visited: usize,
    pub heap_pages_skipped: usize,
    pub tuples_seen: usize,
    pub tuples_visible: usize,
    pub worker_count: usize,
}

/// Build a fresh diagnostics struct. Exposed as a free fn so callers can
/// pre-allocate a single mutable counter and pass it through the scan
/// without rebuilding the struct on every iteration.
pub fn parallel_scan_diagnostics() -> ParallelScanDiagnostics {
    ParallelScanDiagnostics::default()
}

impl PageBackedHeap {
    /// WS-C3 R2: scan a disjoint page range in parallel, returning every
    /// row visible to `snapshot` whose relation matches `rel_filter`
    /// (or every relation when `rel_filter` is `None`).
    ///
    /// `page_range` is a half-open `[start, end)` interval of `PageId`s.
    /// `workers` is clamped to `[1, page_range.len()]`. The caller is
    /// expected to source the upper bound from `Self::page_count()` and
    /// to gate the dispatch on a rayon pool's presence — that's the
    /// SQL-side responsibility. When `workers == 1` we fall through to
    /// the serial path so the kernel test can assert set-equality.
    ///
    /// Each worker pins one page at a time, drops the guard before
    /// touching the next page, and forwards visible rows on a bounded
    /// `mpsc::sync_channel` so memory stays bounded even when the
    /// consumer is slower than the producers. The drain happens on the
    /// dispatcher thread (the still-serial SQL executor mainline).
    pub fn parallel_scan_page_range(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        page_range: std::ops::Range<PageId>,
        rel_filter: Option<RelId>,
        workers: usize,
        diagnostics: Option<&mut ParallelScanDiagnostics>,
    ) -> Result<Vec<HeapScanRow>> {
        let start = page_range.start.0;
        let end = page_range.end.0;
        if start >= end {
            if let Some(d) = diagnostics {
                d.worker_count = 0;
            }
            return Ok(Vec::new());
        }
        let total_pages = (end - start) as usize;
        let workers = workers.max(1).min(total_pages);

        if workers == 1 {
            return self.serial_scan_page_range(
                tx_status,
                snapshot,
                owner,
                page_range,
                rel_filter,
                diagnostics,
            );
        }

        // Bounded sync channel so producers throttle when the consumer
        // is slower. Capacity tuned to keep ~16 rows of head-room per
        // worker in flight; the upper bound is intentionally modest so
        // RSS does not balloon on the 1M-row smoke.
        let channel_cap = (workers * 16).max(64);
        let (tx, rx) = mpsc::sync_channel::<HeapScanRow>(channel_cap);
        let (diag_tx, diag_rx) = mpsc::channel::<WorkerDiag>();

        let (rows, totals) = thread::scope(|scope| -> Result<(Vec<HeapScanRow>, WorkerDiag)> {
            let mut handles = Vec::with_capacity(workers);
            for worker_idx in 0..workers {
                let heap = &*self;
                let tx = tx.clone();
                let diag_tx = diag_tx.clone();
                let handle = scope.spawn(move || -> Result<()> {
                    let mut diag = WorkerDiag::default();
                    let mut page_no = start.saturating_add(worker_idx as u64);
                    while page_no < end {
                        match heap.collect_heap_page(
                            tx_status,
                            snapshot,
                            owner,
                            PageId(page_no),
                            rel_filter,
                            &mut diag,
                            |row| {
                                // Best-effort send: a closed channel
                                // means the consumer dropped (drained
                                // and bailed early); treat as a clean
                                // stop signal rather than an error.
                                tx.send(row).ok();
                            },
                        ) {
                            Ok(_) => {}
                            Err(err) => {
                                let _ = diag_tx.send(diag);
                                return Err(err);
                            }
                        }
                        page_no = page_no.saturating_add(workers as u64);
                    }
                    let _ = diag_tx.send(diag);
                    Ok(())
                });
                handles.push(handle);
            }
            // Drop the dispatcher's clone so `rx.recv` returns `Err`
            // once every worker has finished.
            drop(tx);
            drop(diag_tx);

            // Consumer mainline: drain the channel while workers are
            // still feeding it. This is the "still-serial executor
            // mainline" referenced in the WS-C3 R2 brief.
            let mut rows = Vec::with_capacity(total_pages * 8);
            while let Ok(row) = rx.recv() {
                rows.push(row);
            }
            let mut totals = WorkerDiag::default();
            while let Ok(d) = diag_rx.recv() {
                totals.add(&d);
            }

            for handle in handles {
                match handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => return Err(err),
                    Err(_) => {
                        return Err(Error::CorruptPage(
                            "parallel_scan_page_range worker panicked",
                        ));
                    }
                }
            }

            Ok((rows, totals))
        })?;

        if let Some(d) = diagnostics {
            d.worker_count = workers;
            d.pages_visited = totals.pages_visited;
            d.heap_pages_skipped = totals.heap_pages_skipped;
            d.tuples_seen = totals.tuples_seen;
            d.tuples_visible = rows.len();
        }
        Ok(rows)
    }

    /// Serial reference scan over a page range. Used by
    /// `parallel_scan_page_range` when `workers == 1` and by the
    /// kernel test that asserts set-equality between the two paths.
    pub fn serial_scan_page_range(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        page_range: std::ops::Range<PageId>,
        rel_filter: Option<RelId>,
        diagnostics: Option<&mut ParallelScanDiagnostics>,
    ) -> Result<Vec<HeapScanRow>> {
        let mut rows = Vec::new();
        let mut totals = WorkerDiag::default();
        let mut page_no = page_range.start.0;
        let end = page_range.end.0;
        while page_no < end {
            self.collect_heap_page(
                tx_status,
                snapshot,
                owner,
                PageId(page_no),
                rel_filter,
                &mut totals,
                |row| rows.push(row),
            )?;
            page_no = page_no.saturating_add(1);
        }
        if let Some(d) = diagnostics {
            d.worker_count = 1;
            d.pages_visited = totals.pages_visited;
            d.heap_pages_skipped = totals.heap_pages_skipped;
            d.tuples_seen = totals.tuples_seen;
            d.tuples_visible = rows.len();
        }
        Ok(rows)
    }

    /// Pin a single heap page, walk its slots, and forward every visible
    /// tuple to `sink`. Returns silently when the page is not a heap
    /// page or when no frame is allocated. Errors short-circuit.
    fn collect_heap_page<F>(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        page_id: PageId,
        rel_filter: Option<RelId>,
        diag: &mut WorkerDiag,
        mut sink: F,
    ) -> Result<()>
    where
        F: FnMut(HeapScanRow),
    {
        let guard = match self.buffer_ref().pin(page_id) {
            Ok(guard) => guard,
            // Lazily-grown heap files can contain pages that have never
            // been allocated; treat them as empty rather than aborting.
            Err(Error::InvalidMagic { actual: 0, .. }) => return Ok(()),
            Err(err) => return Err(err),
        };
        diag.pages_visited += 1;
        guard.with_page(|page| {
            let header = page.header()?;
            if header.kind != PageKind::Heap {
                diag.heap_pages_skipped += 1;
                return Ok(());
            }
            let slot_count = page.slot_count()?;
            for slot in 0..slot_count {
                diag.tuples_seen += 1;
                let bytes = page.cell(slot)?;
                let tuple = match TupleVersion::decode(bytes) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let effective_rel = if tuple.rel_id == RelId::ZERO {
                    header.rel_id
                } else {
                    tuple.rel_id
                };
                if let Some(filter) = rel_filter
                    && filter != effective_rel
                {
                    continue;
                }
                match tuple.visibility_concurrent(tx_status, snapshot, owner) {
                    TupleVisibility::Visible => {
                        sink(HeapScanRow {
                            rel_id: effective_rel,
                            row_id: tuple.row_id,
                            payload: tuple.payload,
                        });
                    }
                    TupleVisibility::Deleted | TupleVisibility::Invisible => {}
                }
            }
            Ok(())
        })
    }
}

/// Per-worker counters merged by the dispatcher.
#[derive(Clone, Copy, Debug, Default)]
struct WorkerDiag {
    pages_visited: usize,
    heap_pages_skipped: usize,
    tuples_seen: usize,
}

impl WorkerDiag {
    fn add(&mut self, other: &Self) {
        self.pages_visited += other.pages_visited;
        self.heap_pages_skipped += other.heap_pages_skipped;
        self.tuples_seen += other.tuples_seen;
    }
}
