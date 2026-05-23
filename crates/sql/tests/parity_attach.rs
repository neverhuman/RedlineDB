//! ATTACH / DETACH DATABASE parity (Workstream A2).
//!
//! The minimum implementation maintains a per-connection alias map
//! (`crate::exec::attach::AttachMap`) and resolves `alias.table` via the
//! cross-database binder ([`crate::exec::cross_db`]). These tests
//! exercise the parser/executor path through the public `Connection`
//! API and compare results against a rusqlite oracle wherever the
//! shapes are equivalent.
//!
//!   * ATTACH succeeds and the alias becomes known
//!   * DETACH succeeds for a previously-attached alias
//!   * DETACH errors on an unknown alias
//!   * ATTACH errors on the reserved "main" / "temp" aliases
//!   * `SELECT * FROM aux.t` materializes rows from the sidecar engine
//!   * Cross-DB JOINs work
//!   * Cross-DB writes route to the attached sidecar database

use std::sync::Arc;

use redlinedb_sql::{Database, DbOptions, SqlValue, Step};

fn open_redline() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    (dir, db.connect())
}

fn collect_rows(conn: &Arc<redlinedb_sql::Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let col_count = stmt.column_count();
    let mut out = Vec::new();
    while stmt.step().expect("step") == Step::Row {
        let mut row = Vec::with_capacity(col_count);
        for i in 0..col_count {
            row.push(stmt.column_value(i).expect("column").clone());
        }
        out.push(row);
    }
    out
}

#[test]
fn attach_then_detach_alias_round_trip() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    let attach_sql = format!("ATTACH DATABASE '{aux_str}' AS aux");
    conn.execute(&attach_sql).expect("attach");

    // The DETACH form does not round-trip through sqlparser; our text-level
    // detector accepts it.
    conn.execute("DETACH DATABASE aux").expect("detach");

    // Re-attaching the same alias should now succeed.
    conn.execute(&attach_sql).expect("re-attach");

    // And the short form (`DETACH alias`) without DATABASE should also work.
    conn.execute("DETACH aux").expect("detach short form");
}

#[test]
fn detach_unknown_alias_errors() {
    let (_dir, conn) = open_redline();
    let err = conn.execute("DETACH DATABASE nope").unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("no such database") || msg.contains("UnknownTable"),
        "expected unknown-database error, got: {msg}"
    );
}

#[test]
fn attach_reserved_alias_errors() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();
    for reserved in &["main", "temp"] {
        let sql = format!("ATTACH DATABASE '{aux_str}' AS {reserved}");
        let err = conn.execute(&sql).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("reserved") || msg.contains("Unsupported"),
            "expected reserved-alias error for {reserved}, got: {msg}"
        );
    }
}

#[test]
fn attach_via_rusqlite_oracle_smoke() {
    // Confirms the literal SQL syntax we emit is the same shape SQLite
    // accepts — guards against future divergence of the surface text.
    let dir = tempfile::tempdir().expect("tempdir");
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    let oracle = rusqlite::Connection::open_in_memory().expect("ru open");
    let attach_sql = format!("ATTACH DATABASE '{aux_str}' AS aux");
    oracle.execute_batch(&attach_sql).expect("rusqlite attach");
    oracle
        .execute_batch("DETACH DATABASE aux")
        .expect("rusqlite detach");
}

#[test]
fn select_from_attached_alias_returns_rows() {
    // Seed the aux DB by creating + populating it through a separate
    // connection before the main connection attaches it.
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    {
        let aux_db = Database::create(&aux_path, DbOptions::default()).expect("create aux");
        let aux_conn = aux_db.connect();
        aux_conn
            .execute("CREATE TABLE events(id INTEGER, kind TEXT)")
            .expect("create aux table");
        aux_conn
            .execute("INSERT INTO events VALUES (1, 'login'), (2, 'logout')")
            .expect("insert aux rows");
    }

    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    let rows = collect_rows(&main_conn, "SELECT id, kind FROM aux.events ORDER BY id");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], SqlValue::Integer(1));
    assert_eq!(rows[0][1], SqlValue::Text(std::sync::Arc::from("login")));
    assert_eq!(rows[1][0], SqlValue::Integer(2));
    assert_eq!(rows[1][1], SqlValue::Text(std::sync::Arc::from("logout")));
}

#[test]
fn select_join_across_attached_alias_returns_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    {
        let aux_db = Database::create(&aux_path, DbOptions::default()).expect("create aux");
        let aux_conn = aux_db.connect();
        aux_conn
            .execute("CREATE TABLE events(uid INTEGER, kind TEXT)")
            .expect("create aux table");
        aux_conn
            .execute("INSERT INTO events VALUES (1, 'login'), (2, 'logout')")
            .expect("insert aux rows");
    }

    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    main_conn
        .execute("CREATE TABLE users(id INTEGER, name TEXT)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
        .expect("insert main rows");

    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    let rows = collect_rows(
        &main_conn,
        "SELECT u.name, e.kind FROM users u JOIN aux.events e ON u.id = e.uid ORDER BY u.id",
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], SqlValue::Text(std::sync::Arc::from("Alice")));
    assert_eq!(rows[0][1], SqlValue::Text(std::sync::Arc::from("login")));
    assert_eq!(rows[1][0], SqlValue::Text(std::sync::Arc::from("Bob")));
    assert_eq!(rows[1][1], SqlValue::Text(std::sync::Arc::from("logout")));
}

#[test]
fn select_from_unknown_alias_errors() {
    let (_dir, conn) = open_redline();
    let err = conn
        .prepare("SELECT * FROM nope.t")
        .err()
        .or_else(|| conn.execute("SELECT * FROM nope.t").err())
        .expect("expected error for unknown alias");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("no such database") || msg.contains("UnknownTable"),
        "expected unknown-database error, got: {msg}"
    );
}

#[test]
fn cross_db_write_routes_to_attached_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE aux.t(id INTEGER)")
        .expect("create aux table");
    main_conn
        .execute("INSERT INTO aux.t VALUES (1)")
        .expect("insert aux row");

    let rows = collect_rows(&main_conn, "SELECT id FROM aux.t");
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
}
