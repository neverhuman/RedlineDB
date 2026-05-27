//! Variable-length byte storage for `Text`/`Blob` columns within a morsel.
//!
//! Backed by a caller-provided `bumpalo::Bump` via `bumpalo::collections::Vec`,
//! which supplies amortised-growth allocation while staying in safe Rust. Prior
//! to the W4 fix every `push()` allocated a fresh slab of `(used + len(bytes))`
//! bytes in the bump and copied all prior data into it — O(N²) in row count.
//! The current implementation doubles the slab capacity on growth, so N pushes
//! cost O(N) total.

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use smallvec::SmallVec;

#[derive(Debug)]
pub struct BytesArena<'a> {
    pub buf: &'a Bump,
    pub offsets: SmallVec<[u32; 256]>,
    data: BumpVec<'a, u8>,
}

impl<'a> BytesArena<'a> {
    pub fn new(buf: &'a Bump, capacity: usize) -> Self {
        let mut offsets: SmallVec<[u32; 256]> = SmallVec::with_capacity(capacity + 1);
        offsets.push(0u32);
        Self {
            buf,
            offsets,
            data: BumpVec::new_in(buf),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Append `bytes`. Amortised O(1) — `BumpVec::extend_from_slice` doubles
    /// the underlying allocation on overflow (standard `Vec` growth), so N
    /// pushes of bounded size cost O(N) total.
    pub fn push(&mut self, bytes: &[u8]) -> usize {
        let idx = self.len();
        debug_assert!(
            self.data.len() + bytes.len() <= u32::MAX as usize,
            "BytesArena overflow"
        );
        self.data.extend_from_slice(bytes);
        self.offsets.push(self.data.len() as u32);
        idx
    }

    /// Get the `i`-th byte slice. The returned reference borrows from `&self`
    /// rather than the bump's `'a` because the backing `BumpVec` may have
    /// reallocated during a previous `push`. Callers that need an `'a`-lived
    /// slice should copy the bytes or call `freeze` (TBD).
    pub fn get(&self, i: usize) -> Option<&[u8]> {
        if i >= self.len() {
            return None;
        }
        let start = self.offsets[i] as usize;
        let end = self.offsets[i + 1] as usize;
        Some(&self.data[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_get_roundtrip() {
        let bump = Bump::new();
        let mut a = BytesArena::new(&bump, 4);
        a.push(b"hello");
        a.push(b"world");
        a.push(b"");
        a.push(b"morsel");
        assert_eq!(a.len(), 4);
        assert_eq!(a.get(0), Some(&b"hello"[..]));
        assert_eq!(a.get(1), Some(&b"world"[..]));
        assert_eq!(a.get(2), Some(&b""[..]));
        assert_eq!(a.get(3), Some(&b"morsel"[..]));
        assert_eq!(a.get(4), None);
    }

    #[test]
    fn offsets_are_monotone() {
        let bump = Bump::new();
        let mut a = BytesArena::new(&bump, 0);
        for s in ["a", "bb", "ccc", "dddd"] {
            a.push(s.as_bytes());
        }
        for w in a.offsets.windows(2) {
            assert!(w[0] <= w[1]);
        }
        assert_eq!(*a.offsets.last().unwrap() as usize, a.data.len());
    }

    #[test]
    fn amortised_growth_handles_many_pushes() {
        // Pre-fix this would have been O(n²) — 4096 pushes × growing slab.
        // Post-fix BumpVec doubles, so the total work is O(n).
        let bump = Bump::new();
        let mut a = BytesArena::new(&bump, 0);
        for i in 0..4096 {
            let payload = format!("payload-{i:04}");
            a.push(payload.as_bytes());
        }
        assert_eq!(a.len(), 4096);
        assert_eq!(a.get(0), Some(b"payload-0000".as_slice()));
        assert_eq!(a.get(4095), Some(b"payload-4095".as_slice()));
        // Sanity: amortised growth keeps capacity within a small constant of
        // the live byte count (BumpVec doubles, so capacity ≤ 2 × len).
        assert!(a.data.capacity() <= a.data.len().next_power_of_two() * 2);
    }

    #[test]
    fn growth_preserves_existing_data() {
        // Force a growth event after a few small pushes, then verify all
        // previously-pushed slices still read back correctly.
        let bump = Bump::new();
        let mut a = BytesArena::new(&bump, 0);
        a.push(b"first");
        a.push(b"second");
        let big = vec![b'x'; 1024];
        a.push(&big);
        assert_eq!(a.get(0), Some(&b"first"[..]));
        assert_eq!(a.get(1), Some(&b"second"[..]));
        assert_eq!(a.get(2), Some(big.as_slice()));
    }
}
