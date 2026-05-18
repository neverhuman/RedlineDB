#![cfg(feature = "failpoints")]
//! Lane V2 failpoint hooks for HNSW.
//!
//! Two named failpoints exist in the HNSW kernel:
//!
//! - `vector::hnsw::insert::after_link` — fires once per layer during
//!   `insert_tx`, immediately after the new node and its reciprocal
//!   links at that layer have been wired up.
//! - `vector::hnsw::search::beam_step` — fires inside the layer-0 beam
//!   search at every frontier expansion.
//!
//! These are the seams the failpoint matrix targets when injecting
//! crash-during-build / crash-during-search scenarios. The smoke tests
//! here only need to prove the hooks are *reachable* — they don't
//! exercise the recovery contract; that lives in the bench-level
//! matrix.

use std::sync::Arc;

use tempfile::TempDir;

use redlinedb_kernel::failpoints;
use redlinedb_kernel::format::{PageGeneration, PageId, RowId, TuplePtr, TxId};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::vector::hnsw::{HnswIndex, HnswParams, IndexedRowRef};

fn make_index(temp: &TempDir, dim: usize) -> HnswIndex {
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 4096).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 256).unwrap());
    let mut params = HnswParams::standard(dim);
    params.m = 8;
    params.m_max0 = 16;
    params.ef_construction = 64;
    HnswIndex::create_with_wal(
        buffer,
        redlinedb_kernel::format::RelId(1),
        1,
        params,
        None,
        1,
    )
    .unwrap()
}

fn row_ref(i: u32) -> IndexedRowRef {
    IndexedRowRef::new(
        RowId(i as u64),
        TuplePtr::new_with_generation(PageId(1), i as u16, PageGeneration::ONE),
    )
}

#[test]
fn insert_after_link_failpoint_panics_when_armed() {
    let scenario = fail::FailScenario::setup();
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 4);
    // Insert one without arming so the graph has a non-trivial entry
    // point. The next insert hits the per-layer link loop.
    idx.insert_tx(TxId(1), &[0.1, 0.2, 0.3, 0.4], row_ref(0))
        .unwrap();

    failpoints::cfg("vector::hnsw::insert::after_link", "panic")
        .expect("configure after_link panic");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        idx.insert_tx(TxId(2), &[0.2, 0.4, 0.6, 0.8], row_ref(1))
    }));

    failpoints::cfg("vector::hnsw::insert::after_link", "off").expect("disable after_link panic");
    drop(scenario);

    assert!(
        result.is_err(),
        "vector::hnsw::insert::after_link must panic when armed"
    );
}

#[test]
fn search_beam_step_failpoint_panics_when_armed() {
    let scenario = fail::FailScenario::setup();
    let temp = TempDir::new().unwrap();
    let idx = make_index(&temp, 4);
    // Seed enough nodes that beam search visits at least one expansion.
    for i in 0..16_u32 {
        let v = vec![i as f32, (i % 3) as f32, (i % 5) as f32, (i % 7) as f32];
        idx.insert_tx(TxId(1), &v, row_ref(i)).unwrap();
    }

    failpoints::cfg("vector::hnsw::search::beam_step", "panic").expect("configure beam_step panic");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        idx.search(&[0.1, 0.2, 0.3, 0.4], 5, 32)
    }));

    failpoints::cfg("vector::hnsw::search::beam_step", "off").expect("disable beam_step panic");
    drop(scenario);

    assert!(
        result.is_err(),
        "vector::hnsw::search::beam_step must panic when armed"
    );
}
