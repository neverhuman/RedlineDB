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
    let dir = tempdir().expect("temp dir");
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
