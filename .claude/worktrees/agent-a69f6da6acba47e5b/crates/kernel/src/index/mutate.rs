use crate::engine::ConcurrentTxStatus;
use crate::format::{Csn, Lsn, PAGE_HEADER_LEN, PageId, SLOT_LEN, TxId};
use crate::txn::Snapshot;
use crate::wal::WalPayload;
use crate::{Error, Result};

use super::cells::{Entry, delete_marker_visible};
use super::{
    BtreeIndex, INDEX_SPECIAL_LEN, IndexRowRef, IndexUniqueness, KeyBuf,
    NON_TRANSACTIONAL_DELETE_TX, PAGE_LEAF_KIND,
};

impl BtreeIndex {
    pub fn insert(&self, logical_key: &[u8], row: IndexRowRef) -> Result<()> {
        self.insert_tx(crate::format::TxId::ZERO, logical_key, row)
    }

    pub fn insert_tx(
        &self,
        tx_id: crate::format::TxId,
        logical_key: &[u8],
        row: IndexRowRef,
    ) -> Result<()> {
        self.insert_tx_inner(tx_id, logical_key, row, tx_id != TxId::ZERO, Lsn::new(1))
    }

    pub(crate) fn insert_recovered_tx(
        &self,
        tx_id: crate::format::TxId,
        logical_key: &[u8],
        row: IndexRowRef,
        lsn: Lsn,
    ) -> Result<()> {
        self.insert_tx_inner(tx_id, logical_key, row, false, lsn)
    }

    fn insert_tx_inner(
        &self,
        tx_id: crate::format::TxId,
        logical_key: &[u8],
        row: IndexRowRef,
        emit_wal: bool,
        lsn: Lsn,
    ) -> Result<()> {
        let _structure = self
            .inner
            .structure_lock
            .lock()
            .map_err(|_| Error::CorruptPage("index structure mutex poisoned"))?;
        let mut physical = KeyBuf::new();
        physical.extend_logical(logical_key);
        physical.append_row_ref_suffix(row);
        // Navigate by the candidate's *physical* bytes: separators in the tree
        // are normally logical keys, but a duplicate-key split (where every
        // entry on a leaf shared one logical key) installs a physical-key
        // separator instead. Comparing with physical bytes works for both
        // cases — physical = logical || row_ref_suffix, so it sorts identically
        // to logical for non-duplicate keys and resolves duplicates by row_id.
        loop {
            let root = self.meta()?.root_page_id;
            let path = self.find_leaf_path(root, physical.as_slice())?;
            let leaf_id = *path.last().ok_or(Error::CorruptPage("empty search path"))?;
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
            // Defensive: if the leaf still claims our key belongs to a sibling
            // (concurrent split window), retry the descent.
            if !header.high_key.is_empty() && physical.as_slice() >= header.high_key.as_slice() {
                continue;
            }
            let mut entries = self.read_entries(page_ref)?;
            let candidate = Entry::Leaf {
                logical_key: logical_key.to_vec(),
                row,
                physical: physical.as_slice().to_vec(),
                create_tx: tx_id,
                delete_tx: TxId::ZERO,
            };
            match entries.binary_search_by(|entry| entry.compare(&candidate)) {
                Ok(pos) => {
                    if self.meta()?.uniqueness == IndexUniqueness::Unique {
                        return Err(Error::WriteConflict);
                    }
                    entries.insert(pos, candidate);
                }
                Err(pos) => entries.insert(pos, candidate),
            }
            let body_capacity = page_ref
                .as_bytes()
                .len()
                .saturating_sub(PAGE_HEADER_LEN + INDEX_SPECIAL_LEN);
            let required = Self::encoded_entries_len(&entries) + entries.len() * SLOT_LEN;
            if required > body_capacity {
                drop(page);
                let path = self.find_leaf_path(self.meta()?.root_page_id, physical.as_slice())?;
                let leaf_id = *path.last().ok_or(Error::CorruptPage("empty search path"))?;
                self.split_leaf_and_insert(
                    &path[..path.len().saturating_sub(1)],
                    leaf_id,
                    logical_key,
                    row,
                    physical.as_slice().to_vec(),
                    tx_id,
                    emit_wal,
                    lsn,
                )?;
                return Ok(());
            }
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
                // Lane E failpoint: armed before the leaf-write becomes
                // visible (mark_dirty + WAL append). A crash here proves the
                // index entry is either fully reflected post-recovery or
                // absent.
                crate::fail_point!("index::insert");
                let end_lsn = self.append_index_delta(
                    tx_id,
                    WalPayload::IndexInsert {
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
            return Ok(());
        }
    }

    pub fn insert_unique(&self, owner: u64, logical_key: &[u8], row: IndexRowRef) -> Result<()> {
        let _guard = self.lock_unique_key(owner, logical_key)?;
        if !self.point_lookup(logical_key)?.is_empty() {
            return Err(Error::WriteConflict);
        }
        self.insert(logical_key, row)
    }

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
        let _structure = self
            .inner
            .structure_lock
            .lock()
            .map_err(|_| Error::CorruptPage("index structure mutex poisoned"))?;
        let mut probe = KeyBuf::new();
        probe.extend_logical(logical_key);
        probe.append_row_ref_suffix(row);
        let path = self.find_leaf_path(self.meta()?.root_page_id, probe.as_slice())?;
        let mut leaf_id = *path.last().ok_or(Error::CorruptPage("empty search path"))?;
        loop {
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
            let mut entries = self.read_entries(page_ref)?;
            let mut changed = false;
            let mut last_key_matches_search = false;
            for entry in &mut entries {
                if let Entry::Leaf {
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
                return Ok(());
            }
            // No match here. If duplicates of this logical_key may continue on
            // the right sibling, walk right and keep looking.
            drop(page);
            if last_key_matches_search && let Some(right) = header.right {
                leaf_id = right;
                continue;
            }
            return Ok(());
        }
    }

    pub fn compact_leaf_page(&self, page_id: PageId) -> Result<()> {
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
            .into_iter()
            .filter(|entry| entry.physically_live())
            .collect();
        if live.len() == self.read_entries(page_ref)?.len() {
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
        Ok(())
    }

    pub fn prune_committed_deletes_before(
        &self,
        page_id: PageId,
        tx_status: &ConcurrentTxStatus,
        horizon: Csn,
    ) -> Result<()> {
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
            .filter(|entry| !entry.is_committed_deleted_before(tx_status, horizon))
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
        guard.mark_dirty(crate::format::Lsn(1))?;
        Ok(())
    }
}
