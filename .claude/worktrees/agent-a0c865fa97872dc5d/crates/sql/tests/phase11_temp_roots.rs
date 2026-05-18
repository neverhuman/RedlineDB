mod common;

use std::fs;

use common::step_done;
use redlinedb_sql::{Database, DbOptions, Step};
use tempfile::tempdir;

#[test]
fn query_spills_land_under_the_configured_temp_root() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("db.redline");
    let spill_root = dir.path().join("query-spills");

    let opts = DbOptions {
        temp_dir: Some(spill_root.clone()),
        query_memory: redlinedb_sql::QueryMemoryConfig {
            work_mem_bytes: 1,
            max_spill_bytes: 1024 * 1024,
            batch_rows: 1024,
        },
        ..DbOptions::default()
    };

    let db = Database::create(&db_path, opts).expect("create db");
    let conn = db.connect();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)")
        .expect("create table");
    for id in 0..256_i64 {
        let mut stmt = conn
            .prepare("INSERT INTO t(id, v) VALUES (?1, ?2)")
            .expect("prepare insert");
        stmt.bind_i64(1, id).expect("bind id");
        stmt.bind_i64(2, id % 8).expect("bind value");
        step_done(&mut stmt);
    }

    let mut stmt = conn
        .prepare("SELECT DISTINCT v FROM t")
        .expect("prepare distinct");
    match stmt.step().expect("step distinct") {
        Step::Row => {}
        Step::Done => panic!("expected at least one row"),
    }

    let spill_entries: Vec<_> = fs::read_dir(&spill_root)
        .expect("spill root")
        .map(|entry| entry.expect("spill entry").path())
        .collect();
    assert!(
        !spill_entries.is_empty(),
        "expected spill files under {}",
        spill_root.display()
    );

    drop(stmt);
    let remaining: Vec<_> = fs::read_dir(&spill_root)
        .expect("spill root after drop")
        .map(|entry| entry.expect("spill entry").path())
        .collect();
    assert!(
        remaining.is_empty(),
        "spill root should be empty after statement drop: {:?}",
        remaining
    );
}
