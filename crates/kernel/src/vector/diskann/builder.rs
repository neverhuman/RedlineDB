//! Vamana graph builder (Algorithm 3 in the DiskANN paper).
//!
//! 1. Initialise the graph with a random `R`-regular adjacency.
//! 2. Pick the medoid as the entry point.
//! 3. For two passes (alpha=1, then alpha=alpha_target), insert points in a
//!    random permutation: greedy-search to the candidate set V, run
//!    RobustPrune to compute the new neighbour list `out_p`, and reciprocate
//!    edges (running RobustPrune again on the back-edge target if it
//!    overflows `R`).
//!
//! v1 stays in-memory: every distance is computed against the flat float32
//! buffer. The structure of the build matches the on-disk version since the
//! adjacency layout is the same.

use super::support::{
    candidate_set_for, flatten, greedy_search_visited, init_random_graph, medoid, permutation,
};
use super::vector_at;
use crate::vector::diskann::prune::{Candidate, robust_prune};
use crate::vector::distance::l2_distance_scalar as l2_squared;

/// Internal build params (mirrors the public [`super::DiskAnnParams`]).
#[derive(Clone, Copy, Debug)]
pub struct BuildParams {
    pub max_degree: usize,
    pub search_list_size: usize,
    pub alpha: f32,
    pub seed: u64,
}

/// Diagnostic stats returned by tests; not part of the public surface yet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BuildStats {
    pub passes: u32,
    pub avg_degree: f32,
}

/// Build a Vamana graph and return the entry-point id plus adjacency lists.
pub fn build(dim: usize, vectors: &[Vec<f32>], params: BuildParams) -> (u32, Vec<Vec<u32>>) {
    let n = vectors.len();
    if n == 0 {
        return (0, Vec::new());
    }

    // Flatten once for fast distance lookups.
    let flat = flatten(vectors, dim);

    // Initial neighbour set: random `R`-regular graph. Self-loops are
    // filtered, duplicates collapsed.
    let mut neighbours = init_random_graph(n, params.max_degree, params.seed);

    // Pick the medoid as the entry point. For very small `n` we fall back
    // to id 0 which keeps behaviour deterministic.
    let entry = medoid(&flat, dim, n);

    // Two passes per the paper: first with alpha=1.0 to bootstrap the
    // graph, then with the user-requested alpha to lift recall. We skip
    // the alpha=1.0 pass when the user already requested alpha == 1.0.
    let alphas: &[f32] = if (params.alpha - 1.0).abs() < f32::EPSILON {
        &[1.0]
    } else {
        &[1.0, params.alpha]
    };

    let order = permutation(n, params.seed.wrapping_add(0xA5A5_5A5A));

    for &alpha in alphas {
        for &p in &order {
            // 1) GreedySearch from the entry point to point `p`. Returns
            //    the candidate set V (visited nodes) accumulated along
            //    the way.
            let v_set =
                greedy_search_visited(&flat, dim, &neighbours, entry, p, params.search_list_size);
            // 2) Build (id, dist_to_p) for every visited node, including
            //    the existing neighbours of p (so we don't lose useful
            //    edges between passes).
            let mut cands = candidate_set_for(p, &v_set, &neighbours, &flat, dim);
            if cands.is_empty() {
                continue;
            }
            // 3) RobustPrune to get the new neighbour list of `p`.
            let p_vec = vector_at(&flat, dim, p as u32);
            let new_out = robust_prune(&mut cands, p_vec, &flat, dim, alpha, params.max_degree);
            neighbours[p] = new_out.clone();
            // 4) Reciprocate. For each j in out_p, add p to neighbours[j];
            //    if neighbours[j] overflows R, RobustPrune it back down.
            for j in new_out {
                if j as usize == p {
                    continue;
                }
                let nj = &mut neighbours[j as usize];
                if !nj.contains(&(p as u32)) {
                    nj.push(p as u32);
                }
                if nj.len() > params.max_degree {
                    let j_vec = vector_at(&flat, dim, j);
                    let mut prune_cands: Vec<Candidate> = nj
                        .iter()
                        .filter(|&&id| (id as usize) < n)
                        .map(|&id| {
                            let d = l2_squared(vector_at(&flat, dim, id), j_vec);
                            (id, d)
                        })
                        .collect();
                    let pruned = robust_prune(
                        &mut prune_cands,
                        j_vec,
                        &flat,
                        dim,
                        alpha,
                        params.max_degree,
                    );
                    neighbours[j as usize] = pruned;
                }
            }
        }
    }

    (entry, neighbours)
}
