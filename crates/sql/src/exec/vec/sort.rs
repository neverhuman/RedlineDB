//! Lane VE: spillable external merge-sort.
//!
//! Strategy:
//!   - Buffer rows in memory until the broker says "no more"; sort and
//!     either yield (in-memory path) or flush to a sorted *run* file.
//!   - When all input is consumed, k-way merge any spilled runs (plus the
//!     leftover in-memory buffer) into a single sorted stream.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::PathBuf;

use super::spill::{SpillFile, SpillReader};
use super::topk::SortDirection;
use crate::error::Result;
use crate::exec::expr::row_width;
use crate::value::SqlValue;

/// One item in the merge-priority-queue: the head row of a run plus enough
/// metadata to refill from the right source.
struct MergeItem {
    keys: Vec<SqlValue>,
    row: Vec<SqlValue>,
    directions: std::sync::Arc<[SortDirection]>,
    /// Index into `mem_runs` if `Some`, or into `spill_runs` if `None`.
    mem_index: Option<usize>,
    spill_index: Option<usize>,
    /// Tie-break: lower wins, ensuring stable order across runs.
    tag: u64,
}

super::impl_partial_from_ord!(MergeItem);
impl Ord for MergeItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we invert so the smallest tuple wins.
        for ((l, r), dir) in self
            .keys
            .iter()
            .zip(other.keys.iter())
            .zip(self.directions.iter())
        {
            let ord = dir.compare_values(l, r);
            if ord != Ordering::Equal {
                return ord.reverse();
            }
        }
        self.tag.cmp(&other.tag).reverse()
    }
}

/// Builder/runner for the external merge-sort.
///
/// Caller supplies a way to extract sort keys (`key_fn`) and a budget. Rows
/// are pushed in arbitrary order; once all input has been consumed,
/// [`SpillSort::finish`] streams the sorted result back.
pub struct SpillSort<F>
where
    F: FnMut(&[SqlValue]) -> Result<Vec<SqlValue>>,
{
    directions: std::sync::Arc<[SortDirection]>,
    work_mem_bytes: usize,
    max_spill_bytes: usize,
    spill_root: PathBuf,
    key_fn: F,
    buffer: Vec<(Vec<SqlValue>, Vec<SqlValue>)>,
    buffer_bytes: usize,
    runs: Vec<SpillFile>,
    total_spilled_bytes: u64,
    next_tag: u64,
}

impl<F> SpillSort<F>
where
    F: FnMut(&[SqlValue]) -> Result<Vec<SqlValue>>,
{
    pub fn new(
        directions: Vec<SortDirection>,
        work_mem_bytes: usize,
        max_spill_bytes: usize,
        spill_root: PathBuf,
        key_fn: F,
    ) -> Self {
        Self {
            directions: std::sync::Arc::from(directions.into_boxed_slice()),
            work_mem_bytes,
            max_spill_bytes,
            spill_root,
            key_fn,
            buffer: Vec::new(),
            buffer_bytes: 0,
            runs: Vec::new(),
            total_spilled_bytes: 0,
            next_tag: 0,
        }
    }

    pub fn total_spilled_bytes(&self) -> u64 {
        self.total_spilled_bytes
    }

    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    pub fn push(&mut self, row: Vec<SqlValue>) -> Result<()> {
        let keys = (self.key_fn)(&row)?;
        self.buffer_bytes += row_width(&row) + row_width(&keys);
        self.buffer.push((keys, row));
        if self.buffer_bytes > self.work_mem_bytes {
            self.flush_run()?;
        }
        Ok(())
    }

    fn sort_buffer(&mut self) {
        let directions = std::sync::Arc::clone(&self.directions);
        self.buffer.sort_by(|a, b| {
            for ((l, r), dir) in a.0.iter().zip(b.0.iter()).zip(directions.iter()) {
                let ord = dir.compare_values(l, r);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });
    }

    fn flush_run(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.sort_buffer();
        let file = SpillFile::create_in(&self.spill_root, "sort-run")?;
        let mut writer = file.writer()?;
        for (_keys, row) in &self.buffer {
            writer.write_row(row)?;
        }
        writer.flush()?;
        let bytes = writer.bytes_written();
        self.total_spilled_bytes = self.total_spilled_bytes.saturating_add(bytes);
        if self.total_spilled_bytes > self.max_spill_bytes as u64 {
            return Err(crate::error::Error::ConstraintViolation(
                "query spill limit exceeded during sort".to_owned(),
            ));
        }
        self.runs.push(file);
        self.buffer.clear();
        self.buffer_bytes = 0;
        Ok(())
    }

    /// Consume the sorter, returning sorted rows.
    pub fn finish(mut self) -> Result<Vec<Vec<SqlValue>>> {
        // Fast path: nothing was spilled, we can sort in memory.
        if self.runs.is_empty() {
            self.sort_buffer();
            return Ok(self.buffer.drain(..).map(|(_, row)| row).collect());
        }

        // Final flush of leftover memory buffer.
        self.flush_run()?;

        // K-way merge runs.
        let mut readers: Vec<SpillReader> = self
            .runs
            .iter()
            .map(|f| f.reader())
            .collect::<Result<Vec<_>>>()?;
        let mut heap: BinaryHeap<MergeItem> = BinaryHeap::with_capacity(readers.len());
        for (idx, reader) in readers.iter_mut().enumerate() {
            if let Some(row) = reader.read_row()? {
                let keys = (self.key_fn)(&row)?;
                let tag = self.next_tag;
                self.next_tag = self.next_tag.saturating_add(1);
                heap.push(MergeItem {
                    keys,
                    row,
                    directions: std::sync::Arc::clone(&self.directions),
                    mem_index: None,
                    spill_index: Some(idx),
                    tag,
                });
            }
        }
        let mut out = Vec::new();
        while let Some(item) = heap.pop() {
            out.push(item.row);
            if let Some(idx) = item.spill_index
                && let Some(row) = readers[idx].read_row()?
            {
                let keys = (self.key_fn)(&row)?;
                let tag = self.next_tag;
                self.next_tag = self.next_tag.saturating_add(1);
                heap.push(MergeItem {
                    keys,
                    row,
                    directions: std::sync::Arc::clone(&self.directions),
                    mem_index: None,
                    spill_index: Some(idx),
                    tag,
                });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn key_first(row: &[SqlValue]) -> Result<Vec<SqlValue>> {
        Ok(vec![row[0].clone()])
    }

    #[test]
    fn in_memory_path_sorts_ascending() {
        let root = tempdir().expect("tempdir");
        let mut sorter = SpillSort::new(
            vec![SortDirection::Asc],
            1024 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
            key_first,
        );
        for v in [5, 1, 4, 3, 2] {
            sorter.push(vec![SqlValue::Integer(v)]).expect("push");
        }
        let rows = sorter.finish().expect("finish");
        let values: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(values, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn spill_triggered_path_returns_correct_order() {
        // Tiny budget forces a spill on every row.
        let root = tempdir().expect("tempdir");
        let mut sorter = SpillSort::new(
            vec![SortDirection::Asc],
            8,
            1024 * 1024,
            root.path().to_path_buf(),
            key_first,
        );
        let mut input: Vec<i64> = (0..50).rev().collect();
        // Shuffle deterministically.
        input.sort_by_key(|v| (v.wrapping_mul(2654435761)) as u32);
        for v in &input {
            sorter.push(vec![SqlValue::Integer(*v)]).expect("push");
        }
        let runs_before = sorter.run_count();
        assert!(runs_before > 0, "expected spill runs");
        let rows = sorter.finish().expect("finish");
        let values: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        let mut expected = input.clone();
        expected.sort();
        assert_eq!(values, expected);
    }

    #[test]
    fn merge_of_five_runs() {
        let root = tempdir().expect("tempdir");
        let mut sorter = SpillSort::new(
            vec![SortDirection::Asc],
            32,
            1024 * 1024,
            root.path().to_path_buf(),
            key_first,
        );
        // Construct input s.t. ~5 distinct flush events occur.
        for v in (0..200).rev() {
            sorter.push(vec![SqlValue::Integer(v)]).expect("push");
        }
        assert!(sorter.run_count() >= 1);
        let rows = sorter.finish().expect("finish");
        let values: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        let expected: Vec<i64> = (0..200).collect();
        assert_eq!(values, expected);
    }

    #[test]
    fn descending_path_still_sorts() {
        let root = tempdir().expect("tempdir");
        let mut sorter = SpillSort::new(
            vec![SortDirection::Desc],
            1024 * 1024,
            1024 * 1024,
            root.path().to_path_buf(),
            key_first,
        );
        for v in [5, 1, 4, 3, 2] {
            sorter.push(vec![SqlValue::Integer(v)]).expect("push");
        }
        let rows = sorter.finish().expect("finish");
        let values: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(values, vec![5, 4, 3, 2, 1]);
    }
}
