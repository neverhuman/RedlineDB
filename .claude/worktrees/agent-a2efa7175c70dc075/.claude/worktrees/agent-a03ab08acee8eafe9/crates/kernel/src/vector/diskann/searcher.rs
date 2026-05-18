//! Beam search over the Vamana graph (Algorithm 1 in the DiskANN paper).
//!
//! Maintains a sorted-by-distance "search list" `L` of beam candidates.
//! Each step pops the closest unvisited candidate, expands its neighbours
//! into `L`, and trims `L` back to `beam_width`. Termination: every entry
//! in `L` has been visited.
//!
//! When the on-disk path lands, expansion (`neighbours[u]`) becomes a
//! sector read; candidate-distance computation can use the same sector's
//! vector payload. The in-memory variant keeps the algorithm shape so the
//! switch is structural rather than algorithmic.

use std::collections::HashSet;

use crate::vector::diskann::distance::l2_squared;

/// Search-time configuration.
#[derive(Clone, Copy, Debug)]
pub struct SearchParams {
    /// Number of nearest neighbours the caller asked for.
    pub k: usize,
    /// Working-list size during traversal (`L` in the paper). Typically
    /// 64..200; larger values approach exhaustive recall at higher cost.
    pub beam_width: usize,
}

/// Run beam search and return up to `params.k` node ids, ordered by
/// ascending distance. `entry` is the seed node id (medoid in v1).
///
/// `vectors` is a flat `num_nodes * dim` slice; `neighbours[i]` lists the
/// out-neighbours of node `i` (already `<= R` after build).
pub fn search(
    vectors: &[f32],
    dim: usize,
    neighbours: &[Vec<u32>],
    entry: u32,
    query: &[f32],
    params: SearchParams,
) -> Vec<u32> {
    if neighbours.is_empty() {
        return Vec::new();
    }
    let SearchParams { k, beam_width } = params;
    let n = neighbours.len();
    let entry = (entry as usize).min(n.saturating_sub(1)) as u32;

    // `list` is the working "search list" L, sorted ascending by distance
    // to `query`. We keep it as a `Vec<(id, dist)>`; insertions do a small
    // linear scan because beam_width is bounded (default 64).
    let mut list: Vec<(u32, f32)> = Vec::with_capacity(beam_width + 1);
    let entry_dist = l2_squared(vector_at(vectors, dim, entry), query);
    list.push((entry, entry_dist));

    // `visited` tracks expansion (we never visit a node twice). We expand
    // up to `beam_width * 4` nodes worst-case before convergence on real
    // graphs; the HashSet keeps lookup O(1).
    let mut visited: HashSet<u32> = HashSet::new();

    loop {
        // Find the closest unvisited entry in `list`.
        let pick = list
            .iter()
            .enumerate()
            .find(|(_, (id, _))| !visited.contains(id));
        let Some((_, &(u, _))) = pick else {
            break;
        };
        visited.insert(u);

        // Expand neighbours of `u`.
        for &v in &neighbours[u as usize] {
            // Defensive: out-of-range neighbour ids would be a bug in the
            // builder; skip rather than panic so the searcher stays
            // robust against a corrupt sector image.
            if (v as usize) >= n {
                continue;
            }
            // Avoid recomputing distances for nodes already in the list.
            if list.iter().any(|(id, _)| *id == v) {
                continue;
            }
            let dv = l2_squared(vector_at(vectors, dim, v), query);
            insert_sorted(&mut list, (v, dv), beam_width);
        }
    }

    list.truncate(k);
    list.into_iter().map(|(id, _)| id).collect()
}

/// Insert `(id, dist)` into a list sorted ascending by distance. After the
/// insert, the list is truncated to `cap`.
fn insert_sorted(list: &mut Vec<(u32, f32)>, item: (u32, f32), cap: usize) {
    // Find insertion point. `partition_point` does the binary search and
    // is stable enough for our small `list` size.
    let pos = list.partition_point(|(_, d)| *d <= item.1);
    list.insert(pos, item);
    if list.len() > cap {
        list.truncate(cap);
    }
}

#[inline]
fn vector_at(vectors: &[f32], dim: usize, id: u32) -> &[f32] {
    let id = id as usize;
    &vectors[id * dim..(id + 1) * dim]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(vs: &[&[f32]]) -> Vec<f32> {
        let mut out = Vec::new();
        for v in vs {
            out.extend_from_slice(v);
        }
        out
    }

    #[test]
    fn single_node_graph_returns_self() {
        let vs = flat(&[&[1.0_f32, 2.0]]);
        let neigh: Vec<Vec<u32>> = vec![vec![]];
        let out = search(
            &vs,
            2,
            &neigh,
            0,
            &[1.0, 2.0],
            SearchParams {
                k: 1,
                beam_width: 4,
            },
        );
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn line_graph_finds_nearest() {
        // 5 points on a line, fully connected as a chain.
        let vs = flat(&[&[0.0_f32], &[1.0], &[2.0], &[3.0], &[4.0]]);
        let neigh: Vec<Vec<u32>> = vec![vec![1], vec![0, 2], vec![1, 3], vec![2, 4], vec![3]];
        let out = search(
            &vs,
            1,
            &neigh,
            0,
            &[3.7],
            SearchParams {
                k: 1,
                beam_width: 5,
            },
        );
        assert_eq!(out, vec![4]);
    }

    #[test]
    fn insert_sorted_keeps_order_and_cap() {
        let mut list: Vec<(u32, f32)> = Vec::new();
        insert_sorted(&mut list, (0, 5.0), 3);
        insert_sorted(&mut list, (1, 1.0), 3);
        insert_sorted(&mut list, (2, 3.0), 3);
        insert_sorted(&mut list, (3, 9.0), 3);
        insert_sorted(&mut list, (4, 2.0), 3);
        assert_eq!(list, vec![(1, 1.0), (4, 2.0), (2, 3.0)]);
    }
}
