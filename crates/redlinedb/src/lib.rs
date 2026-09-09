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
mod pool;
mod registry;
mod snapshot;
mod statement;
mod value;

pub mod metrics;

#[cfg(feature = "tokio")]
mod asyncio;

#[cfg(feature = "tokio")]
pub use asyncio::{AsyncConnection, AsyncDatabase};

pub use pool::{Pool, PoolBuilder, PooledConnection};

pub use connection::{Connection, InterruptHandle, Transaction};
pub use error::{Error, ErrorCode, Result};
pub use handle::Database;
pub use iter::{FromRow, FromValue, OwnedStep, QueryMap, Row, Step};
pub use machine::{
    BinaryOp, ColumnRef, DeleteSpec, ExprSpec, InsertSpec, OrderSpec, QuerySpec, SchemaHandle,
    SelectSpec, TableRef, UnaryOp, UpdateSpec,
};
pub use options::{
    AnalyzeOptions, BackupOptions, BackupStats, BenchmarkStats, BufferStats, CheckpointBenchStats,
    CheckpointStats, CommitStats, ConnectionStats, DatabaseStats, Durability, ExecuteSummary,
    FunctionArity, FunctionFlags, LEAN_BUFFER_POOL_PAGES, LEAN_STATEMENT_CACHE_CAPACITY,
    MemoryOptions, OpenOptions, OptimizerOptions, QueryMemoryOptions, TxBenchStats, VacuumStats,
    WalBenchStats,
};
pub use params::Params;
pub use phase8::{
    ArchiveMode, ArchiveStats, PhysicalBackupOptions, PhysicalBackupStats, ReplicationSlot,
    ReplicationSlotStats, RestoreOptions, RestoreStats, RetentionHorizon, SlotKind, WalLevel,
};
pub use redlinedb_kernel::engine::CommitDurability;
pub use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId, WalSegmentNo};
pub use redlinedb_sql::BeginMode;
pub use redlinedb_sql::RecoveryTarget;
pub use redlinedb_sql::{
    RqlBeginMode, RqlBinaryOp, RqlColumnDef, RqlColumnRef, RqlCreateIndex, RqlCreateTable,
    RqlDelete, RqlDropIndex, RqlDropTable, RqlExpr, RqlIndexColumn, RqlInsert, RqlJoin,
    RqlJoinKind, RqlLiteral, RqlName, RqlOrder, RqlProgram, RqlSelect, RqlSelectItem, RqlStatement,
    RqlTableRef, RqlUnaryOp, RqlUpdate, RqlUpdateAssignment,
};
// Re-export the SQLite-style `f64` formatter so the CLI / FFI layers can
// render REAL columns with the same `%!.17g` rounding the SQL evaluator
// uses internally — required for sqlite-parity on math-function output.
pub use redlinedb_sql::format_real_sqlite;
pub use statement::{OwnedStatement, Prepared, Rows, Statement};
pub use value::{Value, ValueRef};

// `registry::open_database` and friends call `crate::sql_options`; keep the
// path stable by re-exporting the implementation hosted in `handle`.
pub(crate) use handle::{private_in_memory_sql_options, sql_options};

/// True when the input contains a complete first SQL statement.
pub fn sql_input_complete(sql: &str) -> bool {
    redlinedb_sql::first_statement_complete(sql)
}

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
                1_i64,
                7_i64,
                1234_i64,
                200_000_i64,
                200_i64,
                50_000_i64,
                3_000_000_i64,
                5_000_000_i64,
                1.5_f64,
                vec![9_u8, 8, 7]
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
    fn execute_batch_runs_script_in_order_without_row_counts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("batch.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute_batch(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER);\
             INSERT INTO t(v) VALUES (10), (20);\
             INSERT INTO t(v) VALUES (30)",
        )
        .expect("batch");

        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM t")
            .expect("count prepared");
        let count = match stmt.step().expect("count step") {
            Step::Row(row) => row.get::<i64>(0).expect("row value"),
            Step::Done => panic!("expected row"),
        };
        assert_eq!(count, 3);
    }

    #[test]
    fn execute_batch_enforces_read_only_for_each_statement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("batch-read-only.redline");
        {
            let db = Database::create(&path).expect("db");
            let mut conn = db.connect().expect("conn");
            conn.execute("CREATE TABLE t(v INTEGER)", ())
                .expect("create");
        }

        let db = Database::open_with_options(
            &path,
            OpenOptions::default()
                .with_create(false)
                .with_read_only(true),
        )
        .expect("read-only db");
        let mut conn = db.connect().expect("read-only conn");
        let err = conn
            .execute_batch("SELECT COUNT(*) FROM t; INSERT INTO t VALUES (1)")
            .expect_err("mutation in a read-only batch must fail");
        assert_eq!(err.code(), ErrorCode::ReadOnly);
        assert_eq!(
            conn.query_row::<_, i64>("SELECT COUNT(*) FROM t", ())
                .expect("readback"),
            0
        );
    }

    #[test]
    fn execute_batch_honors_prearmed_interrupt_before_execution() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("batch-interrupt.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        let interrupt = conn.interrupt_handle();
        interrupt.interrupt();

        let err = conn
            .execute_batch("CREATE TABLE must_not_exist(v INTEGER)")
            .expect_err("pre-armed interrupt must stop the batch");
        assert_eq!(err.code(), ErrorCode::Interrupt);
    }

    #[test]
    fn execute_batch_stops_on_error_and_preserves_prior_statements() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("batch-error.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        let err = conn
            .execute_batch(
                "CREATE TABLE t(id INTEGER PRIMARY KEY);\
                 INSERT INTO t VALUES (1);\
                 INSERT INTO t VALUES (1);\
                 INSERT INTO t VALUES (2)",
            )
            .expect_err("duplicate key must stop the batch");
        assert_eq!(err.code(), ErrorCode::Constraint);
        assert_eq!(
            conn.query_row::<_, i64>("SELECT COUNT(*) FROM t", ())
                .expect("readback"),
            1
        );
    }

    #[test]
    fn execute_batch_preserves_savepoints_and_comment_only_noops() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("batch-savepoint.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute_batch("  -- no statement\n /* still no statement */ ;")
            .expect("comment-only batch");
        conn.execute_batch(
            "CREATE TABLE t(v INTEGER);\
             BEGIN IMMEDIATE;\
             INSERT INTO t VALUES (1);\
             SAVEPOINT discard_second;\
             INSERT INTO t VALUES (2);\
             ROLLBACK TO discard_second;\
             RELEASE discard_second;\
             COMMIT",
        )
        .expect("savepoint batch");

        assert_eq!(
            conn.query_row::<_, i64>("SELECT SUM(v) FROM t", ())
                .expect("readback"),
            1
        );
    }

    #[test]
    fn create_virtual_table_fails_closed_with_unsupported_code() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("virtual.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        let err = conn
            .execute(
                "create\nvirtual\ttable boxes USING rtree (id, x1, x2, y1, y2)",
                (),
            )
            .expect_err("virtual table without a module must fail");
        assert_eq!(
            err.code(),
            ErrorCode::Unsupported,
            "unexpected virtual-table error: {err}"
        );
        assert!(err.message().contains("CREATE VIRTUAL TABLE"));

        let count = conn
            .query_row::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'boxes'",
                (),
            )
            .expect("probe catalog");
        assert_eq!(count, 0);
    }

    #[test]
    fn commented_virtual_table_prefix_is_typed_unsupported_on_every_facade_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("virtual-comments.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        let variants = [
            "/* lead */ CREATE VIRTUAL TABLE boxes USING rtree (id, x1, x2)",
            "CREATE /* between */ VIRTUAL -- line\n TABLE boxes USING rtree (id, x1, x2)",
            "cReAtE\n/* one */ vIrTuAl\t/* two */ TaBlE IF NOT EXISTS boxes USING rtree (id, x1, x2)",
        ];

        for sql in variants {
            let prepare_err = match conn.prepare(sql) {
                Ok(_) => panic!("prepare accepted virtual table: {sql}"),
                Err(err) => err,
            };
            assert_eq!(prepare_err.code(), ErrorCode::Unsupported, "{sql}");

            let execute_err = conn
                .execute(sql, ())
                .expect_err("execute accepted virtual table");
            assert_eq!(execute_err.code(), ErrorCode::Unsupported, "{sql}");

            let batch_err = conn
                .execute_batch(sql)
                .expect_err("batch accepted virtual table");
            assert_eq!(batch_err.code(), ErrorCode::Unsupported, "{sql}");
        }
    }

    #[test]
    fn borrowed_statement_query_map_maps_rows_with_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("query_map.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            (),
        )
        .expect("create");
        conn.execute("INSERT INTO t(name) VALUES (?)", params!["Ada"])
            .expect("insert");
        conn.execute("INSERT INTO t(name) VALUES (?)", params!["Lin"])
            .expect("insert");

        let mut stmt = conn
            .prepare("SELECT name FROM t WHERE id > ? ORDER BY id")
            .expect("prepare query");
        let rows = stmt
            .query_map(params![0_i64], |row| row.get::<String>(0))
            .expect("bind query parameters")
            .collect::<Result<Vec<_>>>()
            .expect("map rows");
        assert_eq!(rows, vec!["Ada".to_string(), "Lin".to_string()]);
    }

    #[test]
    fn query_map_callback_error_and_early_drop_leave_statement_ordered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("query-map-drop.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        conn.execute_batch("CREATE TABLE t(v INTEGER); INSERT INTO t VALUES (1), (2), (3)")
            .expect("setup");

        let mut stmt = conn.prepare("SELECT v FROM t ORDER BY v").expect("prepare");
        let mut rows = stmt
            .query_map((), |row| {
                let value = row.get::<i64>(0)?;
                if value == 2 {
                    Err(Error::unsupported("mapped stop"))
                } else {
                    Ok(value)
                }
            })
            .expect("query map");
        assert_eq!(rows.next().expect("first").expect("first value"), 1);
        assert_eq!(
            rows.next()
                .expect("second")
                .expect_err("mapped error")
                .code(),
            ErrorCode::Unsupported
        );
        drop(rows);

        match stmt.step().expect("resume after iterator drop") {
            Step::Row(row) => assert_eq!(row.get::<i64>(0).expect("third value"), 3),
            Step::Done => panic!("iterator drop consumed the remaining row"),
        }
        assert!(matches!(stmt.step().expect("done"), Step::Done));
    }

    #[test]
    fn from_value_narrowing_overflow_returns_mismatch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("overflow.redline")).expect("db");
        let mut conn = db.connect().expect("conn");

        conn.execute("CREATE TABLE t(big INTEGER)", ())
            .expect("create");
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
    fn query_row_returns_first_row_mapped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("qrow.redline")).expect("db");
        let mut conn = db.connect().expect("conn");
        conn.execute("CREATE TABLE t(id INTEGER, name TEXT)", ())
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 'Ada'), (2, 'Lin')", ())
            .expect("insert");

        let id: i64 = conn
            .query_row("SELECT id FROM t WHERE name = ?", ("Ada",))
            .expect("query_row");
        assert_eq!(id, 1);

        let missing = conn.query_row::<_, i64>("SELECT id FROM t WHERE name = ?", ("Z",));
        assert_eq!(missing.unwrap_err().code(), ErrorCode::NotFound);

        let opt: Option<i64> = conn
            .query_row_opt("SELECT id FROM t WHERE name = ?", ("Z",))
            .expect("opt");
        assert!(opt.is_none());

        let some_opt: Option<i64> = conn
            .query_row_opt("SELECT id FROM t WHERE name = ?", ("Ada",))
            .expect("some_opt");
        assert_eq!(some_opt, Some(1));
    }

    #[test]
    fn open_options_builder_chains_match_field_init() {
        let opts = OpenOptions::default()
            .with_busy_timeout(Duration::from_millis(250))
            .with_read_only(false)
            .with_statement_cache_capacity(64)
            .with_durability(Durability::Normal);
        assert_eq!(opts.busy_timeout, Duration::from_millis(250));
        assert!(!opts.read_only);
        assert_eq!(opts.statement_cache_capacity, 64);
        assert_eq!(opts.durability, Durability::Normal);
    }

    #[test]
    fn open_options_builder_opens_database_with_custom_busy_timeout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("opts.redline");
        let opts = OpenOptions::default().with_busy_timeout(Duration::from_millis(50));
        let db = Database::open_with_options(&path, opts).expect("open");
        let _conn = db.connect().expect("conn");
    }

    #[cfg(feature = "tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn async_database_round_trip_via_spawn_blocking() {
        use crate::{AsyncDatabase, BeginMode, Value};
        let dir = tempfile::tempdir().expect("tempdir");
        let db = AsyncDatabase::create(dir.path().join("async.redline"))
            .await
            .expect("create");
        let conn = db.connect().await.expect("connect");

        conn.execute(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string(),
            vec![],
        )
        .await
        .expect("create table");

        conn.execute(
            "INSERT INTO t(id, name) VALUES (?, ?)".to_string(),
            vec![Value::Integer(1), Value::Text(std::sync::Arc::from("Ada"))],
        )
        .await
        .expect("insert");

        let name: String = conn
            .query_row(
                "SELECT name FROM t WHERE id = ?".to_string(),
                vec![Value::Integer(1)],
            )
            .await
            .expect("query_row");
        assert_eq!(name, "Ada");

        let count: i64 = conn
            .transaction(BeginMode::Immediate, |c| {
                c.execute("INSERT INTO t(id, name) VALUES (?, ?)", (2_i64, "Lin"))?;
                c.query_row::<_, i64>("SELECT COUNT(*) FROM t", ())
            })
            .await
            .expect("transaction");
        assert_eq!(count, 2);
    }

    #[test]
    fn pool_get_returns_connection_and_drop_returns_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("pool.redline")).expect("db");
        let pool = Pool::builder(db).max_connections(2).build().expect("pool");

        assert_eq!(pool.idle(), 0);
        assert_eq!(pool.in_use(), 0);
        {
            let mut a = pool.get().expect("a");
            a.execute("CREATE TABLE t(id INTEGER)", ()).expect("create");
            assert_eq!(pool.in_use(), 1);

            let mut b = pool.get().expect("b");
            b.execute("INSERT INTO t VALUES (1)", ()).expect("insert");
            assert_eq!(pool.in_use(), 2);
        } // both drop
        assert_eq!(pool.in_use(), 0);
        assert_eq!(pool.idle(), 2);

        let mut c = pool.get().expect("c");
        assert_eq!(pool.in_use(), 1);
        assert_eq!(pool.idle(), 1);
        let count: i64 = c.query_row("SELECT COUNT(*) FROM t", ()).expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn pool_busy_timeout_applies_to_handed_out_connections() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("pool-bt.redline")).expect("db");
        let pool = Pool::builder(db)
            .max_connections(2)
            .busy_timeout(Duration::from_millis(15))
            .build()
            .expect("pool");

        let mut a = pool.get().expect("a");
        let mut b = pool.get().expect("b");
        a.execute("CREATE TABLE t(id INTEGER)", ()).expect("create");
        a.begin(BeginMode::Immediate).expect("begin");
        let err = b.begin(BeginMode::Immediate).expect_err("expected busy");
        assert_eq!(err.code(), ErrorCode::Busy);
        a.rollback().expect("rollback");
    }

    #[cfg(feature = "tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pool_acquire_async_returns_connection() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("pool-async.redline")).expect("db");
        let pool = Pool::builder(db).max_connections(4).build().expect("pool");

        {
            let mut conn = pool.acquire().await.expect("acquire");
            conn.execute("CREATE TABLE t(id INTEGER)", ())
                .expect("create");
        }
        assert_eq!(pool.in_use(), 0);
        assert_eq!(pool.idle(), 1);
    }

    #[test]
    fn pool_metrics_hook_fires_on_acquire() {
        use crate::metrics::{MetricResult, Metrics};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Default)]
        struct CountMetrics {
            acquires: AtomicUsize,
        }
        impl Metrics for CountMetrics {
            fn on_pool_acquire(&self, _wait: Duration, _result: MetricResult) {
                self.acquires.fetch_add(1, Ordering::Relaxed);
            }
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::create(dir.path().join("metrics.redline")).expect("db");
        let counter = Arc::new(CountMetrics::default());
        let pool = Pool::builder(db)
            .max_connections(2)
            .metrics(counter.clone())
            .build()
            .expect("pool");
        {
            let _a = pool.get().expect("a");
            let _b = pool.get().expect("b");
        }
        let _c = pool.get().expect("c");
        assert_eq!(counter.acquires.load(Ordering::Relaxed), 3);
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
