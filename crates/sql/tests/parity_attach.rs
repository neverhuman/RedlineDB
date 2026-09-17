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
fn attached_user_version_isolated_from_main() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    conn.execute("PRAGMA main.user_version=10")
        .expect("set main user_version");
    conn.execute(&format!("ATTACH DATABASE '{aux_str}' AS aux"))
        .expect("attach");
    conn.execute("PRAGMA aux.user_version=7")
        .expect("set aux user_version");

    let main = collect_rows(&conn, "PRAGMA main.user_version");
    let aux = collect_rows(&conn, "PRAGMA aux.user_version");
    assert_eq!(main, vec![vec![SqlValue::Integer(10)]]);
    assert_eq!(aux, vec![vec![SqlValue::Integer(7)]]);
}

#[test]
fn attached_schema_version_isolated_from_main() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    conn.execute(&format!("ATTACH DATABASE '{aux_str}' AS aux"))
        .expect("attach");

    let before_main = collect_rows(&conn, "PRAGMA main.schema_version");
    let before_aux = collect_rows(&conn, "PRAGMA aux.schema_version");
    conn.execute("CREATE TABLE main.t(x INTEGER)")
        .expect("create main table");
    let after_main = collect_rows(&conn, "PRAGMA main.schema_version");
    let after_aux = collect_rows(&conn, "PRAGMA aux.schema_version");

    assert_eq!(before_main, vec![vec![SqlValue::Integer(0)]]);
    assert_eq!(before_aux, vec![vec![SqlValue::Integer(0)]]);
    assert_eq!(after_main, vec![vec![SqlValue::Integer(1)]]);
    assert_eq!(after_aux, vec![vec![SqlValue::Integer(0)]]);
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

#[test]
fn cross_db_insert_select_copies_main_rows_to_attached_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE main.src(x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO main.src VALUES (1), (2), (3)")
        .expect("insert main rows");
    main_conn
        .execute("CREATE TABLE aux.dst(x INTEGER)")
        .expect("create aux table");

    main_conn
        .execute("INSERT INTO aux.dst SELECT x FROM main.src")
        .expect("insert select into aux");

    let rows = collect_rows(&main_conn, "SELECT x FROM aux.dst ORDER BY x");
    assert_eq!(
        rows,
        vec![
            vec![SqlValue::Integer(1)],
            vec![SqlValue::Integer(2)],
            vec![SqlValue::Integer(3)],
        ]
    );
    assert_eq!(
        collect_rows(&main_conn, "SELECT changes(), total_changes()"),
        vec![vec![SqlValue::Integer(3), SqlValue::Integer(6)]]
    );
}

#[test]
fn cross_db_insert_select_uses_bound_source_parameters() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE main.src(x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO main.src VALUES (1), (2), (3)")
        .expect("insert main rows");
    main_conn
        .execute("CREATE TABLE aux.dst(y INTEGER)")
        .expect("create aux table");

    let mut stmt = main_conn
        .prepare("INSERT INTO aux.dst(y) SELECT x FROM main.src WHERE x > ?")
        .expect("prepare insert select");
    stmt.bind_i64(1, 1).expect("bind");
    assert_eq!(stmt.step().expect("step"), Step::Done);
    assert_eq!(stmt.affected_rows(), 2);

    let rows = collect_rows(&main_conn, "SELECT y FROM aux.dst ORDER BY y");
    assert_eq!(
        rows,
        vec![vec![SqlValue::Integer(2)], vec![SqlValue::Integer(3)]]
    );
    assert_eq!(
        collect_rows(&main_conn, "SELECT last_insert_rowid(), changes()"),
        vec![vec![SqlValue::Integer(2), SqlValue::Integer(2)]]
    );
}

#[test]
fn cross_db_insert_select_validates_arity_for_empty_sources() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE main.src(x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("CREATE TABLE aux.dst(a INTEGER, b INTEGER)")
        .expect("create aux table");

    let err = main_conn
        .execute("INSERT INTO aux.dst(a, b) SELECT x FROM main.src")
        .expect_err("arity mismatch should fail even when source is empty");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("arity") || msg.contains("column list"),
        "expected arity error, got: {msg}"
    );
    assert_eq!(
        collect_rows(&main_conn, "SELECT count(*) FROM aux.dst"),
        vec![vec![SqlValue::Integer(0)]]
    );
}

#[test]
fn cross_db_insert_select_rejects_active_transactions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE main.src(x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO main.src VALUES (1)")
        .expect("insert main row");
    main_conn
        .execute("CREATE TABLE aux.dst(x INTEGER)")
        .expect("create aux table");

    main_conn.execute("SAVEPOINT s").expect("savepoint");
    let err = main_conn
        .execute("INSERT INTO aux.dst SELECT x FROM main.src")
        .expect_err("active transaction should be rejected");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("cross-database INSERT SELECT") || msg.contains("transaction"),
        "expected transaction guard error, got: {msg}"
    );
    main_conn.execute("ROLLBACK TO s").expect("rollback to");
    main_conn.execute("RELEASE s").expect("release");
    assert_eq!(
        collect_rows(&main_conn, "SELECT count(*) FROM aux.dst"),
        vec![vec![SqlValue::Integer(0)]]
    );
}

#[test]
fn cross_db_insert_select_rejects_modifiers_without_sidecar_rewrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE main.src(x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO main.src VALUES (1)")
        .expect("insert main row");
    main_conn
        .execute("CREATE TABLE aux.dst(x INTEGER UNIQUE)")
        .expect("create aux table");

    let err = main_conn
        .execute("INSERT OR IGNORE INTO aux.dst SELECT x FROM main.src")
        .expect_err("modified cross-db insert select should be rejected");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("cross-database INSERT SELECT") || msg.contains("modifiers"),
        "expected unsupported-modifier error, got: {msg}"
    );
    assert_eq!(
        collect_rows(&main_conn, "SELECT count(*) FROM aux.dst"),
        vec![vec![SqlValue::Integer(0)]]
    );
}

#[test]
fn alias_qualified_update_delete_routes_to_attached_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aux_path = dir.path().join("aux.db");
    let main_db =
        Database::create(dir.path().join("main.db"), DbOptions::default()).expect("create main");
    let main_conn = main_db.connect();
    let attach_sql = format!("ATTACH DATABASE '{}' AS aux", aux_path.display());
    main_conn.execute(&attach_sql).expect("attach");

    main_conn
        .execute("CREATE TABLE t(label TEXT, x INTEGER)")
        .expect("create main table");
    main_conn
        .execute("INSERT INTO t VALUES ('main', 1), ('main', 2)")
        .expect("insert main rows");
    main_conn
        .execute("CREATE TABLE aux.t(label TEXT, x INTEGER)")
        .expect("create aux table");
    main_conn
        .execute("INSERT INTO aux.t VALUES ('aux', 10), ('aux', 20)")
        .expect("insert aux rows");

    main_conn
        .execute("UPDATE aux.t SET x=x+1 WHERE x=10")
        .expect("update aux");
    main_conn
        .execute("DELETE FROM aux.t WHERE x=20")
        .expect("delete aux");

    let rows = collect_rows(
        &main_conn,
        "SELECT label, x FROM t UNION ALL SELECT label, x FROM aux.t ORDER BY label DESC, x",
    );
    assert_eq!(
        rows,
        vec![
            vec![SqlValue::Text(Arc::from("main")), SqlValue::Integer(1)],
            vec![SqlValue::Text(Arc::from("main")), SqlValue::Integer(2)],
            vec![SqlValue::Text(Arc::from("aux")), SqlValue::Integer(11)],
        ]
    );
}
