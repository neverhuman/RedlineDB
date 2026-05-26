//! Vectorized i64 comparison kernels. Each `filter_i64_*` AND-combines
//! its truth-value vector into the caller-supplied `validity` bitmap so
//! chained predicates compose naturally:
//!
//! ```text
//! validity = all-ones(N)
//! filter_i64_ge(col_a, 10, &mut validity);  // validity &= (col_a >= 10)
//! filter_i64_lt(col_b, 20, &mut validity);  // validity &= (col_b <  20)
//! ```
//!
//! Runtime dispatch: on `x86_64` with AVX2 we use 4-lane `__m256i`
//! kernels; everything else falls back to scalar. The AVX2 kernels
//! produce bit-identical results to the scalar reference (a
//! differential test in `morsel_scan_filter` covers 1000 random inputs
//! per op).

use super::bitmap::Bitmap;

/// `validity &= col == target`.
#[inline]
pub fn filter_i64_eq(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_eq(col, target, validity);
}

/// `validity &= col != target`.
#[inline]
pub fn filter_i64_ne(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_ne(col, target, validity);
}

/// `validity &= col < target`.
#[inline]
pub fn filter_i64_lt(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_lt(col, target, validity);
}

/// `validity &= col <= target`.
#[inline]
pub fn filter_i64_le(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_le(col, target, validity);
}

/// `validity &= col > target`.
#[inline]
pub fn filter_i64_gt(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_gt(col, target, validity);
}

/// `validity &= col >= target`.
#[inline]
pub fn filter_i64_ge(col: &[i64], target: i64, validity: &mut Bitmap) {
    dispatch_ge(col, target, validity);
}

// ---------------------- dispatchers ----------------------

#[inline]
fn dispatch_eq(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_eq_avx2`.
            unsafe {
                filter_i64_eq_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_eq_scalar(col, target, validity);
}

#[inline]
fn dispatch_ne(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_ne_avx2`.
            unsafe {
                filter_i64_ne_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_ne_scalar(col, target, validity);
}

#[inline]
fn dispatch_lt(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_lt_avx2`.
            unsafe {
                filter_i64_lt_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_lt_scalar(col, target, validity);
}

#[inline]
fn dispatch_le(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_le_avx2`.
            unsafe {
                filter_i64_le_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_le_scalar(col, target, validity);
}

#[inline]
fn dispatch_gt(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_gt_avx2`.
            unsafe {
                filter_i64_gt_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_gt_scalar(col, target, validity);
}

#[inline]
fn dispatch_ge(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above, satisfying the `target_feature = "avx2"` precondition of `filter_i64_ge_avx2`.
            unsafe {
                filter_i64_ge_avx2(col, target, validity);
                return;
            }
        }
    }
    filter_i64_ge_scalar(col, target, validity);
}

// ---------------------- scalar reference ----------------------

pub(crate) fn filter_i64_eq_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && col[i] != target {
            validity.clear(i);
        }
    }
}

pub(crate) fn filter_i64_ne_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && col[i] == target {
            validity.clear(i);
        }
    }
}

pub(crate) fn filter_i64_lt_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && !(col[i] < target) {
            validity.clear(i);
        }
    }
}

pub(crate) fn filter_i64_le_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && !(col[i] <= target) {
            validity.clear(i);
        }
    }
}

pub(crate) fn filter_i64_gt_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && !(col[i] > target) {
            validity.clear(i);
        }
    }
}

pub(crate) fn filter_i64_ge_scalar(col: &[i64], target: i64, validity: &mut Bitmap) {
    let n = col.len().min(validity.bit_len());
    for i in 0..n {
        if validity.is_set(i) && !(col[i] >= target) {
            validity.clear(i);
        }
    }
}

// ---------------------- AVX2 kernels ----------------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `==` filter. Processes 4 i64 lanes per iteration.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
/// The dispatcher in this module enforces that via
/// `is_x86_feature_detected!("avx2")`.
unsafe fn filter_i64_eq_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + lanes <= n {
        // Loop guard ensures `i..i+4` lies inside `col`; `_mm256_loadu_si256`
        // permits any alignment; AVX2 upheld by the outer `#[target_feature]`.
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            _mm256_cmpeq_epi64(v, tgt)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && col[i] != target {
            validity.clear(i);
        }
        i += 1;
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `!=` filter. Processes 4 i64 lanes per iteration.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn filter_i64_ne_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let ones = _mm256_set1_epi64x(-1);
    let mut i = 0;
    while i + lanes <= n {
        // Loop guard ensures `i..i+4` lies inside `col`; AVX2 upheld by outer `#[target_feature]`.
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            let eq = _mm256_cmpeq_epi64(v, tgt);
            _mm256_xor_si256(eq, ones)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && col[i] == target {
            validity.clear(i);
        }
        i += 1;
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `<` filter via `_mm256_cmpgt_epi64(target, v)`. Processes 4 lanes.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn filter_i64_lt_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + lanes <= n {
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            // `col[i] < target`  <=>  `target > col[i]`.
            _mm256_cmpgt_epi64(tgt, v)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && !(col[i] < target) {
            validity.clear(i);
        }
        i += 1;
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `<=` filter: `!(col > target)`.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn filter_i64_le_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let ones = _mm256_set1_epi64x(-1);
    let mut i = 0;
    while i + lanes <= n {
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            let gt = _mm256_cmpgt_epi64(v, tgt);
            _mm256_xor_si256(gt, ones)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && !(col[i] <= target) {
            validity.clear(i);
        }
        i += 1;
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `>` filter.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn filter_i64_gt_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + lanes <= n {
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            _mm256_cmpgt_epi64(v, tgt)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && !(col[i] > target) {
            validity.clear(i);
        }
        i += 1;
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
/// AVX2 `>=` filter: `!(col < target)` => `!(target > col)`.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn filter_i64_ge_avx2(col: &[i64], target: i64, validity: &mut Bitmap) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let n = col.len().min(validity.bit_len());
    let lanes = 4;
    let tgt = _mm256_set1_epi64x(target);
    let ones = _mm256_set1_epi64x(-1);
    let mut i = 0;
    while i + lanes <= n {
        // SAFETY: bounded by loop guard `i + lanes <= n`; AVX2 gated by outer `#[target_feature]`.
        let mask = unsafe {
            let v = _mm256_loadu_si256(col.as_ptr().add(i) as *const __m256i);
            let lt = _mm256_cmpgt_epi64(tgt, v);
            _mm256_xor_si256(lt, ones)
        };
        // SAFETY: `apply_mask_4` is an AVX2-only `unsafe fn`; we are inside an `unsafe fn` annotated with `#[target_feature(enable = "avx2")]`, so AVX2 availability is upheld. The 4-lane base index satisfies `i + 4 <= n` (loop guard) with `n <= validity.bit_len()`.
        unsafe { apply_mask_4(mask, i, validity) };
        i += lanes;
    }
    while i < n {
        if validity.is_set(i) && !(col[i] >= target) {
            validity.clear(i);
        }
        i += 1;
    }
}

/// Apply a 4-lane `__m256i` boolean mask (lane is all-ones if kept,
/// all-zeros if rejected) to `validity` starting at row `base`. The
/// mask is extracted via `_mm256_movemask_epi8` and clears any
/// previously-set bit whose lane went zero.
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
/// `base + 4 <= validity.bit_len()` is required (callers enforce via
/// the kernel's `i + lanes <= n` loop guard).
#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn apply_mask_4(
    mask: std::arch::x86_64::__m256i,
    base: usize,
    validity: &mut Bitmap,
) {
    use std::arch::x86_64::_mm256_movemask_epi8;

    // movemask_epi8 turns each byte's MSB into one bit of an i32; each
    // 8-byte i64 lane contributes 8 identical bits. Lane k's truth
    // value lives at bit (k * 8).
    let bits = _mm256_movemask_epi8(mask) as u32;
    for lane in 0..4 {
        let kept = ((bits >> (lane * 8)) & 1) == 1;
        if !kept {
            validity.clear(base + lane);
        }
    }
}

#[cfg(target_arch = "x86")]
#[inline]
#[target_feature(enable = "avx2")]
/// 32-bit x86 mirror of [`apply_mask_4`].
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
unsafe fn apply_mask_4(
    mask: std::arch::x86::__m256i,
    base: usize,
    validity: &mut Bitmap,
) {
    use std::arch::x86::_mm256_movemask_epi8;
    let bits = _mm256_movemask_epi8(mask) as u32;
    for lane in 0..4 {
        let kept = ((bits >> (lane * 8)) & 1) == 1;
        if !kept {
            validity.clear(base + lane);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Light smoke tests; the heavy differential + edge coverage lives
    //! in `crates/sql/tests/morsel_scan_filter.rs` per the M3 spec.
    use super::*;

    #[test]
    fn scalar_eq_filters_correctly() {
        let col = [1i64, 2, 3, 2, 5];
        let mut v = Bitmap::new_all_set(col.len());
        filter_i64_eq_scalar(&col, 2, &mut v);
        assert!(!v.is_set(0));
        assert!(v.is_set(1));
        assert!(!v.is_set(2));
        assert!(v.is_set(3));
        assert!(!v.is_set(4));
        assert_eq!(v.count_ones(), 2);
    }

    #[test]
    fn scalar_ge_filters_correctly() {
        let col = [1i64, 2, 3, 4, 5];
        let mut v = Bitmap::new_all_set(col.len());
        filter_i64_ge_scalar(&col, 3, &mut v);
        assert_eq!(v.count_ones(), 3);
        assert!(!v.is_set(0));
        assert!(!v.is_set(1));
        assert!(v.is_set(2));
        assert!(v.is_set(3));
        assert!(v.is_set(4));
    }
}
