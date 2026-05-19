use crate::Result;

use super::super::super::super::IndexRowRef;
use super::super::super::CursorYield;
use super::super::shared::{self, BatchKind};
use super::RawIndexCursor;

impl<'idx> RawIndexCursor<'idx> {
    /// Yield rowids in raw leaf order without cloning logical keys.
    /// This is used by SQL ordered LIMIT scans that still need one
    /// heap load for projection but do not need key bytes in the
    /// executor.
    pub fn next_rowid_batch(
        &mut self,
        out: &mut Vec<IndexRowRef>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        let mut visibility_cache = Vec::new();
        let start = self.start.clone();
        let view = self.view;
        loop {
            if out.len() >= target {
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Range,
                    out.len().saturating_sub(start_len),
                );
            }
            if self.current_leaf.is_none() {
                self.exhausted = true;
                break;
            }
            let stop_at_end_bound = self.scan_current_leaf_entries(|entry| {
                if out.len() >= target {
                    return Ok(false);
                }
                if super::bound_lower_allows(&start, entry.logical_key)
                    && shared::matches_ref_cached(view, &entry, &mut visibility_cache)
                {
                    out.push(entry.row);
                }
                Ok(true)
            });
            let stop_at_end_bound = stop_at_end_bound?;
            if out.len() >= target {
                return shared::finish_batch(
                    self.counters,
                    BatchKind::Range,
                    out.len().saturating_sub(start_len),
                );
            }
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
        let pushed = out.len().saturating_sub(start_len);
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            shared::finish_batch(self.counters, BatchKind::Range, pushed)
        }
    }

    pub fn close(self) {
        shared::close_cursor(self);
    }
}
