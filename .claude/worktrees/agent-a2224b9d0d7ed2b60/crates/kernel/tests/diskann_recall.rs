//! Lane V3 — recall@10 benchmark for DiskANN on a 10k-vector synthetic
//! dataset.
//!
//! The proof bar requires recall@10 >= 0.92 with R=64, alpha=1.2, beam=64.
//! Marked `#[ignore]` so the default `cargo test` stays fast; CI / the
//! recall lane invokes it explicitly via `--include-ignored`.

use redlinedb_kernel::vector::diskann::{DiskAnnIndex, DiskAnnParams, RowId};

const N_VECTORS: usize = 10_000;
const N_QUERIES: usize = 200;
const DIM: usize = 32;
const K: usize = 10;
const BEAM_WIDTH: usize = 64;
const RECALL_BAR: f64 = 0.92;

/// Box-Muller + LCG dataset generator. Stays seeded so failures are
/// reproducible and the benchmark doesn't pull in `rand`.
fn gauss_dataset(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = Vec::with_capacity(dim);
        let mut i = 0;
        while i < dim {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u1 = (((state >> 32) as u32) as f32) / (u32::MAX as f32);
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u2 = (((state >> 32) as u32) as f32) / (u32::MAX as f32);
            // Box-Muller draws two independent N(0, 1) samples per loop.
            let r = (-2.0_f32 * u1.max(1e-10).ln()).sqrt();
            let theta = 2.0 * std::f32::consts::PI * u2;
            v.push(r * theta.cos());
            i += 1;
            if i < dim {
                v.push(r * theta.sin());
                i += 1;
            }
        }
        out.push(v);
    }
    out
}

fn brute_force_top_k(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<u32> {
    let mut scored: Vec<(u32, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut d = 0.0_f32;
            for j in 0..v.len() {
                let diff = v[j] - query[j];
                d += diff * diff;
            }
            (i as u32, d)
        })
        .collect();
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    scored.into_iter().take(k).map(|(i, _)| i).collect()
}

#[test]
#[ignore = "10k-vector recall benchmark; run with --include-ignored"]
fn recall_at_10_meets_bar() {
    let dataset = gauss_dataset(N_VECTORS, DIM, 0x5EED_C0DE);
    let queries = gauss_dataset(N_QUERIES, DIM, 0xDEAD_BEEF_5EED);
    let row_ids: Vec<RowId> = (0..N_VECTORS).map(|i| RowId(i as u64)).collect();

    let params = DiskAnnParams {
        max_degree: 64,
        search_list_size: 100,
        alpha: 1.2,
        seed: 0xBA5E_BA11,
    };
    let build_started = std::time::Instant::now();
    let idx = DiskAnnIndex::build(DIM, &dataset, &row_ids, params).unwrap();
    let build_elapsed = build_started.elapsed();
    eprintln!(
        "diskann: built {N_VECTORS} vectors (R=64, L=100, alpha=1.2) in {:.2?}",
        build_elapsed
    );

    let mut total_hits = 0usize;
    let mut total_truth = 0usize;
    for q in &queries {
        let truth = brute_force_top_k(&dataset, q, K);
        let truth_ids: std::collections::HashSet<RowId> =
            truth.iter().map(|&i| row_ids[i as usize]).collect();
        let approx = idx.search(q, K, BEAM_WIDTH).unwrap();
        total_hits += approx.iter().filter(|h| truth_ids.contains(h)).count();
        total_truth += K;
    }
    let recall = total_hits as f64 / total_truth as f64;
    eprintln!(
        "diskann: recall@{K} = {recall:.4} (hits={total_hits}/{total_truth}, n={N_VECTORS}, q={N_QUERIES}, beam={BEAM_WIDTH})"
    );
    assert!(
        recall >= RECALL_BAR,
        "recall@{K} {recall:.4} did not meet bar {RECALL_BAR:.2}"
    );
}
