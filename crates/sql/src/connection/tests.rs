use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;

use super::cache::{StatementCache, StatementCacheKey};
use super::{Connection, Database, DbOptions};
use crate::error::Error;
use crate::session::BeginMode;
use crate::statement::Step;

fn new_db() -> (tempfile::TempDir, Arc<Database>, Arc<Connection>) {
    new_db_with_timeout(Duration::from_secs(5))
}

fn new_db_with_timeout(timeout: Duration) -> (tempfile::TempDir, Arc<Database>, Arc<Connection>) {
    let dir = tempdir().expect("scratch dir");
    let path = dir.path().join("sql-conn-test.db");
    let opts = DbOptions {
        busy_timeout: timeout,
        ..DbOptions::default()
    };
    let db = Database::create(&path, opts).expect("db");
    let conn = db.connect();
    (dir, db, conn)
}

#[test]
fn execute_uses_active_transaction() {
    let (_dir, db, conn1) = new_db();
    let conn2 = db.connect();

    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    conn1.begin(BeginMode::Deferred).expect("begin");
    conn1
        .execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert");

    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Done);

    conn1.commit().expect("commit");

    let mut stmt = conn2
        .prepare("SELECT v FROM t WHERE id = 1")
        .expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("value"), "one");
}

#[test]
fn prepare_reuses_cached_templates() {
    let (_dir, _db, conn) = new_db();

    let stmt1 = conn.prepare("SELECT 1").expect("prepare");
    let stmt2 = conn.prepare("SELECT 1").expect("prepare");

    assert!(Arc::ptr_eq(&stmt1.template, &stmt2.template));
}

#[test]
fn begin_immediate_reserves_writer_slot() {
    let (_dir, db, conn1) = new_db_with_timeout(Duration::from_millis(25));
    let conn2 = db.connect();

    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    conn1.begin(BeginMode::Immediate).expect("begin immediate");

    let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
    assert_eq!(
        err,
        Error::Kernel(redlinedb_kernel::error::Error::LockTimeout)
    );

    conn1.rollback().expect("rollback");
}

#[test]
fn set_busy_timeout_updates_future_lock_waits() {
    let (_dir, db, conn1) = new_db_with_timeout(Duration::from_secs(5));
    let conn2 = db.connect();

    conn1
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    conn1.begin(BeginMode::Immediate).expect("begin immediate");
    conn2.set_busy_timeout(Duration::from_millis(25));

    let err = conn2.begin(BeginMode::Immediate).expect_err("conflict");
    assert_eq!(
        err,
        Error::Kernel(redlinedb_kernel::error::Error::LockTimeout)
    );

    conn1.rollback().expect("rollback");
}

#[test]
fn schema_with_constant_expressions_reopens() {
    let dir = tempdir().expect("scratch dir");
    let path = dir.path().join("const-expr-schema.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        let conn = db.connect();
        conn.execute(
            "CREATE TABLE t(\
                id INTEGER PRIMARY KEY,\
                v INTEGER DEFAULT 7,\
                label TEXT DEFAULT 'ready',\
                CHECK (1 = 1)\
            )",
        )
        .expect("create table");
    }

    let db = Database::open(&path, DbOptions::default()).expect("reopen db");
    let conn = db.connect();
    conn.execute("INSERT INTO t(id) VALUES (1)")
        .expect("insert defaulted row");
    let mut stmt = conn
        .prepare("SELECT v, label FROM t WHERE id = 1")
        .expect("prepare select");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("default integer"), 7);
    assert_eq!(stmt.column_text(1).expect("default text"), "ready");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

/// New test (jankurai/repair-g): inserts and retrieves into the sharded
/// statement cache to confirm shard-routing and value preservation. The
/// cache has no fixed capacity (it's a per-shard `HashMap` with no
/// eviction), so we verify the lookup contract rather than LRU eviction:
/// an inserted key returns the original template, and an absent key
/// returns `None`.
#[test]
fn statement_cache_insert_and_lookup() {
    use crate::statement::{ParamLayout, PragmaPlan, PreparedKind, PreparedTemplate};
    use redlinedb_kernel::catalog::SchemaEpoch;

    let cache = StatementCache::new();

    // Build a synthetic template that doesn't touch the planner.
    let make_template = |sql: &str| -> Arc<PreparedTemplate> {
        Arc::new(PreparedTemplate {
            sql: Arc::from(sql),
            schema_epoch: SchemaEpoch(0),
            stats_epoch: 0,
            optimizer_hash: 0,
            param_layout: ParamLayout::default(),
            output_columns: Arc::from([]),
            readonly: true,
            kind: PreparedKind::Pragma(PragmaPlan::SetForeignKeys(false)),
        })
    };

    // Insert capacity+1 entries across several distinct SQL strings, then
    // confirm every key still resolves (cache is non-evicting).
    let mut keys = Vec::new();
    for i in 0..128 {
        let sql_str = format!("SELECT {i}");
        let key = StatementCacheKey {
            schema_epoch: 0,
            stats_epoch: 0,
            optimizer_hash: 0,
            sql: Arc::from(sql_str.as_str()),
        };
        let template = make_template(&sql_str);
        cache.insert(key.clone(), Arc::clone(&template));
        keys.push((key, template));
    }
    for (key, template) in &keys {
        let got = cache.get(key).expect("cached entry");
        assert!(Arc::ptr_eq(&got, template));
    }

    // Absent key returns None.
    let missing = StatementCacheKey {
        schema_epoch: 0,
        stats_epoch: 0,
        optimizer_hash: 0,
        sql: Arc::from("SELECT not-cached"),
    };
    assert!(cache.get(&missing).is_none());
}

/// New test (jankurai/repair-g): the public `DbOptions::default()` must
/// produce sane operational invariants. Catches accidental regressions
/// where a future tweak sets a limit to zero (which would silently
/// disable a feature) or yields a negative busy-timeout window.
#[test]
fn db_options_default_invariants() {
    let opts = DbOptions::default();

    assert!(
        opts.unique_lock_shards > 0,
        "unique_lock_shards must be > 0"
    );
    assert!(
        opts.busy_timeout >= Duration::ZERO,
        "busy_timeout must be non-negative"
    );

    let mem = &opts.query_memory;
    assert!(mem.work_mem_bytes > 0, "work_mem_bytes must be > 0");
    assert!(mem.max_spill_bytes > 0, "max_spill_bytes must be > 0");
    assert!(mem.batch_rows > 0, "batch_rows must be > 0");

    let stats = &opts.stats;
    assert!(
        stats.exact_analyze_row_threshold > 0,
        "exact_analyze_row_threshold must be > 0"
    );
    assert!(stats.sample_rows > 0, "sample_rows must be > 0");
    assert!(stats.mcv_capacity > 0, "mcv_capacity must be > 0");
    assert!(stats.histogram_buckets > 0, "histogram_buckets must be > 0");

    let opt = &opts.optimizer;
    assert!(opt.enabled, "optimizer enabled by default");
    assert!(
        opt.max_exact_join_tables > 0,
        "max_exact_join_tables must be > 0"
    );
    assert!(
        opt.max_join_alternatives > 0,
        "max_join_alternatives must be > 0"
    );

    assert!(opts.temp_dir.is_none(), "temp_dir defaults to None");
}

#[test]
fn execute_batch_stops_on_error_and_preserves_previous_writes() {
    let (_dir, _db, conn) = new_db();

    conn.execute_batch(
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER DEFAULT 1);\
         INSERT INTO t(v) VALUES (10);\
         INSERT INTO t(id, v) VALUES (1, 20);\
         INSERT INTO t(v) VALUES (30);",
    )
    .expect_err("duplicate primary-key insert should fail before later statements");

    let mut after = conn
        .prepare("SELECT count(*) FROM t")
        .expect("query row count statement");
    assert_eq!(after.step().expect("step row"), Step::Row);
    assert_eq!(after.column_i64(0).expect("count"), 1);
    assert_eq!(after.step().expect("step done"), Step::Done);
}

#[test]
fn execute_batch_executes_explicit_transactions() {
    let (_dir, _db, conn) = new_db();

    conn.execute_batch(
        "CREATE TABLE t(id INTEGER PRIMARY KEY);\
         BEGIN IMMEDIATE;\
         INSERT INTO t VALUES (1);\
         COMMIT;\
         SELECT COUNT(*) FROM t",
    )
    .expect("batched tx script");

    let mut count = conn
        .prepare("SELECT COUNT(*) FROM t")
        .expect("count prepared");
    assert_eq!(count.step().expect("count step"), Step::Row);
    assert_eq!(count.column_i64(0).expect("count value"), 1);
}

#[test]
fn query_map_maps_rows_and_preserves_order_after_callback_error() {
    let (_dir, _db, conn) = new_db();

    conn.execute_batch(
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER); INSERT INTO t(v) VALUES (1), (2), (3)",
    )
    .expect("setup");

    let mut stmt = conn
        .prepare("SELECT v FROM t ORDER BY v")
        .expect("prepared select");
    let mut rows = stmt.query_map(|row| {
        let value = row.column_i64(0)?;
        if value == 2 {
            Err(Error::UnsupportedSql("mapped stop".to_owned()))
        } else {
            Ok(value)
        }
    });

    assert_eq!(rows.next().expect("first"), Ok(1));
    assert_eq!(
        rows.next().expect("second"),
        Err(Error::UnsupportedSql("mapped stop".to_owned()))
    );
    assert_eq!(rows.next().expect("third"), Ok(3));
    assert!(rows.next().is_none());
}

#[test]
fn create_virtual_table_is_unsupported_without_module_migration() {
    let (_dir, _db, conn) = new_db();

    let err = conn.execute("CREATE VIRTUAL TABLE boxes USING rtree (id, x1, x2, y1, y2)");
    match err {
        Ok(v) => panic!("expected failure, got ok({v:?})"),
        Err(Error::UnsupportedSql(message)) => assert!(
            message.contains("CREATE VIRTUAL TABLE"),
            "unexpected unsupported message: {message}"
        ),
        Err(other) => panic!("unexpected create virtual table error: {other:?}"),
    }

    let mut probe = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='boxes'")
        .expect("probe prepared");
    assert_eq!(probe.step().expect("probe step"), Step::Done);
}
