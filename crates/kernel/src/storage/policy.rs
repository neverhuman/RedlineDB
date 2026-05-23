#![allow(dead_code)]

use crate::format::{Lsn, PageId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameMeta {
    pub pin_count: usize,
    pub dirty: bool,
    pub usage_count: u8,
    pub page_lsn: Lsn,
    pub durable_lsn: Lsn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DirtyFrameMeta {
    pub page_id: PageId,
    pub page_lsn: Lsn,
}

pub(crate) trait BufferPolicy {
    fn victim_score(meta: FrameMeta) -> Option<u32>;
    fn dirty_batch_pages(resident_pages: usize, dirty_pages: usize) -> usize;
    fn sort_dirty_frames(frames: &mut [DirtyFrameMeta]);
    fn prefetch_cold_load(resident_pages: usize, capacity: usize) -> bool;
}

pub(crate) type ActiveBufferPolicy = BufferClockPolicy;

pub(crate) struct BufferClockPolicy;

impl BufferPolicy for BufferClockPolicy {
    fn victim_score(meta: FrameMeta) -> Option<u32> {
        if meta.pin_count > 0
            || meta.usage_count > 0
            || (meta.dirty && meta.page_lsn > meta.durable_lsn)
        {
            return None;
        }
        Some(if meta.dirty { 10 } else { 0 })
    }

    fn dirty_batch_pages(_resident_pages: usize, dirty_pages: usize) -> usize {
        dirty_pages
    }

    fn sort_dirty_frames(_frames: &mut [DirtyFrameMeta]) {}

    fn prefetch_cold_load(_resident_pages: usize, _capacity: usize) -> bool {
        true
    }
}

pub(crate) struct BufferCleanFirstPolicy;

impl BufferPolicy for BufferCleanFirstPolicy {
    fn victim_score(meta: FrameMeta) -> Option<u32> {
        if meta.pin_count > 0
            || meta.usage_count > 0
            || (meta.dirty && meta.page_lsn > meta.durable_lsn)
        {
            return None;
        }
        Some(if meta.dirty { 100 } else { 0 })
    }

    fn dirty_batch_pages(resident_pages: usize, dirty_pages: usize) -> usize {
        BufferClockPolicy::dirty_batch_pages(resident_pages, dirty_pages)
    }

    fn sort_dirty_frames(frames: &mut [DirtyFrameMeta]) {
        BufferClockPolicy::sort_dirty_frames(frames)
    }

    fn prefetch_cold_load(resident_pages: usize, capacity: usize) -> bool {
        resident_pages.saturating_mul(10) < capacity.saturating_mul(9)
    }
}

pub(crate) struct BufferCheckpointThroughputPolicy;

impl BufferPolicy for BufferCheckpointThroughputPolicy {
    fn victim_score(meta: FrameMeta) -> Option<u32> {
        BufferClockPolicy::victim_score(meta)
    }

    fn dirty_batch_pages(_resident_pages: usize, dirty_pages: usize) -> usize {
        dirty_pages.min(256)
    }

    fn sort_dirty_frames(frames: &mut [DirtyFrameMeta]) {
        frames.sort_by_key(|frame| (frame.page_lsn, frame.page_id));
    }

    fn prefetch_cold_load(resident_pages: usize, capacity: usize) -> bool {
        resident_pages.saturating_mul(4) < capacity.saturating_mul(3)
    }
}

pub(crate) struct BufferHotReadPolicy;

impl BufferPolicy for BufferHotReadPolicy {
    fn victim_score(meta: FrameMeta) -> Option<u32> {
        let base = BufferClockPolicy::victim_score(meta)?;
        Some(base.saturating_add(u32::from(meta.usage_count)))
    }

    fn dirty_batch_pages(resident_pages: usize, dirty_pages: usize) -> usize {
        dirty_pages.min((resident_pages / 2).max(1))
    }

    fn sort_dirty_frames(frames: &mut [DirtyFrameMeta]) {
        frames.sort_by_key(|frame| frame.page_id);
    }

    fn prefetch_cold_load(resident_pages: usize, capacity: usize) -> bool {
        resident_pages.saturating_mul(5) < capacity.saturating_mul(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_policy<P: BufferPolicy>() {
        let clean = FrameMeta {
            pin_count: 0,
            dirty: false,
            usage_count: 0,
            page_lsn: Lsn(1),
            durable_lsn: Lsn(1),
        };
        let pinned = FrameMeta {
            pin_count: 1,
            ..clean
        };
        assert!(P::victim_score(clean).is_some());
        assert!(P::victim_score(pinned).is_none());
        assert!(P::dirty_batch_pages(128, 64) > 0);

        let mut dirty = [
            DirtyFrameMeta {
                page_id: PageId(2),
                page_lsn: Lsn(2),
            },
            DirtyFrameMeta {
                page_id: PageId(1),
                page_lsn: Lsn(1),
            },
        ];
        P::sort_dirty_frames(&mut dirty);
        let _ = P::prefetch_cold_load(1, 2);
    }

    #[test]
    fn buffer_policy_drop_ins_preserve_basic_invariants() {
        audit_policy::<BufferClockPolicy>();
        audit_policy::<BufferCleanFirstPolicy>();
        audit_policy::<BufferCheckpointThroughputPolicy>();
        audit_policy::<BufferHotReadPolicy>();
    }
}
