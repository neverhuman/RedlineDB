use redlinedb_kernel::format::{RelId, RowId};
use redlinedb_kernel::heap::MemHeap;
use redlinedb_kernel::txn::TxStatusTable;

#[test]
fn heap_insert_is_visible_after_commit() {
    let mut txs = TxStatusTable::new();
    let tx = txs.begin();
    let mut heap = MemHeap::new(RelId(1)).unwrap();

    let row_id = heap.insert(tx, b"alpha".to_vec()).unwrap();
    let before_commit = txs.snapshot();
    assert_eq!(heap.get(&txs, &before_commit, None, row_id).unwrap(), None);
    assert_eq!(
        heap.get(&txs, &before_commit, Some(tx), row_id).unwrap(),
        Some(b"alpha".to_vec())
    );

    txs.commit(tx);
    let after_commit = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &after_commit, None, row_id).unwrap(),
        Some(b"alpha".to_vec())
    );
}

#[test]
fn heap_update_uses_undo_for_old_snapshot() {
    let mut txs = TxStatusTable::new();
    let mut heap = MemHeap::new(RelId(1)).unwrap();

    let insert_tx = txs.begin();
    let row_id = heap.insert(insert_tx, b"old".to_vec()).unwrap();
    txs.commit(insert_tx);

    let old_snapshot = txs.snapshot();
    let update_tx = txs.begin();
    heap.update(update_tx, row_id, b"new".to_vec()).unwrap();
    txs.commit(update_tx);

    let new_snapshot = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row_id).unwrap(),
        Some(b"old".to_vec())
    );
    assert_eq!(
        heap.get(&txs, &new_snapshot, None, row_id).unwrap(),
        Some(b"new".to_vec())
    );
}

#[test]
fn heap_delete_preserves_old_snapshot_and_hides_new_snapshot() {
    let mut txs = TxStatusTable::new();
    let mut heap = MemHeap::new(RelId(1)).unwrap();

    let insert_tx = txs.begin();
    let row_id = heap.insert(insert_tx, b"live".to_vec()).unwrap();
    txs.commit(insert_tx);

    let old_snapshot = txs.snapshot();
    let delete_tx = txs.begin();
    heap.delete(delete_tx, row_id).unwrap();
    txs.commit(delete_tx);

    let new_snapshot = txs.snapshot();
    assert_eq!(
        heap.get(&txs, &old_snapshot, None, row_id).unwrap(),
        Some(b"live".to_vec())
    );
    assert_eq!(heap.get(&txs, &new_snapshot, None, row_id).unwrap(), None);
}

#[test]
fn missing_row_returns_none() {
    let txs = TxStatusTable::new();
    let snapshot = txs.snapshot();
    let heap = MemHeap::new(RelId(1)).unwrap();

    assert_eq!(heap.get(&txs, &snapshot, None, RowId(404)).unwrap(), None);
}
