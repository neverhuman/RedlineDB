//! Scalar distance kernels.
//!
//! These functions are the correctness oracle for the SIMD implementations in
//! `simd.rs`. They are deliberately written in the simplest, easiest-to-audit
//! form. Performance-sensitive callers should go through [`super::simd`].
//!
//! All kernels assume the input slices have equal, non-zero length; the public
//! entry points in `super` already enforce that contract.

/// Alias matching the name used by the HNSW lane (V2). The canonical type is
/// [`super::VectorMetric`].
pub type Metric = super::VectorMetric;

/// Squared L2 distance: `Σ (a_i - b_i)^2`.
///
/// We intentionally do **not** take the square root: top-K ordering is
/// preserved under the squared metric and skipping `sqrt` is a measurable win
/// in tight loops (and matches pgvector's `<->` operator implementation).
#[inline]
pub fn l2_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0.0_f32;
    for i in 0..a.len() {
        let d = a[i] - b[i];
        acc += d * d;
    }
    acc
}

/// Cosine distance: `1 - (a·b) / (||a|| · ||b||)`.
///
/// If either vector has zero norm the function returns `1.0` (orthogonal /
/// undefined). Callers that care about the difference (e.g. validation) can
/// inspect the input themselves.
#[inline]
pub fn cosine_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = (na.sqrt()) * (nb.sqrt());
    if denom == 0.0 {
        return 1.0;
    }
    1.0 - dot / denom
}

/// Negative inner product (smaller is "closer", matches pgvector `<#>`).
#[inline]
pub fn inner_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0.0_f32;
    for i in 0..a.len() {
        acc += a[i] * b[i];
    }
    -acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_zero_for_identical() {
        let v = vec![1.0_f32, 2.0, 3.0];
        assert_eq!(l2_distance_scalar(&v, &v), 0.0);
    }

    #[test]
    fn cosine_one_for_orthogonal() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let d = cosine_distance_scalar(&a, &b);
        assert!((d - 1.0).abs() < 1e-6, "d = {d}");
    }

    #[test]
    fn cosine_zero_for_parallel() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![2.0_f32, 4.0, 6.0];
        let d = cosine_distance_scalar(&a, &b);
        assert!(d.abs() < 1e-6, "d = {d}");
    }

    #[test]
    fn cosine_one_for_zero_vector() {
        let a = vec![0.0_f32; 4];
        let b = vec![1.0_f32, 2.0, 3.0, 4.0];
        assert_eq!(cosine_distance_scalar(&a, &b), 1.0);
    }

    #[test]
    fn inner_product_negates_dot() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        // 1*4 + 2*5 + 3*6 = 32; negative ⇒ -32
        let d = inner_product_scalar(&a, &b);
        assert!((d + 32.0).abs() < 1e-6, "d = {d}");
    }
}
