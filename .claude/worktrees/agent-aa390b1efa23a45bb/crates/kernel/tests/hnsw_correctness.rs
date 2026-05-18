//! Small-graph correctness tests for the HNSW index.
//!
//! These exercise the *behavior* of the index — empty/insert/delete/MVCC —
//! at scales small enough to verify by exhaustive flat scan. Recall at the
//! production-target scale lives in `hnsw_recall.rs`.

use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::format::{PageGeneration, PageId, RowId, TuplePtr, TxId};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::vector::hnsw::{HnswIndex, HnswParams, IndexedRowRef};

fn make_index(temp: &TempDir, dim: usize) -> HnswIndex {
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 256).unwrap());
    let mut params = HnswParams::standard(dim);
    // Smaller M for correctness tests so the graph is fast to build.
    params.m = 8;
    params.m_max0 = 16;
    params.ef_construction = 64;
    params.ef_search = 32;
    HnswIndex::create_with_wal(
        buffer,
        redlinedb_kernel::format::RelId(1),
        1,
        params,
        None,
        1234,
    )
    .unwrap()
}

fn row_ref(i: u32) -> IndexedRowRef {
    IndexedRowRef::new(
        RowId(i as u64),
        TuplePtr::new_with_generation(PageId(1), i as u16, PageGeneration::ONE),
    )
}

/// Deterministic xorshift PRNG so the tests don't depend on the rand crate.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        let bits = (self.0 >> 32) as u32;
        // Map to [-1, 1)
        (bits as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    fn vector(&mut self, dim: usize) -> Vec<f32> {
        (0..dim).map(|_| self.next_f32()).collect()
    }
}

fn flat_topk(query: &[f32], vectors: &[(IndexedRowRef, Vec<f32>)], k: usize) -> Vec<RowId> {
    let mut scored: Vec<(f32, RowId)> = vectors
        .iter()
        .map(|(rr, v)| {
            let mut acc = 0.0_f32;
            for i in 0..query.len() {
                let d = query[i] - v[i];
                acc += d * d;
            }
            (acc, rr.row_id)
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    scored.into_iter().take(k).map(|(_, r)| r).collect()
}

#[test]
fn empty_search_returns_nothing() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 8);
    let q = vec![0.1; 8];
    let hits = idx.search(&q, 5, 32).unwrap();
    assert!(hits.is_empty(), "empty index must return zero hits");
}

#[test]
fn insert_one_search_returns_self() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 4);
    let v = vec![0.5, -0.5, 0.25, -0.25];
    idx.insert_tx(TxId(1), &v, row_ref(7)).unwrap();
    let hits = idx.search(&v, 1, 16).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].row_id, RowId(7));
    // Distance must be 0 (or numerically tiny) for an identical query.
    assert!(
        hits[0].distance < 1e-6,
        "self-distance must be ~0, got {}",
        hits[0].distance
    );
}

#[test]
fn insert_100_search_returns_results() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 8);
    let mut rng = Rng::new(0xABCD);
    for i in 0..100 {
        let v = rng.vector(8);
        idx.insert_tx(TxId(1), &v, row_ref(i)).unwrap();
    }
    let q = rng.vector(8);
    let hits = idx.search(&q, 10, 64).unwrap();
    assert!(
        !hits.is_empty(),
        "search on a 100-node graph must return hits"
    );
    assert!(hits.len() <= 10);
}

#[test]
fn duplicate_insertion_is_idempotent() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 4);
    let v = vec![1.0, 2.0, 3.0, 4.0];
    let r = row_ref(1);
    idx.insert_tx(TxId(1), &v, r).unwrap();
    idx.insert_tx(TxId(1), &v, r).unwrap();
    idx.insert_tx(TxId(1), &v, r).unwrap();
    assert_eq!(
        idx.live_len().unwrap(),
        1,
        "duplicate row_ref must not multiply nodes"
    );
}

#[test]
fn delete_then_search_excludes_tombstoned_node() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 4);
    let mut rng = Rng::new(0xBEEF);
    for i in 0..50 {
        let v = rng.vector(4);
        idx.insert_tx(TxId(1), &v, row_ref(i)).unwrap();
    }
    let target = row_ref(7);
    idx.delete_tx(TxId(2), target).unwrap();
    // Search anywhere; row_id 7 must not appear regardless of distance.
    for seed in [1_u64, 2, 3, 4, 5] {
        let q = Rng::new(seed).vector(4);
        let hits = idx.search(&q, 50, 64).unwrap();
        for h in hits {
            assert_ne!(h.row_id, RowId(7), "tombstoned node leaked into search");
        }
    }
    assert_eq!(idx.live_len().unwrap(), 49);
}

#[test]
fn small_recall_against_flat_scan() {
    // 200 nodes, dim=16, k=5 — recall@5 must be ≥ 0.9 even on a small
    // graph because ef_search is configured generously.
    let temp = TempDir::new().unwrap();
    let mut params = HnswParams::standard(16);
    params.m = 12;
    params.m_max0 = 24;
    params.ef_construction = 100;
    params.ef_search = 64;
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 512).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 256).unwrap());
    let idx = HnswIndex::create_with_wal(
        buffer,
        redlinedb_kernel::format::RelId(1),
        1,
        params,
        None,
        99,
    )
    .unwrap();
    let mut rng = Rng::new(0x123456);
    let mut vectors = Vec::with_capacity(200);
    for i in 0..200_u32 {
        let v = rng.vector(16);
        let rr = row_ref(i);
        idx.insert_tx(TxId(1), &v, rr).unwrap();
        vectors.push((rr, v));
    }
    let mut total_recall = 0.0_f32;
    let trials = 20;
    for s in 0..trials {
        let q = Rng::new(0xDEAD + s as u64).vector(16);
        let want = flat_topk(&q, &vectors, 5);
        let got: Vec<RowId> = idx
            .search(&q, 5, 64)
            .unwrap()
            .iter()
            .map(|h| h.row_id)
            .collect();
        let intersection = got.iter().filter(|r| want.contains(r)).count();
        total_recall += intersection as f32 / 5.0;
    }
    let recall = total_recall / trials as f32;
    assert!(recall >= 0.9, "small-graph recall@5 dropped to {recall}");
}

#[test]
fn ef_search_controls_quality_tradeoff() {
    // With a tiny ef_search, beam search visits very few candidates and
    // recall must drop. With a large ef_search, recall must be near 1.
    // We don't claim absolute thresholds (tiny graphs are noisy), only
    // that *increasing* ef_search never *decreases* recall.
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 16);
    let mut rng = Rng::new(0x6F6F);
    let mut vectors = Vec::with_capacity(150);
    for i in 0..150_u32 {
        let v = rng.vector(16);
        let rr = row_ref(i);
        idx.insert_tx(TxId(1), &v, rr).unwrap();
        vectors.push((rr, v));
    }
    let mut last_recall = 0.0_f32;
    for &ef in &[1_usize, 4, 16, 64] {
        let trials = 10;
        let mut total = 0.0_f32;
        for s in 0..trials {
            let q = Rng::new(0xABBA + s as u64).vector(16);
            let want = flat_topk(&q, &vectors, 5);
            let got: Vec<RowId> = idx
                .search(&q, 5, ef)
                .unwrap()
                .iter()
                .map(|h| h.row_id)
                .collect();
            let intersection = got.iter().filter(|r| want.contains(r)).count();
            total += intersection as f32 / 5.0;
        }
        let recall = total / trials as f32;
        // Allow a 5% slack for sampling noise. The trend must be
        // monotone-ish (small dips OK) but a 4× ef expansion must
        // never produce a 5%+ drop.
        assert!(
            recall + 0.05 >= last_recall,
            "ef={ef} recall={recall} regressed below previous {last_recall}"
        );
        last_recall = recall;
    }
}

#[test]
fn dimension_mismatch_is_an_error() {
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 8);
    // Insert one to seed the graph (avoids the entry-point-empty fast
    // path which doesn't exercise the dim check).
    idx.insert_tx(TxId(1), &[0.0; 8], row_ref(1)).unwrap();
    assert!(idx.search(&[0.0; 4], 5, 32).is_err());
    assert!(idx.insert_tx(TxId(2), &[0.0; 4], row_ref(2)).is_err());
}
