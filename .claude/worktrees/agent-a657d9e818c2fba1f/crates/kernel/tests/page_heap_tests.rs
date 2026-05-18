use redlinedb_kernel::engine::page_heap::{PageBackedHeap, RelationWriteTarget};
use redlinedb_kernel::engine::tx::ConcurrentTxStatus;
use redlinedb_kernel::format::{Csn, Lsn, RelId, RowId};
use redlinedb_kernel::storage::{BufferPool, PageFile};
use std::sync::Arc;
use tempfile::TempDir;

fn page_heap() -> (TempDir, PageBackedHeap, ConcurrentTxStatus) {
    let temp = TempDir::new().unwrap();
    let page_file = Arc::new(PageFile::create(temp.path().join("data.redline"), 4096).unwrap());
    let buffer = Arc::new(BufferPool::new(page_file, 128).unwrap());
    let heap = PageBackedHeap::new(RelId(1), 8, buffer).unwrap();
    (temp, heap, ConcurrentTxStatus::new())
}

#[test]
fn page_backed_heap_insert_and_read() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"alpha".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let snapshot = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &snapshot, None, row).unwrap(),
        Some(b"alpha".to_vec())
    );
}

#[test]
fn page_backed_heap_update_uses_undo_for_old_snapshot() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"old".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let old_snapshot = txs.snapshot();
    let update_tx = txs.begin();
    heap.update(
        update_tx,
        &txs.snapshot(),
        &txs,
        row,
        b"new".to_vec(),
        Lsn(20),
    )
    .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(update_tx, csn);

    let new_snapshot = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row).unwrap(),
        Some(b"old".to_vec())
    );
    assert_eq!(
        heap.get(&txs, &new_snapshot, None, row).unwrap(),
        Some(b"new".to_vec())
    );
}

#[test]
fn page_backed_heap_delete_keeps_old_snapshot_visible() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"live".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let old_snapshot = txs.snapshot();
    let delete_tx = txs.begin();
    heap.delete(delete_tx, &txs.snapshot(), &txs, row, Lsn(20))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(delete_tx, csn);

    let new_snapshot = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row).unwrap(),
        Some(b"live".to_vec())
    );
    assert_eq!(heap.get(&txs, &new_snapshot, None, row).unwrap(), None);
}

#[test]
fn page_backed_heap_relation_writes_are_keyed_by_relation_and_rowid() {
    let (_temp, heap, txs) = page_heap();
    let rel_a = RelId(11);
    let rel_b = RelId(12);
    let row = RowId(7);

    let tx = txs.begin();
    heap.insert_for_relation(tx, rel_a, row, b"a0".to_vec(), Lsn(10))
        .unwrap();
    heap.insert_for_relation(tx, rel_b, row, b"b0".to_vec(), Lsn(11))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let tx = txs.begin();
    heap.update_for_relation(
        tx,
        &txs.snapshot(),
        &txs,
        RelationWriteTarget {
            rel_id: rel_a,
            row_id: row,
        },
        b"a1".to_vec(),
        Lsn(20),
    )
    .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let tx = txs.begin();
    heap.delete_for_relation(tx, &txs.snapshot(), &txs, rel_b, row, Lsn(30))
        .unwrap();
    txs.abort(tx);

    let snapshot = txs.snapshot();
    assert_eq!(
        heap.get_for_relation(&txs, &snapshot, None, rel_a, row)
            .unwrap(),
        Some(b"a1".to_vec())
    );
    assert_eq!(
        heap.get_for_relation(&txs, &snapshot, None, rel_b, row)
            .unwrap(),
        Some(b"b0".to_vec())
    );
}

#[test]
fn page_backed_heap_allocates_multiple_pages_for_many_rows() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let mut rows = Vec::new();
    for idx in 0..40 {
        let row = heap.reserve_row_id();
        heap.insert_with_row_id(tx, row, vec![idx as u8; 512], Lsn(10 + idx))
            .unwrap();
        rows.push(row);
    }
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    assert!(heap.resident_pages() > 1);
    let snapshot = txs.snapshot();
    for (idx, row) in rows.into_iter().enumerate() {
        assert_eq!(
            heap.get(&txs, &snapshot, None, row).unwrap(),
            Some(vec![idx as u8; 512])
        );
    }
}

#[test]
fn page_backed_heap_undo_chain_can_span_pages() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, vec![0; 1024], Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);
    let old_snapshot = txs.snapshot();

    for idx in 1..12 {
        let tx = txs.begin();
        heap.update(
            tx,
            &txs.snapshot(),
            &txs,
            row,
            vec![idx as u8; 1024],
            Lsn(20 + idx),
        )
        .unwrap();
        let csn = txs.reserve_csn();
        txs.publish_commit(tx, csn);
    }

    assert!(heap.resident_pages() > 2);
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row).unwrap(),
        Some(vec![0; 1024])
    );
    assert_eq!(
        heap.get(&txs, &txs.snapshot(), None, row).unwrap(),
        Some(vec![11; 1024])
    );
}

#[test]
fn page_backed_heap_vacuum_prunes_undo_only_after_horizon_passes_end_csn() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"old".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);
    let old_snapshot = txs.snapshot();

    let update_tx = txs.begin();
    heap.update(
        update_tx,
        &txs.snapshot(),
        &txs,
        row,
        b"new".to_vec(),
        Lsn(20),
    )
    .unwrap();
    let update_csn = txs.reserve_csn();
    txs.publish_commit(update_tx, update_csn);

    let retained = heap.vacuum(update_csn, &txs).unwrap();
    assert_eq!(retained.rows_scanned, 1);
    assert_eq!(retained.chains_pruned, 0);
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row).unwrap(),
        Some(b"old".to_vec())
    );

    let pruned = heap.vacuum(Csn(update_csn.0 + 1), &txs).unwrap();
    assert_eq!(pruned.rows_scanned, 1);
    assert_eq!(pruned.chains_pruned, 1);
    assert_eq!(pruned.undo_links_removed, 1);
    assert_eq!(
        heap.get(&txs, &txs.snapshot(), None, row).unwrap(),
        Some(b"new".to_vec())
    );
    assert_eq!(heap.get(&txs, &old_snapshot, None, row).unwrap(), None);
}

#[test]
fn page_backed_heap_vacuum_removes_committed_tombstone_after_horizon() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"live".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let delete_tx = txs.begin();
    heap.delete(delete_tx, &txs.snapshot(), &txs, row, Lsn(20))
        .unwrap();
    let delete_csn = txs.reserve_csn();
    txs.publish_commit(delete_tx, delete_csn);

    let retained = heap.vacuum(delete_csn, &txs).unwrap();
    assert_eq!(retained.dead_rows_removed, 0);

    let removed = heap.vacuum(Csn(delete_csn.0 + 1), &txs).unwrap();
    assert_eq!(removed.dead_rows_removed, 1);
    assert_eq!(heap.get(&txs, &txs.snapshot(), None, row).unwrap(), None);
}

#[test]
fn page_backed_heap_vacuum_leaves_aborted_latest_version_unpruned() {
    let (_temp, heap, txs) = page_heap();
    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"base".to_vec(), Lsn(10))
        .unwrap();
    let csn = txs.reserve_csn();
    txs.publish_commit(tx, csn);

    let update_tx = txs.begin();
    heap.update(
        update_tx,
        &txs.snapshot(),
        &txs,
        row,
        b"aborted".to_vec(),
        Lsn(20),
    )
    .unwrap();
    txs.abort(update_tx);

    let stats = heap.vacuum(Csn(100), &txs).unwrap();
    assert_eq!(stats.chains_pruned, 0);
    assert_eq!(stats.dead_rows_removed, 0);
    assert_eq!(
        heap.get(&txs, &txs.snapshot(), None, row).unwrap(),
        Some(b"base".to_vec())
    );
}

#[test]
fn page_backed_heap_reuses_empty_pages_after_vacuum() {
    let (_temp, heap, txs) = page_heap();

    let tx = txs.begin();
    let row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, row, b"live".to_vec(), Lsn(10))
        .unwrap();
    let insert_csn = txs.reserve_csn();
    txs.publish_commit(tx, insert_csn);

    let delete_tx = txs.begin();
    heap.delete(delete_tx, &txs.snapshot(), &txs, row, Lsn(20))
        .unwrap();
    let delete_csn = txs.reserve_csn();
    txs.publish_commit(delete_tx, delete_csn);

    let vacuum_stats = heap.vacuum(Csn(delete_csn.0 + 1), &txs).unwrap();
    assert_eq!(vacuum_stats.dead_rows_removed, 1);

    let reused_before = heap.page_count().unwrap();
    let tx = txs.begin();
    let new_row = heap.reserve_row_id();
    heap.insert_with_row_id(tx, new_row, b"again".to_vec(), Lsn(30))
        .unwrap();
    let commit_csn = txs.reserve_csn();
    txs.publish_commit(tx, commit_csn);

    assert_eq!(heap.page_count().unwrap(), reused_before);
    assert_eq!(
        heap.get(&txs, &txs.snapshot(), None, new_row).unwrap(),
        Some(b"again".to_vec())
    );
}
