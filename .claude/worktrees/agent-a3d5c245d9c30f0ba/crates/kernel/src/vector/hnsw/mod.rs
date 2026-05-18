//! Hierarchical Navigable Small World (HNSW) index.
//!
//! See [`HnswIndex`] for the public API. Internal modules:
//!
//! - [`levels`] — geometric layer assignment + per-layer neighbor caps.
//! - [`builder`] — graph stitching during insert (Algorithm 1).
//! - [`searcher`] — greedy descent + beam search (Algorithms 2 & 5).
//! - [`storage`] — meta-page + data-page persistence layered atop the
//!   buffer pool/WAL.
//!
//! The graph itself lives entirely in memory (`Graph`) during steady
//! state; pages exist solely so the index can be reconstructed across
//! restart. Each `insert_tx` writes one page-image WAL record per
//! affected node — typically `1 + M_max0` records (the new node plus
//! reciprocal-link rewrites of each neighbor) — which Lane INT's
//! integrity probe can replay deterministically.

pub mod builder;
pub mod levels;
pub mod searcher;
pub mod storage;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::format::{PageId, RelId, RowId, TuplePtr, TxId};
use crate::storage::BufferPool;
use crate::wal::WalCoordinator;
use crate::{Error, Result};

pub use levels::HnswParams;

use levels::{LevelRng, assign_level};
use storage::{
    MetaSnapshot, NodeRecord, allocate_data_page, allocate_meta_page, append_node_to_page,
    data_page_body_capacity, flush_meta, node_record_slot_cost, overwrite_node_in_page,
    read_data_page_next, read_meta, read_records_from_page, rewrite_data_page,
};

/// MVCC-aware row reference. Mirrors [`crate::index::IndexRowRef`] but
/// is local to the vector module so we don't pull in the b-tree's full
/// surface. Fusion may consolidate the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IndexedRowRef {
    pub row_id: RowId,
    pub tuple: TuplePtr,
}

impl IndexedRowRef {
    pub fn new(row_id: RowId, tuple: TuplePtr) -> Self {
        Self { row_id, tuple }
    }
}

/// In-memory representation of a single graph node.
#[derive(Clone, Debug)]
pub(crate) struct GraphNode {
    pub node_id: u32,
    pub layer: u8,
    pub dead: bool,
    pub committing_tx: TxId,
    pub row_ref: IndexedRowRef,
    pub vector: Vec<f32>,
    /// Adjacency lists per layer; `neighbors[i]` holds layer-i neighbors.
    pub neighbors: Vec<Vec<u32>>,
    /// Backing data page that holds this node's serialized form.
    pub page_id: PageId,
    pub slot: u16,
}

/// In-memory graph. All operations on a `Graph` are pure compute —
/// the storage layer is responsible for spilling state to pages.
#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    nodes: HashMap<u32, GraphNode>,
    entry: Option<(u32, u8)>,
}

impl Graph {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Insert a fully-formed node. Used by builder/storage paths and by
    /// the `searcher` unit tests; kept around the `cfg(test)`-only path
    /// because the production insert path constructs nodes inline and
    /// updates the entry point itself.
    #[cfg(test)]
    pub(crate) fn insert_node(&mut self, node: GraphNode) {
        let id = node.node_id;
        let layer = node.layer;
        let was_empty = self.nodes.is_empty();
        self.nodes.insert(id, node);
        if was_empty || self.entry.map(|(_, l)| layer >= l).unwrap_or(true) {
            self.entry = Some((id, layer));
        }
    }

    pub(crate) fn vector_of(&self, id: u32) -> Result<&[f32]> {
        Ok(&self
            .nodes
            .get(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))?
            .vector)
    }

    pub(crate) fn layer_of(&self, id: u32) -> Result<u8> {
        Ok(self
            .nodes
            .get(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))?
            .layer)
    }

    pub(crate) fn neighbors_at(&self, id: u32, layer: u8) -> Result<&[u32]> {
        let node = self
            .nodes
            .get(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))?;
        match node.neighbors.get(layer as usize) {
            Some(list) => Ok(list),
            None => Ok(&[]),
        }
    }

    pub(crate) fn set_neighbors_at(&mut self, id: u32, layer: u8, list: Vec<u32>) -> Result<()> {
        let node = self
            .nodes
            .get_mut(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))?;
        while node.neighbors.len() <= layer as usize {
            node.neighbors.push(Vec::new());
        }
        node.neighbors[layer as usize] = list;
        Ok(())
    }

    pub(crate) fn entry(&self) -> Option<(u32, u8)> {
        self.entry
    }

    pub(crate) fn set_entry(&mut self, entry: Option<(u32, u8)>) {
        self.entry = entry;
    }

    pub(crate) fn node(&self, id: u32) -> Result<&GraphNode> {
        self.nodes
            .get(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))
    }

    pub(crate) fn node_mut(&mut self, id: u32) -> Result<&mut GraphNode> {
        self.nodes
            .get_mut(&id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&u32, &GraphNode)> {
        self.nodes.iter()
    }
}

/// Public HNSW index handle. Cheap to clone (`Arc`-backed).
#[derive(Clone, Debug)]
pub struct HnswIndex {
    inner: Arc<HnswInner>,
}

#[derive(Debug)]
struct HnswInner {
    buffer: Arc<BufferPool>,
    wal: Option<Arc<WalCoordinator>>,
    rel_id: RelId,
    meta_page_id: PageId,
    state: Mutex<HnswState>,
}

#[derive(Debug)]
struct HnswState {
    params: HnswParams,
    rng: LevelRng,
    rng_seed: u64,
    next_node_id: u32,
    graph: Graph,
    last_data_page: Option<PageId>,
    first_data_page: Option<PageId>,
    /// Reverse map from node_id → row_id, populated only for live nodes.
    /// Searches consult this map after a beam search to surface row_ids
    /// without holding the graph lock through the whole result-set
    /// construction.
    index_id: u64,
}

/// Public hit returned by [`HnswIndex::search`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HnswHit {
    pub row_id: RowId,
    pub tuple: TuplePtr,
    pub distance: f32,
}

impl HnswIndex {
    /// Allocate an empty HNSW index. `index_id` and `rel_id` follow the
    /// b-tree convention so the catalog can store them identically.
    pub fn create(
        buffer: Arc<BufferPool>,
        rel_id: RelId,
        index_id: u64,
        params: HnswParams,
    ) -> Result<Self> {
        Self::create_with_wal(buffer, rel_id, index_id, params, None, 0xCAFEF00D_BAD1DEAA)
    }

    /// Same as [`HnswIndex::create`] but accepts a WAL coordinator and a
    /// deterministic RNG seed for level draws. Tests pin the seed so
    /// recall numbers are reproducible.
    pub fn create_with_wal(
        buffer: Arc<BufferPool>,
        rel_id: RelId,
        index_id: u64,
        params: HnswParams,
        wal: Option<Arc<WalCoordinator>>,
        rng_seed: u64,
    ) -> Result<Self> {
        params.validate()?;
        let meta_page_id = allocate_meta_page(&buffer, rel_id)?;
        let state = HnswState {
            params,
            rng: LevelRng::new(rng_seed),
            rng_seed,
            next_node_id: 0,
            graph: Graph::new(),
            last_data_page: None,
            first_data_page: None,
            index_id,
        };
        flush_meta(
            &buffer,
            meta_page_id,
            &state.snapshot(meta_page_id),
            wal.as_ref(),
            TxId::ZERO,
        )?;
        Ok(Self {
            inner: Arc::new(HnswInner {
                buffer,
                wal,
                rel_id,
                meta_page_id,
                state: Mutex::new(state),
            }),
        })
    }

    /// Reopen an existing HNSW index given its meta page. Walks the
    /// data-page chain and rebuilds the in-memory graph.
    pub fn open_with_wal(
        buffer: Arc<BufferPool>,
        meta_page_id: PageId,
        wal: Option<Arc<WalCoordinator>>,
    ) -> Result<Self> {
        let snap = {
            let guard = buffer.pin(meta_page_id)?;
            guard.with_page(read_meta)?
        };
        let mut graph = Graph::new();
        let mut cursor = snap.first_data_page;
        let mut nodes_by_page: HashMap<u32, (PageId, u16)> = HashMap::new();
        while let Some(pid) = cursor {
            let records = read_records_from_page(&buffer, pid)?;
            cursor = {
                let guard = buffer.pin(pid)?;
                guard.with_page(read_data_page_next)?
            };
            for (slot, record) in records.into_iter().enumerate() {
                nodes_by_page.insert(record.node_id, (pid, slot as u16));
                let node = GraphNode {
                    node_id: record.node_id,
                    layer: record.layer,
                    dead: record.dead,
                    committing_tx: record.committing_tx,
                    row_ref: record.row_ref,
                    vector: record.vector,
                    neighbors: record.neighbors,
                    page_id: pid,
                    slot: slot as u16,
                };
                graph.nodes.insert(node.node_id, node);
            }
        }
        graph.entry = snap.entry;
        let state = HnswState {
            params: snap.params,
            rng: LevelRng::new(snap.rng_seed),
            rng_seed: snap.rng_seed,
            next_node_id: snap.next_node_id,
            graph,
            last_data_page: snap.last_data_page,
            first_data_page: snap.first_data_page,
            index_id: snap.index_id,
        };
        // Re-find the rel_id from the meta page's frame header.
        let rel_id = {
            let guard = buffer.pin(meta_page_id)?;
            guard.with_page(|page| Ok(page.header()?.rel_id))?
        };
        Ok(Self {
            inner: Arc::new(HnswInner {
                buffer,
                wal,
                rel_id,
                meta_page_id,
                state: Mutex::new(state),
            }),
        })
    }

    /// Returns the meta-page id; the catalog persists this so reopen
    /// can find the index after a restart.
    pub fn meta_page_id(&self) -> PageId {
        self.inner.meta_page_id
    }

    /// Insert a vector into the index, tagging the new node with the
    /// committing transaction id so MVCC visibility checks can see
    /// pre-insert snapshots.
    pub fn insert_tx(&self, tx_id: TxId, vector: &[f32], row_ref: IndexedRowRef) -> Result<()> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("hnsw state mutex poisoned"))?;
        if vector.len() != state.params.dim {
            return Err(Error::CorruptPage("vector dimension mismatch"));
        }
        // Idempotence: the same row_ref already in the graph short-circuits.
        // This matches BtreeIndex's behavior on duplicate (key, row_ref) and
        // simplifies "INSERT … ON CONFLICT DO NOTHING" semantics later.
        for (_, node) in state.graph.iter() {
            if !node.dead && node.row_ref == row_ref {
                return Ok(());
            }
        }
        let node_id = state.next_node_id;
        state.next_node_id = state
            .next_node_id
            .checked_add(1)
            .ok_or(Error::CorruptPage("hnsw node id overflow"))?;
        let m = state.params.m;
        let level = assign_level(&mut state.rng, m);
        let level = level.min(u8::MAX as usize) as u8;
        let mut neighbors = Vec::with_capacity((level as usize) + 1);
        for _ in 0..=level {
            neighbors.push(Vec::new());
        }
        let new_node = GraphNode {
            node_id,
            layer: level,
            dead: false,
            committing_tx: tx_id,
            row_ref,
            vector: vector.to_vec(),
            neighbors,
            page_id: PageId(0),
            slot: 0,
        };
        state.graph.nodes.insert(node_id, new_node);
        if state.graph.entry().is_none() {
            state.graph.set_entry(Some((node_id, level)));
        }
        // Stitch the new node into the existing graph. The builder
        // returns the IDs whose adjacency lists were modified — those
        // need their backing pages rewritten too.
        let params = state.params;
        let touched = builder::link_node(&mut state.graph, &params, node_id)?;
        // Persist: append the new node's record to a data page, then
        // rewrite each touched neighbor in place (or fall back to a
        // page rewrite if the new neighbor list grew the cell).
        self.persist_new_node(&mut state, node_id, tx_id)?;
        for nb_id in touched {
            self.persist_existing_node(&mut state, nb_id, tx_id)?;
        }
        // Update meta with the (possibly advanced) entry point + new
        // node counter so a crash here recovers a consistent state.
        let snap = state.snapshot(self.inner.meta_page_id);
        flush_meta(
            &self.inner.buffer,
            self.inner.meta_page_id,
            &snap,
            self.inner.wal.as_ref(),
            tx_id,
        )?;
        Ok(())
    }

    /// Tombstone the node referenced by `row_ref`. The graph node stays
    /// linked so concurrent searches under earlier snapshots still
    /// traverse through it; result-set construction filters by `dead`.
    pub fn delete_tx(&self, tx_id: TxId, row_ref: IndexedRowRef) -> Result<()> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("hnsw state mutex poisoned"))?;
        let target = state.graph.iter().find_map(|(id, n)| {
            if !n.dead && n.row_ref == row_ref {
                Some(*id)
            } else {
                None
            }
        });
        let Some(node_id) = target else {
            return Ok(());
        };
        {
            let node = state.graph.node_mut(node_id)?;
            node.dead = true;
            node.committing_tx = tx_id;
        }
        self.persist_existing_node(&mut state, node_id, tx_id)?;
        let snap = state.snapshot(self.inner.meta_page_id);
        flush_meta(
            &self.inner.buffer,
            self.inner.meta_page_id,
            &snap,
            self.inner.wal.as_ref(),
            tx_id,
        )?;
        Ok(())
    }

    /// Search for the `k` nearest neighbors of `query` using the configured
    /// metric, with beam width `ef_search` (defaults to params if 0).
    /// Returns hits sorted by ascending distance. Tombstoned nodes are
    /// filtered from the result.
    pub fn search(&self, query: &[f32], k: usize, ef_search: usize) -> Result<Vec<HnswHit>> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("hnsw state mutex poisoned"))?;
        if query.len() != state.params.dim {
            return Err(Error::CorruptPage("vector dimension mismatch"));
        }
        let metric = state.params.metric;
        let Some((entry_node, entry_layer)) = state.graph.entry() else {
            return Ok(Vec::new());
        };
        // Greedy-descend through every layer above 0 to land on a
        // good seed for the layer-0 beam search.
        let mut current = entry_node;
        if entry_layer > 0 {
            current =
                searcher::greedy_descend(&state.graph, metric, query, entry_node, entry_layer, 1)?;
        }
        // 0 means "use the index default"; any non-zero value
        // overrides params.ef_search. We always lift `ef` to at least
        // `k` because returning fewer than `k` candidates would defeat
        // the API contract.
        let ef = if ef_search == 0 {
            state.params.ef_search.max(k)
        } else {
            ef_search.max(k)
        };
        let raw = searcher::search_layer(&state.graph, metric, query, &[current], ef, 0)?;
        let mut out = Vec::with_capacity(k.min(raw.len()));
        for (dist, node_id) in raw {
            if out.len() >= k {
                break;
            }
            let node = state.graph.node(node_id)?;
            if node.dead {
                continue;
            }
            out.push(HnswHit {
                row_id: node.row_ref.row_id,
                tuple: node.row_ref.tuple,
                distance: dist,
            });
        }
        Ok(out)
    }

    /// Returns the total number of *live* nodes in the index. Dead
    /// nodes are excluded so callers can use this as a row-count
    /// estimate without re-scanning.
    pub fn live_len(&self) -> Result<usize> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("hnsw state mutex poisoned"))?;
        Ok(state.graph.iter().filter(|(_, n)| !n.dead).count())
    }

    /// Read-only access to construction parameters. Callers use this
    /// to size their queries (e.g. `ef_search >= k`).
    pub fn params(&self) -> Result<HnswParams> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("hnsw state mutex poisoned"))?;
        Ok(state.params)
    }

    fn body_capacity(&self) -> Result<usize> {
        // Read page size from the meta page's frame; the buffer pool
        // doesn't expose `page_size()` publicly so we infer it from a
        // page we already own.
        let guard = self.inner.buffer.pin(self.inner.meta_page_id)?;
        guard.with_page(|page| Ok(data_page_body_capacity(page.as_bytes().len())))
    }

    fn persist_new_node(&self, state: &mut HnswState, node_id: u32, tx_id: TxId) -> Result<()> {
        let record = state.encode_record(node_id)?;
        let body_capacity = self.body_capacity()?;
        // Try the current tail page; if full, allocate a fresh data page
        // and chain it on.
        let target_page = match state.last_data_page {
            Some(pid) => pid,
            None => {
                let pid = allocate_data_page(&self.inner.buffer, self.inner.rel_id)?;
                state.first_data_page = Some(pid);
                state.last_data_page = Some(pid);
                pid
            }
        };
        let cost = node_record_slot_cost(&record);
        if cost > body_capacity {
            return Err(Error::CorruptPage("hnsw node record exceeds page capacity"));
        }
        let slot = match append_node_to_page(
            &self.inner.buffer,
            target_page,
            &record,
            self.inner.wal.as_ref(),
            tx_id,
        ) {
            Ok(slot) => slot,
            Err(Error::PageFull) => {
                let new_pid = allocate_data_page(&self.inner.buffer, self.inner.rel_id)?;
                // Chain the previous tail to the new page.
                self.inner
                    .buffer
                    .pin(target_page)?
                    .with_page_mut(|page| storage::write_data_page_header(page, Some(new_pid)))?;
                state.last_data_page = Some(new_pid);
                append_node_to_page(
                    &self.inner.buffer,
                    new_pid,
                    &record,
                    self.inner.wal.as_ref(),
                    tx_id,
                )?
            }
            Err(other) => return Err(other),
        };
        let resolved_page = state
            .last_data_page
            .ok_or(Error::CorruptPage("hnsw missing data page"))?;
        let node = state.graph.node_mut(node_id)?;
        node.page_id = resolved_page;
        node.slot = slot;
        Ok(())
    }

    fn persist_existing_node(
        &self,
        state: &mut HnswState,
        node_id: u32,
        tx_id: TxId,
    ) -> Result<()> {
        let record = state.encode_record(node_id)?;
        let (page_id, slot) = {
            let node = state.graph.node(node_id)?;
            (node.page_id, node.slot)
        };
        // Try in-place overwrite first. If the record's encoded length
        // changed (neighbor list grew), fall back to rewriting the
        // entire page.
        match overwrite_node_in_page(
            &self.inner.buffer,
            page_id,
            slot,
            &record,
            self.inner.wal.as_ref(),
            tx_id,
        ) {
            Ok(()) => Ok(()),
            Err(Error::CorruptPage("overwrite cell length mismatch")) => {
                self.rewrite_node_page(state, page_id, tx_id)
            }
            Err(other) => Err(other),
        }
    }

    /// Rebuild every NodeRecord cell on `page_id` from the current
    /// in-memory graph. Used when an existing node's encoded size
    /// outgrew its slot — we re-pack the page and (rarely) spill the
    /// last record onto a fresh page.
    fn rewrite_node_page(&self, state: &mut HnswState, page_id: PageId, tx_id: TxId) -> Result<()> {
        // Collect every node currently residing on `page_id`, in the
        // order they appear; we may need to append the spilled tail to
        // a freshly allocated page.
        let mut residents: Vec<(u32, u16)> = state
            .graph
            .iter()
            .filter_map(|(id, n)| {
                if n.page_id == page_id {
                    Some((*id, n.slot))
                } else {
                    None
                }
            })
            .collect();
        residents.sort_by_key(|&(_, slot)| slot);
        let records: Vec<NodeRecord> = residents
            .iter()
            .map(|&(id, _)| state.encode_record(id))
            .collect::<Result<_>>()?;
        let next_data_page = {
            let guard = self.inner.buffer.pin(page_id)?;
            guard.with_page(read_data_page_next)?
        };
        let written = rewrite_data_page(
            &self.inner.buffer,
            page_id,
            &records,
            next_data_page,
            self.inner.wal.as_ref(),
            tx_id,
        )?;
        // Reassign slots for records that survived; spill the rest.
        for (i, &(node_id, _)) in residents.iter().enumerate().take(written) {
            let node = state.graph.node_mut(node_id)?;
            node.page_id = page_id;
            node.slot = i as u16;
        }
        if written == residents.len() {
            return Ok(());
        }
        // Rare: the rewritten records grew enough that the page can no
        // longer hold all of them. Spill the tail onto a fresh data page
        // chained after `page_id`.
        let spill_pid = allocate_data_page(&self.inner.buffer, self.inner.rel_id)?;
        // Splice spill_pid into the chain between page_id and its old next.
        self.inner
            .buffer
            .pin(page_id)?
            .with_page_mut(|page| storage::write_data_page_header(page, Some(spill_pid)))?;
        self.inner
            .buffer
            .pin(spill_pid)?
            .with_page_mut(|page| storage::write_data_page_header(page, next_data_page))?;
        let mut spilled_slot = 0_u16;
        for &(node_id, _) in &residents[written..] {
            let record = state.encode_record(node_id)?;
            let slot = append_node_to_page(
                &self.inner.buffer,
                spill_pid,
                &record,
                self.inner.wal.as_ref(),
                tx_id,
            )?;
            let node = state.graph.node_mut(node_id)?;
            node.page_id = spill_pid;
            node.slot = slot;
            spilled_slot = slot;
        }
        let _ = spilled_slot; // suppress unused-write warning when residents already fit
        // Update last_data_page if we just spilled past the previous tail.
        if state.last_data_page == Some(page_id) {
            state.last_data_page = Some(spill_pid);
        }
        Ok(())
    }
}

impl HnswState {
    fn snapshot(&self, _meta_page: PageId) -> MetaSnapshot {
        MetaSnapshot {
            index_id: self.index_id,
            first_data_page: self.first_data_page,
            last_data_page: self.last_data_page,
            next_node_id: self.next_node_id,
            entry: self.graph.entry(),
            params: self.params,
            rng_seed: self.rng_seed,
        }
    }

    fn encode_record(&self, node_id: u32) -> Result<NodeRecord> {
        let node = self
            .graph
            .nodes
            .get(&node_id)
            .ok_or(Error::CorruptPage("unknown hnsw node id"))?;
        Ok(NodeRecord {
            node_id: node.node_id,
            layer: node.layer,
            dead: node.dead,
            committing_tx: node.committing_tx,
            row_ref: node.row_ref,
            vector: node.vector.clone(),
            neighbors: node.neighbors.clone(),
        })
    }
}
