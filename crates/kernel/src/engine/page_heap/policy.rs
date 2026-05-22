#![allow(dead_code)]

use crate::format::{PageKind, RelId, RowId, UndoPtr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReuseDecision {
    PreferReusable,
    AllocateFresh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct UndoReadContext {
    pub depth: usize,
    pub ptr: UndoPtr,
}

pub(super) trait HeapPlacementPolicy {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize;
    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize;
    fn reusable_page(kind: PageKind, encoded_len: usize, queued_pages: usize) -> ReuseDecision;
}

pub(super) trait UndoReadPolicy {
    fn prefetch_next(ctx: UndoReadContext) -> bool;
    fn depth_limit_hint(ctx: UndoReadContext) -> Option<usize>;
}

pub(super) type ActiveHeapPlacementPolicy = HeapModuloPolicy;
pub(super) type ActiveUndoReadPolicy = HeapModuloPolicy;

pub(super) struct HeapModuloPolicy;

impl HeapPlacementPolicy for HeapModuloPolicy {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize {
        row_id.0 as usize % lane_count.max(1)
    }

    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize {
        rel_id.0 as usize % lane_count.max(1)
    }

    fn reusable_page(_kind: PageKind, _encoded_len: usize, queued_pages: usize) -> ReuseDecision {
        if queued_pages > 0 {
            ReuseDecision::PreferReusable
        } else {
            ReuseDecision::AllocateFresh
        }
    }
}

impl UndoReadPolicy for HeapModuloPolicy {
    fn prefetch_next(_ctx: UndoReadContext) -> bool {
        false
    }

    fn depth_limit_hint(_ctx: UndoReadContext) -> Option<usize> {
        None
    }
}

pub(super) struct HeapHashStripePolicy;

impl HeapPlacementPolicy for HeapHashStripePolicy {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize {
        mixed_lane(row_id.0, lane_count)
    }

    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize {
        mixed_lane(rel_id.0, lane_count)
    }

    fn reusable_page(kind: PageKind, encoded_len: usize, queued_pages: usize) -> ReuseDecision {
        HeapModuloPolicy::reusable_page(kind, encoded_len, queued_pages)
    }
}

impl UndoReadPolicy for HeapHashStripePolicy {
    fn prefetch_next(ctx: UndoReadContext) -> bool {
        HeapModuloPolicy::prefetch_next(ctx)
    }

    fn depth_limit_hint(ctx: UndoReadContext) -> Option<usize> {
        HeapModuloPolicy::depth_limit_hint(ctx)
    }
}

pub(super) struct HeapReuseConservativePolicy;

impl HeapPlacementPolicy for HeapReuseConservativePolicy {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize {
        HeapModuloPolicy::row_lane(row_id, lane_count)
    }

    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize {
        HeapModuloPolicy::relation_lane(rel_id, lane_count)
    }

    fn reusable_page(_kind: PageKind, encoded_len: usize, queued_pages: usize) -> ReuseDecision {
        if queued_pages > 0 && encoded_len <= 2048 {
            ReuseDecision::PreferReusable
        } else {
            ReuseDecision::AllocateFresh
        }
    }
}

impl UndoReadPolicy for HeapReuseConservativePolicy {
    fn prefetch_next(ctx: UndoReadContext) -> bool {
        HeapModuloPolicy::prefetch_next(ctx)
    }

    fn depth_limit_hint(ctx: UndoReadContext) -> Option<usize> {
        HeapModuloPolicy::depth_limit_hint(ctx)
    }
}

pub(super) struct HeapUndoPrefetchPolicy;

impl HeapPlacementPolicy for HeapUndoPrefetchPolicy {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize {
        HeapModuloPolicy::row_lane(row_id, lane_count)
    }

    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize {
        HeapModuloPolicy::relation_lane(rel_id, lane_count)
    }

    fn reusable_page(kind: PageKind, encoded_len: usize, queued_pages: usize) -> ReuseDecision {
        HeapModuloPolicy::reusable_page(kind, encoded_len, queued_pages)
    }
}

impl UndoReadPolicy for HeapUndoPrefetchPolicy {
    fn prefetch_next(ctx: UndoReadContext) -> bool {
        ctx.depth < 32 && ctx.ptr != UndoPtr::ZERO
    }

    fn depth_limit_hint(_ctx: UndoReadContext) -> Option<usize> {
        Some(4096)
    }
}

fn mixed_lane(value: u64, lane_count: usize) -> usize {
    let lane_count = lane_count.max(1);
    let mut x = value;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    x as usize % lane_count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_policy<P: HeapPlacementPolicy + UndoReadPolicy>() {
        assert!(P::row_lane(RowId(9), 4) < 4);
        assert!(P::relation_lane(RelId(11), 4) < 4);
        assert_eq!(
            P::reusable_page(PageKind::Heap, 128, 0),
            ReuseDecision::AllocateFresh
        );
        let _ = P::reusable_page(PageKind::Heap, 128, 2);
        let ctx = UndoReadContext {
            depth: 1,
            ptr: UndoPtr(1),
        };
        let _ = P::prefetch_next(ctx);
        let _ = P::depth_limit_hint(ctx);
    }

    #[test]
    fn heap_policy_drop_ins_preserve_basic_invariants() {
        audit_policy::<HeapModuloPolicy>();
        audit_policy::<HeapHashStripePolicy>();
        audit_policy::<HeapReuseConservativePolicy>();
        audit_policy::<HeapUndoPrefetchPolicy>();
    }
}
