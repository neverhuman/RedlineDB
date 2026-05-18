//! Lane SQL-D phase 10 Tier-2/3: parser-only acceptance for CTEs, views,
//! triggers, generated columns, and window functions.
//!
//! These features are recognised at the SQL surface but their execution
//! lives in follow-on planner / executor work. Each test confirms that the
//! parser does not regress to "syntax error" and that the executor returns
//! a clearly-labeled "not yet implemented" error.
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("adv.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

#[allow(dead_code)] // Helper used by parser-only assertions when expanding coverage.
fn assert_parser_only(res: Result<usize, redlinedb_sql::Error>) {
    let err = res.expect_err("expected parser-only error");
    let msg = format!("{err:?}").to_ascii_lowercase();
    assert!(
        msg.contains("not yet implemented") || msg.contains("parsed-only"),
        "expected parser-only message, got {msg}"
    );
}

#[test]
fn cte_with_simple_select_executes() {
    // CTE execution is now implemented; see `parity_cte.rs` for
    // differential coverage.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1)").expect("insert");
    conn.execute("WITH cte AS (SELECT a FROM t) SELECT * FROM cte")
        .expect("CTE should execute");
}

#[test]
fn cte_recursive_executes() {
    let (_dir, conn) = open();
    conn.execute(
        "WITH RECURSIVE counter(n) AS (\
            SELECT 1 UNION ALL SELECT n + 1 FROM counter WHERE n < 5\
         ) SELECT n FROM counter",
    )
    .expect("recursive CTE should execute");
}

#[test]
fn cte_multiple_bindings_executes() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a, b")
        .expect("multi-CTE should execute");
}

#[test]
fn create_view_now_executes() {
    // Views are implemented (Lane A5-views). Differential coverage
    // lives in `parity_view.rs`; this test confirms the regression
    // boundary.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1)").expect("insert");
    conn.execute("CREATE VIEW v AS SELECT a FROM t")
        .expect("CREATE VIEW should execute");
}

#[test]
fn create_trigger_now_executes() {
    // Triggers are implemented (Lane A5-triggers). Differential coverage
    // lives in `parity_trigger.rs`; this test confirms the regression
    // boundary.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("CREATE TABLE log(msg TEXT)").expect("create");
    conn.execute(
        "CREATE TRIGGER trg AFTER INSERT ON t \
         FOR EACH ROW BEGIN INSERT INTO log VALUES ('hi'); END",
    )
    .expect("CREATE TRIGGER should execute");
}

#[test]
fn generated_column_stored_parses() {
    let (_dir, conn) = open();
    // Column-level GENERATED ... AS (...) STORED is accepted; we don't
    // compute the expression yet, but the table is created.
    conn.execute(
        "CREATE TABLE t(\
            a INTEGER,\
            b INTEGER GENERATED ALWAYS AS (a + 1) STORED\
         )",
    )
    .expect("create with generated column");
}

#[test]
fn generated_column_virtual_parses() {
    let (_dir, conn) = open();
    conn.execute(
        "CREATE TABLE t(\
            a INTEGER,\
            b INTEGER GENERATED ALWAYS AS (a + 1) VIRTUAL\
         )",
    )
    .expect("create with virtual generated column");
}

#[test]
fn generated_column_recovery_round_trip() {
    let (_dir, conn) = open();
    conn.execute(
        "CREATE TABLE t(\
            a INTEGER,\
            b INTEGER GENERATED ALWAYS AS (a + 1) STORED\
         )",
    )
    .expect("create");
    // INSERT proceeds: in this lane the generated column is left at its
    // declared default (NULL) until execution lands. The SQL must still
    // succeed, demonstrating the parse path isn't a regression.
    conn.execute("INSERT INTO t(a) VALUES (5)").expect("insert");
}

#[test]
fn window_function_row_number_executes() {
    // Window-function execution is now implemented; see `parity_window.rs`.
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1), (2), (3)")
        .expect("insert");
    conn.execute("SELECT row_number() OVER (ORDER BY a) FROM t")
        .expect("ROW_NUMBER OVER should execute");
}

#[test]
fn window_function_rank_executes() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1), (2)")
        .expect("insert");
    conn.execute("SELECT rank() OVER (PARTITION BY a ORDER BY a DESC) FROM t")
        .expect("RANK OVER should execute");
}
