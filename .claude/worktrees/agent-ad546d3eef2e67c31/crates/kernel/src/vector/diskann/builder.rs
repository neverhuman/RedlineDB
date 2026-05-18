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

use std::collections::HashSet;

use crate::vector::diskann::distance::l2_squared;
use crate::vector::diskann::prune::{Candidate, robust_prune};

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

/// Greedy search used during build. Returns the set of nodes that were
/// visited (expanded) along the way; this is the candidate pool the paper
/// hands to RobustPrune. We exclude `target` from the result so a point
/// can't end up as its own neighbour.
fn greedy_search_visited(
    flat: &[f32],
    dim: usize,
    neighbours: &[Vec<u32>],
    entry: u32,
    target: usize,
    list_size: usize,
) -> Vec<u32> {
    let n = neighbours.len();
    if n == 0 {
        return Vec::new();
    }
    let target_vec = vector_at(flat, dim, target as u32);
    let entry = (entry as usize).min(n - 1) as u32;

    let mut list: Vec<(u32, f32)> = Vec::with_capacity(list_size + 1);
    list.push((entry, l2_squared(vector_at(flat, dim, entry), target_vec)));
    let mut visited: HashSet<u32> = HashSet::new();
    let mut visited_order: Vec<u32> = Vec::new();

    loop {
        let pick = list.iter().find(|(id, _)| !visited.contains(id)).copied();
        let Some((u, _)) = pick else {
            break;
        };
        visited.insert(u);
        if (u as usize) != target {
            visited_order.push(u);
        }
        for &v in &neighbours[u as usize] {
            if (v as usize) >= n {
                continue;
            }
            if list.iter().any(|(id, _)| *id == v) {
                continue;
            }
            let dv = l2_squared(vector_at(flat, dim, v), target_vec);
            // Insert sorted by distance, cap at list_size.
            let pos = list.partition_point(|(_, d)| *d <= dv);
            list.insert(pos, (v, dv));
            if list.len() > list_size {
                list.truncate(list_size);
            }
        }
    }
    visited_order
}

/// Build the candidate set for point `p`: every visited node from greedy
/// search plus `p`'s current neighbours, deduplicated, with distances.
fn candidate_set_for(
    p: usize,
    visited: &[u32],
    neighbours: &[Vec<u32>],
    flat: &[f32],
    dim: usize,
) -> Vec<Candidate> {
    let p_vec = vector_at(flat, dim, p as u32);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut out: Vec<Candidate> = Vec::with_capacity(visited.len() + neighbours[p].len());
    for &id in visited {
        if id as usize == p {
            continue;
        }
        if seen.insert(id) {
            let d = l2_squared(vector_at(flat, dim, id), p_vec);
            out.push((id, d));
        }
    }
    for &id in &neighbours[p] {
        if id as usize == p {
            continue;
        }
        if seen.insert(id) {
            let d = l2_squared(vector_at(flat, dim, id), p_vec);
            out.push((id, d));
        }
    }
    out
}

/// Pick a representative central point. Uses a sampled medoid: for `n` up
/// to a few thousand we compute the exact medoid on a random subsample of
/// up to 1024 points, which is good enough to seed the entry.
fn medoid(flat: &[f32], dim: usize, n: usize) -> u32 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 0;
    }
    let sample_cap = 1024.min(n);
    let mut rng = SplitMix64::new(0xD1A1_5111_ED15_C0DE_u64);
    let mut sample: Vec<u32> = (0..n as u32).collect();
    // Fisher-Yates partial shuffle to draw `sample_cap` indices.
    for i in 0..sample_cap {
        let j = i + (rng.next() as usize) % (n - i);
        sample.swap(i, j);
    }
    let sample = &sample[..sample_cap];

    let mut best_id = sample[0];
    let mut best_score = f32::INFINITY;
    for &i in sample {
        let mut acc = 0.0_f32;
        let vi = vector_at(flat, dim, i);
        for &j in sample {
            if i == j {
                continue;
            }
            acc += l2_squared(vi, vector_at(flat, dim, j));
        }
        if acc < best_score {
            best_score = acc;
            best_id = i;
        }
    }
    best_id
}

/// Random R-regular adjacency (no self-loops, deduplicated).
fn init_random_graph(n: usize, max_degree: usize, seed: u64) -> Vec<Vec<u32>> {
    let mut out: Vec<Vec<u32>> = vec![Vec::new(); n];
    if n <= 1 || max_degree == 0 {
        return out;
    }
    let mut rng = SplitMix64::new(seed.wrapping_add(0x1B0B_AFE7));
    let degree = max_degree.min(n - 1);
    for (i, slot) in out.iter_mut().enumerate().take(n) {
        let mut set: HashSet<u32> = HashSet::new();
        while set.len() < degree {
            let j = (rng.next() as usize) % n;
            if j == i {
                continue;
            }
            set.insert(j as u32);
        }
        *slot = set.into_iter().collect();
    }
    out
}

/// Deterministic permutation of `0..n`.
fn permutation(n: usize, seed: u64) -> Vec<usize> {
    let mut out: Vec<usize> = (0..n).collect();
    if n <= 1 {
        return out;
    }
    let mut rng = SplitMix64::new(seed);
    for i in (1..n).rev() {
        let j = (rng.next() as usize) % (i + 1);
        out.swap(i, j);
    }
    out
}

#[inline]
fn vector_at(flat: &[f32], dim: usize, id: u32) -> &[f32] {
    let id = id as usize;
    &flat[id * dim..(id + 1) * dim]
}

fn flatten(vectors: &[Vec<f32>], dim: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(vectors.len() * dim);
    for v in vectors {
        out.extend_from_slice(v);
    }
    out
}

/// SplitMix64 — tiny deterministic RNG. Avoids pulling in the `rand` crate
/// for one usage. Reference: <https://prng.di.unimi.it/splitmix64.c>
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_is_a_permutation() {
        let p = permutation(100, 42);
        let mut sorted = p.clone();
        sorted.sort();
        assert_eq!(sorted, (0..100).collect::<Vec<_>>());
    }

    #[test]
    fn init_random_graph_no_self_loops() {
        let g = init_random_graph(20, 5, 7);
        for (i, ns) in g.iter().enumerate() {
            assert!(!ns.contains(&(i as u32)));
            assert!(ns.len() <= 5);
        }
    }

    #[test]
    fn empty_input() {
        let (entry, neigh) = build(
            2,
            &[],
            BuildParams {
                max_degree: 4,
                search_list_size: 8,
                alpha: 1.2,
                seed: 1,
            },
        );
        assert_eq!(entry, 0);
        assert!(neigh.is_empty());
    }

    #[test]
    fn small_build_yields_bounded_degree() {
        let vs: Vec<Vec<f32>> = (0..50)
            .map(|i| vec![i as f32 * 0.1, (i as f32 * 0.7).sin()])
            .collect();
        let (_, neigh) = build(
            2,
            &vs,
            BuildParams {
                max_degree: 8,
                search_list_size: 32,
                alpha: 1.2,
                seed: 1,
            },
        );
        for ns in &neigh {
            assert!(ns.len() <= 8, "out-degree exceeded R");
        }
    }
}
