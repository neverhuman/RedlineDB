use crate::format::{PAGE_HEADER_LEN, PageGeneration, PageId, PageKind, SLOT_LEN, TxId};
use crate::{Error, Result};

use super::cells::Entry;
use super::{
    BtreeIndex, INDEX_SPECIAL_LEN, IndexRowRef, IndexValidationReport, PAGE_INTERNAL_KIND,
    PAGE_LEAF_KIND, PageHeader,
};

impl BtreeIndex {
    pub fn validate(&self) -> Result<IndexValidationReport> {
        let meta = self.meta()?;
        let mut report = IndexValidationReport {
            pages_seen: 0,
            leaf_pages: 0,
            internal_pages: 0,
            errors: Vec::new(),
        };
        self.validate_page(meta.root_page_id, meta.root_level, &mut report)?;
        Ok(report)
    }

    pub(super) fn split_leaf_and_insert(
        &self,
        ancestors: &[PageId],
        leaf_id: PageId,
        logical_key: &[u8],
        row: IndexRowRef,
        physical: Vec<u8>,
        tx_id: crate::format::TxId,
    ) -> Result<()> {
        // Lane E failpoint: armed at the start of leaf split, before any
        // structural change is applied. Crashing here exercises recovery from
        // a half-applied split (no new pages allocated yet on disk).
        crate::fail_point!("index::split");
        let rel_id = self.descriptor().rel_id;
        let index_id = self.descriptor().index_id;
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
                    first_right
                        .physical()
                        .map(|p| p.to_vec())
                        .unwrap_or_default()
                }
                _ => right_entries
                    .first()
                    .and_then(|entry| entry.logical_key())
                    .unwrap_or_default()
                    .to_vec(),
            };
            let header = Self::read_page_header(page)?;
            Ok((entries, right_entries, left_high, header))
        })?;
        let right_guard = self.inner.buffer.allocate(PageKind::BtreeLeaf, rel_id)?;
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
        self.record_page_image(leaf_id, tx_id)?;
        self.record_page_image(right_guard.page_id(), tx_id)?;

        self.propagate_split(
            ancestors,
            leaf_id,
            right_guard.page_id(),
            left_high,
            1,
            tx_id,
        )?;
        Ok(())
    }

    fn propagate_split(
        &self,
        ancestors: &[PageId],
        left_page: PageId,
        right_page: PageId,
        separator: Vec<u8>,
        mut left_level: u16,
        tx_id: crate::format::TxId,
    ) -> Result<()> {
        let meta = self.meta()?;
        let mut ancestors = ancestors.to_vec();
        let mut current_left = left_page;
        let mut current_right = right_page;
        let mut current_separator = separator;

        loop {
            if let Some(parent_id) = ancestors.pop() {
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
                    self.record_page_image(parent_id, tx_id)?;
                    return Ok(());
                }

                let split = entries.len() / 2;
                let right_entries = entries.split_off(split);
                let left_entries = entries;
                let right_guard = self
                    .inner
                    .buffer
                    .allocate(PageKind::BtreeInternal, self.descriptor().rel_id)?;
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
                self.record_page_image(parent_id, tx_id)?;
                self.record_page_image(right_guard.page_id(), tx_id)?;
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
            self.record_page_image(root_guard.page_id(), tx_id)?;
            self.set_meta_root(root_guard.page_id(), left_level, tx_id)?;
            return Ok(());
        }
    }

    fn validate_page(
        &self,
        page_id: PageId,
        expected_level: u16,
        report: &mut IndexValidationReport,
    ) -> Result<()> {
        let guard = self.inner.buffer.pin(page_id)?;
        let (header, entries) = guard.with_page(|page| {
            let header = Self::read_page_header(page)?;
            let entries = self.read_entries(page)?;
            Ok((header, entries))
        })?;
        report.pages_seen += 1;
        match header.kind {
            PAGE_LEAF_KIND => report.leaf_pages += 1,
            PAGE_INTERNAL_KIND => report.internal_pages += 1,
            _ => report.errors.push("unexpected page kind"),
        }
        if header.level != expected_level {
            report.errors.push("level mismatch");
        }
        if header.index_id != self.descriptor().index_id {
            report.errors.push("index id mismatch");
        }
        if header.kind == PAGE_INTERNAL_KIND && header.left.is_none() {
            report.errors.push("internal page missing leftmost child");
        }
        let mut last: Option<Vec<u8>> = None;
        let mut children = Vec::new();
        let entry_count = entries.len();
        for entry in entries {
            match entry {
                Entry::Leaf { physical, .. } => {
                    if let Some(prev) = &last
                        && prev.as_slice() > physical.as_slice()
                    {
                        report.errors.push("leaf entries out of order");
                    }
                    last = Some(physical);
                }
                Entry::Internal { child, .. } => children.push(child),
            }
        }
        if header.kind == PAGE_INTERNAL_KIND {
            if let Some(left) = header.left {
                children.insert(0, left);
            }
            if children.len() != entry_count + 1 {
                report.errors.push("internal child count mismatch");
            }
        }
        drop(guard);
        for child in children {
            self.validate_page(child, expected_level.saturating_sub(1), report)?;
        }
        Ok(())
    }

    /// Choose a split position for a leaf whose entries are sorted by
    /// `physical` (logical_key + row_ref suffix). Prefers a position near the
    /// midpoint where the boundary lies between two distinct logical keys —
    /// that keeps every duplicate of a given key on a single leaf, which is
    /// the invariant `point_lookup`'s right-walk relies on. When the entire
    /// page is one duplicate run the midpoint is returned and the caller
    /// adopts a physical-key separator instead (see `split_leaf_and_insert`).
    pub(super) fn choose_leaf_split(entries: &[Entry]) -> usize {
        let n = entries.len();
        if n <= 1 {
            return n;
        }
        let mid = n / 2;
        // Search outward from the midpoint for a position `i` where
        // entries[i-1].logical_key != entries[i].logical_key.
        for offset in 0..n {
            for &candidate in &[mid.saturating_add(offset), mid.saturating_sub(offset)] {
                if candidate == 0 || candidate >= n {
                    continue;
                }
                let prev = entries[candidate - 1].logical_key();
                let next = entries[candidate].logical_key();
                if prev != next {
                    return candidate;
                }
            }
        }
        // Whole page is one duplicate run; fall back to the midpoint and rely
        // on the physical-key separator path in the caller.
        mid
    }

    pub(super) fn encoded_entries_len(entries: &[Entry]) -> usize {
        // Leaf cells are encoded as 2+2+8+8+2+4 = 26 bytes of header plus the
        // logical_key + physical payload. Internal cells are 2+8 = 10 bytes of
        // header plus the separator. The pre-split size estimator used to say
        // `18` for leaves which underestimated by 8 bytes per entry; with many
        // entries that meant the post-split halves no longer fit, surfacing
        // as `Error::PageFull` ("no free slot space on page").
        entries
            .iter()
            .map(|entry| match entry {
                Entry::Leaf {
                    logical_key,
                    physical,
                    ..
                } => 42 + logical_key.len() + physical.len(),
                Entry::Internal { separator, .. } => 10 + separator.len(),
            })
            .sum()
    }

    pub(super) fn page_body_capacity(&self, page_id: PageId) -> Result<usize> {
        let guard = self.inner.buffer.pin(page_id)?;
        guard.with_page(|page| {
            Ok(page
                .as_bytes()
                .len()
                .saturating_sub(PAGE_HEADER_LEN + INDEX_SPECIAL_LEN))
        })
    }

    pub(super) fn min_key_for_page(&self, page_id: PageId) -> Result<Vec<u8>> {
        let guard = self.inner.buffer.pin(page_id)?;
        let (header, entries) = guard.with_page(|page| {
            let header = Self::read_page_header(page)?;
            let entries = self.read_entries(page)?;
            Ok((header, entries))
        })?;
        if header.kind == PAGE_INTERNAL_KIND {
            let child = header
                .left
                .ok_or(Error::CorruptPage("internal page missing leftmost child"))?;
            return self.min_key_for_page(child);
        }
        // Propagate the LEAF's first key. Use the logical bytes when the
        // entire subtree shares a single logical key — that's the
        // duplicate-run case where we need an unambiguous separator that
        // survives propagation; the physical bytes (logical || row_ref)
        // resolve ties strictly. Otherwise the logical key suffices and is
        // shorter, keeping internal pages compact.
        let first_logical = entries.iter().find_map(|e| e.logical_key());
        let last_logical = entries.iter().rev().find_map(|e| e.logical_key());
        match (first_logical, last_logical) {
            (Some(first), Some(last)) if first == last => {
                // All entries on this leaf share one logical key. Use the
                // first entry's physical bytes so the separator is strictly
                // greater than every key on the left sibling but still sorts
                // before any key with a greater logical value.
                let first_physical = entries.iter().find_map(|e| e.physical());
                Ok(first_physical
                    .map(|p| p.to_vec())
                    .unwrap_or_else(|| first.to_vec()))
            }
            (Some(first), _) => Ok(first.to_vec()),
            _ => Ok(Vec::new()),
        }
    }
}
