use redlinedb_kernel::format::tuple::TupleVisibility;
use redlinedb_kernel::format::{Csn, RelId, RowId, TupleVersion, TxId};
use redlinedb_kernel::txn::{TxStatusTable, UndoKind, UndoRecord};

#[test]
fn tuple_version_round_trips() {
    let tuple = TupleVersion::new(RowId(99), RelId(11), TxId(7), b"row bytes".to_vec());

    let encoded = tuple.encode().unwrap();
    let decoded = TupleVersion::decode(&encoded).unwrap();

    assert_eq!(decoded, tuple);
}

#[test]
fn tuple_visibility_respects_snapshot_and_delete_tx() {
    let mut table = TxStatusTable::new();

    let insert_tx = table.begin();
    table.commit(insert_tx);
    let visible_snapshot = table.snapshot();

    let delete_tx = table.begin();
    let mut tuple = TupleVersion::new(RowId(1), RelId(11), insert_tx, b"payload".to_vec());
    tuple.end_tx = delete_tx;

    assert_eq!(
        tuple.visibility(&table, &visible_snapshot, None),
        TupleVisibility::Visible
    );

    table.commit(delete_tx);
    let deleted_snapshot = table.snapshot();
    assert_eq!(
        tuple.visibility(&table, &deleted_snapshot, None),
        TupleVisibility::Invisible
    );
}

#[test]
fn tuple_with_uncommitted_begin_is_visible_only_to_owner() {
    let mut table = TxStatusTable::new();
    let tx = table.begin();
    let snapshot = table.snapshot();
    let tuple = TupleVersion::new(RowId(1), RelId(11), tx, b"payload".to_vec());

    assert_eq!(
        tuple.visibility(&table, &snapshot, None),
        TupleVisibility::Invisible
    );
    assert_eq!(
        tuple.visibility(&table, &snapshot, Some(tx)),
        TupleVisibility::Visible
    );
}

#[test]
fn undo_record_round_trips_before_image() {
    let undo = UndoRecord {
        kind: UndoKind::UpdateBeforeImage,
        tx_id: TxId(7),
        row_id: RowId(9),
        prev_undo: redlinedb_kernel::format::UndoPtr(3),
        before_image: b"old tuple".to_vec(),
    };

    let encoded = undo.encode().unwrap();
    let decoded = UndoRecord::decode(&encoded).unwrap();

    assert_eq!(decoded, undo);
}

#[test]
fn committed_tuple_can_be_hint_freezed_without_changing_visibility() {
    let mut table = TxStatusTable::new();
    let tx = table.begin();
    let csn = table.commit(tx);
    let snapshot = table.snapshot();
    let mut tuple = TupleVersion::new(RowId(1), RelId(11), tx, b"payload".to_vec());
    tuple.begin_csn_hint = Csn(csn.0);

    assert_eq!(
        tuple.visibility(&table, &snapshot, None),
        TupleVisibility::Visible
    );
}
