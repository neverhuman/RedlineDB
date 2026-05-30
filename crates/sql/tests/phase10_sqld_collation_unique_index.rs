//! Parity case 10340 — NOCASE collated UNIQUE index as UPSERT conflict target.
//!
//! SQLite treats `CREATE UNIQUE INDEX … (col COLLATE NOCASE)` as a
//! case-insensitive uniqueness constraint. An `ON CONFLICT(col COLLATE NOCASE)`
//! UPSERT arm must fire when the incoming value matches an existing value under
//! NOCASE comparison, even if the raw bytes differ.
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("nocase_unique.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn rows(conn: &Arc<Connection>, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while matches!(stmt.step().expect("step"), Step::Row) {
        let cols = (0..stmt.column_count()).map(|i| {
            stmt.column_text(i)
                .map(|s| s.to_owned())
                .unwrap_or_else(|_| stmt.column_i64(i).unwrap_or(0).to_string())
        });
        out.push(cols.collect::<Vec<_>>().join("|"));
    }
    out
}

/// Case 10340: exact reproduction of the redline-testing corpus SQL.
///
/// INSERT 'apple' into a table that already holds 'Apple' under a
/// `COLLATE NOCASE` unique index. The UPSERT arm fires and updates b to
/// `upper(excluded.b)` = 'APPLE'. Result: one row with a=1, b='APPLE'.
#[test]
fn nocase_unique_index_upsert_fires_on_case_insensitive_conflict() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES(1,'Apple')")
        .expect("insert Apple");
    conn.execute(
        "INSERT INTO t VALUES(2,'apple') \
         ON CONFLICT(b COLLATE NOCASE) DO UPDATE SET b = upper(excluded.b)",
    )
    .expect("upsert apple");

    let result = rows(&conn, "SELECT a, b FROM t ORDER BY a");
    assert_eq!(
        result,
        vec!["1|APPLE"],
        "UPSERT should update row 1 to APPLE"
    );
}

/// Sanity: inserting a genuinely distinct value (case-sensitive diverge but
/// NOCASE collision) should NOT produce two rows.
#[test]
fn nocase_unique_index_blocks_duplicate() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES(1,'Apple')")
        .expect("insert Apple");
    // Attempting a plain INSERT without ON CONFLICT should fail.
    let result = conn.execute("INSERT INTO t VALUES(2,'apple')");
    assert!(
        result.is_err(),
        "plain INSERT of a NOCASE duplicate should fail with UNIQUE constraint"
    );
    // Table must still have exactly one row.
    let result = rows(&conn, "SELECT COUNT(*) FROM t");
    assert_eq!(result, vec!["1"]);
}

/// NOCASE uniqueness preserves the original casing of the stored value;
/// only the index key is normalised.
#[test]
fn nocase_unique_index_do_nothing_preserves_original_casing() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES(1,'Apple')")
        .expect("insert Apple");
    conn.execute(
        "INSERT INTO t VALUES(2,'APPLE') \
         ON CONFLICT(b COLLATE NOCASE) DO NOTHING",
    )
    .expect("upsert do-nothing");

    let result = rows(&conn, "SELECT a, b FROM t ORDER BY a");
    // Row 1 is unchanged; row 2 was silently dropped.
    assert_eq!(result, vec!["1|Apple"]);
}

#[test]
fn nocase_unique_index_backfill_rejects_existing_duplicate() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES(1,'Apple')")
        .expect("insert Apple");
    conn.execute("INSERT INTO t VALUES(2,'apple')")
        .expect("insert apple");

    let result = conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)");
    assert!(
        result.is_err(),
        "CREATE UNIQUE INDEX should reject existing NOCASE duplicates"
    );
}

#[test]
fn nocase_unique_index_backfill_blocks_future_duplicate() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES(1,'Apple')")
        .expect("insert Apple");
    conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)")
        .expect("create index");

    let result = conn.execute("INSERT INTO t VALUES(2,'apple')");
    assert!(
        result.is_err(),
        "backfilled NOCASE unique index should block future duplicate"
    );
}

#[test]
fn nocase_unique_index_survives_reopen_without_catalog_format_bump() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("nocase_reopen.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
            .expect("create table");
        conn.execute("CREATE UNIQUE INDEX ix_t_b_ci ON t(b COLLATE NOCASE)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES(1,'Apple')")
            .expect("insert Apple");
    }

    let db = Database::open(&path, DbOptions::default()).expect("reopen db");
    let conn = db.connect();
    conn.execute(
        "INSERT INTO t VALUES(2,'apple') \
         ON CONFLICT(b COLLATE NOCASE) DO UPDATE SET b = upper(excluded.b)",
    )
    .expect("upsert after reopen");

    let result = rows(&conn, "SELECT a, b FROM t ORDER BY a");
    assert_eq!(result, vec!["1|APPLE"]);
}
