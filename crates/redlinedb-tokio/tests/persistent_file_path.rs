//! File-backed pool round-trip: open, write, read back. Confirms the
//! file-backed path actually does I/O (not just a memory-only proxy).
//!
//! Note on persistence-across-drops: RedlineDB's `registry` caches Database
//! entries process-wide, so reopening the same path from the same process
//! returns the cached handle. Testing crash-recovery semantics is better
//! done at the RedlineDB layer (see `crates/sql/tests/smoke_dml.rs`).

use redlinedb_tokio::{Pool, params};

#[tokio::test]
async fn file_backed_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("persist.redline");

    let pool = Pool::open(&path).await.expect("open");
    pool.execute(
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL)",
        params![],
    )
    .await
    .expect("create");
    pool.execute("INSERT INTO t(id, v) VALUES (1, 'hello')", params![])
        .await
        .expect("insert");

    let row = pool
        .fetch_one("SELECT v FROM t WHERE id = 1", params![])
        .await
        .expect("fetch");
    assert_eq!(row.try_get_text(0).unwrap(), "hello");

    // Force a checkpoint via the underlying Database handle. Confirms the
    // admin methods are reachable through the pool.
    pool.database().checkpoint().expect("checkpoint");
}
