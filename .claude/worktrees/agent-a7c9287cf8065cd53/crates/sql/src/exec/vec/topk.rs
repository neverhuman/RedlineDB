//! Lane VE: top-K heap.
//!
//! For `ORDER BY ... LIMIT k` with small `k`, materialising every row only
//! to throw most of them away is wasted work. Instead we keep a binary
//! max-heap of size `k`: the heap root is the worst tuple still admitted,
//! so admitting a new row is O(log k) and we never hold more than `k` rows.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::error::Result;
use crate::value::SqlValue;

/// Threshold above which a top-K plan no longer beats a full sort. The
/// planner consults this to decide between [`PhysicalKind::TopN`] and
/// [`PhysicalKind::Sort`].
pub const TOPK_LIMIT_THRESHOLD: usize = 64;

/// Sort spec: per-key direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// A row + cached sort keys. Cached keys avoid recomputing expression-derived
/// keys at every heap-comparison step.
#[derive(Debug, Clone)]
struct HeapEntry {
    keys: Vec<SqlValue>,
    directions: std::sync::Arc<[SortDirection]>,
    row: Vec<SqlValue>,
    /// Tie-breaker: higher counter sorts later (i.e. "lost" the tie). Keeps
    /// [`BinaryHeap`] popping deterministic in the face of equal keys.
    seq: u64,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        for ((l, r), dir) in self
            .keys
            .iter()
            .zip(other.keys.iter())
            .zip(self.directions.iter())
        {
            let mut ord = crate::value::compare_values(l, r);
            if matches!(dir, SortDirection::Desc) {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        // Tie-break on sequence so equal-key rows stay deterministic and the
        // heap doesn't spuriously evict an earlier row in favor of a later
        // one with identical keys.
        self.seq.cmp(&other.seq)
    }
}

/// Fixed-size max-heap that retains the K smallest tuples (by ASC) or
/// K largest (by DESC) seen so far.
#[derive(Debug)]
pub struct TopKHeap {
    k: usize,
    heap: BinaryHeap<HeapEntry>,
    directions: std::sync::Arc<[SortDirection]>,
    seen: u64,
}

impl TopKHeap {
    pub fn new(k: usize, directions: Vec<SortDirection>) -> Self {
        let directions = std::sync::Arc::from(directions.into_boxed_slice());
        Self {
            k,
            heap: BinaryHeap::with_capacity(k.saturating_add(1)),
            directions,
            seen: 0,
        }
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Admit `row` with precomputed `keys`. The keys vector must have the
    /// same length as the directions slice.
    pub fn push(&mut self, keys: Vec<SqlValue>, row: Vec<SqlValue>) -> Result<()> {
        debug_assert_eq!(keys.len(), self.directions.len());
        if self.k == 0 {
            return Ok(());
        }
        let entry = HeapEntry {
            keys,
            directions: std::sync::Arc::clone(&self.directions),
            row,
            seq: self.seen,
        };
        self.seen = self.seen.saturating_add(1);
        if self.heap.len() < self.k {
            self.heap.push(entry);
            return Ok(());
        }
        // Heap root is the *worst-of-the-best*; if the new entry is better,
        // swap it in.
        if let Some(top) = self.heap.peek()
            && entry < *top
        {
            self.heap.pop();
            self.heap.push(entry);
        }
        Ok(())
    }

    /// Drain the heap into ascending sorted order (per the configured key
    /// directions).
    pub fn into_sorted_rows(self) -> Vec<Vec<SqlValue>> {
        let mut entries = self.heap.into_sorted_vec();
        // `into_sorted_vec` returns entries in ascending order per `Ord`
        // (which already accounts for direction); no further reverse needed.
        let mut out = Vec::with_capacity(entries.len());
        for entry in entries.drain(..) {
            out.push(entry.row);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn keys_int(value: i64) -> Vec<SqlValue> {
        vec![SqlValue::Integer(value)]
    }

    #[test]
    fn topk_k1_keeps_min() {
        let mut heap = TopKHeap::new(1, vec![SortDirection::Asc]);
        for v in [5, 3, 9, 2, 7] {
            heap.push(keys_int(v), vec![SqlValue::Integer(v)])
                .expect("push");
        }
        let rows = heap.into_sorted_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], vec![SqlValue::Integer(2)]);
    }

    #[test]
    fn topk_k10_returns_smallest_ten_sorted() {
        let mut heap = TopKHeap::new(10, vec![SortDirection::Asc]);
        let mut input: Vec<i64> = (0..100).collect();
        // Random-ish shuffle without rand dep.
        input.sort_by_key(|v| (v.wrapping_mul(2654435761)) as u32);
        for v in &input {
            heap.push(keys_int(*v), vec![SqlValue::Integer(*v)])
                .expect("push");
        }
        let rows = heap.into_sorted_rows();
        let got: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(got, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn topk_k64_with_ties_breaks_deterministically() {
        let mut heap = TopKHeap::new(64, vec![SortDirection::Asc]);
        // Insert 200 rows where every key is one of {0, 1}.
        for i in 0..200i64 {
            let key = i % 2;
            heap.push(
                keys_int(key),
                vec![SqlValue::Integer(key), SqlValue::Integer(i)],
            )
            .expect("push");
        }
        let rows = heap.into_sorted_rows();
        assert_eq!(rows.len(), 64);
        // All retained rows must have key=0 (smallest).
        for row in &rows {
            assert_eq!(row[0], SqlValue::Integer(0));
        }
    }

    #[test]
    fn topk_desc_keeps_largest() {
        let mut heap = TopKHeap::new(3, vec![SortDirection::Desc]);
        for v in [5, 3, 9, 2, 7, 1, 8] {
            heap.push(keys_int(v), vec![SqlValue::Integer(v)])
                .expect("push");
        }
        let rows = heap.into_sorted_rows();
        let got: Vec<i64> = rows
            .iter()
            .map(|r| match r[0] {
                SqlValue::Integer(v) => v,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(got, vec![9, 8, 7]);
    }

    #[test]
    fn topk_with_null_keys() {
        let mut heap = TopKHeap::new(3, vec![SortDirection::Asc]);
        heap.push(vec![SqlValue::Null], vec![SqlValue::Text(Arc::from("nul"))])
            .expect("push");
        heap.push(
            vec![SqlValue::Integer(1)],
            vec![SqlValue::Text(Arc::from("one"))],
        )
        .expect("push");
        heap.push(
            vec![SqlValue::Integer(2)],
            vec![SqlValue::Text(Arc::from("two"))],
        )
        .expect("push");
        heap.push(
            vec![SqlValue::Integer(3)],
            vec![SqlValue::Text(Arc::from("three"))],
        )
        .expect("push");
        let rows = heap.into_sorted_rows();
        // SQLite NULL sorts first under ASC.
        assert_eq!(rows[0][0], SqlValue::Text(Arc::from("nul")));
    }

    #[test]
    fn topk_zero_capacity_drops_all() {
        let mut heap = TopKHeap::new(0, vec![SortDirection::Asc]);
        heap.push(keys_int(1), vec![SqlValue::Integer(1)])
            .expect("push");
        assert!(heap.is_empty());
        assert!(heap.into_sorted_rows().is_empty());
    }
}
