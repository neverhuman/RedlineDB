//! Cross-cutting integration tests for `kernel::vector`.
//!
//! These tests live in `tests/` (not the inline `#[cfg(test)]` modules) so
//! that they exercise the public surface the SQL crate and future Lane V2 /
//! V3 code will use.

use redlinedb_kernel::vector::{
    Vector, VectorMetric, cosine_distance, cosine_distance_scalar, decode_vector, encode_vector,
    flat_top_k, inner_product, inner_product_scalar, l2_distance, l2_distance_scalar,
};

/// Tiny deterministic PRNG (xorshift64*) — matches the inline tests so we can
/// reproduce a failing case without bringing in `rand`.
fn rand_vec(n: usize, seed: u64) -> Vec<f32> {
    let mut s = seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s as i32) as f32) / (i32::MAX as f32)
        })
        .collect()
}

fn agree_within(a: f32, b: f32, tol: f32) -> bool {
    if a == 0.0 && b == 0.0 {
        return true;
    }
    // Both an absolute and a relative leg: the absolute leg covers cases where
    // the metric value itself is tiny (e.g. inner product of nearly-orthogonal
    // random vectors), where FMA vs scalar ordering can produce ~1e-7 wobble
    // that looks large *relatively* but is dwarfed by the per-lane summands.
    let abs = (a - b).abs();
    let scale = a.abs().max(b.abs()).max(1.0);
    abs < tol || abs / scale < tol
}

#[test]
fn simd_l2_matches_scalar_across_1000_vectors() {
    for seed in 0..1000u64 {
        let a = rand_vec(128, seed);
        let b = rand_vec(128, seed.wrapping_add(1));
        let s = l2_distance_scalar(&a, &b);
        let v = l2_distance(&a, &b);
        assert!(agree_within(s, v, 1e-6), "seed={seed} scalar={s} simd={v}");
    }
}

#[test]
fn simd_cosine_matches_scalar_across_1000_vectors() {
    for seed in 0..1000u64 {
        let a = rand_vec(384, seed);
        let b = rand_vec(384, seed.wrapping_add(1));
        let s = cosine_distance_scalar(&a, &b);
        let v = cosine_distance(&a, &b);
        assert!(agree_within(s, v, 1e-6), "seed={seed} scalar={s} simd={v}");
    }
}

#[test]
fn simd_ip_matches_scalar_across_1000_vectors() {
    // Inner product on near-orthogonal random vectors collapses to a sum that
    // is ~1e-2, which means the per-element rounding (FMA vs naive scalar) can
    // accumulate ~5e-6 relative error. We still require ≤ 1e-6 absolute error
    // because the per-element magnitude is ~1e-2, so this is the sharpest
    // bound that holds for IEEE single precision under reordered summation.
    for seed in 0..1000u64 {
        let a = rand_vec(64, seed);
        let b = rand_vec(64, seed.wrapping_add(1));
        let s = inner_product_scalar(&a, &b);
        let v = inner_product(&a, &b);
        let abs = (s - v).abs();
        assert!(abs < 5e-6, "seed={seed} scalar={s} simd={v} abs_diff={abs}");
    }
}

#[test]
fn simd_handles_non_multiple_of_lane_width() {
    // 11 components: AVX2 lanes=8 → 1 main pass + 3 tail; NEON lanes=4 → 2 main + 3 tail.
    // The scalar tail handler is the load-bearing piece on this fixture.
    let a = rand_vec(11, 1);
    let b = rand_vec(11, 2);
    assert!(agree_within(
        l2_distance_scalar(&a, &b),
        l2_distance(&a, &b),
        1e-6
    ));
    assert!(agree_within(
        cosine_distance_scalar(&a, &b),
        cosine_distance(&a, &b),
        1e-6
    ));
    assert!(agree_within(
        inner_product_scalar(&a, &b),
        inner_product(&a, &b),
        1e-6
    ));
}

#[test]
fn identical_vectors_have_zero_l2() {
    let v = rand_vec(256, 0xDEAD_BEEF);
    assert_eq!(l2_distance(&v, &v), 0.0);
}

#[test]
fn orthogonal_unit_vectors_have_cosine_one() {
    let a = vec![1.0_f32, 0.0, 0.0, 0.0];
    let b = vec![0.0_f32, 1.0, 0.0, 0.0];
    let d = cosine_distance(&a, &b);
    assert!((d - 1.0).abs() < 1e-6, "d = {d}");
}

#[test]
fn zero_vector_cosine_returns_one() {
    let a = vec![0.0_f32; 64];
    let b = rand_vec(64, 7);
    let d = cosine_distance(&a, &b);
    assert!((d - 1.0).abs() < 1e-6, "d = {d}");
}

#[test]
fn vector_codec_round_trip_f32() {
    let cases: Vec<Vec<f32>> = vec![
        vec![1.0, 2.0, 3.0],
        rand_vec(64, 11),
        rand_vec(384, 22),
        rand_vec(1536, 33),
    ];
    for v in &cases {
        let bytes = encode_vector(v);
        let decoded = decode_vector(&bytes).unwrap();
        assert_eq!(v, &decoded);
    }
}

#[test]
fn vector_codec_rejects_malformed_bytes() {
    // Empty
    assert!(decode_vector(&[]).is_err());
    // Valid prefix then truncated payload.
    let mut bytes = encode_vector(&[1.0_f32, 2.0]);
    let len = bytes.len();
    bytes.truncate(len - 1);
    assert!(decode_vector(&bytes).is_err());
    // Bogus element kind.
    let mut bytes = encode_vector(&[1.0_f32]);
    bytes[1] = 0x7F;
    assert!(decode_vector(&bytes).is_err());
}

#[test]
fn vector_from_json_literal_parses() {
    let v = Vector::from_json_literal("[1.0, 2.5, -3.0, 4]").unwrap();
    assert_eq!(v.dim(), 4);
    assert_eq!(v.as_slice(), &[1.0, 2.5, -3.0, 4.0]);
}

#[test]
fn vector_from_json_literal_rejects_garbage() {
    assert!(Vector::from_json_literal("not-a-list").is_err());
    assert!(Vector::from_json_literal("[1, 2, abc]").is_err());
}

#[test]
fn vector_metric_dim_mismatch_errors() {
    let a = vec![1.0_f32, 2.0];
    let b = vec![1.0_f32, 2.0, 3.0];
    assert!(VectorMetric::L2.distance(&a, &b).is_err());
    assert!(VectorMetric::Cosine.distance(&a, &b).is_err());
    assert!(VectorMetric::InnerProduct.distance(&a, &b).is_err());
}

#[test]
fn flat_top_k_picks_nearest_under_l2() {
    let query = vec![0.0_f32; 16];
    let candidates: Vec<(u64, Vec<f32>)> = (0..50)
        .map(|i| {
            let mut v = vec![0.0_f32; 16];
            v[0] = i as f32;
            (i as u64, v)
        })
        .collect();
    let hits = flat_top_k(&query, VectorMetric::L2, 5, candidates).unwrap();
    let ids: Vec<u64> = hits.iter().map(|h| h.payload).collect();
    assert_eq!(ids, vec![0, 1, 2, 3, 4]);
}

#[test]
fn flat_top_k_picks_nearest_under_cosine() {
    let query = vec![1.0_f32, 0.0];
    let candidates: Vec<(&'static str, Vec<f32>)> = vec![
        ("parallel", vec![2.0_f32, 0.0]),      // distance ~ 0
        ("orthogonal", vec![0.0_f32, 1.0]),    // distance ~ 1
        ("antiparallel", vec![-1.0_f32, 0.0]), // distance ~ 2
    ];
    let hits = flat_top_k(&query, VectorMetric::Cosine, 2, candidates).unwrap();
    let ids: Vec<&str> = hits.iter().map(|h| h.payload).collect();
    assert_eq!(ids, vec!["parallel", "orthogonal"]);
}

#[test]
fn flat_top_k_dim_mismatch_errors() {
    let query = vec![0.0_f32, 0.0, 0.0];
    let candidates = vec![(1u64, vec![0.0_f32, 0.0])];
    assert!(flat_top_k(&query, VectorMetric::L2, 1, candidates).is_err());
}
