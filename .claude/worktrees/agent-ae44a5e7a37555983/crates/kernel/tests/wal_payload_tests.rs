use redlinedb_kernel::Error;
use redlinedb_kernel::format::{Csn, PageGeneration, PageId, RelId, RowId, TxId};
use redlinedb_kernel::index::IndexRowRef;
use redlinedb_kernel::wal::WalPayload;

#[test]
fn wal_payload_variants_round_trip() {
    let payloads = [
        WalPayload::HeapInsert {
            tx_id: TxId(1),
            rel_id: RelId(11),
            row_id: RowId(2),
            payload: b"insert".to_vec(),
        },
        WalPayload::HeapUpdate {
            tx_id: TxId(3),
            rel_id: RelId(11),
            row_id: RowId(4),
            payload: b"update".to_vec(),
        },
        WalPayload::HeapDelete {
            tx_id: TxId(5),
            rel_id: RelId(11),
            row_id: RowId(6),
        },
        WalPayload::IndexInsert {
            tx_id: TxId(7),
            index_id: 12,
            logical_key: b"index-key".to_vec(),
            row: IndexRowRef::with_row_id(
                RowId(8),
                redlinedb_kernel::format::TuplePtr::new_with_generation(
                    PageId(13),
                    14,
                    PageGeneration::ONE,
                ),
            ),
        },
        WalPayload::IndexDelete {
            tx_id: TxId(9),
            index_id: 12,
            logical_key: b"index-key".to_vec(),
            row: IndexRowRef::with_row_id(
                RowId(8),
                redlinedb_kernel::format::TuplePtr::new_with_generation(
                    PageId(13),
                    14,
                    PageGeneration::ONE,
                ),
            ),
        },
        WalPayload::Commit {
            tx_id: TxId(10),
            csn: Csn(11),
        },
        WalPayload::CatalogSnapshot {
            tx_id: TxId(12),
            schema_epoch: 13,
            snapshot: b"catalog-snapshot".to_vec(),
        },
    ];

    for payload in payloads {
        let encoded = payload.encode().unwrap();
        let decoded = WalPayload::decode(&encoded).unwrap();
        assert_eq!(decoded, payload);
    }
}

#[test]
fn wal_payload_rejects_truncated_payload() {
    let payload = WalPayload::HeapInsert {
        tx_id: TxId(1),
        rel_id: RelId(11),
        row_id: RowId(2),
        payload: b"insert".to_vec(),
    };
    let mut encoded = payload.encode().unwrap();
    encoded.pop();

    let err = WalPayload::decode(&encoded).unwrap_err();
    assert!(matches!(err, Error::BufferTooSmall { .. }));
}

#[test]
fn wal_payload_rejects_unknown_tag() {
    let err = WalPayload::decode(&[99]).unwrap_err();
    assert_eq!(err, Error::CorruptWal("unknown wal payload tag"));
}

#[test]
fn wal_payload_index_insert_and_delete_round_trip() {
    let row = IndexRowRef::with_row_id(
        RowId(42),
        redlinedb_kernel::format::TuplePtr::new_with_generation(PageId(7), 3, PageGeneration::ONE),
    );
    let payloads = [
        WalPayload::IndexInsert {
            tx_id: TxId(1),
            index_id: 99,
            logical_key: b"tenant-1".to_vec(),
            row,
        },
        WalPayload::IndexDelete {
            tx_id: TxId(2),
            index_id: 99,
            logical_key: b"tenant-1".to_vec(),
            row,
        },
    ];

    for payload in payloads {
        let encoded = payload.encode().unwrap();
        let decoded = WalPayload::decode(&encoded).unwrap();
        assert_eq!(decoded, payload);
    }
}

#[test]
fn wal_payload_catalog_snapshot_rejects_truncated_body() {
    let payload = WalPayload::CatalogSnapshot {
        tx_id: TxId(9),
        schema_epoch: 10,
        snapshot: b"catalog-snapshot".to_vec(),
    };
    let mut encoded = payload.encode().unwrap();
    encoded.pop();

    let err = WalPayload::decode(&encoded).unwrap_err();
    assert!(matches!(err, Error::BufferTooSmall { .. }));
}
