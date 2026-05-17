//! SIMD-accelerated distance kernels with run-time CPU dispatch.
//!
//! Public functions ([`l2_distance`], [`cosine_distance`], [`inner_product`])
//! pick the best implementation available for the host:
//!
//! * `x86_64` with AVX2 at runtime -> 8-wide AVX2 + FMA path.
//! * `aarch64` (NEON is mandatory in the AArch64 base ISA) -> 4-wide NEON path.
//! * Everything else -> scalar implementation from [`super::distance`].
//!
//! The AVX2 and NEON kernels live in `simd/avx2.rs` and `simd/neon.rs`; this
//! module keeps dispatch and the stable public API in one place.

use super::distance::{cosine_distance_scalar, inner_product_scalar, l2_distance_scalar};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[path = "simd/avx2.rs"]
mod avx2;
#[cfg(target_arch = "aarch64")]
#[path = "simd/neon.rs"]
mod neon;
#[cfg(test)]
#[path = "simd/tests.rs"]
mod tests;

/// Squared L2 distance with SIMD dispatch.
#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // Runtime CPU dispatch upheld AVX2; bounds checked by inner loop;
            // inputs are equal-length slices (debug_asserted above).
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2,fma"` precondition of `l2_distance_avx2`.
            unsafe {
                return avx2::l2_distance_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is mandatory in the AArch64 base ISA; bounds checked by inner
        // loop; inputs are equal-length slices (debug_asserted above).
        // SAFETY: AArch64 base ISA includes NEON, satisfying the `target_feature = "neon"` precondition of `neon::l2_distance_neon`.
        unsafe {
            return neon::l2_distance_neon(a, b);
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
            // Runtime CPU dispatch upheld AVX2; bounds checked by inner loop;
            // inputs are equal-length slices (debug_asserted above).
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2,fma"` precondition of `cosine_distance_avx2`.
            unsafe {
                return avx2::cosine_distance_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is mandatory in the AArch64 base ISA; inputs are equal-length
        // slices (debug_asserted above).
        // SAFETY: AArch64 base ISA includes NEON, satisfying the `target_feature = "neon"` precondition of `neon::cosine_distance_neon`.
        unsafe {
            return neon::cosine_distance_neon(a, b);
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
            // Runtime CPU dispatch upheld AVX2; inputs are equal-length
            // slices (debug_asserted above).
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2,fma"` precondition of `inner_product_avx2`.
            unsafe {
                return avx2::inner_product_avx2(a, b);
            }
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is mandatory in the AArch64 base ISA; inputs are equal-length
        // slices (debug_asserted above).
        // SAFETY: AArch64 base ISA includes NEON, satisfying the `target_feature = "neon"` precondition of `neon::inner_product_neon`.
        unsafe {
            return neon::inner_product_neon(a, b);
        }
    }
    #[cfg_attr(target_arch = "aarch64", allow(unreachable_code))]
    inner_product_scalar(a, b)
}
