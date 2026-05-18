//! SIMD-friendly key comparison for JSONB object lookups.
//!
//! When a query repeatedly probes the same key (e.g. `data->>'name'`), the
//! hot path is byte-comparing the candidate key against every key in the
//! object. For short keys (≤ 16 bytes) we can do this in a single 128-bit
//! comparison using a hand-rolled portable implementation: copy both keys
//! into two 16-byte staging buffers (zero-padded) and compare with two
//! `u64` reads.
//!
//! This file deliberately avoids platform-specific SIMD intrinsics so the
//! kernel keeps building on every Rust target. The win versus the scalar
//! fallback is the elimination of the byte-by-byte loop and the branchless
//! padded compare — modern compilers vectorize these naturally on x86_64
//! (SSE2) and aarch64 (NEON).

/// Maximum key length eligible for the SIMD-optimized 16-byte path.
pub const SIMD_KEY_MAX: usize = 16;

/// Threshold for engaging SIMD: when an object has at least this many keys
/// AND the search key is short enough, we expect amortized wins.
pub const SIMD_THRESHOLD: usize = 4;

/// Compare two byte slices for equality using a 16-byte padded compare when
/// possible, falling back to scalar `==`. Returns `true` iff the slices are
/// byte-for-byte identical.
#[inline]
pub fn key_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if a.len() <= SIMD_KEY_MAX {
        eq_padded16(a, b)
    } else {
        a == b
    }
}

/// Returns `true` when SIMD comparison should be used for an object with the
/// given child count and search key length.
#[inline]
pub fn should_use_simd(child_count: usize, search_key_len: usize) -> bool {
    child_count >= SIMD_THRESHOLD && search_key_len <= SIMD_KEY_MAX
}

#[inline]
fn eq_padded16(a: &[u8], b: &[u8]) -> bool {
    debug_assert_eq!(a.len(), b.len());
    debug_assert!(a.len() <= SIMD_KEY_MAX);
    let mut buf_a = [0_u8; 16];
    let mut buf_b = [0_u8; 16];
    buf_a[..a.len()].copy_from_slice(a);
    buf_b[..b.len()].copy_from_slice(b);
    // Two u64 compares cover the whole 16-byte window in straight-line code.
    let a_lo = u64::from_le_bytes(buf_a[0..8].try_into().unwrap());
    let a_hi = u64::from_le_bytes(buf_a[8..16].try_into().unwrap());
    let b_lo = u64::from_le_bytes(buf_b[0..8].try_into().unwrap());
    let b_hi = u64::from_le_bytes(buf_b[8..16].try_into().unwrap());
    (a_lo ^ b_lo) | (a_hi ^ b_hi) == 0
}

/// Reference scalar comparison used for cross-checking and as a fallback for
/// long keys.
#[inline]
pub fn key_eq_scalar(a: &[u8], b: &[u8]) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_keys_match() {
        assert!(key_eq(b"", b""));
    }

    #[test]
    fn equal_short_keys() {
        assert!(key_eq(b"name", b"name"));
        assert!(key_eq(b"a", b"a"));
        assert!(key_eq(b"0123456789ABCDEF", b"0123456789ABCDEF"));
    }

    #[test]
    fn unequal_short_keys() {
        assert!(!key_eq(b"name", b"Name"));
        assert!(!key_eq(b"abc", b"abd"));
        assert!(!key_eq(b"a", b"b"));
    }

    #[test]
    fn length_mismatch() {
        assert!(!key_eq(b"abc", b"abcd"));
        assert!(!key_eq(b"abcd", b"abc"));
    }

    #[test]
    fn long_keys_use_scalar_fallback() {
        let a = b"this_is_a_long_key_over_sixteen_bytes";
        let b = b"this_is_a_long_key_over_sixteen_bytes";
        assert!(key_eq(a, b));
        let c = b"this_is_a_long_key_over_sixteen_bytey";
        assert!(!key_eq(a, c));
    }

    #[test]
    fn simd_engagement_threshold() {
        assert!(!should_use_simd(3, 4));
        assert!(should_use_simd(4, 4));
        assert!(should_use_simd(100, 16));
        assert!(!should_use_simd(100, 17));
    }
}
