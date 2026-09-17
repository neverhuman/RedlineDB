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

use rayon::slice::ParallelSliceMut;

use super::spill::{SpillFile, SpillReader};
use super::topk::SortDirection;
use crate::error::Result;
use crate::exec::expr::row_width;
use crate::value::SqlValue;

/// Buffer length above which the in-memory sort uses Rayon's parallel
/// `par_sort_by`. Below this threshold the serial path wins on dispatch
/// overhead. The comparator (`SortDirection::compare_values` ->
/// `crate::value::compare_values`) is pure — no executor thread-locals,
/// no UDF callbacks — so parallel execution is sound.
const PARALLEL_SORT_THRESHOLD: usize = 64 * 1024;

/// Maximum number of spill runs opened by one merge pass. Keep enough
/// descriptor headroom for the database, test harness, and output run.
const MAX_MERGE_FAN_IN: usize = 32;

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
        let cmp = move |a: &(Vec<SqlValue>, Vec<SqlValue>), b: &(Vec<SqlValue>, Vec<SqlValue>)| {
            for ((l, r), dir) in a.0.iter().zip(b.0.iter()).zip(directions.iter()) {
                let ord = dir.compare_values(l, r);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        };
        if self.buffer.len() >= PARALLEL_SORT_THRESHOLD {
            self.buffer.par_sort_by(cmp);
        } else {
            self.buffer.sort_by(cmp);
        }
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
        self.record_spilled_bytes(bytes)?;
        self.runs.push(file);
        self.buffer.clear();
        self.buffer_bytes = 0;
        Ok(())
    }

    fn record_spilled_bytes(&mut self, bytes: u64) -> Result<()> {
        self.total_spilled_bytes = self.total_spilled_bytes.saturating_add(bytes);
        if self.total_spilled_bytes > self.max_spill_bytes as u64 {
            return Err(crate::error::Error::ConstraintViolation(
                "query spill limit exceeded during sort".to_owned(),
            ));
        }
        Ok(())
    }

    fn merge_runs(
        &mut self,
        runs: &[SpillFile],
        mut emit: impl FnMut(Vec<SqlValue>) -> Result<()>,
    ) -> Result<()> {
        debug_assert!(runs.len() <= MAX_MERGE_FAN_IN);
        let mut readers: Vec<SpillReader> = runs
            .iter()
            .map(SpillFile::reader)
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
        while let Some(item) = heap.pop() {
            let spill_index = item.spill_index;
            emit(item.row)?;
            if let Some(idx) = spill_index
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
        Ok(())
    }

    fn merge_runs_to_file(&mut self, runs: &[SpillFile]) -> Result<SpillFile> {
        let file = SpillFile::create_in(&self.spill_root, "sort-merge")?;
        let mut writer = file.writer()?;
        self.merge_runs(runs, |row| writer.write_row(&row))?;
        writer.flush()?;
        self.record_spilled_bytes(writer.bytes_written())?;
        Ok(file)
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

        // Bound descriptor usage by consolidating runs in deterministic
        // batches before the final merge.
        while self.runs.len() > MAX_MERGE_FAN_IN {
            let runs = std::mem::take(&mut self.runs);
            let mut runs = runs.into_iter();
            let mut merged = Vec::new();
            loop {
                let batch: Vec<SpillFile> = runs.by_ref().take(MAX_MERGE_FAN_IN).collect();
                if batch.is_empty() {
                    break;
                }
                merged.push(self.merge_runs_to_file(&batch)?);
            }
            self.runs = merged;
        }

        let runs = std::mem::take(&mut self.runs);
        let mut out = Vec::new();
        self.merge_runs(&runs, |row| {
            out.push(row);
            Ok(())
        })?;
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
    fn merge_more_runs_than_fan_in_descending() {
        let root = tempdir().expect("tempdir");
        let mut sorter = SpillSort::new(
            vec![SortDirection::Desc],
            1,
            16 * 1024 * 1024,
            root.path().to_path_buf(),
            key_first,
        );
        let input: Vec<i64> = (0..(MAX_MERGE_FAN_IN as i64 * 3 + 7)).rev().collect();
        for value in &input {
            sorter.push(vec![SqlValue::Integer(*value)]).expect("push");
        }
        assert!(sorter.run_count() > MAX_MERGE_FAN_IN);

        let rows = sorter.finish().expect("finish");
        let values: Vec<i64> = rows
            .iter()
            .map(|row| match row[0] {
                SqlValue::Integer(value) => value,
                _ => unreachable!(),
            })
            .collect();
        let mut expected = input;
        expected.sort_by(|left, right| right.cmp(left));
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
