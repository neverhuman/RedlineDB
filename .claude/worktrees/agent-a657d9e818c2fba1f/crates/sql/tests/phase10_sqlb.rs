//! Phase-10 Lane SQL-B integration tests: multi-statement parsing and
//! SAVEPOINT/RELEASE/ROLLBACK TO. Lives in its own file so the workspace
//! file-size guardrail (active source <= 2000 LOC) keeps `sql_smoke.rs`
//! within budget.

use std::sync::Arc;

use redlinedb_sql::{
    BeginMode, Database, DbOptions, Step, is_blank_sql, split_first_statement, split_statements,
};
use tempfile::tempdir;

fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-phase10-sqlb.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

fn count_rows(conn: &Arc<redlinedb_sql::Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare count");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_i64(0).expect("count")
}

#[test]
fn split_first_statement_separates_two_selects() {
    let (head, tail) = split_first_statement("SELECT 1; SELECT 2;");
    assert_eq!(head, "SELECT 1;");
    assert_eq!(tail, " SELECT 2;");
}

#[test]
fn split_first_statement_no_terminator_returns_whole_input() {
    let (head, tail) = split_first_statement("SELECT 1");
    assert_eq!(head, "SELECT 1");
    assert_eq!(tail, "");
}

#[test]
fn split_statements_skips_blank_chunks() {
    let parts = split_statements("SELECT 1; ; -- comment\n SELECT 2;");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "SELECT 1;");
    // Second statement keeps its leading whitespace + comment trim.
    assert!(parts[1].trim_end().ends_with("SELECT 2;"));
}

#[test]
fn split_first_statement_respects_string_literals() {
    // Semicolon inside string literal must not split.
    let (head, tail) = split_first_statement("INSERT INTO t VALUES('a;b'); SELECT 1;");
    assert_eq!(head, "INSERT INTO t VALUES('a;b');");
    assert_eq!(tail, " SELECT 1;");
}

#[test]
fn split_first_statement_respects_line_comment() {
    let (head, tail) = split_first_statement("SELECT 1 -- has ; in comment\n; SELECT 2;");
    assert_eq!(head, "SELECT 1 -- has ; in comment\n;");
    assert_eq!(tail, " SELECT 2;");
}

#[test]
fn is_blank_sql_for_pure_comments() {
    assert!(is_blank_sql(""));
    assert!(is_blank_sql("   ;  "));
    assert!(is_blank_sql("-- only a comment\n"));
    assert!(is_blank_sql("/* block */"));
    assert!(!is_blank_sql("SELECT 1"));
}

#[test]
fn execute_runs_multiple_statements_in_one_call() {
    let (_dir, conn) = open_database();
    let n = conn
        .execute(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v INT); \
             INSERT INTO t VALUES (1, 10); \
             INSERT INTO t VALUES (2, 20);",
        )
        .expect("multi exec");
    assert_eq!(n, 1, "last stmt is INSERT with 1 affected row");
    let mut stmt = conn
        .prepare("SELECT count(*) FROM t")
        .expect("prepare count");
    assert_eq!(stmt.step().unwrap(), Step::Row);
    assert_eq!(stmt.column_i64(0).unwrap(), 2);
}

#[test]
fn prepare_v2_returns_remaining_tail() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    let sql = "INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);";
    let (stmt, tail) = conn.prepare_v2(sql).expect("prepare v2");
    assert!(stmt.is_some());
    assert_eq!(tail, " INSERT INTO t VALUES (2);");
    // Step the first stmt to actually apply it.
    let mut stmt = stmt.unwrap();
    while let Step::Row = stmt.step().unwrap() {}
    // Now prepare the tail: must succeed and consume rest.
    let (stmt2, tail2) = conn.prepare_v2(tail).expect("prepare tail");
    assert!(stmt2.is_some());
    assert_eq!(tail2, "");
}

#[test]
fn execute_stops_on_first_error_in_multi_stmt() {
    let (_dir, conn) = open_database();
    // First creates the table, second is invalid SQL, third would insert
    // — we verify the third does NOT run because the second errored.
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    let err = conn
        .execute("INSERT INTO t VALUES (1); BOGUS SQL HERE; INSERT INTO t VALUES (2);")
        .expect_err("middle stmt errors");
    assert!(
        format!("{err:?}").to_lowercase().contains("parse")
            || format!("{err:?}").to_lowercase().contains("unsupported")
    );
    // Verify state: row 1 inserted, row 2 not.
    let mut stmt = conn.prepare("SELECT id FROM t ORDER BY id").unwrap();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        rows.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(rows, vec![1]);
}

#[test]
fn prepare_returns_only_first_statement_for_back_compat() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    // Old-style prepare: silently ignores the tail.
    let mut stmt = conn
        .prepare("INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);")
        .expect("prepare");
    while let Step::Row = stmt.step().unwrap() {}
    // Only the first statement ran.
    let mut count = conn.prepare("SELECT count(*) FROM t").unwrap();
    assert_eq!(count.step().unwrap(), Step::Row);
    assert_eq!(count.column_i64(0).unwrap(), 1);
}
#[test]
fn savepoint_then_release_keeps_changes() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("SAVEPOINT sp1").expect("savepoint");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    conn.execute("RELEASE sp1").expect("release");
    conn.commit().expect("commit");
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 2);
}

#[test]
fn rollback_to_rewinds_post_savepoint_inserts() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("SAVEPOINT sp1").expect("savepoint");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    conn.execute("INSERT INTO t VALUES (3)").expect("ins 3");
    conn.execute("ROLLBACK TO sp1").expect("rollback to");
    // Only row (1) should be visible now.
    let mut stmt = conn.prepare("SELECT id FROM t ORDER BY id").expect("sel");
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        ids.push(stmt.column_i64(0).expect("id"));
    }
    assert_eq!(ids, vec![1]);
    // sp1 stays on the stack — release closes it cleanly.
    conn.execute("RELEASE sp1").expect("release");
    conn.commit().expect("commit");
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 1);
}

#[test]
fn nested_savepoints_independent_rewinds() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("SAVEPOINT sp1").expect("sp1");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    conn.execute("SAVEPOINT sp2").expect("sp2");
    conn.execute("INSERT INTO t VALUES (3)").expect("ins 3");
    // Roll back to sp2: drop only (3).
    conn.execute("ROLLBACK TO sp2").expect("rb sp2");
    let mut stmt = conn.prepare("SELECT id FROM t ORDER BY id").unwrap();
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        ids.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(ids, vec![1, 2]);
    // Now roll back to sp1: drop (2) too.
    conn.execute("ROLLBACK TO sp1").expect("rb sp1");
    let mut stmt = conn.prepare("SELECT id FROM t ORDER BY id").unwrap();
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        ids.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(ids, vec![1]);
    conn.execute("RELEASE sp1").expect("release");
    conn.commit().expect("commit");
}

#[test]
fn release_without_rewind_propagates_changes_to_outer_tx() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("SAVEPOINT sp1").expect("sp1");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    conn.execute("RELEASE sp1").expect("release sp1");
    // After release, both rows are still in the outer tx.
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 2);
    conn.commit().expect("commit");
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 2);
}

#[test]
fn shadowing_same_name_savepoints_picks_most_recent() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("SAVEPOINT sp").expect("first sp");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    conn.execute("SAVEPOINT sp").expect("shadow sp");
    conn.execute("INSERT INTO t VALUES (3)").expect("ins 3");
    // ROLLBACK TO sp must hit the SHADOW one, dropping only (3).
    conn.execute("ROLLBACK TO sp").expect("rb sp");
    let mut stmt = conn.prepare("SELECT id FROM t ORDER BY id").unwrap();
    let mut ids = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        ids.push(stmt.column_i64(0).unwrap());
    }
    assert_eq!(ids, vec![1, 2]);
    // Release inner sp: outer sp still on stack.
    conn.execute("RELEASE sp").expect("release shadow");
    // Releasing again pops the outer one too.
    conn.execute("RELEASE sp").expect("release outer");
    conn.commit().expect("commit");
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 2);
}

#[test]
fn implicit_tx_savepoint_outside_transaction() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    // No BEGIN: SAVEPOINT outside a tx must implicitly open one.
    assert!(!conn.in_transaction());
    conn.execute("SAVEPOINT sp1").expect("implicit savepoint");
    assert!(conn.in_transaction());
    conn.execute("INSERT INTO t VALUES (1)").expect("ins");
    // RELEASE sp1: stack empties, implicit tx commits.
    conn.execute("RELEASE sp1")
        .expect("release commits implicit");
    assert!(!conn.in_transaction());
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 1);
}

#[test]
fn rollback_to_within_implicit_tx_then_release_commits_prefix() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.execute("SAVEPOINT outer").expect("implicit outer");
    conn.execute("INSERT INTO t VALUES (1)").expect("ins 1");
    conn.execute("SAVEPOINT inner").expect("inner");
    conn.execute("INSERT INTO t VALUES (2)").expect("ins 2");
    // Rewind to inner, drop (2), then release both.
    conn.execute("ROLLBACK TO inner")
        .expect("rollback to inner");
    conn.execute("RELEASE inner").expect("release inner");
    conn.execute("RELEASE outer").expect("release outer");
    // Implicit tx commits via outer release; (1) is durable.
    assert!(!conn.in_transaction());
    assert_eq!(count_rows(&conn, "SELECT count(*) FROM t"), 1);
}

#[test]
fn release_unknown_savepoint_errors() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    let err = conn
        .execute("RELEASE no_such_sp")
        .expect_err("must error on unknown");
    assert!(format!("{err:?}").to_lowercase().contains("savepoint"));
    conn.rollback().expect("rollback");
}

#[test]
fn rollback_to_unknown_savepoint_errors() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)")
        .expect("create");
    conn.begin(BeginMode::Deferred).expect("begin");
    let err = conn
        .execute("ROLLBACK TO ghost")
        .expect_err("must error on unknown");
    assert!(format!("{err:?}").to_lowercase().contains("savepoint"));
    conn.rollback().expect("rollback");
}

#[test]
fn savepoint_rewind_with_updates_and_deletes() {
    // Mid-savepoint scenario covering the journal-replay path on UPDATE
    // and DELETE: rewind must restore the rows AND undo the deletions.
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 'a')").expect("a");
    conn.execute("INSERT INTO t VALUES (2, 'b')").expect("b");
    conn.execute("INSERT INTO t VALUES (3, 'c')").expect("c");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("SAVEPOINT sp1").expect("sp1");
    conn.execute("UPDATE t SET v='X' WHERE id=1").expect("upd");
    conn.execute("DELETE FROM t WHERE id=2").expect("del");
    // Rewind: rows back to a/b/c.
    conn.execute("ROLLBACK TO sp1").expect("rb");
    let mut stmt = conn.prepare("SELECT id, v FROM t ORDER BY id").unwrap();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().unwrap() {
        rows.push((
            stmt.column_i64(0).unwrap(),
            stmt.column_text(1).unwrap().to_owned(),
        ));
    }
    assert_eq!(
        rows,
        vec![
            (1, "a".to_owned()),
            (2, "b".to_owned()),
            (3, "c".to_owned()),
        ]
    );
    conn.execute("RELEASE sp1").expect("release");
    conn.commit().expect("commit");
}
