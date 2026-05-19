use crate::Result;

use super::super::super::CursorYield;
use super::super::shared::{self, BatchKind};
use super::RawIndexCursor;

impl<'idx> RawIndexCursor<'idx> {
    pub fn next_count_batch(&mut self, max_batch: usize) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let mut pushed = 0_usize;
        let mut visibility_cache = Vec::new();
        let start = self.start.clone();
        let end = self.end.clone();
        let view = self.view;
        loop {
            if pushed > 0 && pushed >= max_batch {
                return shared::finish_batch(self.counters, BatchKind::Range, pushed);
            }
            if self.current_leaf.is_none() {
                self.exhausted = true;
                break;
            }
            let stop_at_end_bound = self.scan_current_leaf_entries(|entry| {
                if pushed > 0 && pushed >= max_batch {
                    return Ok(false);
                }
                if shared::matches_ref_cached(view, &entry, &mut visibility_cache)
                    && super::bound_in_range(&start, &end, entry.logical_key)
                {
                    pushed += 1;
                }
                Ok(true)
            });
            let stop_at_end_bound = stop_at_end_bound?;
            if pushed > 0 && pushed >= max_batch {
                return shared::finish_batch(self.counters, BatchKind::Range, pushed);
            }
            if stop_at_end_bound || self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            shared::finish_batch(self.counters, BatchKind::Range, pushed)
        }
    }

    pub fn count_remaining(&mut self) -> Result<usize> {
        if self.exhausted {
            return Ok(0);
        }
        let mut count = 0_usize;
        let mut visibility_cache = Vec::new();
        let start = self.start.clone();
        let end = self.end.clone();
        let view = self.view;
        loop {
            if self.current_leaf.is_none() {
                self.exhausted = true;
                break;
            }
            let stop_at_end_bound = self.scan_current_leaf_entries(|entry| {
                if super::bound_lower_allows(&start, entry.logical_key)
                    && !super::bound_at_or_past_end(&end, entry.logical_key)
                    && shared::matches_ref_cached(view, &entry, &mut visibility_cache)
                {
                    count = count.saturating_add(1);
                }
                Ok(true)
            });
            let stop_at_end_bound = stop_at_end_bound?;
            if stop_at_end_bound {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.current_leaf = Some(next_id);
                    self.entry_idx = 0;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        if count > 0 {
            shared::bump_batch_counters(self.counters, BatchKind::Range);
        }
        Ok(count)
    }
}
