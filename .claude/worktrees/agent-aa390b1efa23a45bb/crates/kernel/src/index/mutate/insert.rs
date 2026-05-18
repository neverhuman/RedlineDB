use crate::format::{Lsn, TxId};
use crate::wal::WalPayload;
use crate::{Error, Result};

use super::super::cells::Entry;
use super::super::{
    BtreeIndex, INDEX_SPECIAL_LEN, IndexRowRef, IndexUniqueness, KeyBuf, PAGE_LEAF_KIND,
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
            // Defensive: if the leaf still claims our key belongs to a sibling
            // (concurrent split window), retry the descent.
            if !header.high_key.is_empty() && physical.as_slice() >= header.high_key.as_slice() {
                drop(page);
                drop(leaf_write);
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
                .saturating_sub(crate::format::PAGE_HEADER_LEN + INDEX_SPECIAL_LEN);
            let required =
                Self::encoded_entries_len(&entries) + entries.len() * crate::format::SLOT_LEN;
            if required > body_capacity {
                drop(page);
                drop(guard);
                drop(leaf_write);
                let _structure = self.lock_structure()?;
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
            drop(leaf_write);
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
}
