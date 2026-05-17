use crate::format::{PageGeneration, PageId, PageKind, SLOT_LEN, TxId};
use crate::{Error, Result};

use super::super::cells::Entry;
use super::super::{BtreeIndex, INDEX_SPECIAL_LEN, IndexRowRef, PAGE_INTERNAL_KIND, PageHeader};

impl BtreeIndex {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::index) fn split_leaf_and_insert(
        &self,
        ancestors: &[PageId],
        leaf_id: PageId,
        logical_key: &[u8],
        row: IndexRowRef,
        physical: Vec<u8>,
        tx_id: crate::format::TxId,
        emit_wal: bool,
        lsn: crate::format::Lsn,
    ) -> Result<()> {
        // Lane E failpoint: armed at the start of leaf split, before any
        // structural change is applied. Crashing here exercises recovery from
        // a half-applied split (no new pages allocated yet on disk).
        crate::fail_point!("index::split");
        self.record_leaf_split();
        let rel_id = self.descriptor().rel_id;
        let index_id = self.descriptor().index_id;
        let leaf_latch = self.inner.latches.get(leaf_id);
        let leaf_write = leaf_latch.write();
        let guard = self.inner.buffer.pin(leaf_id)?;
        let (left_entries, right_entries, left_high, header) = guard.with_page(|page| {
            let mut entries = self.read_entries(page)?;
            entries.push(Entry::Leaf {
                logical_key: logical_key.to_vec(),
                row,
                physical,
                create_tx: tx_id,
                delete_tx: TxId::ZERO,
            });
            entries.sort_by(|a, b| a.compare(b));
            // Pick a split position that does not cleave a duplicate-key run
            // unless every entry on the page shares one logical key. This keeps
            // `point_lookup` correct under the existing right-walk traversal:
            // every duplicate of a given logical_key sits on a single leaf,
            // and the parent separator is the right-half's first key (strictly
            // greater than every key on the left half).
            let split = Self::choose_leaf_split(&entries);
            let right_entries = entries.split_off(split);
            // Most splits land on a clean key boundary (chosen by
            // `choose_leaf_split`) and can use the right half's first logical
            // key as a compact separator. When duplicates of one logical key
            // span both halves (only possible when the page held one logical
            // key throughout), fall back to the right half's first *physical*
            // key (logical_key || row_ref suffix) so the separator is strictly
            // greater than every left-side entry.
            let left_high = match (entries.last(), right_entries.first()) {
                (Some(last_left), Some(first_right))
                    if last_left.logical_key() == first_right.logical_key() =>
                {
                    match first_right.physical().map(|p| p.to_vec()) {
                        Some(vec) => vec,
                        None => Vec::new(),
                    }
                }
                _ => match right_entries.first().and_then(|entry| entry.logical_key()) {
                    Some(key) => key.to_vec(),
                    None => Vec::new(),
                },
            };
            let header = Self::read_page_header(page)?;
            Ok((entries, right_entries, left_high, header))
        })?;
        let right_guard = self.inner.buffer.allocate(PageKind::BtreeLeaf, rel_id)?;
        let right_latch = self.inner.latches.get(right_guard.page_id());
        let right_write = right_latch.write();
        guard.with_page_mut(|page| {
            Self::rewrite_leaf(
                page,
                index_id,
                &left_entries,
                header.left,
                Some(right_guard.page_id()),
                left_high.clone(),
            )
        })?;
        right_guard.with_page_mut(|right_page| {
            right_page.reinitialize_with_special(
                PageKind::BtreeLeaf,
                right_guard.page_id(),
                rel_id,
                PageGeneration::ONE,
                INDEX_SPECIAL_LEN,
            )?;
            Self::rewrite_leaf(
                right_page,
                index_id,
                &right_entries,
                Some(leaf_id),
                header.right,
                header.high_key.clone(),
            )
        })?;
        if emit_wal {
            self.record_page_image(leaf_id, tx_id)?;
            self.record_page_image(right_guard.page_id(), tx_id)?;
        } else {
            guard.mark_dirty(lsn)?;
            right_guard.mark_dirty(lsn)?;
        }

        self.propagate_split(
            ancestors,
            leaf_id,
            right_guard.page_id(),
            left_high,
            1,
            tx_id,
            emit_wal,
            lsn,
        )?;
        drop(right_write);
        drop(leaf_write);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn propagate_split(
        &self,
        ancestors: &[PageId],
        left_page: PageId,
        right_page: PageId,
        separator: Vec<u8>,
        mut left_level: u16,
        tx_id: crate::format::TxId,
        emit_wal: bool,
        lsn: crate::format::Lsn,
    ) -> Result<()> {
        let meta = self.meta()?;
        let mut ancestors = ancestors.to_vec();
        let mut current_left = left_page;
        let mut current_right = right_page;
        let mut current_separator = separator;

        loop {
            if let Some(parent_id) = ancestors.pop() {
                let parent_latch = self.inner.latches.get(parent_id);
                let parent_write = parent_latch.write();
                let guard = self.inner.buffer.pin(parent_id)?;
                let (header, mut entries) = guard.with_page(|page| {
                    let header = Self::read_page_header(page)?;
                    let entries = self.read_entries(page)?;
                    Ok((header, entries))
                })?;
                if header.kind != PAGE_INTERNAL_KIND {
                    return Err(Error::CorruptPage("expected internal page"));
                }
                let parent_level = header.level;
                entries.push(Entry::Internal {
                    separator: current_separator.clone(),
                    child: current_right,
                });
                entries.sort_by(|a, b| a.compare(b));
                let body_capacity = self.page_body_capacity(parent_id)?;
                let required = Self::encoded_entries_len(&entries) + entries.len() * SLOT_LEN;
                if required <= body_capacity {
                    guard.with_page_mut(|page| {
                        Self::rewrite_internal(
                            page,
                            meta.index_id,
                            header.level,
                            &entries,
                            header.left,
                            header.right,
                            header.high_key.clone(),
                        )
                    })?;
                    if emit_wal {
                        self.record_page_image(parent_id, tx_id)?;
                    } else {
                        guard.mark_dirty(lsn)?;
                    }
                    drop(parent_write);
                    return Ok(());
                }

                let split = entries.len() / 2;
                let right_entries = entries.split_off(split);
                let left_entries = entries;
                let right_guard = self
                    .inner
                    .buffer
                    .allocate(PageKind::BtreeInternal, self.descriptor().rel_id)?;
                let right_latch = self.inner.latches.get(right_guard.page_id());
                let right_write = right_latch.write();
                let right_left_child = match right_entries.first() {
                    Some(Entry::Internal { child, .. }) => *child,
                    _ => {
                        return Err(Error::CorruptPage(
                            "internal split produced empty right side",
                        ));
                    }
                };
                let right_separator = self.min_key_for_page(right_left_child)?;
                guard.with_page_mut(|page| {
                    Self::rewrite_internal(
                        page,
                        meta.index_id,
                        parent_level,
                        &left_entries,
                        header.left,
                        Some(right_guard.page_id()),
                        right_separator.clone(),
                    )
                })?;
                right_guard.with_page_mut(|page| {
                    page.reinitialize_with_special(
                        PageKind::BtreeInternal,
                        right_guard.page_id(),
                        self.descriptor().rel_id,
                        PageGeneration::ONE,
                        INDEX_SPECIAL_LEN,
                    )?;
                    Self::rewrite_internal(
                        page,
                        meta.index_id,
                        parent_level,
                        &right_entries,
                        Some(right_left_child),
                        header.right,
                        header.high_key.clone(),
                    )
                })?;
                if emit_wal {
                    self.record_page_image(parent_id, tx_id)?;
                    self.record_page_image(right_guard.page_id(), tx_id)?;
                } else {
                    guard.mark_dirty(lsn)?;
                    right_guard.mark_dirty(lsn)?;
                }
                drop(right_write);
                drop(parent_write);
                current_left = parent_id;
                current_right = right_guard.page_id();
                current_separator = right_separator;
                left_level = parent_level.saturating_add(1);
                continue;
            }

            let rel_id = self.descriptor().rel_id;
            let root_guard = self
                .inner
                .buffer
                .allocate(PageKind::BtreeInternal, rel_id)?;
            let root_latch = self.inner.latches.get(root_guard.page_id());
            let root_write = root_latch.write();
            root_guard.with_page_mut(|page| {
                page.reinitialize_with_special(
                    PageKind::BtreeInternal,
                    root_guard.page_id(),
                    rel_id,
                    PageGeneration::ONE,
                    INDEX_SPECIAL_LEN,
                )?;
                Self::write_page_header(
                    page,
                    &PageHeader {
                        kind: PAGE_INTERNAL_KIND,
                        level: left_level,
                        index_id: meta.index_id,
                        left: Some(current_left),
                        right: None,
                        high_key: Vec::new(),
                    },
                )?;
                let entries = vec![Entry::Internal {
                    separator: current_separator.clone(),
                    child: current_right,
                }];
                Self::rewrite_internal(
                    page,
                    meta.index_id,
                    left_level,
                    &entries,
                    Some(current_left),
                    None,
                    Vec::new(),
                )
            })?;
            if emit_wal {
                self.record_page_image(root_guard.page_id(), tx_id)?;
                self.set_meta_root(root_guard.page_id(), left_level, tx_id, true, lsn)?;
            } else {
                root_guard.mark_dirty(lsn)?;
                self.set_meta_root(root_guard.page_id(), left_level, tx_id, false, lsn)?;
            }
            drop(root_write);
            return Ok(());
        }
    }
}
