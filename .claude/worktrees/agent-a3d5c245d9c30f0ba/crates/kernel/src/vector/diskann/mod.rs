//! DiskANN (Vamana) single-layer SSD-resident vector graph index.
//!
//! Reference: Subramanya, Devvrit, Kadekodi, Krishnaswamy, Simhadri,
//! "DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a Single
//! Node," NeurIPS 2019.
//!
//! # Layout sketch
//!
//! Each graph node is materialised as a fixed-size record holding (a) the
//! float32 vector payload, (b) the in-degree neighbour list (padded to `R`
//! u32 entries), and (c) a small header. Records are aligned to a 4 KiB
//! sector ([`sectors::SECTOR_SIZE`]) so search can read exactly one sector
//! per node visit. v1 keeps the byte image in memory (`Arc<[u8]>`) for
//! correctness; the same encoder/decoder is used for the on-disk format
//! once the IO path lands.
//!
//! # Scope (v1)
//!
//! - In-memory Vamana build (R, L, alpha tunable).
//! - Beam search at query time with an arbitrary `beam_width`.
//! - Sector-aligned layout, so `to_sectors()` / `from_sectors()` round-trip.
//! - Recall@10 >= 0.92 on a 10k synthetic dataset (seeded).
//! - `// TODO: disk-resident search via mmap` — deferred to a follow-up wave.
//!
//! # Lane separation
//!
//! Lane V1 (VECTOR type + SIMD distance) and Lane V2 (HNSW) are independent;
//! this module uses a scalar L2 fallback ([`distance::l2_squared`]) marked for
//! cleanup once `crate::vector::distance` ships. The fallback is plain Rust
//! and doesn't claim SIMD.

mod builder;
mod prune;
mod searcher;
mod sectors;

pub use builder::{BuildParams, BuildStats, build};
pub use prune::robust_prune;
pub use searcher::{SearchParams, search};
pub use sectors::{SECTOR_SIZE, SectorLayout, decode_node, encode_node};

use std::sync::Arc;

/// Stable identifier for a vector row stored in the graph. Carries the
/// original row id supplied at build time so callers can map search hits back
/// to heap tuples without an extra lookup table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowId(pub u64);

/// User-facing build configuration (degree, search list size, alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskAnnParams {
    /// Maximum out-degree per node (`R` in the paper). Standard 64.
    pub max_degree: usize,
    /// Search-list size used during build (`L` in the paper). Should be
    /// >= max_degree; 100 is a typical baseline.
    pub search_list_size: usize,
    /// RobustPrune relaxation factor. 1.0 == strict; 1.2 trades a few
    /// extra edges for substantially better recall.
    pub alpha: f32,
    /// Random seed for the medoid bootstrap and tie-breaks. Tests pin this.
    pub seed: u64,
}

impl Default for DiskAnnParams {
    fn default() -> Self {
        Self {
            max_degree: 64,
            search_list_size: 100,
            alpha: 1.2,
            seed: 0x5EED_D15C_0A99_BABE_u64,
        }
    }
}

/// A built Vamana graph. Cheaply cloneable; underlying storage is shared.
#[derive(Clone)]
pub struct DiskAnnIndex {
    inner: Arc<Inner>,
}

struct Inner {
    /// Vector dimensionality. All inputs share this dim.
    dim: usize,
    /// Configured params (recorded for diagnostics + sector serialisation).
    params: DiskAnnParams,
    /// Entry point id chosen at build time (medoid).
    entry: u32,
    /// Flat float32 vectors, length = num_nodes * dim.
    vectors: Vec<f32>,
    /// Adjacency: for each node, up to `max_degree` neighbour indices.
    /// `Vec<Vec<u32>>` keeps construction simple; the sector encoder
    /// flattens it into the 4 KiB record layout.
    neighbours: Vec<Vec<u32>>,
    /// Per-node row ids supplied by the caller. Search returns these.
    row_ids: Vec<RowId>,
}

impl DiskAnnIndex {
    /// Build a new graph from `vectors` (each of length `dim`) and the
    /// matching `row_ids`. Empty input produces a valid empty index.
    pub fn build(
        dim: usize,
        vectors: &[Vec<f32>],
        row_ids: &[RowId],
        params: DiskAnnParams,
    ) -> Result<Self, BuildError> {
        if vectors.len() != row_ids.len() {
            return Err(BuildError::Mismatch {
                vectors: vectors.len(),
                ids: row_ids.len(),
            });
        }
        if params.max_degree == 0 {
            return Err(BuildError::InvalidParams("max_degree must be > 0"));
        }
        if !(params.alpha.is_finite() && params.alpha >= 1.0) {
            return Err(BuildError::InvalidParams("alpha must be finite and >= 1.0"));
        }
        for v in vectors {
            if v.len() != dim {
                return Err(BuildError::DimMismatch {
                    expected: dim,
                    actual: v.len(),
                });
            }
            for x in v {
                if !x.is_finite() {
                    return Err(BuildError::NonFinite);
                }
            }
        }
        let (entry, neighbours) = builder::build(
            dim,
            vectors,
            BuildParams {
                max_degree: params.max_degree,
                search_list_size: params.search_list_size,
                alpha: params.alpha,
                seed: params.seed,
            },
        );
        let mut flat = Vec::with_capacity(vectors.len() * dim);
        for v in vectors {
            flat.extend_from_slice(v);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                dim,
                params,
                entry,
                vectors: flat,
                neighbours,
                row_ids: row_ids.to_vec(),
            }),
        })
    }

    /// Returns top-`k` row ids ranked by ascending L2 distance to `query`.
    /// `beam_width` controls the search-list size used during traversal;
    /// larger values trade latency for recall.
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        beam_width: usize,
    ) -> Result<Vec<RowId>, SearchError> {
        if query.len() != self.inner.dim {
            return Err(SearchError::DimMismatch {
                expected: self.inner.dim,
                actual: query.len(),
            });
        }
        if self.inner.row_ids.is_empty() || k == 0 {
            return Ok(Vec::new());
        }
        let beam = beam_width.max(k);
        let hits = searcher::search(
            &self.inner.vectors,
            self.inner.dim,
            &self.inner.neighbours,
            self.inner.entry,
            query,
            SearchParams {
                k,
                beam_width: beam,
            },
        );
        Ok(hits
            .into_iter()
            .map(|node_id| self.inner.row_ids[node_id as usize])
            .collect())
    }

    /// Number of indexed vectors.
    pub fn len(&self) -> usize {
        self.inner.row_ids.len()
    }

    /// True iff no vectors are indexed.
    pub fn is_empty(&self) -> bool {
        self.inner.row_ids.is_empty()
    }

    /// Vector dimensionality recorded at build time.
    pub fn dim(&self) -> usize {
        self.inner.dim
    }

    /// Configured build parameters.
    pub fn params(&self) -> DiskAnnParams {
        self.inner.params
    }

    /// Entry-point node id chosen at build time (medoid). Visible mainly for
    /// tests and diagnostics.
    pub fn entry(&self) -> u32 {
        self.inner.entry
    }

    /// Borrow the adjacency list for testing/inspection. Not part of the
    /// stable public surface.
    #[doc(hidden)]
    pub fn neighbours_for(&self, node: u32) -> &[u32] {
        &self.inner.neighbours[node as usize]
    }

    /// Serialise every node into the canonical 4 KiB sector layout. The
    /// concatenated bytes are page-aligned: nodes are laid out contiguously,
    /// each starting on a [`SECTOR_SIZE`]-aligned offset (node `i` starts at
    /// `i * SECTOR_SIZE`). Returned buffer length is `len() * SECTOR_SIZE`.
    ///
    /// TODO: disk-resident search via mmap — once the storage path is wired,
    /// the searcher will pull individual sectors from a memory-mapped file
    /// instead of the in-memory `Inner`.
    pub fn to_sectors(&self) -> Result<Vec<u8>, sectors::SectorError> {
        let layout = SectorLayout::for_dim_degree(self.inner.dim, self.inner.params.max_degree)?;
        let total = self.inner.row_ids.len().saturating_mul(SECTOR_SIZE);
        let mut out = vec![0u8; total];
        for i in 0..self.inner.row_ids.len() {
            let start = i * SECTOR_SIZE;
            let buf = &mut out[start..start + SECTOR_SIZE];
            let vec_slice = &self.inner.vectors[i * self.inner.dim..(i + 1) * self.inner.dim];
            encode_node(
                buf,
                &layout,
                self.inner.row_ids[i],
                vec_slice,
                &self.inner.neighbours[i],
            )?;
        }
        Ok(out)
    }

    /// Inverse of [`Self::to_sectors`]. Reconstructs the in-memory graph
    /// from a buffer produced by `to_sectors` (or a future on-disk file).
    /// The caller must supply the dim/params/entry/total node count that
    /// match the original; in v1 those metadata are kept alongside the
    /// blob, since the sidecar header is out of scope for this lane.
    pub fn from_sectors(
        buf: &[u8],
        dim: usize,
        params: DiskAnnParams,
        entry: u32,
        node_count: usize,
    ) -> Result<Self, sectors::SectorError> {
        let layout = SectorLayout::for_dim_degree(dim, params.max_degree)?;
        if buf.len() < node_count * SECTOR_SIZE {
            return Err(sectors::SectorError::Truncated {
                expected: node_count * SECTOR_SIZE,
                actual: buf.len(),
            });
        }
        let mut vectors = Vec::with_capacity(node_count * dim);
        let mut row_ids = Vec::with_capacity(node_count);
        let mut neighbours = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let start = i * SECTOR_SIZE;
            let sector = &buf[start..start + SECTOR_SIZE];
            let (row_id, vec_data, neigh) = decode_node(sector, &layout)?;
            vectors.extend_from_slice(vec_data);
            row_ids.push(row_id);
            neighbours.push(neigh);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                dim,
                params,
                entry,
                vectors,
                neighbours,
                row_ids,
            }),
        })
    }
}

impl std::fmt::Debug for DiskAnnIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskAnnIndex")
            .field("dim", &self.inner.dim)
            .field("len", &self.inner.row_ids.len())
            .field("entry", &self.inner.entry)
            .field("params", &self.inner.params)
            .finish()
    }
}

/// Build-time errors.
#[derive(Debug, PartialEq)]
pub enum BuildError {
    /// `vectors.len()` did not match `row_ids.len()`.
    Mismatch { vectors: usize, ids: usize },
    /// One of the input vectors had an unexpected dimension.
    DimMismatch { expected: usize, actual: usize },
    /// A coordinate was NaN or infinite. We refuse to build.
    NonFinite,
    /// The supplied [`DiskAnnParams`] were unusable; message gives the gist.
    InvalidParams(&'static str),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Mismatch { vectors, ids } => {
                write!(f, "vector/row_id length mismatch: {vectors} vs {ids}")
            }
            BuildError::DimMismatch { expected, actual } => {
                write!(f, "dimension mismatch: expected {expected}, got {actual}")
            }
            BuildError::NonFinite => write!(f, "vector contained a non-finite coordinate"),
            BuildError::InvalidParams(reason) => write!(f, "invalid build params: {reason}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Search-time errors.
#[derive(Debug, PartialEq)]
pub enum SearchError {
    /// Query length did not match the index dim.
    DimMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::DimMismatch { expected, actual } => {
                write!(f, "query dim mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for SearchError {}

/// Scalar-only L2-squared fallback shared by builder and searcher. When Lane
/// V1 lands, callers should switch to `crate::vector::distance::l2_squared`
/// and this helper can be deleted.
// TODO(lane-v1-cleanup): replace with `crate::vector::distance::l2_squared`.
pub(crate) mod distance {
    /// Squared L2 distance — sufficient for ranking; we never need the
    /// square root because monotonicity is preserved.
    #[inline]
    pub fn l2_squared(a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());
        let mut acc = 0.0f32;
        for i in 0..a.len() {
            let d = a[i] - b[i];
            acc += d * d;
        }
        acc
    }
}
