use crate::format::{PAGE_HEADER_LEN, PageId};
use crate::{Error, Result};

use super::cells::Entry;
use super::{
    BtreeIndex, INDEX_SPECIAL_LEN, IndexValidationReport, PAGE_INTERNAL_KIND, PAGE_LEAF_KIND,
};
#[path = "maintenance/split.rs"]
mod split;

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
                Ok(match first_physical {
                    Some(physical) => physical.to_vec(),
                    None => first.to_vec(),
                })
            }
            (Some(first), _) => Ok(first.to_vec()),
            _ => Ok(Vec::new()),
        }
    }
}
