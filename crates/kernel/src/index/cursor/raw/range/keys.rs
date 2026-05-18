use std::sync::atomic::Ordering as AtomicOrdering;

use crate::Result;

use super::super::super::super::IndexRowRef;
use super::super::super::CursorYield;
use super::super::shared::{self, BatchKind};
use super::RawIndexCursor;

impl<'idx> RawIndexCursor<'idx> {
    /// Phase 11 W1-E variant of [`next_batch`] that also yields the
    /// encoded `logical_key` bytes alongside each row. Covering-scan
    /// callers (SQL `SELECT k, v FROM t WHERE k BETWEEN ? AND ?`
    /// against an index covering `k, v`) read columns straight off
    /// these bytes without ever hitting the heap.
    ///
    /// The `Vec<u8>` carries the same encoded shape that
    /// `encode_index_key` produced: per-part type tag + body + `0xff`
    /// terminator, with `Desc` parts bit-inverted in place. Decoding
    /// is the caller's job (the SQL layer keeps the part-shape
    /// awareness it already needs for predicate matching).
    ///
    /// Telemetry: bumps `Phase11Counters::cursor_batches_emitted`
    /// exactly like [`next_batch`].
    pub fn next_batch_with_keys(
        &mut self,
        out: &mut Vec<(Vec<u8>, IndexRowRef)>,
        max_batch: usize,
    ) -> Result<CursorYield> {
        if self.exhausted {
            return Ok(CursorYield::End);
        }
        let start_len = out.len();
        let target = start_len.saturating_add(max_batch);
        loop {
            while self.entry_idx < self.entries.len() {
                if out.len() >= target {
                    let pushed = out.len() - start_len;
                    if let Some(c) = self.counters {
                        c.cursor_batches_emitted
                            .fetch_add(1, AtomicOrdering::Relaxed);
                    }
                    return Ok(CursorYield::Batch(pushed));
                }
                let entry = &self.entries[self.entry_idx];
                self.entry_idx += 1;
                if self.view.matches(entry) && self.in_range(&entry.logical_key) {
                    out.push((entry.logical_key.clone(), entry.row));
                }
            }
            if self.leaf_chain_past_end() {
                self.exhausted = true;
                break;
            }
            match self.next_leaf {
                Some(next_id) => {
                    self.advance_to(next_id)?;
                }
                None => {
                    self.exhausted = true;
                    break;
                }
            }
        }
        let pushed = out.len() - start_len;
        if pushed == 0 {
            Ok(CursorYield::End)
        } else {
            shared::finish_batch(self.counters, BatchKind::Range, pushed)
        }
    }
}
