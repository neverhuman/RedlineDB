use super::vector_at;
use crate::vector::diskann::prune::Candidate;
use crate::vector::distance::l2_distance_scalar as l2_squared;

use std::collections::HashSet;

/// Greedy search used during build. Returns the set of nodes that were
/// visited (expanded) along the way; this is the candidate pool the paper
/// hands to RobustPrune. We exclude `target` from the result so a point
/// can't end up as its own neighbour.
pub(super) fn greedy_search_visited(
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
pub(super) fn candidate_set_for(
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
pub(super) fn medoid(flat: &[f32], dim: usize, n: usize) -> u32 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 0;
    }
    let sample_cap = 1024.min(n);
    let mut rng = SplitMix64::new(0xD1A1_5111_ED15_C0DE_u64);
    let mut sample: Vec<u32> = (0..n as u32).collect();
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
pub(super) fn init_random_graph(n: usize, max_degree: usize, seed: u64) -> Vec<Vec<u32>> {
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
pub(super) fn permutation(n: usize, seed: u64) -> Vec<usize> {
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

pub(super) fn flatten(vectors: &[Vec<f32>], dim: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(vectors.len() * dim);
    for v in vectors {
        out.extend_from_slice(v);
    }
    out
}

/// SplitMix64 — tiny deterministic RNG. Avoids pulling in the `rand` crate
/// for one usage. Reference: <https://prng.di.unimi.it/splitmix64.c>
pub(super) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub(super) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub(super) fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
