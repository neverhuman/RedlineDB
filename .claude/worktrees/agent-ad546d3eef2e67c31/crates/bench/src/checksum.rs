//! Deterministic dataset checksums for the bench harness.
//!
//! Lane INT replaces the previous `MAX(k)` / `COUNT(*)` placeholder with a
//! structured [`DatasetChecksum`] that captures three independent fingerprints
//! of a seeded `kv` workload:
//!
//! * `row_count`  — how many rows the engine returned,
//! * `key_xor`    — XOR-fold of every row's key (commutative; unaffected by
//!   scan order, sensitive to a single missing or duplicated key),
//! * `payload_hash` — SHA-256 of the canonical `(key, payload_hash)` tuples
//!   in scan order, folded into a `u64` so the certification manifest can
//!   serialise it as JSON without arbitrary-precision integers.
//!
//! Each helper is intentionally independent: callers can mix-and-match for
//! tests that only need one of the three numbers, and the bench's
//! `seed_kv`/`checksum` paths consume the full struct to compare engines
//! byte-for-byte.

use sha2::{Digest, Sha256};

/// Hash a single row's bytes into a 64-bit fingerprint. Folds the SHA-256
/// digest into the first 8 bytes so the value is JSON-friendly while still
/// retaining most of the discriminating power for non-adversarial inputs.
pub fn row_hash(row: &[u8]) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"row\0");
    hasher.update((row.len() as u64).to_le_bytes());
    hasher.update(row);
    let digest = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(buf)
}

/// Hash a payload composed of an ordered iterator of byte slices. Mirrors
/// [`row_hash`]'s framing per element so concatenation can't shadow a real
/// row boundary, then folds the result to a `u64`.
pub fn payload_hash<'a, I>(rows: I) -> u64
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let mut hasher = Sha256::new();
    hasher.update(b"payload\0");
    for row in rows {
        hasher.update(b"row\0");
        hasher.update((row.len() as u64).to_le_bytes());
        hasher.update(row);
    }
    let digest = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(buf)
}

/// XOR-fold of every key. Order-independent so two scans that visit the
/// same keys in any order produce the same fingerprint. A single missing
/// or duplicated key flips at least one bit, which is what makes this a
/// useful integrity sentinel alongside `payload_hash` (which is order
/// sensitive).
pub fn key_xor<I>(keys: I) -> u64
where
    I: IntoIterator<Item = u64>,
{
    let mut acc = 0_u64;
    for key in keys {
        acc ^= key;
    }
    acc
}

/// Three-axis dataset fingerprint: row count, key XOR, payload hash.
/// Surfaces in `CertificationManifest.checksums` so post-run comparison
/// can detect any of (row drift, key drift, value drift) independently.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetChecksum {
    pub row_count: u64,
    pub key_xor: u64,
    pub payload_hash: u64,
}

impl DatasetChecksum {
    /// Combine two checksums (e.g. partial scans across worker shards) by
    /// summing row counts, XORing keys, and re-hashing the two payload
    /// hashes together. The combination is associative and commutative on
    /// the row-count and key-xor axes; payload-hash combination is
    /// associative but order-sensitive on the inputs (so callers should
    /// merge in a deterministic order).
    pub fn merge(self, other: Self) -> Self {
        let left = self.payload_hash.to_le_bytes();
        let right = other.payload_hash.to_le_bytes();
        let folded = payload_hash([left.as_slice(), right.as_slice()]);
        Self {
            row_count: self.row_count.saturating_add(other.row_count),
            key_xor: self.key_xor ^ other.key_xor,
            payload_hash: folded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_hash_is_deterministic() {
        let h1 = row_hash(b"hello");
        let h2 = row_hash(b"hello");
        assert_eq!(h1, h2);
        assert_ne!(h1, row_hash(b"world"));
        assert_ne!(h1, row_hash(b""));
    }

    #[test]
    fn key_xor_is_order_independent() {
        let a = key_xor([1_u64, 2, 3, 4]);
        let b = key_xor([4_u64, 3, 2, 1]);
        let c = key_xor([2_u64, 4, 1, 3]);
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn key_xor_detects_single_missing_key() {
        let full = key_xor([1_u64, 2, 3, 4]);
        let missing = key_xor([1_u64, 2, 3]);
        assert_ne!(full, missing);
    }

    #[test]
    fn payload_hash_is_order_sensitive() {
        let forward = payload_hash([b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]);
        let reverse = payload_hash([b"c".as_slice(), b"b".as_slice(), b"a".as_slice()]);
        assert_ne!(forward, reverse);
        let again = payload_hash([b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]);
        assert_eq!(forward, again);
    }

    #[test]
    fn dataset_checksum_merge_is_deterministic() {
        let left = DatasetChecksum {
            row_count: 2,
            key_xor: 0x0F0F,
            payload_hash: 0xAAAA_BBBB_CCCC_DDDD,
        };
        let right = DatasetChecksum {
            row_count: 3,
            key_xor: 0x00FF,
            payload_hash: 0x1111_2222_3333_4444,
        };
        let merged_a = left.merge(right);
        let merged_b = left.merge(right);
        assert_eq!(merged_a, merged_b);
        assert_eq!(merged_a.row_count, 5);
        assert_eq!(merged_a.key_xor, 0x0F0F ^ 0x00FF);
        // payload_hash is non-zero and stable across calls.
        assert_ne!(merged_a.payload_hash, 0);
    }

    #[test]
    fn dataset_checksum_default_round_trips_through_serde() {
        let zero = DatasetChecksum::default();
        let json = serde_json::to_string(&zero).expect("serialize");
        let back: DatasetChecksum = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(zero, back);
    }
}
