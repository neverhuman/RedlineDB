//! Native VECTOR column support: typed vector value, distance kernels, and an
//! exact flat-scan operator.
//!
//! Lane V1 of phase-10 (audit tip6 + tip1). The distance API exposed here is
//! the foundation that Lane V2 (HNSW) and Lane V3 (DiskANN) build upon, so it
//! is intentionally narrow and stable:
//!
//! * `Vector` — owned, dimension-checked container of `f32` lanes.
//! * `VectorMetric` — L2 / Cosine / InnerProduct, with stateless dispatch into
//!   either SIMD-accelerated or scalar implementations.
//! * `codec` — wire format used by the SQL layer (varint dim prefix + LE f32).
//! * `flat` — exact top-K scan over an iterator of stored vectors.
//!
//! Quantisation (`f16`, `i8`) is not implemented yet but the codec carries an
//! `ElementKind` byte so future readers can distinguish formats without a
//! breaking schema change.
//!
//! # SIMD dispatch
//!
//! At compile time, AVX2 and NEON impls are gated on `target_feature` /
//! `target_arch`. At run time, the AVX2 path additionally checks
//! `is_x86_feature_detected!("avx2")` so binaries built without `-Ctarget-cpu`
//! still benefit when the host actually supports AVX2.
//!
//! See `simd.rs` for the dispatcher and `distance.rs` for the scalar
//! reference implementations used as a correctness oracle.

pub mod codec;
pub mod diskann;
pub mod distance;
pub mod flat;
pub mod hnsw;
pub mod simd;

use std::sync::Arc;

pub use codec::{ElementKind, decode_vector, encode_vector};
pub use distance::{cosine_distance_scalar, inner_product_scalar, l2_distance_scalar};
pub use flat::{FlatScanHit, flat_top_k};
pub use simd::{cosine_distance, inner_product, l2_distance};

/// Errors returned from the vector subsystem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VectorError {
    /// Stored bytes do not satisfy the vector wire format.
    #[error("malformed vector encoding: {0}")]
    Decode(String),
    /// Attempt to combine vectors with mismatching dimensions.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    /// Catch-all for invalid inputs (e.g. zero dim, NaN component).
    #[error("invalid vector: {0}")]
    Invalid(String),
}

/// Convenience alias.
pub type VectorResult<T> = std::result::Result<T, VectorError>;

/// Distance metric a vector index / scan operates under.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum VectorMetric {
    /// Squared Euclidean distance: `Σ (a_i - b_i)^2`.
    ///
    /// We return the squared form (no `sqrt`) because the scan layer only
    /// needs ordering, and skipping the square root keeps the kernel ~30%
    /// faster on AVX2. SQL surface always exposes the squared form too — this
    /// is documented behaviour, not an oversight.
    L2,
    /// Cosine distance: `1 - (a · b) / (||a|| · ||b||)`.
    Cosine,
    /// Negative inner product (so smaller is better, matching pgvector's
    /// `<#>` semantics): `-(a · b)`.
    InnerProduct,
}

impl VectorMetric {
    /// Compute the distance between two raw `f32` slices using the SIMD
    /// dispatcher. Slices must have the same length.
    pub fn distance(self, a: &[f32], b: &[f32]) -> VectorResult<f32> {
        if a.len() != b.len() {
            return Err(VectorError::DimensionMismatch {
                expected: a.len(),
                actual: b.len(),
            });
        }
        Ok(match self {
            VectorMetric::L2 => l2_distance(a, b),
            VectorMetric::Cosine => cosine_distance(a, b),
            VectorMetric::InnerProduct => inner_product(a, b),
        })
    }

    /// Stable on-disk discriminant (used by V2 HNSW persistence).
    pub fn as_u8(self) -> u8 {
        match self {
            VectorMetric::L2 => 0,
            VectorMetric::Cosine => 1,
            VectorMetric::InnerProduct => 2,
        }
    }

    /// Inverse of [`as_u8`]; returns `None` for an unknown discriminant.
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(VectorMetric::L2),
            1 => Some(VectorMetric::Cosine),
            2 => Some(VectorMetric::InnerProduct),
            _ => None,
        }
    }
}

/// Owned vector value: a dimension-tagged `Arc<[f32]>`.
///
/// Cloning is cheap (refcount bump). Lengths are validated at construction so
/// downstream kernels can rely on `data.len() == dim`.
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    data: Arc<[f32]>,
}

impl Vector {
    /// Build a vector from an owned `Vec<f32>`. Empty vectors are rejected so
    /// the SIMD kernels never see a zero-length slice.
    pub fn from_vec(values: Vec<f32>) -> VectorResult<Self> {
        if values.is_empty() {
            return Err(VectorError::Invalid("empty vector".to_owned()));
        }
        Ok(Self {
            data: Arc::from(values),
        })
    }

    /// Build a vector from a slice of `f32`.
    pub fn from_slice(values: &[f32]) -> VectorResult<Self> {
        if values.is_empty() {
            return Err(VectorError::Invalid("empty vector".to_owned()));
        }
        Ok(Self {
            data: Arc::from(values.to_vec()),
        })
    }

    /// Parse a vector from a JSON-array literal of the form `[1.0, 2.0, ...]`.
    /// Tolerates surrounding whitespace and integer entries (`1` parses as
    /// `1.0`). Designed to match pgvector's `'[...]'::vector` syntax without
    /// pulling in a JSON dependency.
    pub fn from_json_literal(text: &str) -> VectorResult<Self> {
        let trimmed = text.trim();
        let inner = trimmed
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| {
                VectorError::Invalid(format!(
                    "vector literal must be enclosed in '[' ']': {trimmed}"
                ))
            })?;
        let mut values = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let v: f32 = part
                .parse()
                .map_err(|_| VectorError::Invalid(format!("invalid vector component: {part}")))?;
            values.push(v);
        }
        Self::from_vec(values)
    }

    /// Number of components.
    #[inline]
    pub fn dim(&self) -> usize {
        self.data.len()
    }

    /// Borrow the raw component slice.
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Encode to the wire format consumed by `decode_vector`.
    pub fn encode(&self) -> Vec<u8> {
        encode_vector(self.as_slice())
    }

    /// Decode from the wire format. Returns `Err` on malformed bytes.
    pub fn decode(bytes: &[u8]) -> VectorResult<Self> {
        let values = decode_vector(bytes)?;
        Self::from_vec(values)
    }
}
