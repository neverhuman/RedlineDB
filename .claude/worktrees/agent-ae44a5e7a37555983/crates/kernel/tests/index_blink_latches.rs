use std::sync::{Arc, Barrier};
use std::thread;

use tempfile::TempDir;

use redlinedb_kernel::Error;
use redlinedb_kernel::format::{PageGeneration, PageId, RelId, RowId, TuplePtr};
use redlinedb_kernel::index::{BtreeIndex, IndexDescriptor, IndexId, IndexRowRef, IndexUniqueness};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use redlinedb_kernel::telemetry::Phase11Counters;

fn build_index(
    index_id: u64,
    uniqueness: IndexUniqueness,
    page_size: usize,
) -> (TempDir, BtreeIndex) {
    let dir = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(dir.path().join("data.redline"), page_size).unwrap());
    let buffer = Arc::new(BufferPool::new(Arc::clone(&page_file), 1024).unwrap());
    let index = BtreeIndex::create(
        buffer,
        IndexDescriptor::new(IndexId(index_id), RelId(1), uniqueness),
    )
    .unwrap();
    (dir, index)
}

fn row_ref(id: u64) -> IndexRowRef {
    IndexRowRef::with_row_id(
        RowId(id),
        TuplePtr::new_with_generation(PageId(20_000 + id), id as u16, PageGeneration::ONE),
    )
}

#[test]
fn non_split_writers_do_not_acquire_structure_lock() {
    let (_dir, index) = build_index(22_001, IndexUniqueness::NonUnique, 512);
    for i in 0..96_u64 {
        let key = format!("k{i:03}");
        index.insert(key.as_bytes(), row_ref(i)).unwrap();
    }

    let counters = Arc::new(Phase11Counters::new());
    index.set_phase11_counters(Arc::clone(&counters));
    counters.reset();

    let barrier = Arc::new(Barrier::new(5));
    let keys = ["k010a", "k030a", "k060a", "k090a"];
    let mut handles = Vec::new();
    for (idx, key) in keys.into_iter().enumerate() {
        let index = index.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            index.insert(key.as_bytes(), row_ref(1_000 + idx as u64))
        }));
    }
    barrier.wait();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    let snap = counters.snapshot();
    assert_eq!(snap.index_structure_lock_acquires, 0);
    assert_eq!(snap.index_leaf_splits, 0);
    assert!(index.validate().unwrap().errors.is_empty());
}

#[test]
fn same_leaf_writers_serialize_without_losing_duplicates() {
    let (_dir, index) = build_index(22_002, IndexUniqueness::NonUnique, 4096);
    let counters = Arc::new(Phase11Counters::new());
    index.set_phase11_counters(Arc::clone(&counters));

    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for i in 0..8_u64 {
        let index = index.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            index.insert(b"dup", row_ref(2_000 + i))
        }));
    }
    barrier.wait();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    let rows = index.point_lookup(b"dup").unwrap();
    assert_eq!(rows.len(), 8);
    assert_eq!(counters.snapshot().index_structure_lock_acquires, 0);
    assert!(index.validate().unwrap().errors.is_empty());
}

#[test]
fn unique_key_race_allows_one_winner() {
    let (_dir, index) = build_index(22_003, IndexUniqueness::Unique, 512);
    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for i in 0..8_u64 {
        let index = index.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            index.insert_unique(i + 1, b"unique", row_ref(3_000 + i))
        }));
    }
    barrier.wait();

    let mut ok = 0;
    let mut conflicts = 0;
    for handle in handles {
        match handle.join().unwrap() {
            Ok(()) => ok += 1,
            Err(Error::WriteConflict) => conflicts += 1,
            Err(err) => panic!("unexpected error: {err:?}"),
        }
    }
    assert_eq!(ok, 1);
    assert_eq!(conflicts, 7);
    assert_eq!(index.point_lookup(b"unique").unwrap().len(), 1);
}
