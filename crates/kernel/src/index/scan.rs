use crate::engine::ConcurrentTxStatus;
use crate::format::{PageId, TxId};
use crate::txn::Snapshot;
use crate::{Error, Result};

use super::cells::Entry;
use super::cursor::{CursorYield, IndexCursor, KeyRange, SnapshotView};
use super::{BtreeIndex, IndexEntry, IndexRowRef, PAGE_INTERNAL_KIND, PAGE_LEAF_KIND};

/// Default batch size for the materialise-into-`Vec` wrappers that
/// preserve the pre-cursor entry points.
/// Sized to amortise the per-batch counter bumps without committing to
/// hold any particular number of pinned pages — the cursor still
/// releases its leaf guards between batches.
const VEC_WRAPPER_BATCH: usize = 256;

impl BtreeIndex {
    pub fn range_scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<IndexRowRef>> {
        let range = KeyRange::half_open(start, end);
        let mut out = Vec::new();
        let mut cur = IndexCursor::open(self, range, SnapshotView::all())?;
        while let CursorYield::Batch(_) = cur.next_batch(&mut out, VEC_WRAPPER_BATCH)? {}
        Ok(out)
    }

    pub fn range_scan_visible(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<IndexRowRef>> {
        let range = KeyRange::half_open(start, end);
        let view = SnapshotView::visible(tx_status, snapshot, owner);
        let mut out = Vec::new();
        let mut cur = IndexCursor::open(self, range, view)?;
        while let CursorYield::Batch(_) = cur.next_batch(&mut out, VEC_WRAPPER_BATCH)? {}
        Ok(out)
    }

    /// Direct entry-filter range scan. Retained for the equivalence
    /// test (`tests/index_cursor_equivalence.rs`) which asserts the
    /// cursor reproduces the pre-cursor `Vec` output bit-for-bit. Crate-
    /// internal so production code goes through the cursor.
    #[doc(hidden)]
    pub fn range_scan_pre_cursor_for_test(
        &self,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<IndexRowRef>> {
        self.range_scan_filter_pre_cursor(start, end, |entry| entry.physically_live())
    }

    /// Snapshot-visible variant of [`range_scan_pre_cursor_for_test`].
    #[doc(hidden)]
    pub fn range_scan_visible_pre_cursor_for_test(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<IndexRowRef>> {
        self.range_scan_filter_pre_cursor(start, end, |entry| {
            super::cells::entry_visible(entry, tx_status, snapshot, owner)
        })
    }

    /// Verbatim copy of the pre-cursor `range_scan_filter` body. The
    /// equivalence test calls this to compare against the cursor
    /// output. Production callers must not use this — go through
    /// `IndexCursor` (or the `range_scan` wrappers above).
    fn range_scan_filter_pre_cursor(
        &self,
        start: &[u8],
        end: &[u8],
        mut visible: impl FnMut(&Entry) -> bool,
    ) -> Result<Vec<IndexRowRef>> {
        use std::sync::atomic::Ordering as AtomicOrdering;
        let mut out = Vec::new();
        let mut leaf_id = self.find_leaf(self.meta()?.root_page_id, start)?;
        loop {
            let leaf_latch = self.inner.latches.get(leaf_id);
            let _leaf_read = leaf_latch.read();
            let guard = self.inner.buffer.pin(leaf_id)?;
            self.inner
                .range_scan_leaves_visited
                .fetch_add(1, AtomicOrdering::Relaxed);
            let (next, last_logical_key) = guard.with_page(|page| {
                let _header = Self::read_page_header(page)?;
                let mut last_key: Option<Vec<u8>> = None;
                for entry in self.read_entries(page)? {
                    if !visible(&entry) {
                        continue;
                    }
                    if let Entry::Leaf {
                        logical_key, row, ..
                    } = entry
                    {
                        if logical_key.as_slice() >= start && logical_key.as_slice() < end {
                            out.push(row);
                        }
                        last_key = Some(logical_key);
                    }
                }
                Ok((_header.right, last_key))
            })?;
            if let Some(last) = last_logical_key.as_deref()
                && last >= end
            {
                break;
            }
            match next {
                Some(next_id) => {
                    leaf_id = next_id;
                }
                None => break,
            }
        }
        Ok(out)
    }

    /// Walk every live leaf entry in physical-key order. Lane INT uses this
    /// to dump the index for a heap/index equivalence check. The walk:
    ///
    /// 1. Descends from the meta root to the leftmost leaf (following
    ///    `header.left` on each internal page).
    /// 2. Reads each leaf, emits its non-`dead` `Entry::Leaf` rows, then
    ///    follows the `header.right` sibling pointer.
    ///
    /// The result is materialised into a `Vec<IndexEntry>` rather than a
    /// streaming iterator because `BufferPool::pin` returns RAII guards;
    /// keeping a guard live across an iterator boundary would tangle
    /// lifetimes for callers. For paper-grade integrity checks the index
    /// fits in O(N) memory anyway — this is a one-shot diagnostic.
    pub fn iter_all_entries(&self) -> Result<Vec<IndexEntry>> {
        // Lane E failpoint: armed before the integrity checker dumps the
        // tree so the failpoint matrix can later exercise crash-during-check
        // semantics without taking a structural lock or recording any new
        // page images.
        crate::fail_point!("integrity::index::dump");
        let meta = self.meta()?;
        let mut leaf_id = self.leftmost_leaf(meta.root_page_id)?;
        let mut out = Vec::new();
        loop {
            let leaf_latch = self.inner.latches.get(leaf_id);
            let _leaf_read = leaf_latch.read();
            let guard = self.inner.buffer.pin(leaf_id)?;
            let (next, entries) = guard.with_page(|page| {
                let header = Self::read_page_header(page)?;
                if header.kind != PAGE_LEAF_KIND {
                    return Err(Error::CorruptPage(
                        "iter_all_entries: expected leaf in chain",
                    ));
                }
                let entries = self.read_entries(page)?;
                Ok((header.right, entries))
            })?;
            for entry in entries {
                if let Entry::Leaf {
                    logical_key,
                    row,
                    delete_tx,
                    ..
                } = entry
                {
                    if delete_tx != TxId::ZERO {
                        continue;
                    }
                    out.push(IndexEntry {
                        logical_key,
                        row,
                        leaf_page_id: leaf_id,
                    });
                }
            }
            match next {
                Some(next_id) if next_id != leaf_id => leaf_id = next_id,
                _ => return Ok(out),
            }
        }
    }

    pub(super) fn leftmost_leaf(&self, page_id: PageId) -> Result<PageId> {
        let leaf_latch = self.inner.latches.get(page_id);
        let _leaf_read = leaf_latch.read();
        let guard = self.inner.buffer.pin(page_id)?;
        let next = guard.with_page(|page| {
            let header = Self::read_page_header(page)?;
            if header.kind == PAGE_LEAF_KIND {
                return Ok(None);
            }
            if header.kind == PAGE_INTERNAL_KIND {
                let child = header
                    .left
                    .ok_or(Error::CorruptPage("internal page missing leftmost child"))?;
                return Ok(Some(child));
            }
            Err(Error::CorruptPage(
                "leftmost_leaf: unexpected page kind in tree",
            ))
        })?;
        match next {
            Some(child) => self.leftmost_leaf(child),
            None => Ok(page_id),
        }
    }
}
