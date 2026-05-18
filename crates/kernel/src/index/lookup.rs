use crate::Result;
use crate::engine::ConcurrentTxStatus;
use crate::format::TxId;
use crate::txn::Snapshot;

use super::cells::{Entry, entry_visible};
use super::{BtreeIndex, IndexRowRef};

impl BtreeIndex {
    pub fn point_lookup(&self, logical_key: &[u8]) -> Result<Vec<IndexRowRef>> {
        self.point_lookup_filter(logical_key, |entry| entry.physically_live())
    }

    pub fn point_lookup_visible(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        logical_key: &[u8],
    ) -> Result<Vec<IndexRowRef>> {
        self.point_lookup_filter(logical_key, |entry| {
            entry_visible(entry, tx_status, snapshot, owner)
        })
    }

    pub(crate) fn point_lookup_filter(
        &self,
        logical_key: &[u8],
        mut visible: impl FnMut(&Entry) -> bool,
    ) -> Result<Vec<IndexRowRef>> {
        // Descend to the leaf whose physical-key range *starts* at or below
        // the search key. With physical separators throughout, navigation
        // keyed on `logical_key` always lands on a leaf whose first entry's
        // logical key is <= search_key OR is the smallest logical key strictly
        // greater than the search key (the latter happens when duplicates
        // straddled an internal split). From there we walk right through
        // sibling leaves until a leaf whose entries are entirely past the
        // search key — at that point all matches have been collected.
        let mut page_id = self.find_leaf(self.meta()?.root_page_id, logical_key)?;
        let mut out: Vec<IndexRowRef> = Vec::new();
        loop {
            let leaf_latch = self.inner.latches.get(page_id);
            let _leaf_read = leaf_latch.read();
            let guard = self.inner.buffer.pin(page_id)?;
            let (first_past_key, right_link) = guard.with_page(|page| {
                let header = Self::read_page_header(page)?;
                let entries = self.read_entries(page)?;
                let mut matched_here = false;
                let mut first_entry_key: Option<Vec<u8>> = None;
                for entry in &entries {
                    if let Entry::Leaf {
                        logical_key: key,
                        row,
                        ..
                    } = entry
                    {
                        if first_entry_key.is_none() {
                            first_entry_key = Some(key.clone());
                        }
                        if !visible(entry) {
                            continue;
                        }
                        if key.as_slice() == logical_key {
                            out.push(*row);
                            matched_here = true;
                        } else if matched_here {
                            // Within one leaf, matches are contiguous because
                            // entries are sorted by physical (= logical || row
                            // suffix). Once we pass them we can stop scanning.
                            break;
                        }
                    }
                }
                let first_past = first_entry_key
                    .as_deref()
                    .map(|k| k > logical_key)
                    .unwrap_or(false);
                Ok((first_past, header.right))
            })?;
            // Stop once we land on a leaf whose first entry's logical key is
            // already strictly past the search key — the leaf chain is sorted,
            // so nothing further could match.
            if first_past_key {
                break;
            }
            match right_link {
                Some(next) => page_id = next,
                None => break,
            }
        }
        Ok(out)
    }
}
