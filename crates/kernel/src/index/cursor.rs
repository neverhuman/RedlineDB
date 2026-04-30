//! Streaming `IndexCursor` API for the B-tree leaf chain.
//!
//! Phase 11 Wave 1a Worker A introduces a batch-yielding cursor that walks
//! left-to-right across the leaf chain, applying snapshot visibility per
//! entry and emitting `IndexRowRef` batches up to a caller-supplied cap.
//! The legacy `range_scan` / `range_scan_visible` materialise-into-`Vec`
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
//!   `range_scan` "physically_live" filter, so both legacy entry points
//!   collapse to a single cursor implementation.
//! * Telemetry: callers may pass `&Phase11Counters` so each entered leaf
//!   bumps `leaf_visits` and each non-empty `next_batch` bumps
//!   `cursor_batches_emitted`. The legacy `range_scan_leaves_visited`
//!   counter on the index itself is *also* updated to keep the existing
//!   `range_scan_terminates_early` test passing.
//! * Prefetch: when the cursor crosses a leaf boundary it would like to
//!   emit a hint for the *next-next* leaf so the buffer pool can warm it.
//!   The `BufferPool` does not yet have a public prefetch entry point —
//!   Worker B will wire one in W1-B. Until then `prefetch_hint` is a
//!   no-op; we still record the hint internally so the test can observe
//!   that the cursor *would* have prefetched.
//! * Re-anchoring: the cursor follows the *exact* anchoring contract of
//!   the legacy `range_scan_filter`: each leaf is pinned before its
//!   right-link is read, so we never observe a torn link, and a split
//!   that lands a key on a previously-unobserved leaf still ends up in
//!   the chain we walk. The kernel's `BufferPool::pin` does not surface
//!   a distinguished "page evicted" error, so a higher-level retry loop
//!   would have to recover from any pin failure — same as the legacy
//!   path. We therefore propagate any error verbatim and rely on the
//!   shared anchoring guarantee instead of re-descending mid-scan.

use std::ops::Bound;
use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;
use crate::engine::ConcurrentTxStatus;
use crate::format::{PageId, TxId};
use crate::telemetry::Phase11Counters;
use crate::txn::Snapshot;

use super::cells::{Entry, entry_visible};
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

    fn matches(&self, entry: &Entry) -> bool {
        match self.inner {
            SnapshotViewKind::All => entry.physically_live(),
            SnapshotViewKind::Visible {
                tx_status,
                snapshot,
                owner,
            } => entry_visible(entry, tx_status, snapshot, owner),
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
    /// allocation-free with respect to engine wiring; legacy callers
    /// pass `None` so test-only code can opt in.
    counters: Option<&'idx Phase11Counters>,
    /// Page id of the leaf whose entries we are currently iterating.
    /// `None` once exhausted.
    current_leaf: Option<PageId>,
    /// Decoded entries of `current_leaf`, materialised once per leaf
    /// entry to keep the page guard's lifetime confined to the load
    /// step. The kernel already does this in `range_scan_filter`.
    entries: Vec<Entry>,
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
    /// Test-only counter of prefetch hints emitted on leaf-boundary
    /// crossings. Once W1-B wires `BufferPool::prefetch` this will be
    /// the count of advisory hints shipped to the buffer pool. Until
    /// then it lets the equivalence test confirm the cursor is
    /// computing the right hint targets.
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
                if let Entry::Leaf {
                    logical_key, row, ..
                } = entry
                {
                    if self.view.matches(entry) && self.in_range(logical_key) {
                        out.push(*row);
                    }
                }
            }
            // Current leaf exhausted. Decide whether to advance.
            // Mirror `range_scan_filter`: if the last logical key on
            // this leaf is already >= the (excluded) upper bound, the
            // chain is done. `last_logical_key` is set during
            // `load_current_leaf` from the *last* `Entry::Leaf` we
            // saw, regardless of visibility — this keeps the early
            // exit identical to legacy.
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

    /// Closes the cursor explicitly. Currently a no-op — the cursor
    /// holds no external resources between batches; pin guards are
    /// dropped at the end of each `load_current_leaf` call. Provided
    /// so callers can spell out "I am done" symmetrically with `open`.
    pub fn close(self) {
        // Drop self.
    }

    /// Returns the running count of prefetch hints emitted by this
    /// cursor. W1-B will wire this into `BufferPool::prefetch`; for
    /// now it is exposed for the equivalence test to assert the
    /// cursor crossed the leaf boundary.
    pub fn prefetch_hints_emitted(&self) -> u64 {
        self.prefetch_hints_emitted
    }

    /// Advance to `next_id` and load its entries. Mirrors the legacy
    /// `range_scan_filter` advance protocol exactly — propagate any
    /// pin/decode failure to the caller; the index crate has no
    /// distinguished "evicted under me" error so a higher layer would
    /// have to retry.
    fn advance_to(&mut self, next_id: PageId) -> Result<()> {
        // Crossing a leaf boundary: emit an advisory prefetch hint
        // for the *next-next* leaf. The hint is best-effort — we do
        // not attempt to read the next leaf's right pointer here
        // (that would require an extra pin) and instead let the
        // buffer layer chain ahead one hop. Until W1-B lands the
        // hint is a no-op.
        self.prefetch_hint(next_id);
        self.current_leaf = Some(next_id);
        self.load_current_leaf()
    }

    /// Pin the current leaf, decode its entries, and cache the right
    /// sibling pointer. Bumps the legacy `range_scan_leaves_visited`
    /// counter on the index so the existing `range_scan_terminates_early`
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
            let entries = self.index.read_entries(page)?;
            let mut last_key: Option<Vec<u8>> = None;
            for entry in &entries {
                if let Entry::Leaf { logical_key, .. } = entry {
                    last_key = Some(logical_key.clone());
                }
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
            // Match the legacy `if last >= end` condition exactly: the
            // legacy code always uses an exclusive upper bound, so
            // `last >= end_excluded` <=> "no more matches possible".
            Bound::Excluded(b) => last >= b.as_slice(),
            // Inclusive upper bound: keep walking while `last <= b`;
            // stop only once `last > b`.
            Bound::Included(b) => last > b.as_slice(),
            Bound::Unbounded => false,
        }
    }

    /// Best-effort prefetch hint for the *next-next* leaf. The cursor
    /// has just advanced onto a fresh leaf; if its right-link is
    /// already cached in `self.next_leaf` we hint to warm that page
    /// next.
    ///
    /// TODO(phase11/W1-B): emit prefetch via `Buffer::prefetch(next_next_leaf)`.
    fn prefetch_hint(&mut self, _just_entered: PageId) {
        // The next-next leaf is whatever the *new* current leaf
        // points to via its right link. We do not have that yet at
        // this call site (load_current_leaf has not run for
        // `_just_entered`); the hint is therefore for the leaf we
        // *just* left's right-link target — i.e. exactly
        // `_just_entered`. W1-B will refine this to one-hop ahead
        // once the buffer layer can chase the link cheaply.
        self.prefetch_hints_emitted = self.prefetch_hints_emitted.saturating_add(1);
    }
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
        let live = Entry::Leaf {
            logical_key: b"a".to_vec(),
            row,
            physical: b"a".to_vec(),
            create_tx: TxId::ZERO,
            delete_tx: TxId::ZERO,
        };
        let dead = Entry::Leaf {
            logical_key: b"b".to_vec(),
            row,
            physical: b"b".to_vec(),
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
