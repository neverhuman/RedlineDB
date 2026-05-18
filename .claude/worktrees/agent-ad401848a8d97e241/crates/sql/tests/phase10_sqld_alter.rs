//! Lane SQL-D phase 10: ALTER TABLE expansion (add / rename / drop column).
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("alter.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

#[test]
fn alter_table_add_column_with_default() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1)").expect("insert");
    conn.execute("ALTER TABLE t ADD COLUMN b TEXT DEFAULT 'x'")
        .expect("alter add");
    let mut stmt = conn.prepare("SELECT a, b FROM t").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 1);
    assert_eq!(stmt.column_text(1).expect("b"), "x");
}

#[test]
fn alter_table_rename_column_updates_schema() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (42)").expect("insert");
    conn.execute("ALTER TABLE t RENAME COLUMN a TO renamed")
        .expect("rename column");
    let mut stmt = conn.prepare("SELECT renamed FROM t").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("renamed"), 42);
}

#[test]
fn alter_table_add_column_default_fill() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    for v in 1..=3 {
        conn.execute(&format!("INSERT INTO t VALUES ({v})"))
            .expect("insert");
    }
    conn.execute("ALTER TABLE t ADD COLUMN c INTEGER DEFAULT 99")
        .expect("alter add default");
    let mut stmt = conn
        .prepare("SELECT a, c FROM t ORDER BY a")
        .expect("prepare");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((stmt.column_i64(0).unwrap(), stmt.column_i64(1).unwrap()));
    }
    assert_eq!(rows, vec![(1, 99), (2, 99), (3, 99)]);
}

#[test]
fn alter_table_drop_column_is_parser_only() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    let res = conn.execute("ALTER TABLE t DROP COLUMN b");
    // Parser must accept the syntax; execution surfaces UnsupportedDdl /
    // UnsupportedSql so a future writer can convert it into a rewrite.
    assert!(res.is_err(), "drop column should be parser-only-error");
    let msg = format!("{:?}", res.unwrap_err());
    assert!(
        msg.to_ascii_lowercase().contains("not yet implemented")
            || msg.to_ascii_lowercase().contains("unsupported"),
        "expected unsupported/not-yet message, got {msg}"
    );
}
