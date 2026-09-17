//! Packed validity bitmap. 1024 bits live inline (16 × u64), heap above that.
//!
//! Used by `Morsel` to track which rows are still live after a filter pass.
//! All arithmetic is safe `u64`; SIMD kernels arrive in Phase 6 M3.

use smallvec::{SmallVec, smallvec};

const BITS_PER_WORD: usize = 64;
const INLINE_WORDS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    words: SmallVec<[u64; INLINE_WORDS]>,
    bit_len: usize,
}

impl Bitmap {
    pub fn new(cap_rows: usize) -> Self {
        let words = (cap_rows + BITS_PER_WORD - 1) / BITS_PER_WORD;
        Self {
            words: smallvec![0u64; words],
            bit_len: cap_rows,
        }
    }

    pub fn new_all_set(cap_rows: usize) -> Self {
        let mut b = Self::new(cap_rows);
        for w in b.words.iter_mut() {
            *w = u64::MAX;
        }
        b.mask_tail();
        b
    }

    #[inline]
    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    #[inline]
    pub fn set(&mut self, i: usize) {
        debug_assert!(i < self.bit_len, "Bitmap::set out of range");
        let (w, b) = (i / BITS_PER_WORD, i % BITS_PER_WORD);
        self.words[w] |= 1u64 << b;
    }

    #[inline]
    pub fn clear(&mut self, i: usize) {
        debug_assert!(i < self.bit_len, "Bitmap::clear out of range");
        let (w, b) = (i / BITS_PER_WORD, i % BITS_PER_WORD);
        self.words[w] &= !(1u64 << b);
    }

    #[inline]
    pub fn is_set(&self, i: usize) -> bool {
        if i >= self.bit_len {
            return false;
        }
        let (w, b) = (i / BITS_PER_WORD, i % BITS_PER_WORD);
        (self.words[w] >> b) & 1 == 1
    }

    pub fn count_ones(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn and(&mut self, other: &Bitmap) {
        debug_assert_eq!(self.bit_len, other.bit_len, "Bitmap::and length mismatch");
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a &= *b;
        }
        self.mask_tail();
    }

    pub fn or(&mut self, other: &Bitmap) {
        debug_assert_eq!(self.bit_len, other.bit_len, "Bitmap::or length mismatch");
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a |= *b;
        }
        self.mask_tail();
    }

    fn mask_tail(&mut self) {
        let tail_bits = self.bit_len % BITS_PER_WORD;
        if tail_bits == 0 {
            return;
        }
        if let Some(last) = self.words.last_mut() {
            let mask = (1u64 << tail_bits) - 1;
            *last &= mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bitmap_has_no_bits() {
        let b = Bitmap::new(0);
        assert_eq!(b.bit_len(), 0);
        assert_eq!(b.count_ones(), 0);
        assert!(!b.is_set(0));
    }

    #[test]
    fn set_and_query() {
        let mut b = Bitmap::new(128);
        b.set(0);
        b.set(63);
        b.set(64);
        b.set(127);
        assert!(b.is_set(0));
        assert!(b.is_set(63));
        assert!(b.is_set(64));
        assert!(b.is_set(127));
        assert!(!b.is_set(1));
        assert_eq!(b.count_ones(), 4);
    }

    #[test]
    fn clear_works() {
        let mut b = Bitmap::new_all_set(10);
        assert_eq!(b.count_ones(), 10);
        b.clear(3);
        assert!(!b.is_set(3));
        assert_eq!(b.count_ones(), 9);
    }

    #[test]
    fn and_or_correctness() {
        let mut a = Bitmap::new(8);
        let mut c = Bitmap::new(8);
        a.set(0);
        a.set(1);
        a.set(2);
        c.set(1);
        c.set(2);
        c.set(3);

        let mut ab = a.clone();
        ab.and(&c);
        assert!(!ab.is_set(0));
        assert!(ab.is_set(1));
        assert!(ab.is_set(2));
        assert!(!ab.is_set(3));
        assert_eq!(ab.count_ones(), 2);

        let mut ao = a.clone();
        ao.or(&c);
        assert!(ao.is_set(0));
        assert!(ao.is_set(1));
        assert!(ao.is_set(2));
        assert!(ao.is_set(3));
        assert_eq!(ao.count_ones(), 4);
    }

    #[test]
    fn all_set_respects_tail() {
        let b = Bitmap::new_all_set(70);
        assert_eq!(b.count_ones(), 70);
    }
}
