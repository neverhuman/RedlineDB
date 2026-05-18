//! HNSW beam search.
//!
//! Implements the canonical multi-layer descent from Malkov & Yashunin
//! (2016, Algorithms 2 + 5):
//!
//! 1. From the entry point at the topmost layer, run a *greedy* one-best
//!    walk down through every layer above 0. This collapses to "find the
//!    nearest neighbor of the query at level k+1 and use it as the
//!    starting point at level k".
//! 2. At layer 0, run a *dynamic* beam search with width `ef`. The
//!    result list grows past `ef` candidates while the visited set
//!    expands; the closest `k` survivors become the answer.
//!
//! Tombstoned nodes (set by `delete_tx`) participate in *graph
//! traversal* — the edges they hold still wire the small-world property
//! together — but never become candidates in the result set. Skipping
//! their edges entirely would shred recall for snapshots that pre-date
//! the delete.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use crate::Result;
use crate::vector::distance::Metric;

use super::Graph;

/// Total-ordered f32 wrapper. HNSW only ever compares distances, which
/// are non-negative and finite under all metrics we ship; the
/// `total_cmp` fallback handles the edge case where a query supplies
/// degenerate input (Inf/NaN) without panicking the heap.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Distance(f32);

impl Eq for Distance {}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Greedy one-best descent from `entry` through layers `entry_layer..top`
/// down to `target_layer`. Returns the closest node found at
/// `target_layer`.
pub(super) fn greedy_descend(
    graph: &Graph,
    metric: Metric,
    query: &[f32],
    entry: u32,
    entry_layer: u8,
    target_layer: u8,
) -> Result<u32> {
    debug_assert!(entry_layer >= target_layer);
    let mut current = entry;
    let mut current_dist = metric.distance(query, graph.vector_of(current)?)?;
    for layer in (target_layer..=entry_layer).rev() {
        loop {
            let mut improved = false;
            // Snapshot the neighbor list to avoid holding a borrow across
            // the inner distance loop (which itself reads from the graph).
            let neighbors: Vec<u32> = graph.neighbors_at(current, layer)?.to_vec();
            for nb in neighbors {
                let d = metric.distance(query, graph.vector_of(nb)?)?;
                if d < current_dist {
                    current = nb;
                    current_dist = d;
                    improved = true;
                }
            }
            if !improved {
                break;
            }
        }
        if layer == target_layer {
            break;
        }
    }
    Ok(current)
}

/// Layer-0 beam search (also used during construction for layers ≥ 1
/// with `ef = ef_construction`). Returns up to `ef` closest candidates,
/// sorted by ascending distance. Caller filters tombstones from the
/// result.
pub(super) fn search_layer(
    graph: &Graph,
    metric: Metric,
    query: &[f32],
    entry_points: &[u32],
    ef: usize,
    layer: u8,
) -> Result<Vec<(f32, u32)>> {
    if entry_points.is_empty() || ef == 0 {
        return Ok(Vec::new());
    }
    let mut visited: HashSet<u32> = HashSet::with_capacity(ef * 4);
    // Frontier: min-heap of (distance, node) — pop the closest unvisited
    // candidate next. BinaryHeap is max-heap; std::cmp::Reverse flips it.
    let mut frontier: BinaryHeap<std::cmp::Reverse<(Distance, u32)>> = BinaryHeap::new();
    // Result: max-heap of (distance, node) so its top is the *farthest*
    // current best — that's the threshold incoming candidates compete
    // against.
    let mut result: BinaryHeap<(Distance, u32)> = BinaryHeap::new();
    for &ep in entry_points {
        if !visited.insert(ep) {
            continue;
        }
        let d = Distance(metric.distance(query, graph.vector_of(ep)?)?);
        frontier.push(std::cmp::Reverse((d, ep)));
        result.push((d, ep));
        if result.len() > ef {
            result.pop();
        }
    }
    while let Some(std::cmp::Reverse((d_curr, curr))) = frontier.pop() {
        // Lane V2 failpoint: armed at every beam expansion. Tests in the
        // failpoint matrix can simulate a partial search by panicking
        // here mid-traversal.
        crate::fail_point!("vector::hnsw::search::beam_step");
        if let Some((d_far, _)) = result.peek()
            && result.len() >= ef
            && d_curr > *d_far
        {
            // No frontier candidate could improve the top-ef any further.
            break;
        }
        let neighbors: Vec<u32> = graph.neighbors_at(curr, layer)?.to_vec();
        for nb in neighbors {
            if !visited.insert(nb) {
                continue;
            }
            let d = Distance(metric.distance(query, graph.vector_of(nb)?)?);
            let admit = match result.peek() {
                Some((d_far, _)) => d < *d_far || result.len() < ef,
                None => true,
            };
            if admit {
                frontier.push(std::cmp::Reverse((d, nb)));
                result.push((d, nb));
                if result.len() > ef {
                    result.pop();
                }
            }
        }
    }
    let mut out: Vec<(f32, u32)> = result.into_iter().map(|(d, n)| (d.0, n)).collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{PageGeneration, PageId, RowId, TuplePtr, TxId};
    use crate::vector::distance::Metric;
    use crate::vector::hnsw::{Graph, GraphNode, IndexedRowRef};

    fn mk_node(id: u32, layer: u8, vector: Vec<f32>) -> GraphNode {
        GraphNode {
            node_id: id,
            layer,
            dead: false,
            committing_tx: TxId::ZERO,
            row_ref: IndexedRowRef {
                row_id: RowId(id as u64),
                tuple: TuplePtr::new_with_generation(PageId(0), 0, PageGeneration::ONE),
            },
            vector,
            neighbors: vec![Vec::new()],
            page_id: PageId(0),
            slot: 0,
        }
    }

    #[test]
    fn beam_search_on_empty_returns_empty() {
        let graph = Graph::new();
        let r = search_layer(&graph, Metric::L2, &[1.0, 2.0], &[], 16, 0).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn beam_search_returns_top_ef() {
        let mut graph = Graph::new();
        for i in 0..4 {
            graph.insert_node(mk_node(i, 0, vec![i as f32, 0.0]));
        }
        let result = search_layer(&graph, Metric::L2, &[0.0, 0.0], &[0, 1, 2, 3], 2, 0).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, 0);
        assert_eq!(result[1].1, 1);
    }
}
