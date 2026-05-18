//! On-disk representation for an HNSW index.
//!
//! The disk layout is intentionally minimal: HNSW lives entirely in memory
//! during steady-state operation, and pages are touched only on
//! insert/delete. We mirror the buffer/WAL conventions established by
//! [`BtreeIndex`](crate::index::BtreeIndex) so Lane INT's index integrity
//! checker can treat the meta page like any other index page.
//!
//! Layout:
//!
//! ```text
//! meta page (PageKind::BtreeMeta with HNSW special-area magic)
//!   ┌───────────────────────────────────────────────────────────┐
//!   │ magic "RDHN" │ version u16 │ kind=Meta u8 │ pad │
//!   │ index_id u64 │ first_data_page u64 │ last_data_page u64 │
//!   │ next_node_id u32 │ entry_node u32 │ entry_layer u8 │
//!   │ dim u16 │ m u16 │ m_max0 u16 │ ef_construction u16 │
//!   │ ef_search u16 │ metric u8 │ rng_seed u64 │
//!   └───────────────────────────────────────────────────────────┘
//!
//! data page (PageKind::BtreeLeaf reused, special-area carries chain ptrs)
//!   slots hold serialized NodeRecord cells: each cell is one node's
//!   full state — vector + per-layer neighbor list + row_id + tombstone +
//!   committing tx_id (for MVCC).
//! ```
//!
//! On open we walk the data-page chain, decode every `NodeRecord`, and
//! rebuild the in-memory graph. A node is uniquely identified by its
//! `node_id` (a monotonic u32 assigned at insert time). Tombstones are
//! kept rather than physically removed so MVCC visibility checks can
//! still see the pre-delete state.

use std::sync::Arc;

use crate::format::PAGE_HEADER_LEN;
use crate::format::bytes::{read_u16, read_u32, read_u64, write_u16, write_u32, write_u64};
use crate::format::{Lsn, PageGeneration, PageId, PageKind, RelId, SLOT_LEN, TuplePtr, TxId};
use crate::storage::BufferPool;
use crate::vector::distance::Metric;
use crate::wal::{WalCoordinator, WalPayload, WalRecordKind};
use crate::{Error, Result};

use super::levels::HnswParams;

pub(crate) const HNSW_SPECIAL_LEN: usize = 96;
const HNSW_MAGIC: u32 = 0x5244_484E; // "RDHN"
const HNSW_VERSION: u16 = 1;

const KIND_META: u8 = 1;
const KIND_DATA: u8 = 2;

const META_INDEX_ID_OFF: usize = 8;
const META_FIRST_DATA_OFF: usize = 16;
const META_LAST_DATA_OFF: usize = 24;
const META_NEXT_NODE_OFF: usize = 32;
const META_ENTRY_NODE_OFF: usize = 36;
const META_ENTRY_LAYER_OFF: usize = 40;
const META_DIM_OFF: usize = 42;
const META_M_OFF: usize = 44;
const META_M_MAX0_OFF: usize = 46;
const META_EF_CONSTR_OFF: usize = 48;
const META_EF_SEARCH_OFF: usize = 50;
const META_METRIC_OFF: usize = 52;
const META_RNG_SEED_OFF: usize = 54;

const DATA_NEXT_PAGE_OFF: usize = 8;

/// Persisted form of a single graph node. The on-disk encoding is
/// length-prefixed and self-describing so a future schema bump can
/// extend the trailing bytes without breaking older decoders.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NodeRecord {
    pub node_id: u32,
    pub layer: u8,
    pub dead: bool,
    pub committing_tx: TxId,
    pub row_ref: super::IndexedRowRef,
    pub vector: Vec<f32>,
    /// Neighbor lists per layer, layer 0 first. `neighbors[i]` is the
    /// adjacency for layer `i`; an entry shorter than `layer + 1` means
    /// the higher levels were never linked (legal for the entry point
    /// during the very first insert).
    pub neighbors: Vec<Vec<u32>>,
}

impl NodeRecord {
    pub(crate) fn encode(&self) -> Vec<u8> {
        // Header: node_id u32, layer u8, flags u8, committing_tx u64,
        //         row_id u64, page_id u64, slot u16, generation u32,
        //         dim u16, layer_count u16
        let dim = self.vector.len();
        let mut out = Vec::with_capacity(
            40 + dim * 4
                + self
                    .neighbors
                    .iter()
                    .map(|n| 4 + n.len() * 4)
                    .sum::<usize>(),
        );
        out.extend_from_slice(&self.node_id.to_le_bytes());
        out.push(self.layer);
        out.push(if self.dead { 1 } else { 0 });
        out.extend_from_slice(&self.committing_tx.0.to_le_bytes());
        out.extend_from_slice(&self.row_ref.row_id.0.to_le_bytes());
        out.extend_from_slice(&self.row_ref.tuple.page_id.0.to_le_bytes());
        out.extend_from_slice(&self.row_ref.tuple.slot.to_le_bytes());
        out.extend_from_slice(&self.row_ref.tuple.generation.0.to_le_bytes());
        out.extend_from_slice(&(dim as u16).to_le_bytes());
        out.extend_from_slice(&(self.neighbors.len() as u16).to_le_bytes());
        for v in &self.vector {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for layer_neighbors in &self.neighbors {
            out.extend_from_slice(&(layer_neighbors.len() as u32).to_le_bytes());
            for n in layer_neighbors {
                out.extend_from_slice(&n.to_le_bytes());
            }
        }
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 40 {
            return Err(Error::CorruptPage("hnsw node record too short"));
        }
        let node_id = read_u32(bytes, 0)?;
        let layer = bytes[4];
        let dead = bytes[5] != 0;
        let committing_tx = TxId(read_u64(bytes, 6)?);
        let row_id = crate::format::RowId(read_u64(bytes, 14)?);
        let page_id = PageId(read_u64(bytes, 22)?);
        let slot = read_u16(bytes, 30)?;
        let generation = PageGeneration(read_u32(bytes, 32)?);
        let dim = read_u16(bytes, 36)? as usize;
        let layer_count = read_u16(bytes, 38)? as usize;
        let mut cursor = 40;
        if cursor + dim * 4 > bytes.len() {
            return Err(Error::CorruptPage("hnsw vector overflow"));
        }
        let mut vector = Vec::with_capacity(dim);
        for _ in 0..dim {
            vector.push(f32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]));
            cursor += 4;
        }
        let mut neighbors = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            if cursor + 4 > bytes.len() {
                return Err(Error::CorruptPage("hnsw neighbor list overflow"));
            }
            let n = read_u32(bytes, cursor)? as usize;
            cursor += 4;
            if cursor + n * 4 > bytes.len() {
                return Err(Error::CorruptPage("hnsw neighbor list overflow"));
            }
            let mut layer_neighbors = Vec::with_capacity(n);
            for _ in 0..n {
                layer_neighbors.push(read_u32(bytes, cursor)?);
                cursor += 4;
            }
            neighbors.push(layer_neighbors);
        }
        Ok(NodeRecord {
            node_id,
            layer,
            dead,
            committing_tx,
            row_ref: super::IndexedRowRef {
                row_id,
                tuple: TuplePtr::new_with_generation(page_id, slot, generation),
            },
            vector,
            neighbors,
        })
    }
}

/// Snapshot of meta-page fields. Decoded from the meta page on open and
/// re-encoded on each meta-touching mutation.
#[derive(Clone, Debug)]
pub(crate) struct MetaSnapshot {
    pub index_id: u64,
    pub first_data_page: Option<PageId>,
    pub last_data_page: Option<PageId>,
    pub next_node_id: u32,
    pub entry: Option<(u32, u8)>,
    pub params: HnswParams,
    pub rng_seed: u64,
}

pub(crate) fn write_meta(page: &mut crate::format::Page, snap: &MetaSnapshot) -> Result<()> {
    page.reinitialize_with_special(
        PageKind::BtreeMeta,
        page.header()?.page_id,
        page.header()?.rel_id,
        PageGeneration::ONE,
        HNSW_SPECIAL_LEN,
    )?;
    let special = page.special_bytes_mut()?;
    special.fill(0);
    write_u32(special, 0, HNSW_MAGIC)?;
    write_u16(special, 4, HNSW_VERSION)?;
    special[6] = KIND_META;
    write_u64(special, META_INDEX_ID_OFF, snap.index_id)?;
    write_u64(
        special,
        META_FIRST_DATA_OFF,
        snap.first_data_page.map(|p| p.0).unwrap_or(u64::MAX),
    )?;
    write_u64(
        special,
        META_LAST_DATA_OFF,
        snap.last_data_page.map(|p| p.0).unwrap_or(u64::MAX),
    )?;
    write_u32(special, META_NEXT_NODE_OFF, snap.next_node_id)?;
    write_u32(
        special,
        META_ENTRY_NODE_OFF,
        snap.entry.map(|(n, _)| n).unwrap_or(u32::MAX),
    )?;
    special[META_ENTRY_LAYER_OFF] = snap.entry.map(|(_, l)| l).unwrap_or(0);
    write_u16(special, META_DIM_OFF, snap.params.dim as u16)?;
    write_u16(special, META_M_OFF, snap.params.m as u16)?;
    write_u16(special, META_M_MAX0_OFF, snap.params.m_max0 as u16)?;
    write_u16(
        special,
        META_EF_CONSTR_OFF,
        snap.params.ef_construction as u16,
    )?;
    write_u16(special, META_EF_SEARCH_OFF, snap.params.ef_search as u16)?;
    special[META_METRIC_OFF] = snap.params.metric.as_u8();
    write_u64(special, META_RNG_SEED_OFF, snap.rng_seed)?;
    Ok(())
}

pub(crate) fn read_meta(page: &crate::format::Page) -> Result<MetaSnapshot> {
    let special = page.special_bytes()?;
    if read_u32(special, 0)? != HNSW_MAGIC {
        return Err(Error::CorruptPage("hnsw meta magic mismatch"));
    }
    if read_u16(special, 4)? != HNSW_VERSION {
        return Err(Error::UnsupportedVersion(read_u16(special, 4)?));
    }
    if special[6] != KIND_META {
        return Err(Error::CorruptPage("expected hnsw meta page"));
    }
    let index_id = read_u64(special, META_INDEX_ID_OFF)?;
    let first_data_page = decode_opt_page_id(read_u64(special, META_FIRST_DATA_OFF)?);
    let last_data_page = decode_opt_page_id(read_u64(special, META_LAST_DATA_OFF)?);
    let next_node_id = read_u32(special, META_NEXT_NODE_OFF)?;
    let entry_node = read_u32(special, META_ENTRY_NODE_OFF)?;
    let entry_layer = special[META_ENTRY_LAYER_OFF];
    let dim = read_u16(special, META_DIM_OFF)? as usize;
    let m = read_u16(special, META_M_OFF)? as usize;
    let m_max0 = read_u16(special, META_M_MAX0_OFF)? as usize;
    let ef_construction = read_u16(special, META_EF_CONSTR_OFF)? as usize;
    let ef_search = read_u16(special, META_EF_SEARCH_OFF)? as usize;
    let metric = Metric::from_u8(special[META_METRIC_OFF])
        .ok_or(crate::Error::CorruptPage("hnsw: invalid metric byte"))?;
    let rng_seed = read_u64(special, META_RNG_SEED_OFF)?;
    let entry = if entry_node == u32::MAX {
        None
    } else {
        Some((entry_node, entry_layer))
    };
    Ok(MetaSnapshot {
        index_id,
        first_data_page,
        last_data_page,
        next_node_id,
        entry,
        params: HnswParams {
            m,
            m_max0,
            ef_construction,
            ef_search,
            dim,
            metric,
        },
        rng_seed,
    })
}

pub(crate) fn write_data_page_header(
    page: &mut crate::format::Page,
    next_data_page: Option<PageId>,
) -> Result<()> {
    let special = page.special_bytes_mut()?;
    special.fill(0);
    write_u32(special, 0, HNSW_MAGIC)?;
    write_u16(special, 4, HNSW_VERSION)?;
    special[6] = KIND_DATA;
    write_u64(
        special,
        DATA_NEXT_PAGE_OFF,
        next_data_page.map(|p| p.0).unwrap_or(u64::MAX),
    )?;
    Ok(())
}

pub(crate) fn read_data_page_next(page: &crate::format::Page) -> Result<Option<PageId>> {
    let special = page.special_bytes()?;
    if read_u32(special, 0)? != HNSW_MAGIC {
        return Err(Error::CorruptPage("hnsw data magic mismatch"));
    }
    if special[6] != KIND_DATA {
        return Err(Error::CorruptPage("expected hnsw data page"));
    }
    Ok(decode_opt_page_id(read_u64(special, DATA_NEXT_PAGE_OFF)?))
}

fn decode_opt_page_id(raw: u64) -> Option<PageId> {
    if raw == u64::MAX {
        None
    } else {
        Some(PageId(raw))
    }
}

/// Body capacity (bytes) on a freshly initialized HNSW data page.
pub(crate) fn data_page_body_capacity(page_size: usize) -> usize {
    page_size.saturating_sub(PAGE_HEADER_LEN + HNSW_SPECIAL_LEN)
}

/// Encoded size of a NodeRecord cell including its slot overhead.
pub(crate) fn node_record_slot_cost(record: &NodeRecord) -> usize {
    record.encode().len() + SLOT_LEN
}

/// Append `record` to `page_id`. Returns `Err(Error::PageFull)` if the
/// record cannot fit; the caller is expected to allocate a new data page
/// and retry. On success, mutates the page in-place and marks it dirty.
pub(crate) fn append_node_to_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    record: &NodeRecord,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<u16> {
    let guard = buffer.pin(page_id)?;
    let slot = guard.with_page_mut(|page| {
        let bytes = record.encode();
        page.insert_cell(&bytes)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(slot)
}

/// Overwrite the cell at `(page_id, slot)` with `record`. Length must
/// match — neighbor-list resize during link-pruning uses
/// [`rewrite_data_page`] instead, which compacts the entire page.
pub(crate) fn overwrite_node_in_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    slot: u16,
    record: &NodeRecord,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    guard.with_page_mut(|page| {
        let bytes = record.encode();
        page.overwrite_cell(slot, &bytes)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(())
}

/// Rewrite an HNSW data page from a list of records. Used when a record's
/// encoded size grows (neighbor list extended) and an in-place
/// [`overwrite_node_in_page`] would no longer fit. Returns the leftover
/// records that didn't fit; caller pushes them into a new page.
pub(crate) fn rewrite_data_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    records: &[NodeRecord],
    next_data_page: Option<PageId>,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<usize> {
    let guard = buffer.pin(page_id)?;
    let written = guard.with_page_mut(|page| {
        let rel_id = page.header()?.rel_id;
        page.reinitialize_with_special(
            PageKind::BtreeLeaf,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )?;
        write_data_page_header(page, next_data_page)?;
        let capacity = data_page_body_capacity(page.as_bytes().len());
        let mut used = 0_usize;
        let mut written = 0_usize;
        for record in records {
            let cost = node_record_slot_cost(record);
            if used + cost > capacity {
                break;
            }
            page.insert_cell(&record.encode())?;
            used += cost;
            written += 1;
        }
        Ok::<usize, Error>(written)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(written)
}

/// Read every NodeRecord cell from a data page. Used by `open()` to
/// reconstruct the in-memory graph.
pub(crate) fn read_records_from_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
) -> Result<Vec<NodeRecord>> {
    let guard = buffer.pin(page_id)?;
    guard.with_page(|page| {
        let count = page.slot_count()?;
        let mut out = Vec::with_capacity(count as usize);
        for slot in 0..count {
            let cell = page.cell(slot)?;
            out.push(NodeRecord::decode(cell)?);
        }
        Ok(out)
    })
}

pub(crate) fn allocate_meta_page(buffer: &Arc<BufferPool>, rel_id: RelId) -> Result<PageId> {
    let guard = buffer.allocate(PageKind::BtreeMeta, rel_id)?;
    let page_id = guard.page_id();
    guard.with_page_mut(|page| {
        page.reinitialize_with_special(
            PageKind::BtreeMeta,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )
    })?;
    guard.mark_dirty(Lsn(1))?;
    Ok(page_id)
}

pub(crate) fn allocate_data_page(buffer: &Arc<BufferPool>, rel_id: RelId) -> Result<PageId> {
    let guard = buffer.allocate(PageKind::BtreeLeaf, rel_id)?;
    let page_id = guard.page_id();
    guard.with_page_mut(|page| {
        page.reinitialize_with_special(
            PageKind::BtreeLeaf,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )?;
        write_data_page_header(page, None)
    })?;
    guard.mark_dirty(Lsn(1))?;
    Ok(page_id)
}

pub(crate) fn flush_meta(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    snap: &MetaSnapshot,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    guard.with_page_mut(|page| write_meta(page, snap))?;
    record_page_image(buffer, page_id, wal, tx_id)
}

fn record_page_image(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    if tx_id == TxId::ZERO {
        return guard.mark_dirty(Lsn(1));
    }
    let Some(wal) = wal else {
        return guard.mark_dirty(Lsn(1));
    };
    let mut page = guard.with_page(|page| Ok(page.clone()))?;
    page.set_page_lsn(Lsn::ZERO)?;
    let payload = WalPayload::PageImage {
        page_id,
        page_lsn: Lsn::ZERO,
        page_bytes: page.as_bytes().to_vec(),
    };
    let append = wal.append(WalRecordKind::PageImage, tx_id, payload.encode()?)?;
    guard.mark_dirty(append.end_lsn)
}
