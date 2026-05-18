use std::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
///
/// # Safety
///
/// The caller must only invoke this on an AArch64 CPU where NEON is
/// available. The dispatcher in `simd.rs` relies on the fact that NEON is
/// part of the AArch64 base ISA.
pub(super) unsafe fn l2_distance_neon(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 4;
    let mut acc = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+4` lies inside both slices; `vld1q_f32`
        // permits any alignment; NEON is upheld by the outer
        // `#[target_feature(enable = "neon")]`.
        // SAFETY: bounded by loop guard `i + lanes <= len`; NEON gated by outer `#[target_feature]`.
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
///
/// # Safety
///
/// The caller must only invoke this on an AArch64 CPU where NEON is
/// available. The dispatcher in `simd.rs` relies on the fact that NEON is
/// part of the AArch64 base ISA.
pub(super) unsafe fn cosine_distance_neon(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 4;
    let mut dot = vdupq_n_f32(0.0);
    let mut na = vdupq_n_f32(0.0);
    let mut nb = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+4` lies inside both slices; `vld1q_f32`
        // permits any alignment; NEON is upheld by the outer
        // `#[target_feature(enable = "neon")]`.
        // SAFETY: bounded by loop guard `i + lanes <= len`; NEON gated by outer `#[target_feature]`.
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
///
/// # Safety
///
/// The caller must only invoke this on an AArch64 CPU where NEON is
/// available. The dispatcher in `simd.rs` relies on the fact that NEON is
/// part of the AArch64 base ISA.
pub(super) unsafe fn inner_product_neon(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let lanes = 4;
    let mut acc = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + lanes <= len {
        // Loop guard ensures `i..i+4` lies inside both slices; `vld1q_f32`
        // permits any alignment; NEON is upheld by the outer
        // `#[target_feature(enable = "neon")]`.
        // SAFETY: bounded by loop guard `i + lanes <= len`; NEON gated by outer `#[target_feature]`.
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
