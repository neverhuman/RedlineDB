//! Lane V3 — DiskANN structural / correctness tests.
//!
//! These tests cover the public `DiskAnnIndex` surface: empty inputs,
//! exact-nearest search on a tiny dataset, sector round-trip, and the
//! `beam_width → result quality` knob. The recall benchmark on 10k
//! synthetic vectors lives in `diskann_recall.rs` so this file stays fast.

use redlinedb_kernel::vector::diskann::{DiskAnnIndex, DiskAnnParams, RowId, SECTOR_SIZE};

fn synth_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed | 1;
    (0..n)
        .map(|_| {
            (0..dim)
                .map(|_| {
                    // Tiny LCG so the dataset is deterministic without a
                    // dev-dep on rand. Map to roughly N(0, 1) via two
                    // uniform draws + Box-Muller.
                    state = state
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let u1 = ((state >> 32) as u32 as f32) / (u32::MAX as f32);
                    state = state
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let u2 = ((state >> 32) as u32 as f32) / (u32::MAX as f32);
                    let r = (-2.0 * (u1.max(1e-10)).ln()).sqrt();
                    let theta = 2.0 * std::f32::consts::PI * u2;
                    r * theta.cos()
                })
                .collect()
        })
        .collect()
}

fn brute_force_top_k(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<u32> {
    let mut scored: Vec<(u32, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let d: f32 = v.iter().zip(query).map(|(a, b)| (a - b) * (a - b)).sum();
            (i as u32, d)
        })
        .collect();
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    scored.into_iter().take(k).map(|(i, _)| i).collect()
}

#[test]
fn empty_index_search_returns_empty() {
    let idx = DiskAnnIndex::build(4, &[], &[], DiskAnnParams::default()).unwrap();
    assert!(idx.is_empty());
    let hits = idx.search(&[0.0; 4], 5, 16).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn build_100_vectors_search_finds_nearest() {
    let dim = 8;
    let vectors = synth_vectors(100, dim, 0xCAFE);
    let row_ids: Vec<RowId> = (0..100).map(|i| RowId(i as u64 + 1000)).collect();
    let params = DiskAnnParams {
        max_degree: 16,
        search_list_size: 64,
        alpha: 1.2,
        seed: 1,
    };
    let idx = DiskAnnIndex::build(dim, &vectors, &row_ids, params).unwrap();
    assert_eq!(idx.len(), 100);

    // Use one of the dataset points as a query — its row id must come back
    // first (zero distance, so even a single hop suffices).
    let query = vectors[42].clone();
    let hits = idx.search(&query, 1, 32).unwrap();
    assert_eq!(hits, vec![row_ids[42]]);

    // For an arbitrary query, top-10 should overlap heavily with the brute
    // force top-10. We tolerate <= 2 misses on this small dataset.
    let q: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1).collect();
    let truth = brute_force_top_k(&vectors, &q, 10);
    let approx = idx.search(&q, 10, 64).unwrap();
    let truth_ids: std::collections::HashSet<RowId> =
        truth.iter().map(|&i| row_ids[i as usize]).collect();
    let overlap = approx.iter().filter(|h| truth_ids.contains(h)).count();
    assert!(
        overlap >= 8,
        "expected >=8/10 recall on small graph, got {overlap}"
    );
}

#[test]
fn beam_width_controls_result_quality() {
    // Build a moderately sized graph and compare recall under a small vs
    // large beam. Larger beam should never lose recall (the search list is
    // monotone in beam_width).
    let dim = 8;
    let n = 500;
    let vectors = synth_vectors(n, dim, 0xBABE);
    let row_ids: Vec<RowId> = (0..n).map(|i| RowId(i as u64)).collect();
    let params = DiskAnnParams {
        max_degree: 16,
        search_list_size: 64,
        alpha: 1.2,
        seed: 7,
    };
    let idx = DiskAnnIndex::build(dim, &vectors, &row_ids, params).unwrap();

    let queries: Vec<Vec<f32>> = synth_vectors(20, dim, 0xFACE);
    let mut narrow_hits = 0;
    let mut wide_hits = 0;
    let k = 10;
    for q in &queries {
        let truth = brute_force_top_k(&vectors, q, k);
        let truth_set: std::collections::HashSet<RowId> =
            truth.iter().map(|&i| row_ids[i as usize]).collect();
        let narrow = idx.search(q, k, k).unwrap();
        let wide = idx.search(q, k, 128).unwrap();
        narrow_hits += narrow.iter().filter(|h| truth_set.contains(h)).count();
        wide_hits += wide.iter().filter(|h| truth_set.contains(h)).count();
    }
    assert!(
        wide_hits >= narrow_hits,
        "wide-beam recall {wide_hits} should be >= narrow-beam recall {narrow_hits}"
    );
    // Sanity floor: even narrow beam should land most of the time.
    let total = queries.len() * k;
    assert!(
        narrow_hits * 100 / total >= 50,
        "narrow-beam recall too low: {narrow_hits}/{total}"
    );
}

#[test]
fn sector_round_trip_preserves_search() {
    // Build a graph, serialize to sectors, deserialize, and confirm search
    // returns the same results bit-for-bit. Locks in the on-disk layout
    // contract for the eventual mmap path.
    let dim = 16;
    let n = 80;
    let vectors = synth_vectors(n, dim, 0xDEAD);
    let row_ids: Vec<RowId> = (0..n).map(|i| RowId(i as u64 + 7)).collect();
    let params = DiskAnnParams {
        max_degree: 16,
        search_list_size: 64,
        alpha: 1.2,
        seed: 13,
    };
    let idx = DiskAnnIndex::build(dim, &vectors, &row_ids, params).unwrap();
    let blob = idx.to_sectors().unwrap();
    assert_eq!(blob.len(), n * SECTOR_SIZE);

    let restored = DiskAnnIndex::from_sectors(&blob, dim, params, idx.entry(), n).unwrap();
    assert_eq!(restored.len(), n);
    assert_eq!(restored.entry(), idx.entry());

    let q = vectors[3].clone();
    let a = idx.search(&q, 5, 32).unwrap();
    let b = restored.search(&q, 5, 32).unwrap();
    assert_eq!(a, b);
}

#[test]
fn rejects_dim_mismatch() {
    let dim = 4;
    let vectors = synth_vectors(10, dim, 1);
    let row_ids: Vec<RowId> = (0..10).map(|i| RowId(i as u64)).collect();
    let idx = DiskAnnIndex::build(dim, &vectors, &row_ids, DiskAnnParams::default()).unwrap();
    let err = idx.search(&[0.0; 8], 5, 16).unwrap_err();
    assert!(format!("{err}").contains("dim mismatch"));
}
