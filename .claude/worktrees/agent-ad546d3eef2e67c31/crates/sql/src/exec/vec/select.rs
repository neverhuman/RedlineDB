//! Lane VE: selection vectors for batch-aware filtering.
//!
//! A selection vector is a list of row indices into a [`RowBatch`]. Filter
//! operators populate one instead of physically copying rows; downstream
//! operators iterate the indices and read the underlying columns directly.
//! This keeps a CPU-efficient batch shape for predicate-heavy plans without
//! making row materialisation the only API.

use std::sync::Arc;

use crate::batch::{RowBatch, RowLayout};
use crate::error::Result;
use crate::value::SqlValue;

/// Default vectorized batch size (rows per `RowBatch`). Tip1/Tip2 advise
/// 1024–4096; we anchor at 1024 for cache friendliness, but operators that
/// know they have wide rows may consult [`MIN_BATCH_ROWS`] and
/// [`MAX_BATCH_ROWS`] as practical bounds.
pub const DEFAULT_BATCH_ROWS: usize = 1024;
pub const MIN_BATCH_ROWS: usize = 64;
pub const MAX_BATCH_ROWS: usize = 4096;

/// Indices into a [`RowBatch`]; preserves order.
#[derive(Debug, Clone, Default)]
pub struct SelectionVector {
    indices: Vec<u32>,
}

impl SelectionVector {
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            indices: Vec::with_capacity(cap),
        }
    }

    /// Build a vector that selects every row in a batch of length `len`.
    pub fn full(len: usize) -> Self {
        Self {
            indices: (0..len as u32).collect(),
        }
    }

    pub fn clear(&mut self) {
        self.indices.clear();
    }

    pub fn push(&mut self, index: u32) {
        self.indices.push(index);
    }

    pub fn extend_from_slice(&mut self, indices: &[u32]) {
        self.indices.extend_from_slice(indices);
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, u32> {
        self.indices.iter()
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.indices
    }

    /// Consumes the vector and yields the raw indices.
    pub fn into_indices(self) -> Vec<u32> {
        self.indices
    }
}

impl<'a> IntoIterator for &'a SelectionVector {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;

    fn into_iter(self) -> Self::IntoIter {
        self.indices.iter()
    }
}

/// Run a row-level predicate against `batch` and emit the surviving indices.
///
/// The predicate is invoked once per row with a borrowed slice. This keeps
/// the dispatch path narrow without forcing every operator onto a custom
/// expression IR.
pub fn filter_batch<F>(batch: &RowBatch, mut predicate: F) -> Result<SelectionVector>
where
    F: FnMut(&[SqlValue]) -> Result<bool>,
{
    let mut out = SelectionVector::with_capacity(batch.len);
    let mut row_buffer: Vec<SqlValue> = Vec::with_capacity(batch.columns.len());
    for index in 0..batch.len {
        row_buffer.clear();
        for column in &batch.columns {
            row_buffer.push(column[index].clone());
        }
        if predicate(&row_buffer)? {
            out.push(index as u32);
        }
    }
    Ok(out)
}

/// Materialise a single row from a batch by selection-vector position.
pub fn row_from_batch(batch: &RowBatch, sel_pos: usize, sel: &SelectionVector) -> Vec<SqlValue> {
    let row_idx = sel.as_slice()[sel_pos] as usize;
    batch
        .columns
        .iter()
        .map(|column| column[row_idx].clone())
        .collect()
}

/// Helper: build an empty batch with a fixed column layout. Used when an
/// operator knows up-front what columns it will emit but hasn't yet filled
/// any rows.
pub fn batch_with_layout(layout: Arc<RowLayout>) -> RowBatch {
    RowBatch::new(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_batch(rows: Vec<Vec<SqlValue>>) -> RowBatch {
        let columns: Vec<String> = vec!["a".to_owned(), "b".to_owned()];
        let layout = Arc::new(RowLayout {
            columns: Arc::from(columns),
        });
        let mut batch = RowBatch::new(layout);
        for row in rows {
            batch.push_row(row);
        }
        batch
    }

    #[test]
    fn selection_vector_basic_ops() {
        let mut sel = SelectionVector::new();
        assert!(sel.is_empty());
        sel.push(0);
        sel.push(2);
        sel.extend_from_slice(&[5, 7]);
        assert_eq!(sel.len(), 4);
        assert_eq!(sel.as_slice(), &[0, 2, 5, 7]);
        sel.clear();
        assert!(sel.is_empty());
    }

    #[test]
    fn selection_vector_full_covers_all_rows() {
        let sel = SelectionVector::full(5);
        assert_eq!(sel.as_slice(), &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn filter_batch_selects_matching_rows() {
        let batch = make_batch(vec![
            vec![SqlValue::Integer(1), SqlValue::Integer(10)],
            vec![SqlValue::Integer(2), SqlValue::Integer(20)],
            vec![SqlValue::Integer(3), SqlValue::Integer(30)],
            vec![SqlValue::Integer(4), SqlValue::Integer(40)],
        ]);
        let sel = filter_batch(&batch, |row| match row.first() {
            Some(SqlValue::Integer(v)) => Ok(*v % 2 == 0),
            _ => Ok(false),
        })
        .expect("filter");
        assert_eq!(sel.as_slice(), &[1, 3]);
    }

    #[test]
    fn row_from_batch_reads_correct_position() {
        let batch = make_batch(vec![
            vec![SqlValue::Integer(1), SqlValue::Integer(10)],
            vec![SqlValue::Integer(2), SqlValue::Integer(20)],
            vec![SqlValue::Integer(3), SqlValue::Integer(30)],
        ]);
        let sel = SelectionVector {
            indices: vec![0, 2],
        };
        let row = row_from_batch(&batch, 1, &sel);
        assert_eq!(row, vec![SqlValue::Integer(3), SqlValue::Integer(30)]);
    }

    #[test]
    fn extend_iter_round_trips() {
        let mut sel = SelectionVector::with_capacity(3);
        sel.extend_from_slice(&[1, 4, 9]);
        let collected: Vec<u32> = sel.iter().copied().collect();
        assert_eq!(collected, vec![1, 4, 9]);
    }
}
