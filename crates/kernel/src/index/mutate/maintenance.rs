use crate::format::{Csn, PageId};
use crate::{Error, Result};

use super::super::{BtreeIndex, PAGE_LEAF_KIND};
use crate::engine::ConcurrentTxStatus;
use crate::index::cells::Entry;
use crate::txn::TxState;

impl BtreeIndex {
    pub fn compact_leaf_page(&self, page_id: PageId) -> Result<()> {
        self.rewrite_filtered_leaf(page_id, |entry| entry.physically_live())
    }

    pub fn prune_committed_deletes_before(
        &self,
        page_id: PageId,
        tx_status: &ConcurrentTxStatus,
        horizon: Csn,
    ) -> Result<()> {
        self.rewrite_filtered_leaf(page_id, |entry| match entry {
            Entry::Leaf { delete_tx, .. } if *delete_tx != crate::format::TxId::ZERO => !matches!(
                tx_status.state(*delete_tx),
                TxState::Committed(csn) if csn < horizon
            ),
            _ => true,
        })
    }

    fn rewrite_filtered_leaf<F>(&self, page_id: PageId, mut keep: F) -> Result<()>
    where
        F: FnMut(&Entry) -> bool,
    {
        let leaf_latch = self.inner.latches.get(page_id);
        let leaf_write = leaf_latch.write();
        let guard = self.inner.buffer.pin(page_id)?;
        let mut page = guard.mutable_frame()?;
        let page_ref = page
            .page
            .as_mut()
            .ok_or(Error::CorruptPage("resident frame missing page"))?;
        let header = Self::read_page_header(page_ref)?;
        if header.kind != PAGE_LEAF_KIND {
            return Err(Error::CorruptPage("expected leaf page"));
        }
        let entries = self.read_entries(page_ref)?;
        let live: Vec<_> = entries
            .iter()
            .filter(|entry| keep(entry))
            .cloned()
            .collect();
        if live.len() == entries.len() {
            return Ok(());
        }
        Self::rewrite_leaf(
            page_ref,
            self.descriptor().index_id,
            &live,
            header.left,
            header.right,
            header.high_key,
        )?;
        drop(page);
        // LSN sentinel: mutation. Leaf compaction rewrites the page in place.
        guard.mark_dirty(crate::format::Lsn(1))?;
        drop(leaf_write);
        Ok(())
    }
}
