#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "x86")]
type Avx2Vec = std::arch::x86::__m256;
#[cfg(target_arch = "x86_64")]
type Avx2Vec = std::arch::x86_64::__m256;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 and FMA are
/// available. The dispatcher in `simd.rs` enforces that by checking
/// `is_x86_feature_detected!("avx2")` first.
pub(super) unsafe fn l2_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 8;
    let mut acc = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+8` lies inside both slices; `_mm256_loadu_ps`
        // permits any alignment; AVX2 + FMA are upheld by the outer
        // `#[target_feature(enable = "avx2,fma")]` on this function.
        // SAFETY: bounded by loop guard `i + lanes <= len`; AVX2+FMA gated by outer `#[target_feature]`.
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
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 and FMA are
/// available. The dispatcher in `simd.rs` enforces that by checking
/// `is_x86_feature_detected!("avx2")` first.
pub(super) unsafe fn cosine_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 8;
    let mut dot = _mm256_setzero_ps();
    let mut na = _mm256_setzero_ps();
    let mut nb = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+8` lies inside both slices; `_mm256_loadu_ps`
        // permits any alignment; AVX2 + FMA are upheld by the outer
        // `#[target_feature(enable = "avx2,fma")]`.
        // SAFETY: bounded by loop guard `i + lanes <= len`; AVX2+FMA gated by outer `#[target_feature]`.
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
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 and FMA are
/// available. The dispatcher in `simd.rs` enforces that by checking
/// `is_x86_feature_detected!("avx2")` first.
pub(super) unsafe fn inner_product_avx2(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 8;
    let mut acc = _mm256_setzero_ps();
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+8` lies inside both slices; `_mm256_loadu_ps`
        // permits any alignment; AVX2 + FMA are upheld by the outer
        // `#[target_feature(enable = "avx2,fma")]`.
        // SAFETY: bounded by loop guard `i + lanes <= len`; AVX2+FMA gated by outer `#[target_feature]`.
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
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
/// The only caller is the AVX2 dispatcher path in `simd.rs`.
pub(super) unsafe fn horizontal_sum_avx2(v: Avx2Vec) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
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
