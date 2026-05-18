use crate::format::{Lsn, TxId};
use crate::txn::Snapshot;
use crate::{Error, Result};

use super::super::cells::delete_marker_visible;
use super::super::{BtreeIndex, IndexRowRef, KeyBuf, NON_TRANSACTIONAL_DELETE_TX, PAGE_LEAF_KIND};
use crate::engine::ConcurrentTxStatus;
use crate::wal::WalPayload;

impl BtreeIndex {
    pub fn delete_mark(&self, logical_key: &[u8], row: IndexRowRef) -> Result<()> {
        self.delete_mark_tx(NON_TRANSACTIONAL_DELETE_TX, logical_key, row)
    }

    pub fn delete_mark_tx(
        &self,
        tx_id: crate::format::TxId,
        logical_key: &[u8],
        row: IndexRowRef,
    ) -> Result<()> {
        self.delete_mark_tx_inner(
            tx_id,
            logical_key,
            row,
            None,
            tx_id != TxId::ZERO,
            Lsn::new(1),
        )
    }

    pub(crate) fn delete_mark_recovered_tx(
        &self,
        tx_id: crate::format::TxId,
        logical_key: &[u8],
        row: IndexRowRef,
        lsn: Lsn,
    ) -> Result<()> {
        self.delete_mark_tx_inner(tx_id, logical_key, row, None, false, lsn)
    }

    pub fn delete_mark_tx_visible(
        &self,
        tx_status: &ConcurrentTxStatus,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        tx_id: TxId,
        logical_key: &[u8],
        row: IndexRowRef,
    ) -> Result<()> {
        self.delete_mark_tx_inner(
            tx_id,
            logical_key,
            row,
            Some((tx_status, snapshot, owner)),
            tx_id != TxId::ZERO,
            Lsn::new(1),
        )
    }

    fn delete_mark_tx_inner(
        &self,
        tx_id: TxId,
        logical_key: &[u8],
        row: IndexRowRef,
        visibility: Option<(&ConcurrentTxStatus, &Snapshot, Option<TxId>)>,
        emit_wal: bool,
        lsn: Lsn,
    ) -> Result<()> {
        let mut probe = KeyBuf::new();
        probe.extend_logical(logical_key);
        probe.append_row_ref_suffix(row);
        let path = self.find_leaf_path(self.meta()?.root_page_id, probe.as_slice())?;
        let mut leaf_id = *path.last().ok_or(Error::CorruptPage("empty search path"))?;
        loop {
            let leaf_latch = self.inner.latches.get(leaf_id);
            let leaf_write = leaf_latch.write();
            let guard = self.inner.buffer.pin(leaf_id)?;
            let mut page = guard.mutable_frame()?;
            let page_ref = page
                .page
                .as_mut()
                .ok_or(Error::CorruptPage("resident frame missing page"))?;
            let header = Self::read_page_header(page_ref)?;
            if header.kind != PAGE_LEAF_KIND {
                return Err(Error::CorruptPage("expected leaf page"));
            }
            if !header.high_key.is_empty() && probe.as_slice() >= header.high_key.as_slice() {
                let Some(right) = header.right else {
                    return Err(Error::CorruptPage("index high key missing right link"));
                };
                drop(page);
                drop(leaf_write);
                leaf_id = right;
                continue;
            }
            let mut entries = self.read_entries(page_ref)?;
            let mut changed = false;
            let mut last_key_matches_search = false;
            for entry in &mut entries {
                if let crate::index::cells::Entry::Leaf {
                    logical_key: key,
                    row: entry_row,
                    delete_tx,
                    ..
                } = entry
                {
                    if key.as_slice() == logical_key {
                        last_key_matches_search = true;
                    }
                    if key.as_slice() == logical_key && *entry_row == row {
                        if delete_marker_visible(*delete_tx, visibility) {
                            return Ok(());
                        }
                        *delete_tx = tx_id;
                        changed = true;
                        break;
                    }
                }
            }
            if changed {
                Self::rewrite_leaf(
                    page_ref,
                    self.descriptor().index_id,
                    &entries,
                    header.left,
                    header.right,
                    header.high_key,
                )?;
                drop(page);
                if emit_wal {
                    // Lane E failpoint: armed before the delete-mark becomes
                    // durable; verifies that recovery either restores the
                    // entry (if pre-fsync) or surfaces the tombstone (if
                    // post-fsync) but never both.
                    crate::fail_point!("index::delete");
                    let end_lsn = self.append_index_delta(
                        tx_id,
                        WalPayload::IndexDelete {
                            tx_id,
                            index_id: self.descriptor().index_id.0,
                            logical_key: logical_key.to_vec(),
                            row,
                        },
                    )?;
                    guard.mark_dirty(end_lsn)?;
                } else {
                    guard.mark_dirty(lsn)?;
                }
                drop(leaf_write);
                return Ok(());
            }
            // No match here. If duplicates of this logical_key may continue on
            // the right sibling, walk right and keep looking.
            drop(page);
            if last_key_matches_search && let Some(right) = header.right {
                drop(leaf_write);
                leaf_id = right;
                continue;
            }
            drop(leaf_write);
            return Ok(());
        }
    }
}
