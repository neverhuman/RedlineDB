//! SIMD-accelerated distance kernels with run-time CPU dispatch.
//!
//! Public functions ([`l2_distance`], [`cosine_distance`], [`inner_product`])
//! pick the best implementation available for the host:
//!
//! * `x86_64` with AVX2 at runtime → 8-wide AVX2 + FMA path.
//! * `aarch64` (NEON is mandatory in the AArch64 base ISA) → 4-wide NEON path.
//! * Everything else → scalar fallback from [`super::distance`].
//!
//! Run-time dispatch is deliberately resolved on every call. For phase-10
//! Lane V1 the call sites are coarse (one call per scanned row), so the cost
//! of `is_x86_feature_detected!` (~a single TLS load after the first call) is
//! negligible. If profiling shows otherwise we can switch to the function
//! pointer cache pattern, but doing so today would be premature optimisation.
//!
//! Lane V2 (HNSW) and Lane V3 (DiskANN) will reuse these entry points
//! verbatim — keep the signatures stable.
//!
//! # Unsafety policy
//!
//! Each `target_feature`-gated kernel is `unsafe fn` because the safety
//! invariant is "the named feature is available on this CPU". The dispatcher
//! upholds that invariant via `is_x86_feature_detected!` (x86) or the
//! `target_arch` gate (NEON is in the AArch64 base ISA). Inside each kernel,
//! Rust 2024 edition still demands per-call `unsafe { … }` blocks for any
//! operation flagged unsafe (notably the unaligned loads `_mm256_loadu_ps`
//! and `vld1q_f32`).

use super::distance::{cosine_distance_scalar, inner_product_scalar, l2_distance_scalar};

/// Squared L2 distance with SIMD dispatch.
#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 confirmed at runtime.
            unsafe {
                return l2_distance_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is in the AArch64 base ISA.
        unsafe {
            return l2_distance_neon(a, b);
        }
    }
    #[cfg_attr(target_arch = "aarch64", allow(unreachable_code))]
    l2_distance_scalar(a, b)
}

/// Cosine distance with SIMD dispatch.
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 confirmed at runtime.
            unsafe {
                return cosine_distance_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is in the AArch64 base ISA.
        unsafe {
            return cosine_distance_neon(a, b);
        }
    }
    #[cfg_attr(target_arch = "aarch64", allow(unreachable_code))]
    cosine_distance_scalar(a, b)
}

/// Negative inner product with SIMD dispatch.
#[inline]
pub fn inner_product(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 confirmed at runtime.
            unsafe {
                return inner_product_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is in the AArch64 base ISA.
        unsafe {
            return inner_product_neon(a, b);
        }
    }
    #[cfg_attr(target_arch = "aarch64", allow(unreachable_code))]
    inner_product_scalar(a, b)
}

// -----------------------------------------------------------------------------
// AVX2 (x86 / x86_64)
// -----------------------------------------------------------------------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn l2_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let len = a.len();
    let lanes = 8;
    let mut acc = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded by the loop; AVX2 + FMA enabled by feature.
        unsafe {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            let d = _mm256_sub_ps(va, vb);
            acc = _mm256_fmadd_ps(d, d, acc);
        }
        i += lanes;
    }
    let mut tail = horizontal_sum_avx2(acc);
    while i < len {
        let d = a[i] - b[i];
        tail += d * d;
        i += 1;
    }
    tail
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn cosine_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let len = a.len();
    let lanes = 8;
    let mut dot = _mm256_setzero_ps();
    let mut na = _mm256_setzero_ps();
    let mut nb = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded; AVX2 + FMA enabled.
        unsafe {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            dot = _mm256_fmadd_ps(va, vb, dot);
            na = _mm256_fmadd_ps(va, va, na);
            nb = _mm256_fmadd_ps(vb, vb, nb);
        }
        i += lanes;
    }
    let mut dot_s = horizontal_sum_avx2(dot);
    let mut na_s = horizontal_sum_avx2(na);
    let mut nb_s = horizontal_sum_avx2(nb);
    while i < len {
        dot_s += a[i] * b[i];
        na_s += a[i] * a[i];
        nb_s += b[i] * b[i];
        i += 1;
    }
    let denom = na_s.sqrt() * nb_s.sqrt();
    if denom == 0.0 {
        return 1.0;
    }
    1.0 - dot_s / denom
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn inner_product_avx2(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let len = a.len();
    let lanes = 8;
    let mut acc = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded; AVX2 + FMA enabled.
        unsafe {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            acc = _mm256_fmadd_ps(va, vb, acc);
        }
        i += lanes;
    }
    let mut tail = horizontal_sum_avx2(acc);
    while i < len {
        tail += a[i] * b[i];
        i += 1;
    }
    -tail
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_avx2(v: std::arch::x86_64::__m256) -> f32 {
    use std::arch::x86_64::*;
    let lo = _mm256_castps256_ps128(v);
    let hi = _mm256_extractf128_ps(v, 1);
    let s = _mm_add_ps(lo, hi);
    let shuf = _mm_movehdup_ps(s);
    let sums = _mm_add_ps(s, shuf);
    let shuf2 = _mm_movehl_ps(shuf, sums);
    let sums2 = _mm_add_ss(sums, shuf2);
    _mm_cvtss_f32(sums2)
}

// -----------------------------------------------------------------------------
// NEON (aarch64)
// -----------------------------------------------------------------------------

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn l2_distance_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let len = a.len();
    let lanes = 4;
    let mut acc = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded by the loop; NEON in base ISA.
        unsafe {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            let d = vsubq_f32(va, vb);
            acc = vfmaq_f32(acc, d, d);
        }
        i += lanes;
    }
    let mut tail = vaddvq_f32(acc);
    while i < len {
        let d = a[i] - b[i];
        tail += d * d;
        i += 1;
    }
    tail
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn cosine_distance_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let len = a.len();
    let lanes = 4;
    let mut dot = vdupq_n_f32(0.0);
    let mut na = vdupq_n_f32(0.0);
    let mut nb = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded; NEON in base ISA.
        unsafe {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            dot = vfmaq_f32(dot, va, vb);
            na = vfmaq_f32(na, va, va);
            nb = vfmaq_f32(nb, vb, vb);
        }
        i += lanes;
    }
    let (mut dot_s, mut na_s, mut nb_s) = (vaddvq_f32(dot), vaddvq_f32(na), vaddvq_f32(nb));
    while i < len {
        dot_s += a[i] * b[i];
        na_s += a[i] * a[i];
        nb_s += b[i] * b[i];
        i += 1;
    }
    let denom = na_s.sqrt() * nb_s.sqrt();
    if denom == 0.0 {
        return 1.0;
    }
    1.0 - dot_s / denom
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn inner_product_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let len = a.len();
    let lanes = 4;
    let mut acc = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // SAFETY: index bounded; NEON in base ISA.
        unsafe {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            acc = vfmaq_f32(acc, va, vb);
        }
        i += lanes;
    }
    let mut tail = vaddvq_f32(acc);
    while i < len {
        tail += a[i] * b[i];
        i += 1;
    }
    -tail
}

#[cfg(test)]
mod tests {
    use super::super::distance::{
        cosine_distance_scalar, inner_product_scalar, l2_distance_scalar,
    };
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
}
