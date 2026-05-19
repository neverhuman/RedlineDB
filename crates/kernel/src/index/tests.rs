use super::*;

#[test]
fn leaf_cell_round_trips_payload_and_visibility_fields() {
    let row = IndexRowRef {
        row_id: crate::format::RowId(17),
        tuple: TuplePtr::new_with_generation(PageId(23), 4, PageGeneration(5)),
    };
    let encoded = LeafCell::encode(b"leaf", row, b"payload", TxId(31), TxId(0));
    let decoded = LeafCell::decode(&encoded).unwrap();
    assert_eq!(
        decoded,
        Entry::Leaf {
            logical_key: b"leaf".to_vec(),
            row,
            physical: b"payload".to_vec(),
            create_tx: TxId(31),
            delete_tx: TxId(0),
        }
    );
    let cell_ref = LeafCell::decode_ref(&encoded).unwrap();
    assert_eq!(cell_ref.logical_key, b"leaf");
    assert_eq!(cell_ref.row, row);
    assert_eq!(cell_ref.create_tx, TxId(31));
    assert_eq!(cell_ref.delete_tx, TxId(0));
}

#[test]
fn internal_cell_round_trips_separator_and_child() {
    let encoded = InternalCell::encode(b"branch", PageId(99));
    let decoded = InternalCell::decode(&encoded).unwrap();
    assert_eq!(
        decoded,
        Entry::Internal {
            separator: b"branch".to_vec(),
            child: PageId(99),
        }
    );
}
