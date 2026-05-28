//! Miscellaneous smoke tests: explicit transactions (BEGIN/COMMIT/ROLLBACK),
//! `sqlite_schema` introspection, ALTER TABLE, statement parameter binding,
//! prepared-statement reprepare on schema change, write-count reporting,
//! `ANALYZE` / `EXPLAIN` / `EXPLAIN QUERY PLAN` / `EXPLAIN FORMAT JSON` /
//! `EXPLAIN ANALYZE`, the optional SQLite oracle diff, query-spill file
//! lifecycle, and the failpoint-gated commit-failure index-undo regression.
//!
//! Split off from the original `tests/sql_smoke.rs` (Phase 11 Wave 0).
//! Each `#[test] fn` here is verbatim from the source file.

mod common;

use common::{open_database, open_database_with_options, step_done};
use redlinedb_sql::{Connection, Database, DbOptions, Step};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn begin_commit_and_rollback_persist_and_discard_rows() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create table");

    {
        let mut begin = conn.prepare("BEGIN").expect("prepare begin");
        step_done(&mut begin);
        let mut insert = conn
            .prepare("INSERT INTO t VALUES (1, 'committed')")
            .expect("prepare insert");
        step_done(&mut insert);
        let mut commit = conn.prepare("COMMIT").expect("prepare commit");
        step_done(&mut commit);
    }

    {
        let mut select = conn
            .prepare("SELECT a, b FROM t ORDER BY a")
            .expect("prepare select");
        assert_eq!(select.step().expect("step"), Step::Row);
        assert_eq!(select.column_i64(0).expect("a"), 1);
        assert_eq!(select.column_text(1).expect("b"), "committed");
        assert_eq!(select.step().expect("done"), Step::Done);
    }

    {
        let mut begin = conn.prepare("BEGIN").expect("prepare begin");
        step_done(&mut begin);
        let mut insert = conn
            .prepare("INSERT INTO t VALUES (2, 'rolled back')")
            .expect("prepare insert");
        step_done(&mut insert);
        let mut rollback = conn.prepare("ROLLBACK").expect("prepare rollback");
        step_done(&mut rollback);
    }

    let mut select = conn
        .prepare("SELECT a, b FROM t ORDER BY a")
        .expect("prepare select");
    assert_eq!(select.step().expect("step"), Step::Row);
    assert_eq!(select.column_i64(0).expect("a"), 1);
    assert_eq!(select.column_text(1).expect("b"), "committed");
    assert_eq!(select.step().expect("done"), Step::Done);
}

#[test]
fn sqlite_schema_lists_created_objects() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT NOT NULL)")
        .expect("create table");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");

    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name FROM sqlite_schema ORDER BY name")
        .expect("prepare schema query");

    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((
            stmt.column_text(0).expect("type").to_owned(),
            stmt.column_text(1).expect("name").to_owned(),
            stmt.column_text(2).expect("tbl").to_owned(),
        ));
    }

    assert!(
        rows.iter()
            .any(|row| row.0 == "table" && row.1 == "t" && row.2 == "t")
    );
    assert!(
        rows.iter()
            .any(|row| row.0 == "index" && row.1 == "t_b_idx" && row.2 == "t")
    );
}

#[test]
fn execute_participates_in_explicit_transactions() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");

    let mut begin = conn.prepare("BEGIN").expect("prepare begin");
    step_done(&mut begin);
    conn.execute("INSERT INTO t VALUES (1, 'tx row')")
        .expect("insert in tx");
    let mut commit = conn.prepare("COMMIT").expect("prepare commit");
    step_done(&mut commit);

    let mut stmt = conn
        .prepare("SELECT b FROM t WHERE a = 1")
        .expect("prepare select");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("b"), "tx row");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn statement_parameters_and_clear_bindings_work() {
    let (_dir, conn) = open_database();

    let mut stmt = conn
        .prepare("SELECT ?1 + ?2, :named, ?2")
        .expect("prepare statement");
    assert_eq!(stmt.parameter_count(), 3);
    assert_eq!(stmt.parameter_index("?1"), Some(1));
    assert_eq!(stmt.parameter_index("?2"), Some(2));
    assert_eq!(stmt.parameter_index(":named"), Some(3));

    stmt.bind_i64(1, 4).expect("bind 1");
    stmt.bind_i64(2, 5).expect("bind 2");
    stmt.bind_named(":named", redlinedb_sql::SqlValue::Integer(9))
        .expect("bind named");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("sum"), 9);
    assert_eq!(stmt.column_i64(1).expect("named"), 9);
    assert_eq!(stmt.column_i64(2).expect("repeat"), 5);
    assert_eq!(stmt.step().expect("done"), Step::Done);

    stmt.reset().expect("reset");
    stmt.clear_bindings();
    stmt.bind_i64(1, 1).expect("bind 1");
    stmt.bind_i64(2, 2).expect("bind 2");
    stmt.bind_named(":named", redlinedb_sql::SqlValue::Integer(3))
        .expect("bind named");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("sum"), 3);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn execute_returns_read_and_write_counts() {
    let (_dir, conn) = open_database();

    assert_eq!(
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
            .expect("create table"),
        1
    );
    assert_eq!(
        conn.execute("INSERT INTO t VALUES (1, 'one')")
            .expect("insert"),
        1
    );
    assert_eq!(conn.execute("SELECT a, b FROM t").expect("select"), 1);
}

#[test]
fn changes_and_total_changes_scalar_functions_track_dml() {
    let (_dir, conn) = open_database();

    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (0, 0));

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (0, 0));

    conn.execute("INSERT INTO t VALUES(1, 'a')")
        .expect("insert one");
    assert_eq!(scalar_i64(&conn, "SELECT changes()"), 1);

    conn.execute("INSERT INTO t VALUES(2, 'b'),(3, 'c')")
        .expect("insert two");
    assert_eq!(scalar_i64(&conn, "SELECT changes()"), 2);
    assert_eq!(scalar_i64(&conn, "SELECT total_changes()"), 3);

    conn.execute("UPDATE t SET b = 'X'").expect("update all");
    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (3, 6));

    conn.execute("DELETE FROM t WHERE a = 1")
        .expect("delete one");
    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (1, 7));

    assert_eq!(scalar_i64(&conn, "SELECT count(*) FROM t"), 2);
    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (1, 7));

    conn.execute("UPDATE t SET b = 'none' WHERE a = 99")
        .expect("update none");
    assert_eq!(scalar_i64_pair(&conn, "SELECT changes(), total_changes()"), (0, 7));
}

#[test]
fn alter_table_rename_add_and_rename_column_work() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT NOT NULL DEFAULT 'bee')")
        .expect("create table");
    conn.execute("INSERT INTO t(a) VALUES (1)")
        .expect("insert row");

    conn.execute("ALTER TABLE t RENAME TO t2")
        .expect("rename table");
    conn.execute("ALTER TABLE t2 ADD COLUMN c TEXT DEFAULT 'cee'")
        .expect("add column");
    conn.execute("ALTER TABLE t2 RENAME COLUMN b TO renamed_b")
        .expect("rename column");

    let mut stmt = conn
        .prepare("SELECT a, renamed_b, c FROM t2 ORDER BY a")
        .expect("select renamed table");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 1);
    assert_eq!(stmt.column_text(1).expect("renamed_b"), "bee");
    assert_eq!(stmt.column_text(2).expect("c"), "cee");
    assert_eq!(stmt.step().expect("done"), Step::Done);

    conn.execute("INSERT INTO t2(a, renamed_b) VALUES (2, 'row2')")
        .expect("insert post alter");
    let mut stmt = conn
        .prepare("SELECT a, renamed_b, c FROM t2 ORDER BY a")
        .expect("select after insert");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(2).expect("c"), "cee");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(1).expect("renamed_b"), "row2");
    assert_eq!(stmt.column_text(2).expect("c"), "cee");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn prepared_statements_auto_reprepare_after_schema_change() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert row");

    let mut stmt = conn
        .prepare("SELECT b FROM t WHERE a = 1")
        .expect("prepare select");

    conn.execute("CREATE TABLE bump(x INTEGER)")
        .expect("bump schema");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("b"), "one");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn sqlite_oracle_smoke_if_available() {
    if std::env::var_os("REDLINEDB_SQLITE_DIFF").is_none() {
        return;
    }

    let sqlite3 = match std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
    {
        Ok(output) if output.status.success() => "sqlite3",
        _ => return,
    };

    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("oracle.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert");
    conn.execute("INSERT INTO t VALUES (2, 'two')")
        .expect("insert");

    let redline_rows = {
        let mut stmt = conn
            .prepare("SELECT a, b FROM t ORDER BY a")
            .expect("prepare select");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push((
                stmt.column_i64(0).expect("a"),
                stmt.column_text(1).expect("b").to_owned(),
            ));
        }
        rows
    };

    let sql = "CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT); \
               INSERT INTO t VALUES (1, 'one'); \
               INSERT INTO t VALUES (2, 'two'); \
               SELECT a, b FROM t ORDER BY a;";
    let output = std::process::Command::new(sqlite3)
        .arg(&path)
        .arg("-batch")
        .arg("-noheader")
        .arg("-separator")
        .arg("|")
        .arg(sql)
        .output()
        .expect("run sqlite3");
    assert!(output.status.success(), "sqlite3 diff command failed");
    let sqlite_rows = String::from_utf8(output.stdout)
        .expect("sqlite utf8")
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split('|');
            let a = parts.next().expect("a").parse::<i64>().expect("a int");
            let b = parts.next().expect("b").to_owned();
            (a, b)
        })
        .collect::<Vec<_>>();

    assert_eq!(redline_rows, sqlite_rows);
}

#[test]
fn analyze_and_explain_return_rows() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert");
    conn.execute("INSERT INTO t VALUES (2, 'two')")
        .expect("insert");

    conn.execute("ANALYZE").expect("analyze");

    let mut explain = conn
        .prepare("EXPLAIN QUERY PLAN SELECT b FROM t WHERE a = 1")
        .expect("prepare explain");
    assert_eq!(explain.column_count(), 4);
    let mut plan_rows = 0usize;
    while let Step::Row = explain.step().expect("step") {
        plan_rows += 1;
        assert!(!explain.column_text(3).expect("detail").is_empty());
    }
    assert!(plan_rows >= 1);

    let mut explain_json = conn
        .prepare("EXPLAIN FORMAT JSON SELECT b FROM t")
        .expect("prepare explain json");
    assert_eq!(explain_json.step().expect("step"), Step::Row);
    assert!(
        explain_json
            .column_text(0)
            .expect("json")
            .contains("\"kind\"")
    );
    assert_eq!(explain_json.step().expect("done"), Step::Done);

    let mut analyze = conn
        .prepare("EXPLAIN ANALYZE SELECT b FROM t ORDER BY a")
        .expect("prepare explain analyze");
    assert_eq!(analyze.step().expect("step"), Step::Row);
    assert!(!analyze.column_text(0).expect("analyze").is_empty());
    assert_eq!(analyze.step().expect("done"), Step::Done);
}

#[test]
fn spill_files_are_created_and_removed_for_sort_queries() {
    let opts = DbOptions {
        query_memory: redlinedb_sql::QueryMemoryConfig {
            work_mem_bytes: 1,
            max_spill_bytes: 1024 * 1024,
            batch_rows: 1024,
        },
        ..DbOptions::default()
    };
    let (_dir, conn) = open_database_with_options(opts);

    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create table");
    for i in 0..128 {
        conn.execute(&format!("INSERT INTO t VALUES ({i}, 'value-{i:03}')"))
            .expect("insert row");
    }

    let baseline = spill_file_count();
    {
        let mut stmt = conn
            .prepare("SELECT a FROM t ORDER BY b")
            .expect("prepare select");
        assert_eq!(stmt.step().expect("first row"), Step::Row);
        while let Step::Row = stmt.step().expect("step to completion") {}
    }
    assert_eq!(spill_file_count(), baseline, "spill file should be removed");
}

fn spill_file_count() -> usize {
    std::fs::read_dir(std::env::temp_dir())
        .expect("temp dir listing")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("redline-query-")
        })
        .count()
}

/// P0 #3: when `engine.commit` reports a maybe-committed outcome after
/// the SQL layer has already mutated physical index pages, we must not
/// run SQL-side index repair. The durable index entry needs to remain
/// visible, even though the client still sees an error from the commit.
#[cfg(feature = "failpoints")]
#[test]
fn commit_failure_surfaces_maybe_committed_without_index_repair() {
    use redlinedb_kernel::engine::arm_commit_failure_for_thread;
    use redlinedb_kernel::failpoints;
    use std::sync::Mutex;

    // The fail-crate registry is process-wide, so we serialize all
    // tests that touch the `engine::commit::before_publish`
    // configuration through one mutex. The closure inside the
    // failpoint additionally checks a thread-local flag (armed below)
    // so that other tests running on parallel threads keep
    // committing normally even while our action is in the registry.
    static GUARD: Mutex<()> = Mutex::new(());
    let _serial = GUARD.lock().unwrap_or_else(|p| p.into_inner());

    failpoints::cfg(
        "engine::commit::before_publish",
        "return(commit-failure-replays-index-undo)",
    )
    .expect("configure commit failpoint");

    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v TEXT)")
        .expect("create");
    conn.execute("CREATE UNIQUE INDEX t_k_idx ON t(k)")
        .expect("create unique index");

    // Arm AFTER DDL so the index is already physically allocated; we
    // want the *INSERT* commit to fail, not the DDL. The thread-local
    // flag scopes the failpoint to this test's thread; other parallel
    // tests' commits see the closure but skip the injection.
    arm_commit_failure_for_thread(true);

    let err = conn
        .execute("INSERT INTO t VALUES (1, 'first')")
        .expect_err("commit must be ambiguous");
    assert!(
        format!("{err:?}").contains("commit outcome uncertain"),
        "unexpected error from injected commit failure: {err:?}"
    );

    // Disarm before the next statement; the durable row/index bytes
    // should remain visible and no repair path should run.
    arm_commit_failure_for_thread(false);
    failpoints::cfg("engine::commit::before_publish", "off").expect("disable commit failpoint");

    let mut stmt = conn
        .prepare("SELECT v FROM t WHERE k = 1")
        .expect("prepare");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push(stmt.column_text(0).expect("v").to_owned());
    }
    assert_eq!(
        rows,
        vec!["first".to_owned()],
        "maybe-committed INSERT must leave the durable row visible"
    );

    let duplicate = conn
        .execute("INSERT INTO t VALUES (1, 'second')")
        .expect_err("duplicate unique key must still conflict");
    assert!(
        format!("{duplicate:?}").contains("constraint")
            || format!("{duplicate:?}").contains("unique"),
        "unexpected duplicate-key error: {duplicate:?}"
    );

    // Total row count must match: only the first insert survived.
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM t")
        .expect("prepare count");
    assert_eq!(stmt.step().expect("step count"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("count"), 1);
}

fn scalar_i64(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare scalar");
    assert_eq!(stmt.step().expect("step scalar"), Step::Row);
    let value = stmt.column_i64(0).expect("read scalar");
    assert_eq!(stmt.step().expect("done scalar"), Step::Done);
    value
}

fn scalar_i64_pair(conn: &Arc<Connection>, sql: &str) -> (i64, i64) {
    let mut stmt = conn.prepare(sql).expect("prepare scalar pair");
    assert_eq!(stmt.step().expect("step scalar pair"), Step::Row);
    let left = stmt.column_i64(0).expect("read left");
    let right = stmt.column_i64(1).expect("read right");
    assert_eq!(stmt.step().expect("done scalar pair"), Step::Done);
    (left, right)
}
