//! Differential and edge-case tests for `index::keycmp::cmp_keys`.
//!
//! The dispatched compare must be byte-identical to `<[u8]>::cmp` on
//! every input shape: empty, single byte, long common prefix, mismatch
//! at various offsets relative to the 32-byte AVX2 chunk boundary, and
//! random pairs.

use std::cmp::Ordering;

use redlinedb_kernel::index::cmp_keys;

fn check(a: &[u8], b: &[u8]) {
    let expected = a.cmp(b);
    let got = cmp_keys(a, b);
    assert_eq!(
        got, expected,
        "cmp_keys diverged from slice::cmp for a={:?} b={:?}",
        a, b
    );
}

#[test]
fn empty_vs_empty() {
    check(b"", b"");
}

#[test]
fn empty_vs_non_empty() {
    check(b"", b"a");
    check(b"a", b"");
    check(b"", &[0u8; 64]);
    check(&[0u8; 64], b"");
}

#[test]
fn equal_short() {
    check(b"a", b"a");
    check(b"hello", b"hello");
}

#[test]
fn equal_long() {
    let a = vec![0xABu8; 1024];
    check(&a, &a);
}

#[test]
fn prefix_relations() {
    check(b"abc", b"abcd");
    check(b"abcd", b"abc");
    let mut long = vec![0u8; 200];
    for (i, byte) in long.iter_mut().enumerate() {
        *byte = (i & 0xff) as u8;
    }
    let short = &long[..150];
    check(short, &long);
    check(&long, short);
}

#[test]
fn single_byte_differ_at_offset_0() {
    check(&[0x01], &[0x02]);
    check(&[0xff], &[0x00]);
}

#[test]
fn differ_at_offsets_in_first_chunk() {
    for offset in [1usize, 15, 31] {
        let mut a = vec![0xAAu8; 64];
        let mut b = vec![0xAAu8; 64];
        a[offset] = 0x10;
        b[offset] = 0x20;
        check(&a, &b);
        check(&b, &a);
    }
}

#[test]
fn differ_at_chunk_boundary_and_tail() {
    for offset in [32usize, 33, 63, 64, 95, 96, 127] {
        let len = offset + 16;
        let mut a = vec![0x55u8; len];
        let mut b = vec![0x55u8; len];
        a[offset] = 0xA0;
        b[offset] = 0xB0;
        check(&a, &b);
        check(&b, &a);
    }
}

#[test]
fn long_common_prefix_then_differ() {
    let mut a = vec![0xEEu8; 4096];
    let mut b = a.clone();
    for offset in [0usize, 1, 31, 32, 33, 95, 96, 1023, 1024, 1025, 4095] {
        let orig = a[offset];
        a[offset] = 0x01;
        b[offset] = 0x02;
        check(&a, &b);
        check(&b, &a);
        a[offset] = orig;
        b[offset] = orig;
    }
}

#[test]
fn length_mismatch_at_chunk_boundary() {
    // Common prefix of exactly 32, 33, 64 — verify length tie-breaker.
    for common_len in [0usize, 1, 31, 32, 33, 63, 64, 65, 127, 128] {
        let a = vec![0x77u8; common_len];
        let mut b = a.clone();
        b.push(0x77);
        check(&a, &b);
        check(&b, &a);
    }
}

// Tiny xorshift PRNG so the test stays dependency-free.
struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn fill(&mut self, buf: &mut [u8]) {
        for byte in buf.iter_mut() {
            *byte = (self.next() & 0xff) as u8;
        }
    }
}

#[test]
fn differential_random_pairs_10k() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for _ in 0..10_000 {
        let len_a = (rng.next() as usize) % 257;
        let len_b = (rng.next() as usize) % 257;
        let mut a = vec![0u8; len_a];
        let mut b = vec![0u8; len_b];
        rng.fill(&mut a);
        rng.fill(&mut b);
        // Bias toward common prefixes — copy a leading slice of `a` into `b`.
        let prefix = (rng.next() as usize) % (len_a.min(len_b).saturating_add(1));
        if prefix > 0 {
            b[..prefix].copy_from_slice(&a[..prefix]);
        }
        check(&a, &b);
    }
}

#[test]
fn differential_random_equal_length_long_pairs() {
    let mut rng = XorShift(0x1234_5678_9ABC_DEF0);
    for _ in 0..1_000 {
        let len = 64 + (rng.next() as usize) % 1024;
        let mut a = vec![0u8; len];
        rng.fill(&mut a);
        let mut b = a.clone();
        // Flip a single byte at a random position.
        let pos = (rng.next() as usize) % len;
        b[pos] ^= 0xff;
        check(&a, &b);
        check(&b, &a);
    }
}
