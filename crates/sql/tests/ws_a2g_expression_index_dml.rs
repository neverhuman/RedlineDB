use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};

fn open_db() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ws-a2g-expression-index-dml.db");
    let db = Database::create(&path, DbOptions::default()).expect("create");
    let conn = db.connect();
    (dir, conn)
}

fn collect_i64(conn: &Arc<Connection>, sql: &str) -> Vec<i64> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        out.push(stmt.column_i64(0).expect("column"));
    }
    out
}

#[test]
fn expression_index_leaf_stays_live_across_insert_update_delete() {
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)")
        .expect("create table");
    conn.execute("CREATE INDEX t_lname ON t(lower(name))")
        .expect("create expression index");

    conn.execute("INSERT INTO t VALUES (1, 'Foo'), (2, 'BAR'), (3, 'foo')")
        .expect("insert");
    assert_eq!(
        collect_i64(
            &conn,
            "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'foo' ORDER BY id"
        ),
        vec![1, 3]
    );

    conn.execute("UPDATE t SET name = 'Zap' WHERE id = 3")
        .expect("update");
    assert_eq!(
        collect_i64(
            &conn,
            "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'foo' ORDER BY id"
        ),
        vec![1]
    );
    assert_eq!(
        collect_i64(
            &conn,
            "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'zap' ORDER BY id"
        ),
        vec![3]
    );

    conn.execute("DELETE FROM t WHERE id = 1").expect("delete");
    assert_eq!(
        collect_i64(
            &conn,
            "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'foo' ORDER BY id"
        ),
        Vec::<i64>::new()
    );
}

#[test]
fn unique_expression_index_checks_evaluated_key() {
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)")
        .expect("create table");
    conn.execute("CREATE UNIQUE INDEX t_lname_unique ON t(lower(name))")
        .expect("create unique expression index");

    conn.execute("INSERT INTO t VALUES (1, 'Foo')")
        .expect("first insert");
    let duplicate = conn.execute("INSERT INTO t VALUES (2, 'foo')");
    assert!(
        duplicate.is_err(),
        "duplicate lower(name) value should violate the unique expression index"
    );
    assert_eq!(
        collect_i64(
            &conn,
            "SELECT id FROM t INDEXED BY t_lname_unique WHERE lower(name) = 'foo'"
        ),
        vec![1]
    );
}
