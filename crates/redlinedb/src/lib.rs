//! Public API surface for the `redlinedb` embedded database crate.
//!
//! This module is the public-facing facade: it declares the sub-modules,
//! re-exports the user-visible types/traits, and keeps the rest of the
//! implementation factored into focused files (`handle.rs`, `connection.rs`,
//! `statement.rs`, `iter.rs`, plus the existing `error`, `options`,
//! `params`, `phase8`, `registry`, `snapshot`, `value`, `machine`).

mod connection;
mod error;
mod handle;
mod iter;
mod machine;
mod options;
mod params;
mod phase8;
mod registry;
mod snapshot;
mod statement;
mod value;

pub use connection::{Connection, InterruptHandle, Transaction};
pub use error::{Error, ErrorCode, Result};
pub use handle::Database;
pub use iter::{FromValue, OwnedStep, Row, Step};
pub use machine::{
    BinaryOp, ColumnRef, DeleteSpec, ExprSpec, InsertSpec, OrderSpec, QuerySpec, SchemaHandle,
    SelectSpec, TableRef, UnaryOp, UpdateSpec,
};
pub use options::{
    AnalyzeOptions, BackupOptions, BackupStats, BenchmarkStats, BufferStats, CheckpointBenchStats,
    CheckpointStats, CommitStats, ConnectionStats, DatabaseStats, Durability, ExecuteSummary,
    FunctionArity, FunctionFlags, MemoryOptions, OpenOptions, OptimizerOptions, QueryMemoryOptions,
    TxBenchStats, VacuumStats, WalBenchStats,
};
pub use params::Params;
pub use phase8::{
    ArchiveMode, ArchiveStats, PhysicalBackupOptions, PhysicalBackupStats, ReplicationSlot,
    ReplicationSlotStats, RestoreOptions, RestoreStats, RetentionHorizon, SlotKind, WalLevel,
};
pub use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId, WalSegmentNo};
pub use redlinedb_sql::BeginMode;
pub use redlinedb_sql::RecoveryTarget;
pub use statement::{OwnedStatement, Prepared, Rows, Statement};
pub use value::{Value, ValueRef};

// `registry::open_database` and friends call `crate::sql_options`; keep the
// path stable by re-exporting the implementation hosted in `handle`.
pub(crate) use handle::sql_options;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn public_handle_thread_contracts_compile() {
        assert_send::<Database>();
        assert_sync::<Database>();
        assert_send::<Connection>();
    }

    #[test]
    fn owned_and_borrowed_statements_return_the_same_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("owned.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            (),
        )
        .expect("create");
        conn.execute(
            "INSERT INTO items(id, name) VALUES (?, ?)",
            params![1_i64, "Ada"],
        )
        .expect("insert");

        {
            let mut borrowed = conn
                .prepare("SELECT name FROM items WHERE id = ?")
                .expect("borrowed");
            borrowed.bind_all(params![1_i64]).expect("bind");
            match borrowed.step().expect("step") {
                Step::Row(row) => {
                    assert_eq!(row.get_ref(0).expect("ref"), ValueRef::Text("Ada"));
                }
                Step::Done => panic!("expected row"),
            }
            assert!(matches!(borrowed.step().expect("done"), Step::Done));
        }

        let mut owned = conn
            .prepare_owned("SELECT name FROM items WHERE id = ?")
            .expect("owned");
        owned.bind_value(1, Value::Integer(1)).expect("bind");
        match owned.step().expect("step") {
            OwnedStep::Row => {
                assert_eq!(owned.column_ref(0).expect("ref"), ValueRef::Text("Ada"));
            }
            OwnedStep::Done => panic!("expected row"),
        }
        assert!(matches!(owned.step().expect("done"), OwnedStep::Done));
    }

    #[test]
    fn benchmark_stats_surface_current_storage_counters() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("stats.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        conn.execute(
            "CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
            (),
        )
        .expect("create");
        conn.execute(
            "INSERT INTO kv(k, tenant, v, version) VALUES (?, ?, ?, ?)",
            params![1_i64, 1_i64, vec![1_u8, 2, 3], 1_i64],
        )
        .expect("insert");

        let stats = db.benchmark_stats().expect("benchmark stats");
        assert!(stats.buffer.resident_pages > 0);
        assert!(stats.wal.written_lsn >= stats.wal.durable_lsn);
        assert!(stats.tx.next_tx >= 1);
    }

    #[test]
    fn row_get_dispatches_to_typed_from_value_impls() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("fromvalue.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute(
            "CREATE TABLE t(\
                bool_col INTEGER, i8_col INTEGER, i16_col INTEGER, i32_col INTEGER,\
                u8_col INTEGER, u16_col INTEGER, u32_col INTEGER, u64_col INTEGER,\
                f32_col REAL, blob_col BLOB, null_text TEXT)",
            (),
        )
        .expect("create");
        conn.execute(
            "INSERT INTO t VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
            params![
                1_i64, 7_i64, 1234_i64, 200_000_i64, 200_i64, 50_000_i64, 3_000_000_i64,
                5_000_000_i64, 1.5_f64, vec![9_u8, 8, 7]
            ],
        )
        .expect("insert");

        let mut stmt = conn.prepare("SELECT * FROM t").expect("prep");
        match stmt.step().expect("step") {
            Step::Row(row) => {
                assert!(row.get::<bool>(0).expect("bool"));
                assert_eq!(row.get::<i8>(1).expect("i8"), 7);
                assert_eq!(row.get::<i16>(2).expect("i16"), 1234);
                assert_eq!(row.get::<i32>(3).expect("i32"), 200_000);
                assert_eq!(row.get::<u8>(4).expect("u8"), 200);
                assert_eq!(row.get::<u16>(5).expect("u16"), 50_000);
                assert_eq!(row.get::<u32>(6).expect("u32"), 3_000_000);
                assert_eq!(row.get::<u64>(7).expect("u64"), 5_000_000);
                assert!((row.get::<f32>(8).expect("f32") - 1.5).abs() < 1e-6);
                assert_eq!(row.get::<Vec<u8>>(9).expect("blob"), vec![9_u8, 8, 7]);
                assert_eq!(row.get::<Option<String>>(10).expect("none"), None);
            }
            Step::Done => panic!("expected row"),
        }
    }

    #[test]
    fn from_value_narrowing_overflow_returns_mismatch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("overflow.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute("CREATE TABLE t(big INTEGER)", ()).expect("create");
        conn.execute("INSERT INTO t VALUES (?)", params![i64::MAX])
            .expect("insert");

        let mut stmt = conn.prepare("SELECT big FROM t").expect("prep");
        match stmt.step().expect("step") {
            Step::Row(row) => {
                let err = row.get::<i32>(0).expect_err("should overflow");
                assert_eq!(err.code(), ErrorCode::Mismatch);
            }
            Step::Done => panic!("expected row"),
        }
    }

    #[test]
    fn tuple_params_bind_heterogeneous_types_directly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("tuples.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute(
            "CREATE TABLE t(a INTEGER, b TEXT, c REAL, d BLOB, e INTEGER)",
            (),
        )
        .expect("create");

        conn.execute(
            "INSERT INTO t VALUES (?, ?, ?, ?, ?)",
            (1_i64, "hello", 3.14_f64, vec![1_u8, 2, 3], true),
        )
        .expect("insert via 5-tuple");

        conn.execute(
            "INSERT INTO t VALUES (?, ?, ?, ?, ?)",
            (
                42_i32,
                String::from("world"),
                2.71_f32,
                &b"xyz"[..],
                Some(0_u8),
            ),
        )
        .expect("insert via 5-tuple with mixed Into<Value>");

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM t").expect("prep");
        match stmt.step().expect("step") {
            Step::Row(row) => assert_eq!(row.get::<i64>(0).expect("count"), 2),
            Step::Done => panic!("expected row"),
        }
    }

    #[test]
    fn connection_set_busy_timeout_applies_to_future_lock_conflicts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("timeout.redline")).expect("db");
        let mut conn1 = db.connect().expect("conn1");
        let mut conn2 = db.connect().expect("conn2");

        conn1
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)", ())
            .expect("create");
        conn1.begin(BeginMode::Immediate).expect("begin immediate");
        conn2.set_busy_timeout(Duration::from_millis(25));

        let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
        assert_eq!(err.code(), ErrorCode::Busy);

        conn1.rollback().expect("rollback");
    }
}
