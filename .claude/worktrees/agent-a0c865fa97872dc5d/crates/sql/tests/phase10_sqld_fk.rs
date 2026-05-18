//! Lane SQL-D phase 10: FOREIGN KEY declarations (parser-only).
//!
//! Enforcement is not implemented in this lane; the SQL crate accepts
//! the syntax so schema-migration tools can round-trip CREATE TABLE
//! statements while the kernel work is sequenced behind it.
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("fk.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

#[test]
fn column_level_foreign_key_parses() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE parent(id INTEGER PRIMARY KEY)")
        .expect("create parent");
    conn.execute(
        "CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             parent_id INTEGER REFERENCES parent(id)\
         )",
    )
    .expect("create child with FK");
    conn.execute("INSERT INTO parent VALUES (1)")
        .expect("insert parent");
    conn.execute("INSERT INTO child VALUES (10, 1)")
        .expect("insert child with valid parent");
}

#[test]
fn table_level_foreign_key_parses() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE parent(id INTEGER PRIMARY KEY)")
        .expect("create parent");
    conn.execute(
        "CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             parent_id INTEGER,\
             FOREIGN KEY(parent_id) REFERENCES parent(id)\
         )",
    )
    .expect("create child with FK constraint");
}

#[test]
fn foreign_key_with_on_delete_cascade_parses() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE p(id INTEGER PRIMARY KEY)")
        .expect("create p");
    conn.execute(
        "CREATE TABLE c(\
             id INTEGER PRIMARY KEY,\
             p_id INTEGER REFERENCES p(id) ON DELETE CASCADE\
         )",
    )
    .expect("create child with cascade");
}

#[test]
fn pragma_foreign_keys_toggle_round_trips() {
    let (_dir, conn) = open();
    conn.execute("PRAGMA foreign_keys = ON")
        .expect("enable foreign keys pragma");
    conn.execute("PRAGMA foreign_keys = OFF")
        .expect("disable foreign keys pragma");
    let mut stmt = conn.prepare("PRAGMA foreign_keys").expect("read pragma");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let value = stmt.column_i64(0).expect("pragma value");
    assert_eq!(value, 0);
}

#[test]
fn foreign_key_referential_actions_parse() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE p(id INTEGER PRIMARY KEY)")
        .expect("create p");
    conn.execute(
        "CREATE TABLE c(\
             id INTEGER PRIMARY KEY,\
             p_id INTEGER REFERENCES p(id) ON DELETE SET NULL ON UPDATE CASCADE\
         )",
    )
    .expect("create child with set-null/update-cascade");
}

#[test]
fn deferrable_foreign_key_parses() {
    let (_dir, conn) = open();
    conn.execute("CREATE TABLE p(id INTEGER PRIMARY KEY)")
        .expect("create p");
    // Deferred FKs are also parser-only.
    conn.execute(
        "CREATE TABLE c(\
             id INTEGER PRIMARY KEY,\
             p_id INTEGER,\
             FOREIGN KEY(p_id) REFERENCES p(id) DEFERRABLE INITIALLY DEFERRED\
         )",
    )
    .expect("create child with deferrable");
}
