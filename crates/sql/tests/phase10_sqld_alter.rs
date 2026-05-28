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

fn sqlite_master_sql(conn: &Arc<Connection>, kind: &str, name: &str) -> String {
    let sql = format!(
        "SELECT sql FROM sqlite_master WHERE type='{kind}' AND name='{name}'"
    );
    let mut stmt = conn.prepare(&sql).expect("prepare sqlite_master query");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let sql = stmt.column_text(0).expect("sql").to_owned();
    assert_eq!(stmt.step().expect("done"), Step::Done);
    sql
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
fn alter_table_add_column_if_not_exists_is_idempotent() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER)").expect("create");
    conn.execute("INSERT INTO t VALUES (1)").expect("insert");

    conn.execute("ALTER TABLE t ADD COLUMN IF NOT EXISTS b TEXT NOT NULL DEFAULT 'x'")
        .expect("add column first time");
    conn.execute("ALTER TABLE t ADD COLUMN IF NOT EXISTS b TEXT NOT NULL DEFAULT 'x'")
        .expect("add column second time");

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
fn alter_table_rename_column_rewrites_dependent_view_sql() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    conn.execute("CREATE VIEW v AS SELECT a, b FROM t")
        .expect("create view");
    conn.execute("ALTER TABLE t RENAME COLUMN b TO bb")
        .expect("rename column");
    let sql = sqlite_master_sql(&conn, "view", "v");
    assert_eq!(sql, "CREATE VIEW v AS SELECT a, bb FROM t");
}

#[test]
fn alter_table_rename_column_rewrites_dependent_trigger_sql() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    conn.execute("CREATE TRIGGER trg AFTER INSERT ON t BEGIN SELECT NEW.b; END")
        .expect("create trigger");
    conn.execute("ALTER TABLE t RENAME COLUMN b TO bb")
        .expect("rename column");
    let sql = sqlite_master_sql(&conn, "trigger", "trg");
    assert_eq!(sql, "CREATE TRIGGER trg AFTER INSERT ON t BEGIN SELECT NEW.bb; END");
}

#[test]
fn alter_table_rename_table_rewrites_dependent_view_sql() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    conn.execute("CREATE VIEW v AS SELECT * FROM t")
        .expect("create view");
    conn.execute("ALTER TABLE t RENAME TO tt")
        .expect("rename table");
    let sql = sqlite_master_sql(&conn, "view", "v");
    assert_eq!(sql, "CREATE VIEW v AS SELECT * FROM \"tt\"");
}

#[test]
fn alter_table_rename_table_rewrites_dependent_trigger_sql() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create");
    conn.execute("CREATE TRIGGER tr AFTER INSERT ON t BEGIN SELECT NEW.b; END")
        .expect("create trigger");
    conn.execute("ALTER TABLE t RENAME TO tt")
        .expect("rename table");
    let sql = sqlite_master_sql(&conn, "trigger", "tr");
    assert_eq!(sql, "CREATE TRIGGER tr AFTER INSERT ON \"tt\" BEGIN SELECT NEW.b; END");
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
fn alter_table_drop_column_updates_empty_table_schema() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("ALTER TABLE t ADD COLUMN c TEXT DEFAULT 'd'")
        .expect("add");
    conn.execute("ALTER TABLE t DROP COLUMN b")
        .expect("drop column");
    conn.execute("INSERT INTO t(a) VALUES (1)").expect("insert");
    let mut stmt = conn.prepare("SELECT a, c FROM t").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 1);
    assert_eq!(stmt.column_text(1).expect("c"), "d");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn alter_table_drop_column_rewrites_populated_table() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b TEXT, c INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 'x', 5)")
        .expect("insert");
    conn.execute("ALTER TABLE t DROP COLUMN b")
        .expect("drop column");

    let mut stmt = conn.prepare("SELECT * FROM t").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 1);
    assert_eq!(stmt.column_i64(1).expect("c"), 5);
    assert_eq!(stmt.step().expect("done"), Step::Done);
    assert_eq!(
        sqlite_master_sql(&conn, "table", "t"),
        "CREATE TABLE t(a INTEGER, c INTEGER)"
    );
}

#[test]
fn alter_table_drop_column_rejects_indexed_column() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER, c INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");
    let res = conn.execute("ALTER TABLE t DROP COLUMN b");
    assert!(
        res.is_err(),
        "drop column should reject indexed column"
    );
    let msg = format!("{:?}", res.unwrap_err());
    assert!(
        msg.to_ascii_lowercase().contains("rewrite")
            || msg.to_ascii_lowercase().contains("indexed")
            || msg.to_ascii_lowercase().contains("unsupported"),
        "expected indexed-column rejection, got {msg}"
    );
}

#[test]
fn alter_table_add_column_with_foreign_key_rewrites_schema() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE p(id INTEGER PRIMARY KEY)")
        .expect("create parent");
    conn.execute("INSERT INTO p VALUES (1)")
        .expect("seed parent");
    conn.execute("CREATE TABLE c(id INTEGER)")
        .expect("create child");
    conn.execute("ALTER TABLE c ADD COLUMN pid INTEGER REFERENCES p(id)")
        .expect("alter add fk");

    let mut fk = conn
        .prepare("SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete, match FROM pragma_foreign_key_list('c')")
        .expect("prepare fk list");
    assert_eq!(fk.step().expect("step"), Step::Row);
    assert_eq!(fk.column_i64(0).expect("id"), 0);
    assert_eq!(fk.column_i64(1).expect("seq"), 0);
    assert_eq!(fk.column_text(2).expect("table"), "p");
    assert_eq!(fk.column_text(3).expect("from"), "pid");
    assert_eq!(fk.column_text(4).expect("to"), "id");
    assert_eq!(fk.column_text(5).expect("on_update"), "NO ACTION");
    assert_eq!(fk.column_text(6).expect("on_delete"), "NO ACTION");
    assert_eq!(fk.column_text(7).expect("match"), "NONE");
    assert_eq!(fk.step().expect("done"), Step::Done);
}
