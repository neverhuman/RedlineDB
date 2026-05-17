//! Reproducer for CREATE INDEX backfill after INSERT.

use redlinedb_sql::{Database, DbOptions, Step};
use tempfile::tempdir;

#[test]
fn create_index_after_insert_backfills_rows() {
    let dir = tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("t.db"), DbOptions::default()).expect("create");
    let conn = db.connect();
    conn.execute("CREATE TABLE t(id INTEGER, name TEXT)").expect("table");
    conn.execute("INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c')").expect("insert");
    conn.execute("CREATE INDEX idx_t_name ON t(name)").expect("index");
    let mut stmt = conn.prepare("SELECT id FROM t WHERE name = 'b'").expect("prepare");
    let mut got = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        got.push(stmt.column_value(0).expect("col").clone());
    }
    assert_eq!(got.len(), 1, "expected 1 row, got {got:?}");
}
