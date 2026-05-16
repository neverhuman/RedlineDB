//! Streaming `IndexCursor` API for the B-tree leaf chain.
//!
//! Phase 11 Wave 1a Worker A introduces a batch-yielding cursor that walks
//! left-to-right across the leaf chain, applying snapshot visibility per
//! entry and emitting `IndexRowRef` batches up to a caller-supplied cap.
//! The pre-cursor `range_scan` / `range_scan_visible` materialise-into-`Vec`
//! entry points are preserved as thin wrappers around the cursor; their
//! observable behavior (ordering, early-exit on out-of-range leaves,
//! `range_scan_leaves_visited` counter, and visibility filtering) is bit-
//! for-bit identical so existing tests and callers keep working.
//!
//! ## Design notes
//!
//! * `KeyRange<'a>` uses `std::ops::Bound<&'a [u8]>` so callers can express
//!   `[a, b)`, `(a, b]`, fully-open, etc. The cursor stores an *owned* copy
//!   of the bound bytes internally so its lifetime is decoupled from the
//!   open-time borrow.
//! * `SnapshotView<'a>` wraps the existing `(ConcurrentTxStatus, Snapshot,
//!   Option<TxId>)` triple used by `range_scan_visible`. A second variant
//!   `SnapshotView::all()` skips the visibility check and matches the
//!   `range_scan` "physically_live" filter, so both pre-cursor entry points
//!   collapse to a single cursor implementation.
//! * Telemetry: callers may pass `&Phase11Counters` so each entered leaf
//!   bumps `leaf_visits` and each non-empty `next_batch` bumps
//!   `cursor_batches_emitted`. The pre-cursor `range_scan_leaves_visited`
//!   counter on the index itself is *also* updated to keep the existing
//!   `range_scan_terminates_early` test passing.
//! * Prefetch: when the cursor crosses a leaf boundary it emits an
//!   advisory hint for the *next-next* leaf so the buffer pool can
//!   warm it. W1-B (this commit) wires `BufferPool::prefetch` so the
//!   cursor calls it whenever it has a `Phase11Counters` sink and the
//!   newly-loaded leaf advertises a right-link. The hint is purely
//!   advisory: it never blocks the cursor, never propagates errors,
//!   and bumps `Phase11Counters::prefetch_hits` (page was already
//!   resident) or `Phase11Counters::prefetch_misses` (page was cold
//!   or the shard was contended) honestly. The per-cursor
//!   `prefetch_hints_emitted` counter is retained for
//!   diagnostics — it counts hints *attempted*, not hits.
//! * Re-anchoring: the cursor follows the *exact* anchoring contract of
//!   the pre-cursor `range_scan_filter`: each leaf is pinned before its
//!   right-link is read, so we never observe a torn link, and a split
//!   that lands a key on a previously-unobserved leaf still ends up in
//!   the chain we walk. The kernel's `BufferPool::pin` does not surface
//!   a distinguished "page evicted" error, so a higher-level retry loop
//!   would have to recover from any pin failure — same as the pre-cursor
//!   path. We therefore propagate any error verbatim and rely on the
//!   shared anchoring guarantee instead of re-descending mid-scan.

use std::ops::Bound;
use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;
use crate::engine::ConcurrentTxStatus;
use crate::format::{PageId, TxId};
use crate::telemetry::Phase11Counters;
use crate::txn::Snapshot;

use super::cells::{LeafEntry, leaf_entry_visible};
use super::{BtreeIndex, IndexRowRef};

/// Half-open key range expressed via [`std::ops::Bound`]. Callers borrow
/// the bound bytes; the cursor takes an owned copy at `open` time so the
/// borrow may end as soon as `open` returns.
#[derive(Clone, Copy, Debug)]
pub struct KeyRange<'a> {
    pub start: Bound<&'a [u8]>,
    pub end: Bound<&'a [u8]>,
}

impl<'a> KeyRange<'a> {
    /// Convenience constructor for the common `[start, end)` case used
    /// by `range_scan` / `range_scan_visible`.
    pub fn half_open(start: &'a [u8], end: &'a [u8]) -> Self {
        Self {
            start: Bound::Included(start),
            end: Bound::Excluded(end),
        }
    }
}

/// Snapshot-visibility view consumed by [`IndexCursor`]. Matches the
/// existing `range_scan_visible` argument shape; an `all()` constructor
/// produces a "no filter, just physically-live" view used by the
/// snapshot-free `range_scan` wrapper.
#[derive(Clone, Copy)]
pub struct SnapshotView<'a> {
    inner: SnapshotViewKind<'a>,
}

#[derive(Clone, Copy)]
enum SnapshotViewKind<'a> {
    /// Match every physically-live (non-tombstoned) leaf entry. Used by
    /// `BtreeIndex::range_scan` which does not have a transactional view.
    All,
    /// Apply the existing `entry_visible` test using the supplied
    /// `(tx_status, snapshot, owner)` triple.
    Visible {
        tx_status: &'a ConcurrentTxStatus,
        snapshot: &'a Snapshot,
        owner: Option<TxId>,
    },
}

impl<'a> SnapshotView<'a> {
    /// "Show every physically-live entry" view — equivalent to the
    /// `range_scan` filter `entry.physically_live()`.
    pub fn all() -> SnapshotView<'static> {
        SnapshotView {
            inner: SnapshotViewKind::All,
        }
    }

    /// Snapshot-visible view matching `range_scan_visible`'s filter.
    pub fn visible(
        tx_status: &'a ConcurrentTxStatus,
        snapshot: &'a Snapshot,
        owner: Option<TxId>,
    ) -> Self {
        Self {
            inner: SnapshotViewKind::Visible {
                tx_status,
                snapshot,
                owner,
            },
        }
    }

    fn matches(&self, entry: &LeafEntry) -> bool {
        match self.inner {
            SnapshotViewKind::All => entry.delete_tx == TxId::ZERO,
            SnapshotViewKind::Visible {
                tx_status,
                snapshot,
                owner,
            } => leaf_entry_visible(entry, tx_status, snapshot, owner),
        }
    }
}

/// Outcome of a single [`IndexCursor::next_batch`] call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorYield {
    /// `n` rows were appended to the caller's buffer in this call.
    Batch(usize),
    /// No further rows will be produced; the cursor is exhausted.
    End,
}

/// Streaming index cursor. Walks the leaf chain left-to-right starting
/// at the leaf containing `range.start`, applies snapshot visibility per
/// entry, and yields `IndexRowRef`s in batches of up to `max_batch`. See
/// the module docs for design rationale.
pub struct IndexCursor<'idx> {
    index: &'idx BtreeIndex,
    /// Owned start-bound bytes (decoupled from caller's borrow).
    start: Bound<Vec<u8>>,
    /// Owned end-bound bytes (decoupled from caller's borrow).
    end: Bound<Vec<u8>>,
    /// Snapshot visibility view. Held as an owned copy because
    /// `SnapshotView` is `Copy`.
    view: SnapshotView<'idx>,
    /// Optional Phase 11 telemetry sink. `None` keeps the cursor
    /// allocation-free with respect to engine wiring; pre-Phase-11 callers
    /// pass `None` so test-only code can opt in.
    counters: Option<&'idx Phase11Counters>,
    /// Page id of the leaf whose entries we are currently iterating.
    /// `None` once exhausted.
    current_leaf: Option<PageId>,
    /// Decoded entries of `current_leaf`, materialised once per leaf
    /// entry to keep the page guard's lifetime confined to the load
    /// step. The kernel already does this in `range_scan_filter`.
    entries: Vec<LeafEntry>,
    /// Index into `entries` of the next entry to consider.
    entry_idx: usize,
    /// Right-sibling page id of `current_leaf`, cached when the leaf is
    /// loaded so we can advance without re-pinning.
    next_leaf: Option<PageId>,
    /// Logical key of the last `Entry::Leaf` seen on `current_leaf`,
    /// regardless of visibility. Drives the same early-exit short-
    /// circuit that `range_scan_filter` uses to stop walking sibling
    /// leaves once the upper bound is reached.
    last_logical_key: Option<Vec<u8>>,
    /// Set once the cursor has determined no further leaves can yield
    /// in-range rows. After it flips, `next_batch` returns `End` even
    /// if `entries` still has un-visited slots (those slots have keys
    /// past the upper bound).
    exhausted: bool,
    /// Diagnostic counter of prefetch hints emitted on leaf-boundary
    /// crossings. Each emission is also dispatched to
    /// [`BufferPool::prefetch`] when `counters` is `Some`, so the
    /// W1-B equivalence test asserts this counter equals
    /// `prefetch_hits + prefetch_misses` on the wired sink.
    prefetch_hints_emitted: u64,
}

impl<'idx> IndexCursor<'idx> {
    /// Open a cursor over `range` against `index`, applying `view` for
    /// per-entry visibility filtering. `counters` is optional; passing
    /// `None` skips Phase 11 telemetry but the cursor still updates the
    /// per-index `range_scan_leaves_visited` counter so existing tests
    /// pass through the wrappers.
    pub fn open(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
    ) -> Result<Self> {
        Self::open_with_counters(index, range, view, None)
    }

    /// Same as [`open`](Self::open) but additionally bumps Phase 11
    /// counters for every leaf entered and every non-empty batch.
    pub fn open_with_counters(
        index: &'idx BtreeIndex,
        range: KeyRange<'_>,
        view: SnapshotView<'idx>,
        counters: Option<&'idx Phase11Counters>,
    ) -> Result<Self> {
        let start_owned = bound_to_owned(range.start);
        let end_owned = bound_to_owned(range.end);
        // `find_leaf` requires a concrete probe key. Map an unbounded
        // start to the empty key (the leftmost possible byte string).
        let probe: &[u8] = match &start_owned {
            Bound::Included(b) | Bound::Excluded(b) => b.as_slice(),
            Bound::Unbounded => &[],
        };
        let leaf_id = index.find_leaf(index.meta()?.root_page_id, probe)?;
        let mut cursor = Self {
            index,
            start: start_owned,
            end: end_owned,
            view,
            counters,
            current_leaf: Some(leaf_id),
            entries: Vec::new(),
            entry_idx: 0,
            next_leaf: None,
            last_logical_key: None,
            exhausted: false,
            prefetch_hints_emitted: 0,
        };
        cursor.load_current_leaf()?;
        // Warm the second leaf before the cursor crosses its first
        // boundary. Same advisory contract as the per-advance hint —
        // see `prefetch_hint` for details.
        if let Some(next_next) = cursor.next_leaf {
            cursor.prefetch_hint(next_next);
        }
        Ok(cursor)
    }

    /// Drain rows up to `max_batch` into `out`. Returns:
    ///
    /// * `CursorYield::Batch(n)` — `n` rows were appended; more may
    ///   follow on subsequent calls.
    /// * `CursorYield::End` — the cursor is exhausted; subsequent
    ///   calls keep returning `End`.
    ///
    /// Telemetry: increments [`Phase11Counters::cursor_batches_emitted`]
    /// once per `Batch(_)` return.
    pub fn next_batch(
        &mut self,
        out: &mut Vec<IndexRowRef>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        loop {
            // Drain whatever is left in the current leaf first.
            while self.entry_idx < self.entries.len() {
                if out.len() >= target {
                    let pushed = out.len() - start_len;
                    if let Some(c) = self.counters {
                        c.cursor_batches_emitted
                            .fetch_add(1, AtomicOrdering::Relaxed);
                    }
                    return Ok(CursorYield::Batch(pushed));
                }
                let entry = &self.entries[self.entry_idx];
                self.entry_idx += 1;
                if self.view.matches(entry) && self.in_range(&entry.logical_key) {
                    out.push(entry.row);
                }
            }
            // Current leaf exhausted. Decide whether to advance.
            // Mirror `range_scan_filter`: if the last logical key on
            // this leaf is already >= the (excluded) upper bound, the
            // chain is done. `last_logical_key` is set during
            // `load_current_leaf` from the *last* `Entry::Leaf` we
            // saw, regardless of visibility — this keeps the early
            // exit identical to the pre-cursor wrapper.
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.advance_to(next_id)?;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        let pushed = out.len() - start_len;
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    /// Phase 11 W1-E variant of [`next_batch`] that also yields the
    /// encoded `logical_key` bytes alongside each row. Covering-scan
    /// callers (SQL `SELECT k, v FROM t WHERE k BETWEEN ? AND ?`
    /// against an index covering `k, v`) read columns straight off
    /// these bytes without ever hitting the heap.
    ///
    /// The `Vec<u8>` carries the same encoded shape that
    /// `encode_index_key` produced: per-part type tag + body + `0xff`
    /// terminator, with `Desc` parts bit-inverted in place. Decoding
    /// is the caller's job (the SQL layer keeps the part-shape
    /// awareness it already needs for predicate matching).
    ///
    /// Telemetry: bumps `Phase11Counters::cursor_batches_emitted`
    /// exactly like [`next_batch`].
    pub fn next_batch_with_keys(
        &mut self,
        out: &mut Vec<(Vec<u8>, IndexRowRef)>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        loop {
            while self.entry_idx < self.entries.len() {
                if out.len() >= target {
                    let pushed = out.len() - start_len;
                    if let Some(c) = self.counters {
                        c.cursor_batches_emitted
                            .fetch_add(1, AtomicOrdering::Relaxed);
                    }
                    return Ok(CursorYield::Batch(pushed));
                }
                let entry = &self.entries[self.entry_idx];
                self.entry_idx += 1;
                if self.view.matches(entry) && self.in_range(&entry.logical_key) {
                    out.push((entry.logical_key.clone(), entry.row));
                }
            }
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.advance_to(next_id)?;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        let pushed = out.len() - start_len;
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    /// Count rows in the current scan window without materialising rowids.
    pub fn next_count_batch(&mut self, max_batch: usize) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let mut pushed = 0_usize;
        loop {
            while self.entry_idx < self.entries.len() {
                if pushed >= max_batch {
                    if let Some(c) = self.counters {
                        c.cursor_batches_emitted
                            .fetch_add(1, AtomicOrdering::Relaxed);
                    }
                    return Ok(CursorYield::Batch(pushed));
                }
                let entry = &self.entries[self.entry_idx];
                self.entry_idx += 1;
                if self.view.matches(entry) && self.in_range(&entry.logical_key) {
                    pushed += 1;
                }
            }
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => self.advance_to(next_id)?,
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            if let Some(c) = self.counters {
                c.cursor_batches_emitted
                    .fetch_add(1, AtomicOrdering::Relaxed);
            }
            Ok(CursorYield::Batch(pushed))
        }
    }

    /// Closes the cursor explicitly. Currently a no-op — the cursor
    /// holds no external resources between batches; pin guards are
    /// dropped at the end of each `load_current_leaf` call. Provided
    /// so callers can spell out "I am done" symmetrically with `open`.
    pub fn close(self) {
        // Drop self.
    }

    /// Returns the running count of prefetch hints emitted by this
    /// cursor. Each hint is also dispatched to
    /// [`BufferPool::prefetch`] when the cursor was opened with a
    /// `Phase11Counters` sink (W1-B), so this counter equals
    /// `prefetch_hits + prefetch_misses` on the sink in the
    /// counters-wired case. Exposed for tests and diagnostics.
    pub fn prefetch_hints_emitted(&self) -> u64 {
        self.prefetch_hints_emitted
    }

    /// Advance to `next_id` and load its entries. Mirrors the pre-cursor
    /// `range_scan_filter` advance protocol exactly — propagate any
    /// pin/decode failure to the caller; the index crate has no
    /// distinguished "evicted under me" error so a higher layer would
    /// have to retry.
    fn advance_to(&mut self, next_id: PageId) -> Result<()> {
        self.current_leaf = Some(next_id);
        self.load_current_leaf()?;
        // Crossing a leaf boundary: now that `next_id` is loaded we
        // know its right-link (cached in `self.next_leaf`), which is
        // the *next-next* leaf the cursor will visit. Emit an
        // advisory prefetch hint to warm it. The hint is purely
        // best-effort — see `BufferPool::prefetch` for the contract.
        if let Some(next_next) = self.next_leaf {
            self.prefetch_hint(next_next);
        }
        Ok(())
    }

    /// Pin the current leaf, decode its entries, and cache the right
    /// sibling pointer. Bumps the per-index `range_scan_leaves_visited`
    /// counter so the existing `range_scan_terminates_early`
    /// test passes through the wrapper, and (if present) the Phase 11
    /// `leaf_visits` counter.
    fn load_current_leaf(&mut self) -> Result<()> {
        let leaf_id = match self.current_leaf {
            Some(id) => id,
            None => {
                self.entries.clear();
                self.entry_idx = 0;
                self.next_leaf = None;
                self.last_logical_key = None;
                return Ok(());
            }
        };
        let guard = self.index.inner.buffer.pin(leaf_id)?;
        self.index
            .inner
            .range_scan_leaves_visited
            .fetch_add(1, AtomicOrdering::Relaxed);
        if let Some(c) = self.counters {
            c.leaf_visits.fetch_add(1, AtomicOrdering::Relaxed);
        }
        let (right, entries, last_key) = guard.with_page(|page| {
            let header = BtreeIndex::read_page_header(page)?;
            let entries = self.index.read_leaf_entries(page)?;
            let mut last_key: Option<Vec<u8>> = None;
            for entry in &entries {
                last_key = Some(entry.logical_key.clone());
            }
            Ok((header.right, entries, last_key))
        })?;
        self.entries = entries;
        self.entry_idx = 0;
        self.next_leaf = right;
        self.last_logical_key = last_key;
        Ok(())
    }

    /// `true` if `logical_key` lies within `[start, end)` (or whatever
    /// the user-supplied bounds declare).
    fn in_range(&self, logical_key: &[u8]) -> bool {
        let lower_ok = match &self.start {
            Bound::Included(b) => logical_key >= b.as_slice(),
            Bound::Excluded(b) => logical_key > b.as_slice(),
            Bound::Unbounded => true,
        };
        if !lower_ok {
            return false;
        }
        match &self.end {
            Bound::Included(b) => logical_key <= b.as_slice(),
            Bound::Excluded(b) => logical_key < b.as_slice(),
            Bound::Unbounded => true,
        }
    }

    /// Mirror of `range_scan_filter`'s post-leaf early-exit: if the
    /// last leaf entry's logical key has met-or-passed the upper
    /// bound, no further sibling can contribute. `Bound::Unbounded`
    /// callers always continue.
    fn leaf_chain_past_end(&self) -> bool {
        let Some(last) = self.last_logical_key.as_deref() else {
            return false;
        };
        match &self.end {
            // Match the pre-cursor `if last >= end` condition exactly: the
            // pre-cursor code always uses an exclusive upper bound, so
            // `last >= end_excluded` <=> "no more matches possible".
            Bound::Excluded(b) => last >= b.as_slice(),
            // Inclusive upper bound: keep walking while `last <= b`;
            // stop only once `last > b`.
            Bound::Included(b) => last > b.as_slice(),
            Bound::Unbounded => false,
        }
    }

    /// Best-effort prefetch hint for the *next-next* leaf. Bumps the
    /// per-cursor diagnostic counter (`prefetch_hints_emitted`) and,
    /// when a `Phase11Counters` sink is wired in, dispatches to
    /// [`BufferPool::prefetch`] to warm the target page.
    ///
    /// The cursor only emits hints when it has somewhere to record
    /// the resulting hit/miss — a callerless cursor (no counters)
    /// still bumps its internal `prefetch_hints_emitted` for the
    /// existing equivalence test, but skips the buffer-pool call so
    /// the no-counters case stays observably unchanged.
    fn prefetch_hint(&mut self, target: PageId) {
        self.prefetch_hints_emitted = self.prefetch_hints_emitted.saturating_add(1);
        if let Some(c) = self.counters {
            // `BufferPool::prefetch` is purely advisory: it never
            // blocks on a contended shard, never holds a write lock,
            // and swallows any I/O error so the cursor's hot path
            // remains correctness-preserving.
            self.index.inner.buffer.prefetch(target, c);
        }
    }
}

#[path = "cursor/raw.rs"]
mod raw;

pub use raw::RawIndexCursor;

fn cached_tx_visible(
    tx_status: &ConcurrentTxStatus,
    snapshot: &Snapshot,
    owner: Option<TxId>,
    tx: TxId,
    cache: &mut Vec<(TxId, bool)>,
) -> bool {
    if let Some((_, visible)) = cache.iter().find(|(cached, _)| *cached == tx) {
        return *visible;
    }
    let visible = tx_status.is_tx_visible(tx, snapshot, owner);
    cache.push((tx, visible));
    visible
}

fn bound_to_owned(bound: Bound<&[u8]>) -> Bound<Vec<u8>> {
    match bound {
        Bound::Included(b) => Bound::Included(b.to_vec()),
        Bound::Excluded(b) => Bound::Excluded(b.to_vec()),
        Bound::Unbounded => Bound::Unbounded,
    }
}

#[cfg(test)]
mod tests {
    use super::super::LeafEntry;
    use super::*;

    #[test]
    fn snapshot_view_all_filters_to_physically_live() {
        // Pure structural sanity: an `All` view delegates to
        // `physically_live`. A leaf entry with `delete_tx == ZERO`
        // is live; a tombstoned one is not.
        use crate::format::{PageGeneration, PageId, RowId, TuplePtr, TxId};
        let row = IndexRowRef {
            row_id: RowId(0),
            tuple: TuplePtr::new_with_generation(PageId(1), 0, PageGeneration::ONE),
        };
        let live = LeafEntry {
            logical_key: b"a".to_vec(),
            row,
            create_tx: TxId::ZERO,
            delete_tx: TxId::ZERO,
        };
        let dead = LeafEntry {
            logical_key: b"b".to_vec(),
            row,
            create_tx: TxId::ZERO,
            delete_tx: TxId(7),
        };
        let view = SnapshotView::all();
        assert!(view.matches(&live));
        assert!(!view.matches(&dead));
    }

    #[test]
    fn key_range_helpers() {
        let r = KeyRange::half_open(b"a", b"z");
        assert!(matches!(r.start, Bound::Included(b) if b == b"a"));
        assert!(matches!(r.end, Bound::Excluded(b) if b == b"z"));
    }

    #[test]
    fn bound_to_owned_round_trips() {
        let b: Bound<&[u8]> = Bound::Included(b"hi");
        match bound_to_owned(b) {
            Bound::Included(v) => assert_eq!(v, b"hi".to_vec()),
            _ => panic!("wrong bound kind"),
        }
        assert!(matches!(bound_to_owned(Bound::Unbounded), Bound::Unbounded));
    }
}
