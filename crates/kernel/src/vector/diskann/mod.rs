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
//! - Disk-resident search via mmap is tracked as a follow-up wave.
//!
//! # Lane separation
//!
//! Lane V1 (VECTOR type + SIMD distance) and Lane V2 (HNSW) are independent;
//! this module currently delegates its L2 distance kernel to the shared
//! [`crate::vector::distance::l2_distance_scalar`] scalar implementation
//! and does not claim SIMD itself.

mod builder;
mod errors;
mod index;
mod params;
mod prune;
mod searcher;
mod sectors;
mod support;

#[cfg(test)]
mod tests;

pub use builder::{BuildParams, BuildStats, build};
pub use errors::{BuildError, SearchError};
pub use index::{DiskAnnIndex, RowId};
pub use params::DiskAnnParams;
pub use prune::robust_prune;
pub use searcher::{SearchParams, search};
pub use sectors::{
    SECTOR_SIZE, SectorBufferPool, SectorLayout, decode_node, encode_node, process_sector_pool,
};

/// Slice the `id`-th `dim`-wide vector out of the flat `vectors` buffer
/// the builder/searcher/prune passes share. Identical helper used to be
/// inlined in `prune.rs` and `searcher.rs`; centralised here so the
/// stride convention has one definition.
#[inline]
pub(super) fn vector_at(vectors: &[f32], dim: usize, id: u32) -> &[f32] {
    let id = id as usize;
    &vectors[id * dim..(id + 1) * dim]
}
