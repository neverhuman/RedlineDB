//! HNSW graph construction.
//!
//! Builder logic — the part that takes a candidate vector + level
//! assignment and stitches it into the existing graph. Mirrors
//! Algorithm 1 of Malkov & Yashunin (2016):
//!
//! 1. Greedy-descend from the entry point through every layer above
//!    the new node's assigned level.
//! 2. From the new node's level down to 0, run a beam search with
//!    `ef_construction` width to find the `M` nearest existing nodes.
//! 3. Link the new node to those neighbors *and* link each neighbor
//!    back, pruning their lists down to `M` (or `M_max0` at level 0)
//!    by keeping the closest entries.
//!
//! The neighbor cap is enforced symmetrically: when a back-link would
//! push a neighbor over its layer cap we drop its current farthest
//! neighbor. This is the simple cap-by-distance rule from §4.3 — the
//! full "heuristic" rule from the paper would buy a bit more recall
//! but adds substantial complexity for little practical gain at our
//! scale.

use crate::Result;
use crate::vector::distance::Metric;

use super::Graph;
use super::levels::HnswParams;
use super::searcher::{greedy_descend, search_layer};

/// Insert a node into the in-memory graph following the assigned level.
/// The caller has already created the `GraphNode` with its vector,
/// row_ref, layer, and empty neighbor lists; this fn populates the
/// neighbor lists and (optionally) updates the global entry point.
///
/// Returns the list of *neighbor node IDs* whose adjacency was modified
/// — the storage layer needs to flush their pages back to disk.
pub(super) fn link_node(graph: &mut Graph, params: &HnswParams, new_id: u32) -> Result<Vec<u32>> {
    let mut touched = Vec::new();
    let new_layer = graph.layer_of(new_id)?;
    let dim = params.dim;
    let metric = params.metric;
    let query: Vec<f32> = graph.vector_of(new_id)?.to_vec();
    if query.len() != dim {
        return Err(crate::Error::CorruptPage("vector dimension mismatch"));
    }
    let entry = graph.entry();
    if let Some((entry_node, entry_layer)) = entry {
        // Step 1: greedy-descend from the top through layers above
        // `new_layer`. Even when `entry_layer < new_layer`, this just
        // skips straight to step 2 — the new node becomes the entry
        // point for the layers it occupies that nobody else does.
        let mut current = entry_node;
        if entry_layer > new_layer {
            current = greedy_descend(
                graph,
                metric,
                &query,
                entry_node,
                entry_layer,
                new_layer.saturating_add(1),
            )?;
        }
        // Step 2: beam search at every layer from min(entry_layer, new_layer)
        // down to 0; collect M nearest, link both directions. After
        // each layer the *full* candidate set seeds the next layer's
        // search (paper: ep ← W). Single-seed seeding leaves recall on
        // the table for nodes whose layer-k beam diverges from layer-0.
        let top = entry_layer.min(new_layer);
        let mut entry_points: Vec<u32> = vec![current];
        for layer in (0..=top).rev() {
            // Construction beam width: paper's `ef_construction` directly.
            // The candidate set is the input to neighbor selection; a
            // wider beam gives the heuristic more diverse candidates
            // to choose from.
            let candidates = search_layer(
                graph,
                metric,
                &query,
                &entry_points,
                params.ef_construction,
                layer,
            )?;
            // Use the paper's diversifying heuristic (Algorithm 4) to
            // pick neighbors. A pure cap-by-distance rule produces hub
            // nodes that everyone points at — recall on dense
            // embeddings drops to ~0.75. The heuristic admits a
            // candidate `e` only when no already-selected neighbor
            // sits *between* `e` and the query, which spreads
            // adjacency across the unit sphere.
            let cap = params.neighbor_cap(layer as usize);
            let extended = extend_candidates(graph, metric, &query, &candidates, cap)?;
            let new_neighbors =
                select_neighbors_heuristic(graph, metric, &query, new_id, &extended, cap)?;
            // Update the new node's adjacency at this layer.
            graph.set_neighbors_at(new_id, layer, new_neighbors.clone())?;
            // Reciprocal links: add `new_id` to each neighbor's adjacency,
            // then re-prune that list back to the layer cap.
            for nb in &new_neighbors {
                back_link_with_pruning(graph, metric, *nb, layer, new_id, cap)?;
                touched.push(*nb);
            }
            // The next layer's beam starts from the *full* candidate
            // set we just discovered (Algorithm 1 line 12: ep ← W).
            entry_points = candidates.iter().map(|(_, id)| *id).collect();
            // Failpoint: armed *after* the new node and its reciprocal
            // links have been wired up at this layer but *before* the
            // next layer is processed. A crash here lets recovery exercise
            // partial-build scenarios.
            crate::fail_point!("vector::hnsw::insert::after_link");
        }
    } else {
        // First node in the index — every layer's neighbor list stays
        // empty; the new node simply becomes the entry point.
        for layer in 0..=(new_layer as usize) {
            graph.set_neighbors_at(new_id, layer as u8, Vec::new())?;
        }
    }
    // Promote the new node to the entry point if it sits higher than
    // (or equal to) the current entry's layer. Equal-layer ties favor
    // the new node so the entry advances on every "first node at a
    // given level" event — that keeps the topmost layer's neighborhood
    // representative of the most recently inserted vectors.
    let promote = match graph.entry() {
        Some((_, current_layer)) => new_layer >= current_layer,
        None => true,
    };
    if promote {
        graph.set_entry(Some((new_id, new_layer)));
    }
    Ok(touched)
}

fn back_link_with_pruning(
    graph: &mut Graph,
    metric: Metric,
    nb: u32,
    layer: u8,
    new_id: u32,
    cap: usize,
) -> Result<()> {
    let mut nb_list: Vec<u32> = graph.neighbors_at(nb, layer)?.to_vec();
    if !nb_list.contains(&new_id) {
        nb_list.push(new_id);
    }
    if nb_list.len() <= cap {
        graph.set_neighbors_at(nb, layer, nb_list)?;
        return Ok(());
    }
    // Over the cap — re-select via the heuristic from `nb`'s POV. This
    // mirrors the paper: each side of a reciprocal link runs the same
    // diversifying rule, so the topology doesn't degenerate when
    // popular hubs accumulate too many in-edges.
    let nb_vec: Vec<f32> = graph.vector_of(nb)?.to_vec();
    let mut scored: Vec<(f32, u32)> = Vec::with_capacity(nb_list.len());
    for cand in &nb_list {
        let d = metric.distance(&nb_vec, graph.vector_of(*cand)?)?;
        scored.push((d, *cand));
    }
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pruned = select_neighbors_heuristic(graph, metric, &nb_vec, nb, &scored, cap)?;
    graph.set_neighbors_at(nb, layer, pruned)
}

/// `extendCandidates=true` form from Algorithm 4 §4.2: grow the
/// candidate set with the *neighbors of neighbors* of the existing
/// pool. Helps recall on highly-clustered data because the heuristic
/// otherwise rejects candidates that share a hub with the query;
/// pulling in nodes one hop further out gives it diverse alternatives
/// to admit. We keep the original candidates plus their layer-0
/// neighbors, deduplicated by node id, and sorted by distance to
/// `query`.
fn extend_candidates(
    graph: &Graph,
    metric: Metric,
    query: &[f32],
    candidates: &[(f32, u32)],
    cap: usize,
) -> Result<Vec<(f32, u32)>> {
    use std::collections::HashSet;
    let mut seen: HashSet<u32> = HashSet::with_capacity(candidates.len() * 4);
    let mut out: Vec<(f32, u32)> = Vec::with_capacity(candidates.len() * 4);
    for (d, id) in candidates {
        if seen.insert(*id) {
            out.push((*d, *id));
        }
    }
    // Cap the extension so we don't blow up runtime for layer 0 where
    // candidates can be 200 long. We only need a few extra hops to
    // diversify; 4*cap is the hnswlib default.
    let cap_ext = cap * 4;
    let originals: Vec<u32> = candidates.iter().map(|(_, id)| *id).collect();
    'outer: for cand_id in originals {
        let nbrs: Vec<u32> = graph
            .neighbors_at(cand_id, 0)
            .map(|s| s.to_vec())
            .unwrap_or_default();
        for nb in nbrs {
            if !seen.insert(nb) {
                continue;
            }
            let d = metric.distance(query, graph.vector_of(nb)?)?;
            out.push((d, nb));
            if out.len() >= cap_ext {
                break 'outer;
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

/// Algorithm 4 from Malkov & Yashunin: pick up to `cap` candidates that
/// span the neighborhood of `query`. Admits a candidate `e` only if no
/// already-selected neighbor is closer to `e` than `query` is — this
/// is the "diversifying" condition that breaks hub formation.
///
/// `query_owner` (the node whose neighborhood we're picking) is excluded
/// from the result so callers don't link a node to itself.
fn select_neighbors_heuristic(
    graph: &Graph,
    metric: Metric,
    _query: &[f32],
    query_owner: u32,
    candidates: &[(f32, u32)],
    cap: usize,
) -> Result<Vec<u32>> {
    let mut sorted: Vec<(f32, u32)> = candidates
        .iter()
        .copied()
        .filter(|(_, id)| *id != query_owner)
        .collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut result: Vec<u32> = Vec::with_capacity(cap);
    let mut overflow: Vec<u32> = Vec::new();
    for (d_to_q, cand) in &sorted {
        if result.len() >= cap {
            break;
        }
        let mut admit = true;
        let cand_vec = graph.vector_of(*cand)?.to_vec();
        for r in &result {
            let d_to_r = metric.distance(&cand_vec, graph.vector_of(*r)?)?;
            // If a previously-admitted neighbor is closer to `cand`
            // than `query` is, then `cand` would be redundant: any
            // search that reached `cand` would also have reached `r`,
            // and the edge to `cand` adds no new coverage.
            if d_to_r < *d_to_q {
                admit = false;
                break;
            }
        }
        if admit {
            result.push(*cand);
        } else {
            overflow.push(*cand);
        }
    }
    // Backfill with the closest *rejected* candidates if the heuristic
    // didn't fill the cap. On high-dim random data the diversifying
    // condition rejects almost everything because distances are
    // concentrated; the cap-by-distance fallback recovers the recall
    // headroom that hnswlib achieves with `ef = M`. The result stays
    // sorted by distance because `overflow` was iterated in sorted
    // order.
    while result.len() < cap && !overflow.is_empty() {
        result.push(overflow.remove(0));
    }
    Ok(result)
}
