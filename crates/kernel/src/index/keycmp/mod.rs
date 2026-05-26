//! Runtime-dispatched key-prefix compare. Phase 5 WS-B3a.
//!
//! AVX2 path finds the first mismatch byte 32 bytes at a time;
//! scalar fallback is byte-identical to `<[u8]>::cmp` for non-x86 /
//! non-AVX2.

use std::cmp::Ordering;

/// Lexicographic byte-slice compare; byte-identical to `<[u8]>::cmp`.
///
/// On `x86_64` with AVX2 detected at runtime, dispatches to a 32-byte
/// SIMD prefix scan. Otherwise falls through to `slice::cmp`.
#[inline]
pub fn cmp_keys(a: &[u8], b: &[u8]) -> Ordering {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: `is_x86_feature_detected!("avx2")` returned true above,
            // satisfying the `target_feature = "avx2"` precondition of
            // `cmp_keys_avx2`.
            unsafe {
                return cmp_keys_avx2(a, b);
            }
        }
    }
    a.cmp(b)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
///
/// # Safety
///
/// The caller must only invoke this on a CPU where AVX2 is available.
/// The dispatcher in [`cmp_keys`] enforces that by checking
/// `is_x86_feature_detected!("avx2")` first.
unsafe fn cmp_keys_avx2(a: &[u8], b: &[u8]) -> Ordering {
    use std::arch::x86_64::{__m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8};

    let common = a.len().min(b.len());
    let mut i = 0;
    while i + 32 <= common {
        // SAFETY: loop guard `i + 32 <= common` bounds the 32-byte unaligned
        // loads to `a[i..i+32]` and `b[i..i+32]`; `_mm256_loadu_si256` accepts
        // any alignment; AVX2 upheld by the outer `#[target_feature(enable = "avx2")]`.
        let mask = unsafe {
            let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
            _mm256_movemask_epi8(_mm256_cmpeq_epi8(va, vb)) as u32
        };
        if mask != 0xFFFF_FFFF {
            let mismatch_off = i + mask.trailing_ones() as usize;
            return a[mismatch_off].cmp(&b[mismatch_off]);
        }
        i += 32;
    }
    while i < common {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => i += 1,
            other => return other,
        }
    }
    a.len().cmp(&b.len())
}
