use std::fs;

use redlinedb_kernel::catalog::{ColumnId, IndexId, TableId};
use redlinedb_kernel::catalog::{
    ColumnStats, HistogramBucket, IndexStats, MostCommonValue, StatsEpoch, StatsSnapshot,
    StatsStore, TableStats,
};
use redlinedb_kernel::format::{Csn, RelId};
use tempfile::TempDir;

#[test]
fn stats_snapshot_round_trips_through_binary_encoding() {
    let mut snapshot = StatsSnapshot::empty(StatsEpoch(7));
    snapshot.tables.insert(
        TableId(1),
        TableStats {
            table_id: TableId(1),
            rel_id: RelId(10),
            row_count: 42,
            live_row_count: 40,
            heap_pages: 3,
            avg_row_bytes: 12.5,
            analyzed_at_csn: Csn(99),
            data_change_count: 8,
        },
    );
    snapshot.columns.insert(
        (TableId(1), ColumnId(2)),
        ColumnStats {
            null_frac: 0.25,
            ndv: 3.0,
            avg_width: 4.0,
            min: Some(redlinedb_kernel::catalog::OwnedValue::Integer(1)),
            max: Some(redlinedb_kernel::catalog::OwnedValue::Integer(9)),
            mcv: vec![MostCommonValue {
                value: redlinedb_kernel::catalog::OwnedValue::Integer(1),
                frequency: 0.5,
            }],
            histogram: vec![HistogramBucket {
                lower: Some(redlinedb_kernel::catalog::OwnedValue::Integer(1)),
                upper: Some(redlinedb_kernel::catalog::OwnedValue::Integer(9)),
                frequency: 1.0,
            }],
        },
    );
    snapshot.indexes.insert(
        IndexId(3),
        IndexStats {
            index_id: IndexId(3),
            entries: 42,
            leaf_pages: 2,
            height: 1,
            distinct_prefix_counts: vec![12.0],
            avg_key_bytes: 8.0,
            clustering_factor: 1.0,
        },
    );

    let dir = TempDir::new().expect("temp dir");
    let store = StatsStore::new(dir.path());
    store.save(&snapshot).expect("save");
    let loaded = store.load().expect("load").expect("present");
    assert_eq!(&*loaded, &snapshot);
}

#[test]
fn malformed_stats_file_falls_back_to_empty_snapshot() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("stats.redline");
    fs::write(&path, b"not-a-valid-stats-file").expect("write");

    let store = StatsStore::new(dir.path());
    assert!(store.load().expect("load").is_none());
}
