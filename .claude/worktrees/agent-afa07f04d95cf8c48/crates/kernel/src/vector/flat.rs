//! Exact flat-scan top-K operator.
//!
//! Given a query vector, a metric, and an iterator of `(payload, vector)`
//! pairs, returns the K closest payloads in **ascending distance order**.
//!
//! Implementation: a fixed-capacity max-heap of size `k`. Each input row pays
//! one distance computation plus an O(log k) heap update; total cost is
//! `O(n · (dim + log k))`. The dispatcher behind [`super::simd`] picks the
//! best vectorised kernel for the host.
//!
//! The operator is generic over the payload type, so the SQL layer can return
//! `RowId`s while a Rust caller could return embeddings, table identifiers, or
//! whatever else. This is the fallback path used whenever the planner picks
//! the brute-force strategy (no HNSW/DiskANN index available).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::{VectorMetric, VectorResult};

/// One result from a flat scan: a caller-supplied payload and its distance to
/// the query vector.
#[derive(Debug, Clone)]
pub struct FlatScanHit<P> {
    pub payload: P,
    pub distance: f32,
}

#[derive(Debug, Clone)]
struct HeapEntry<P> {
    distance: f32,
    payload: P,
}

impl<P> PartialEq for HeapEntry<P> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}
impl<P> Eq for HeapEntry<P> {}

impl<P> Ord for HeapEntry<P> {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we want to evict the *largest* distance,
        // so the natural ordering matches BinaryHeap directly.
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}
impl<P> PartialOrd for HeapEntry<P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Run an exact top-K flat scan.
///
/// * `query` — the query vector. Must match every candidate's dimension.
/// * `metric` — distance function to use.
/// * `k` — number of results to return. `k = 0` short-circuits to an empty
///   vector. If fewer than `k` candidates are seen, all of them are returned
///   (sorted).
/// * `candidates` — iterator of `(payload, vector_slice)` pairs.
///
/// Returns hits sorted by ascending distance.
pub fn flat_top_k<P, I>(
    query: &[f32],
    metric: VectorMetric,
    k: usize,
    candidates: I,
) -> VectorResult<Vec<FlatScanHit<P>>>
where
    I: IntoIterator<Item = (P, Vec<f32>)>,
{
    if k == 0 {
        return Ok(Vec::new());
    }
    let mut heap: BinaryHeap<HeapEntry<P>> = BinaryHeap::with_capacity(k + 1);
    for (payload, vector) in candidates {
        let distance = metric.distance(query, &vector)?;
        if heap.len() < k {
            heap.push(HeapEntry { distance, payload });
        } else if let Some(top) = heap.peek()
            && distance < top.distance
        {
            heap.pop();
            heap.push(HeapEntry { distance, payload });
        }
    }
    let mut out: Vec<FlatScanHit<P>> = heap
        .into_iter()
        .map(|e| FlatScanHit {
            payload: e.payload,
            distance: e.distance,
        })
        .collect();
    out.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(Ordering::Equal)
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_topk_in_ascending_distance() {
        let query = vec![0.0_f32, 0.0];
        let cands = vec![
            (1usize, vec![5.0_f32, 0.0]),
            (2, vec![1.0, 0.0]),
            (3, vec![3.0, 0.0]),
            (4, vec![2.0, 0.0]),
        ];
        let hits = flat_top_k(&query, VectorMetric::L2, 3, cands).unwrap();
        let ids: Vec<usize> = hits.iter().map(|h| h.payload).collect();
        assert_eq!(ids, vec![2, 4, 3]);
    }

    #[test]
    fn k_zero_returns_empty() {
        let query = vec![0.0_f32];
        let cands = vec![(1usize, vec![1.0_f32])];
        let hits = flat_top_k(&query, VectorMetric::L2, 0, cands).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn k_greater_than_n_returns_all_sorted() {
        let query = vec![0.0_f32];
        let cands = vec![(1usize, vec![3.0_f32]), (2, vec![1.0]), (3, vec![2.0])];
        let hits = flat_top_k(&query, VectorMetric::L2, 10, cands).unwrap();
        let ids: Vec<usize> = hits.iter().map(|h| h.payload).collect();
        assert_eq!(ids, vec![2, 3, 1]);
    }

    #[test]
    fn dim_mismatch_propagates() {
        let query = vec![0.0_f32, 0.0];
        let cands = vec![(1usize, vec![1.0_f32, 2.0, 3.0])];
        assert!(flat_top_k(&query, VectorMetric::L2, 1, cands).is_err());
    }
}
