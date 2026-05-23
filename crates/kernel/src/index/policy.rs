#![allow(dead_code)]

use std::ops::Bound;

use super::cells::Entry;

pub(super) trait IndexCursorPolicy {
    const VEC_WRAPPER_BATCH: usize;
    const RAW_CURSOR_BATCH: usize;

    fn prefetch_right_sibling(entries_in_leaf: usize, has_right: bool) -> bool;
    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool;
}

pub(super) enum DuplicateSplitMode {
    KeepRunTogether,
    PhysicalSeparator,
}

pub(super) trait LeafSplitPolicy {
    fn split_point(entries: &[Entry], body_capacity: usize) -> usize;
    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode;
}

pub(super) type ActiveIndexCursorPolicy = IndexCurrentPolicy;
pub(super) type ActiveLeafSplitPolicy = IndexCurrentPolicy;

pub(super) struct IndexCurrentPolicy;

impl IndexCurrentPolicy {
    fn midpoint_distinct_boundary(entries: &[Entry]) -> usize {
        let n = entries.len();
        if n <= 1 {
            return n;
        }
        let mid = n / 2;
        for offset in 0..n {
            for &candidate in &[mid.saturating_add(offset), mid.saturating_sub(offset)] {
                if candidate == 0 || candidate >= n {
                    continue;
                }
                if entries[candidate - 1].logical_key() != entries[candidate].logical_key() {
                    return candidate;
                }
            }
        }
        mid
    }
}

impl IndexCursorPolicy for IndexCurrentPolicy {
    const VEC_WRAPPER_BATCH: usize = 256;
    const RAW_CURSOR_BATCH: usize = 256;

    fn prefetch_right_sibling(_entries_in_leaf: usize, has_right: bool) -> bool {
        has_right
    }

    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool {
        let Some(last) = last_logical_key else {
            return false;
        };
        match end {
            Bound::Excluded(b) => last >= b.as_slice(),
            Bound::Included(b) => last > b.as_slice(),
            Bound::Unbounded => false,
        }
    }
}

impl LeafSplitPolicy for IndexCurrentPolicy {
    fn split_point(entries: &[Entry], _body_capacity: usize) -> usize {
        Self::midpoint_distinct_boundary(entries)
    }

    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode {
        if split == 0 || split >= entries.len() {
            return DuplicateSplitMode::KeepRunTogether;
        }
        if entries[split - 1].logical_key() == entries[split].logical_key() {
            DuplicateSplitMode::PhysicalSeparator
        } else {
            DuplicateSplitMode::KeepRunTogether
        }
    }
}

pub(super) struct IndexLargeBatchPolicy;

impl IndexCursorPolicy for IndexLargeBatchPolicy {
    const VEC_WRAPPER_BATCH: usize = 1024;
    const RAW_CURSOR_BATCH: usize = 1024;

    fn prefetch_right_sibling(entries_in_leaf: usize, has_right: bool) -> bool {
        has_right && entries_in_leaf >= 32
    }

    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool {
        IndexCurrentPolicy::stop_after_leaf(last_logical_key, end)
    }
}

impl LeafSplitPolicy for IndexLargeBatchPolicy {
    fn split_point(entries: &[Entry], body_capacity: usize) -> usize {
        IndexCurrentPolicy::split_point(entries, body_capacity)
    }

    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode {
        IndexCurrentPolicy::duplicate_mode(entries, split)
    }
}

pub(super) struct IndexDuplicateHeavyPolicy;

impl IndexCursorPolicy for IndexDuplicateHeavyPolicy {
    const VEC_WRAPPER_BATCH: usize = 256;
    const RAW_CURSOR_BATCH: usize = 256;

    fn prefetch_right_sibling(entries_in_leaf: usize, has_right: bool) -> bool {
        IndexCurrentPolicy::prefetch_right_sibling(entries_in_leaf, has_right)
    }

    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool {
        IndexCurrentPolicy::stop_after_leaf(last_logical_key, end)
    }
}

impl LeafSplitPolicy for IndexDuplicateHeavyPolicy {
    fn split_point(entries: &[Entry], _body_capacity: usize) -> usize {
        let n = entries.len();
        if n <= 1 {
            return n;
        }
        let mid = n / 2;
        for candidate in mid..n {
            if candidate > 0
                && entries[candidate - 1].logical_key() != entries[candidate].logical_key()
            {
                return candidate;
            }
        }
        for candidate in (1..mid).rev() {
            if entries[candidate - 1].logical_key() != entries[candidate].logical_key() {
                return candidate;
            }
        }
        mid
    }

    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode {
        IndexCurrentPolicy::duplicate_mode(entries, split)
    }
}

pub(super) struct IndexLowLatencyPolicy;

impl IndexCursorPolicy for IndexLowLatencyPolicy {
    const VEC_WRAPPER_BATCH: usize = 128;
    const RAW_CURSOR_BATCH: usize = 128;

    fn prefetch_right_sibling(entries_in_leaf: usize, has_right: bool) -> bool {
        has_right && entries_in_leaf >= 8
    }

    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool {
        IndexCurrentPolicy::stop_after_leaf(last_logical_key, end)
    }
}

impl LeafSplitPolicy for IndexLowLatencyPolicy {
    fn split_point(entries: &[Entry], _body_capacity: usize) -> usize {
        let n = entries.len();
        if n <= 1 {
            return n;
        }
        let target = (n * 3 / 5).clamp(1, n - 1);
        for offset in 0..n {
            for &candidate in &[target.saturating_add(offset), target.saturating_sub(offset)] {
                if candidate == 0 || candidate >= n {
                    continue;
                }
                if entries[candidate - 1].logical_key() != entries[candidate].logical_key() {
                    return candidate;
                }
            }
        }
        target
    }

    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode {
        IndexCurrentPolicy::duplicate_mode(entries, split)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{PageId, RowId, TuplePtr, TxId};
    use crate::index::IndexRowRef;

    fn leaf(key: &[u8], slot: u16) -> Entry {
        Entry::Leaf {
            logical_key: key.to_vec(),
            row: IndexRowRef {
                row_id: RowId(slot as u64),
                tuple: TuplePtr::new(PageId(1), slot),
            },
            physical: key.to_vec(),
            create_tx: TxId::ZERO,
            delete_tx: TxId::ZERO,
        }
    }

    fn audit_cursor<P: IndexCursorPolicy>() {
        assert!(P::VEC_WRAPPER_BATCH > 0);
        assert!(P::RAW_CURSOR_BATCH > 0);
        assert!(P::prefetch_right_sibling(64, true) || !P::prefetch_right_sibling(0, false));
        assert!(P::stop_after_leaf(
            Some(b"z"),
            &Bound::Excluded(b"m".to_vec())
        ));
        assert!(!P::stop_after_leaf(
            Some(b"a"),
            &Bound::Excluded(b"m".to_vec())
        ));
    }

    fn audit_split<P: LeafSplitPolicy>() {
        let entries = vec![leaf(b"a", 1), leaf(b"b", 2), leaf(b"c", 3), leaf(b"d", 4)];
        let split = P::split_point(&entries, 4096);
        assert!(split > 0);
        assert!(split < entries.len());
        let _ = P::duplicate_mode(&entries, split);
    }

    #[test]
    fn index_policy_drop_ins_preserve_basic_invariants() {
        audit_cursor::<IndexCurrentPolicy>();
        audit_cursor::<IndexLargeBatchPolicy>();
        audit_cursor::<IndexDuplicateHeavyPolicy>();
        audit_cursor::<IndexLowLatencyPolicy>();

        audit_split::<IndexCurrentPolicy>();
        audit_split::<IndexLargeBatchPolicy>();
        audit_split::<IndexDuplicateHeavyPolicy>();
        audit_split::<IndexLowLatencyPolicy>();
    }
}
