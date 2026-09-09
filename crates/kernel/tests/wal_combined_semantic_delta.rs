//! WS-A6 wave 2: round-trip + recovery tests for the
//! [`WalPayload::CombinedSemanticDelta`] variant.
//!
//! The variant is emitted by the SQL-layer hot-row coordinator alongside
//! per-batch HeapUpdate records. Recovery treats it as an audit /
//! observability marker (the HeapUpdate provides the actual state) and
//! must accept and decode it cleanly without disturbing the existing
//! recovery pipeline.

use redlinedb_kernel::format::{RelId, RowId, TxId};
use redlinedb_kernel::wal::WalRecordKind;
use redlinedb_kernel::wal::manager::{WalConfig, WalCoordinator, WalReader};
use redlinedb_kernel::wal::{CombinedReplacementValue, WalPayload};
use tempfile::TempDir;

#[test]
fn combined_semantic_delta_round_trip_minimal() {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(42),
        rel_id: RelId(7),
        row_id: RowId(101),
        deltas: vec![(2, 1)],
        replacements: Vec::new(),
        batched_count: 1,
    };
    let encoded = payload.encode().expect("encode");
    let decoded = WalPayload::decode(&encoded).expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn combined_semantic_delta_round_trip_mixed() {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(1),
        rel_id: RelId(2),
        row_id: RowId(3),
        deltas: vec![(0, 5), (1, -3), (4, i64::MAX), (7, i64::MIN)],
        replacements: vec![
            (2, CombinedReplacementValue::Null),
            (3, CombinedReplacementValue::Integer(99)),
            (5, CombinedReplacementValue::Real(3.14159)),
            (6, CombinedReplacementValue::Text(b"hello".to_vec())),
            (
                8,
                CombinedReplacementValue::Blob(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            ),
        ],
        batched_count: 64,
    };
    let encoded = payload.encode().expect("encode");
    let decoded = WalPayload::decode(&encoded).expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn combined_semantic_delta_empty_deltas_and_replacements() {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(0),
        rel_id: RelId(0),
        row_id: RowId(0),
        deltas: Vec::new(),
        replacements: Vec::new(),
        batched_count: 0,
    };
    let encoded = payload.encode().expect("encode");
    let decoded = WalPayload::decode(&encoded).expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn combined_semantic_delta_rejects_truncated_body() {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(1),
        rel_id: RelId(2),
        row_id: RowId(3),
        deltas: vec![(0, 1)],
        replacements: Vec::new(),
        batched_count: 1,
    };
    let mut encoded = payload.encode().expect("encode");
    encoded.pop();
    let err = WalPayload::decode(&encoded).expect_err("should fail");
    assert!(
        matches!(err, redlinedb_kernel::Error::BufferTooSmall { .. }),
        "expected BufferTooSmall, got {err:?}",
    );
}

#[test]
fn combined_semantic_delta_tx_id_accessor() {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(777),
        rel_id: RelId(1),
        row_id: RowId(1),
        deltas: Vec::new(),
        replacements: Vec::new(),
        batched_count: 1,
    };
    assert_eq!(payload.tx_id(), TxId(777));
}

#[test]
fn unknown_payload_tag_still_rejected() {
    // Backward-compat invariant: any older binary that doesn't know
    // the CombinedSemanticDelta tag returns `CorruptWal("unknown wal
    // payload tag")`. Verify that adding tag 14 didn't change the
    // behaviour for an unrelated unknown tag (we pick 250, which
    // remains unallocated).
    let err = WalPayload::decode(&[250]).expect_err("must reject");
    assert_eq!(
        err,
        redlinedb_kernel::Error::CorruptWal("unknown wal payload tag"),
    );
}

#[test]
fn combined_semantic_delta_serial_order_replays_to_same_state() {
    // Document the correctness gate: any interleaving of (commutative
    // delta, last-write-wins replacement) updates collapses to a
    // single CombinedSemanticDelta whose serial replay produces the
    // same final row.
    //
    // This is a property-style test on the merge math; it doesn't go
    // through the engine because the kernel test crate has no SQL
    // executor. The SQL-side multi-writer test exercises the same
    // invariants end-to-end against the real heap.

    let initial = [10i64, 20i64, 30i64];
    let batched_deltas = vec![(0u16, 5i64), (0, -3), (1, 100)];
    let batched_replacements = vec![(2u16, CombinedReplacementValue::Integer(999))];

    // Serial replay: apply every delta and then every replacement.
    let mut state = initial;
    for (col, d) in &batched_deltas {
        state[*col as usize] = state[*col as usize].wrapping_add(*d);
    }
    for (col, value) in &batched_replacements {
        if let CombinedReplacementValue::Integer(n) = value {
            state[*col as usize] = *n;
        }
    }
    assert_eq!(state, [12, 120, 999]);

    // Encode/decode round-trip preserves the same final state when
    // re-applied.
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id: TxId(1),
        rel_id: RelId(1),
        row_id: RowId(1),
        deltas: batched_deltas.clone(),
        replacements: batched_replacements.clone(),
        batched_count: 3,
    };
    let encoded = payload.encode().expect("encode");
    let decoded = WalPayload::decode(&encoded).expect("decode");
    let WalPayload::CombinedSemanticDelta {
        deltas: decoded_deltas,
        replacements: decoded_replacements,
        ..
    } = decoded
    else {
        panic!("variant mismatch");
    };
    let mut state2 = initial;
    for (col, d) in &decoded_deltas {
        state2[*col as usize] = state2[*col as usize].wrapping_add(*d);
    }
    for (col, value) in &decoded_replacements {
        if let CombinedReplacementValue::Integer(n) = value {
            state2[*col as usize] = *n;
        }
    }
    assert_eq!(state2, state);
}

fn append_combined_semantic_delta(
    wal: &WalCoordinator,
    tx_id: TxId,
    rel_id: RelId,
    row_id: RowId,
    deltas: Vec<(u16, i64)>,
    replacements: Vec<(u16, CombinedReplacementValue)>,
    batched_count: u32,
) -> redlinedb_kernel::format::Lsn {
    let payload = WalPayload::CombinedSemanticDelta {
        tx_id,
        rel_id,
        row_id,
        deltas,
        replacements,
        batched_count,
    };
    wal.append(
        WalRecordKind::PageDelta,
        tx_id,
        payload.encode().expect("encode"),
    )
    .expect("append")
    .end_lsn
}

#[test]
fn semantic_combiner_off_keeps_adjacent_audit_records_separate() {
    let temp = TempDir::new().expect("tempdir");
    let wal = WalCoordinator::create(temp.path(), WalConfig::default()).expect("wal");

    let first_end = append_combined_semantic_delta(
        &wal,
        TxId(7),
        RelId(3),
        RowId(9),
        vec![(1, 2)],
        vec![(2, CombinedReplacementValue::Integer(3))],
        1,
    );
    let second_end = append_combined_semantic_delta(
        &wal,
        TxId(7),
        RelId(3),
        RowId(9),
        vec![(4, 5)],
        vec![(2, CombinedReplacementValue::Integer(11))],
        2,
    );
    wal.flush_until(second_end).expect("flush");

    let report = WalReader::new(temp.path(), WalConfig::default())
        .scan_report()
        .expect("scan");
    assert_eq!(report.records.len(), 2);
    assert!(first_end < second_end);
}

#[test]
fn semantic_combiner_on_folds_adjacent_audit_records() {
    // Repeated construction exercises the writer-startup schedule that used
    // to drain the first unnotified candidate before the second append could
    // fold it. The write-request predicate makes every iteration deterministic.
    for iteration in 0..32 {
        let temp = TempDir::new().expect("tempdir");
        let config = WalConfig {
            semantic_combiner: true,
            ..WalConfig::default()
        };
        let wal = WalCoordinator::create(temp.path(), config.clone()).expect("wal");

        let first_end = append_combined_semantic_delta(
            &wal,
            TxId(7),
            RelId(3),
            RowId(9),
            vec![(1, 2)],
            vec![(2, CombinedReplacementValue::Integer(3))],
            1,
        );
        let second_end = append_combined_semantic_delta(
            &wal,
            TxId(7),
            RelId(3),
            RowId(9),
            vec![(4, 5)],
            vec![(2, CombinedReplacementValue::Integer(11))],
            2,
        );
        wal.flush_until(second_end).expect("flush");

        let report = WalReader::new(temp.path(), config)
            .scan_report()
            .expect("scan");
        assert_eq!(
            report.records.len(),
            1,
            "iteration {iteration} did not fold the pending pair"
        );
        assert!(first_end < second_end);

        let decoded = WalPayload::decode(&report.records[0].payload).expect("decode merged");
        assert_eq!(
            decoded,
            WalPayload::CombinedSemanticDelta {
                tx_id: TxId(7),
                rel_id: RelId(3),
                row_id: RowId(9),
                deltas: vec![(1, 2), (4, 5)],
                replacements: vec![(2, CombinedReplacementValue::Integer(11))],
                batched_count: 3,
            }
        );
    }
}
