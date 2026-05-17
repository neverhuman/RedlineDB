use super::super::distance::{cosine_distance_scalar, inner_product_scalar, l2_distance_scalar};
use super::*;

fn agree(a: f32, b: f32) -> bool {
    // Squared-norm accumulators on a 1536-d random vector reach ~5e2.
    // We accept agreement on the relative leg OR the absolute leg: FMA vs
    // scalar order-of-summation can produce ~1e-7 wobble on near-zero
    // metric values (notably inner product of near-orthogonal vectors).
    if a == 0.0 && b == 0.0 {
        return true;
    }
    let abs = (a - b).abs();
    let scale = a.abs().max(b.abs()).max(1.0);
    abs < 1e-6 || abs / scale < 1e-6
}

fn rand_vec(n: usize, seed: u64) -> Vec<f32> {
    // Tiny xorshift so we don't pull in `rand`. Matches the kernel's
    // existing zero-dependency posture.
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

#[test]
fn simd_matches_scalar_l2_various_dims() {
    for &dim in &[8usize, 64, 128, 512, 1536] {
        let a = rand_vec(dim, 0xA);
        let b = rand_vec(dim, 0xB);
        let s = l2_distance_scalar(&a, &b);
        let v = l2_distance(&a, &b);
        assert!(agree(s, v), "dim={dim} scalar={s} simd={v}");
    }
}

#[test]
fn simd_matches_scalar_cosine_various_dims() {
    for &dim in &[8usize, 64, 128, 512, 1536] {
        let a = rand_vec(dim, 0xC);
        let b = rand_vec(dim, 0xD);
        let s = cosine_distance_scalar(&a, &b);
        let v = cosine_distance(&a, &b);
        assert!(agree(s, v), "dim={dim} scalar={s} simd={v}");
    }
}

#[test]
fn simd_matches_scalar_ip_various_dims() {
    for &dim in &[8usize, 64, 128, 512, 1536] {
        let a = rand_vec(dim, 0xE);
        let b = rand_vec(dim, 0xF);
        let s = inner_product_scalar(&a, &b);
        let v = inner_product(&a, &b);
        assert!(agree(s, v), "dim={dim} scalar={s} simd={v}");
    }
}
