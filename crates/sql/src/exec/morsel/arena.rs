//! Variable-length byte storage for `Text`/`Blob` columns within a morsel.
//!
//! Backed by a caller-provided `bumpalo::Bump` so the entire batch can be
//! freed in one shot when the morsel is dropped at the operator boundary.

use bumpalo::Bump;
use smallvec::SmallVec;

#[derive(Debug)]
pub struct BytesArena<'a> {
    pub buf: &'a Bump,
    pub offsets: SmallVec<[u32; 256]>,
    pub data: &'a [u8],
}

impl<'a> BytesArena<'a> {
    pub fn new(buf: &'a Bump, capacity: usize) -> Self {
        let mut offsets: SmallVec<[u32; 256]> = SmallVec::with_capacity(capacity + 1);
        offsets.push(0u32);
        Self {
            buf,
            offsets,
            data: &[],
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

    pub fn push(&mut self, bytes: &[u8]) -> usize {
        let idx = self.len();
        let new_len = self.data.len() + bytes.len();
        debug_assert!(new_len <= u32::MAX as usize, "BytesArena overflow");

        let combined = self.buf.alloc_slice_fill_copy(new_len, 0u8);
        combined[..self.data.len()].copy_from_slice(self.data);
        combined[self.data.len()..].copy_from_slice(bytes);
        self.data = combined;

        self.offsets.push(new_len as u32);
        idx
    }

    pub fn get(&self, i: usize) -> Option<&'a [u8]> {
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
}
