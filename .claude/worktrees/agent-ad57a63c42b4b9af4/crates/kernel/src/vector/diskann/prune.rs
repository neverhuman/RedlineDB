//! RobustPrune (Algorithm 2 in the DiskANN paper).
//!
//! Given a candidate set `V` of node ids and their distances to a target
//! point `p`, RobustPrune returns up to `R` neighbours whose pairwise
//! distances (under relaxation factor `alpha`) keep the graph well
//! connected. Conceptually:
//!
//! ```text
//! while V not empty and |out| < R:
//!     pick nearest p* in V
//!     append p* to out
//!     drop every q in V where alpha * d(p*, q) <= d(p, q)
//! ```
//!
//! The pruning is what gives Vamana its low diameter. Higher `alpha` keeps
//! more "long" edges and improves recall at the cost of out-degree budget.

use crate::vector::diskann::distance::l2_squared;

/// `(node_id, dist_to_target)` pairs as fed in / returned by RobustPrune.
pub type Candidate = (u32, f32);

/// Run RobustPrune. `vectors` is a flat `num_nodes * dim` slice; `dim` is
/// the per-vector length. `target` is the point whose neighbour set we are
/// computing (its own id should already be excluded from `candidates`).
///
/// Returns at most `max_degree` ids. The order is by ascending distance to
/// `target`, matching the paper's "out_p" sequence.
pub fn robust_prune(
    candidates: &mut [Candidate],
    _target: &[f32],
    vectors: &[f32],
    dim: usize,
    alpha: f32,
    max_degree: usize,
) -> Vec<u32> {
    if candidates.is_empty() || max_degree == 0 {
        return Vec::new();
    }
    // Sort ascending by distance to target so we always pop the closest
    // remaining node next. Sorting once is O(n log n) and lets the prune
    // loop run in O(R * n) which is fine for the candidate-set sizes
    // Vamana produces (typically L = 100..200).
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<u32> = Vec::with_capacity(max_degree);
    // `live[i]` toggled false once candidate i has been pruned away.
    let mut live = vec![true; candidates.len()];
    for i in 0..candidates.len() {
        if !live[i] {
            continue;
        }
        if out.len() >= max_degree {
            break;
        }
        let (p_star, _d_star) = candidates[i];
        out.push(p_star);
        // Disable every later candidate q where alpha * d(p*, q) <= d(p, q).
        let p_star_vec = vector_at(vectors, dim, p_star);
        for j in (i + 1)..candidates.len() {
            if !live[j] {
                continue;
            }
            let (q, d_pq) = candidates[j];
            let q_vec = vector_at(vectors, dim, q);
            let d_p_star_q = l2_squared(p_star_vec, q_vec);
            // Distances are squared L2; compare squared inputs after
            // squaring `alpha`. d_p_star_q and d_pq are already squared,
            // so the inequality `alpha * d_l2(p*, q) <= d_l2(p, q)` becomes
            // `alpha^2 * d_l2_sq(p*, q) <= d_l2_sq(p, q)`.
            if alpha * alpha * d_p_star_q <= d_pq {
                live[j] = false;
            }
        }
    }
    out
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
    fn empty_input_returns_empty() {
        let target = [0.0_f32; 2];
        let mut cands: Vec<Candidate> = Vec::new();
        let out = robust_prune(&mut cands, &target, &[], 2, 1.2, 4);
        assert!(out.is_empty());
    }

    #[test]
    fn collinear_points_collapse_to_nearest() {
        // Four points along the x-axis at (1,0), (2,0), (3,0), (4,0); target
        // at the origin. The paper's prune is *supposed* to discard
        // near-collinear runs because each subsequent point sits closer to
        // the kept candidate than to the target. That diversity-favouring
        // behaviour is what bounds graph diameter, so we lock it in.
        let vs = flat(&[&[1.0, 0.0], &[2.0, 0.0], &[3.0, 0.0], &[4.0, 0.0]]);
        let target = [0.0_f32, 0.0];
        let mut cands: Vec<Candidate> = vec![(0, 1.0), (1, 4.0), (2, 9.0), (3, 16.0)];
        let out = robust_prune(&mut cands, &target, &vs, 2, 1.0, 4);
        // Only the nearest survives; the rest are dominated by id 0.
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn diverse_directions_all_survive() {
        // Four points at 90° apart on a unit circle. None of them dominates
        // another (each sits as far from its peers as from the origin), so
        // the prune keeps the whole set.
        let vs = flat(&[&[1.0, 0.0], &[0.0, 1.0], &[-1.0, 0.0], &[0.0, -1.0]]);
        let target = [0.0_f32, 0.0];
        let mut cands: Vec<Candidate> = vec![(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)];
        let out = robust_prune(&mut cands, &target, &vs, 2, 1.0, 4);
        let mut sorted = out.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3]);
    }

    #[test]
    fn alpha_relaxation_increases_diversity() {
        // Two collinear pairs at increasing radii. With strict alpha=1.0
        // RobustPrune collapses each pair to a single representative; with
        // a relaxed alpha a few extra candidates squeeze through.
        let vs = flat(&[&[1.0_f32, 0.0], &[2.0, 0.0], &[0.0, 1.0], &[0.0, 2.0]]);
        let target = [0.0_f32, 0.0];

        let mut cands_strict: Vec<Candidate> = vec![(0, 1.0), (1, 4.0), (2, 1.0), (3, 4.0)];
        let strict = robust_prune(&mut cands_strict, &target, &vs, 2, 1.0, 8);
        // Strict prune keeps one anchor per direction (ids 0 and 2).
        let mut sorted_strict = strict.clone();
        sorted_strict.sort();
        assert_eq!(sorted_strict, vec![0, 2]);

        let mut cands_relaxed: Vec<Candidate> = vec![(0, 1.0), (1, 4.0), (2, 1.0), (3, 4.0)];
        // alpha must exceed 2.0 to admit id 1 behind id 0:
        //   alpha^2 * d_sq(0, 1) = 4 alpha^2 > d_sq(target, 1) = 4
        //   ⇒ alpha > 1.0.
        // Use 2.5 to keep a clean margin.
        let relaxed = robust_prune(&mut cands_relaxed, &target, &vs, 2, 2.5, 8);
        let mut sorted_relaxed = relaxed.clone();
        sorted_relaxed.sort();
        assert_eq!(sorted_relaxed, vec![0, 1, 2, 3]);
    }

    #[test]
    fn respects_max_degree_cap() {
        // Spread the candidates so RobustPrune doesn't dominate them away;
        // place them along orthogonal-ish axes at increasing radii.
        let vs = flat(&[
            &[1.0, 0.0, 0.0, 0.0, 0.0],
            &[0.0, 2.0, 0.0, 0.0, 0.0],
            &[0.0, 0.0, 3.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0, 4.0, 0.0],
            &[0.0, 0.0, 0.0, 0.0, 5.0],
        ]);
        let target = [0.0_f32; 5];
        let mut cands: Vec<Candidate> = vec![(0, 1.0), (1, 4.0), (2, 9.0), (3, 16.0), (4, 25.0)];
        let out = robust_prune(&mut cands, &target, &vs, 5, 1.0, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], 0); // closest first
    }
}
