//! Track K — beyond-SQLite (Postgres) parity coverage for portability
//! syntax features (MERGE, DISTINCT ON, FETCH FIRST, LATERAL,
//! data-modifying CTEs, GROUPING SETS / ROLLUP / CUBE).

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("trackk.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn rows(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let row: Vec<SqlValue> = (0..ncols)
            .map(|i| stmt.column_value(i).unwrap().clone())
            .collect();
        out.push(row);
    }
    out
}

// ── FETCH FIRST ────────────────────────────────────────────────────────────

#[test]
fn fetch_first_n_rows_only_limits_output() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (x int)").expect("create");
    for v in 1..=5 {
        c.execute(&format!("INSERT INTO v VALUES ({v})")).expect("insert");
    }
    let r = rows(&c, "SELECT x FROM v ORDER BY x FETCH FIRST 2 ROWS ONLY");
    assert_eq!(r, vec![vec![SqlValue::Integer(1)], vec![SqlValue::Integer(2)]]);
}

#[test]
fn fetch_next_n_rows_only_limits_output() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (x int)").expect("create");
    for v in 1..=5 {
        c.execute(&format!("INSERT INTO v VALUES ({v})")).expect("insert");
    }
    let r = rows(&c, "SELECT x FROM v ORDER BY x FETCH NEXT 3 ROWS ONLY");
    assert_eq!(
        r,
        vec![
            vec![SqlValue::Integer(1)],
            vec![SqlValue::Integer(2)],
            vec![SqlValue::Integer(3)]
        ]
    );
}

#[test]
fn offset_rows_fetch_next_paginates() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (x int)").expect("create");
    for v in 1..=5 {
        c.execute(&format!("INSERT INTO v VALUES ({v})")).expect("insert");
    }
    let r = rows(
        &c,
        "SELECT x FROM v ORDER BY x OFFSET 2 ROWS FETCH NEXT 2 ROWS ONLY",
    );
    assert_eq!(r, vec![vec![SqlValue::Integer(3)], vec![SqlValue::Integer(4)]]);
}

#[test]
fn fetch_first_row_only_defaults_quantity_to_one() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (x int)").expect("create");
    for v in 1..=3 {
        c.execute(&format!("INSERT INTO v VALUES ({v})")).expect("insert");
    }
    let r = rows(&c, "SELECT x FROM v ORDER BY x FETCH FIRST ROW ONLY");
    assert_eq!(r, vec![vec![SqlValue::Integer(1)]]);
}

// ── DISTINCT ON ─────────────────────────────────────────────────────────────

#[test]
fn distinct_on_first_per_group_uses_order_by_to_break_ties() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (g int, x text)").expect("create");
    for (g, x) in [(1, "a"), (1, "b"), (1, "c"), (2, "p"), (2, "q")] {
        c.execute(&format!("INSERT INTO v VALUES ({g}, '{x}')"))
            .expect("insert");
    }
    let r = rows(&c, "SELECT DISTINCT ON (g) g, x FROM v ORDER BY g, x DESC");
    assert_eq!(
        r,
        vec![
            vec![SqlValue::Integer(1), SqlValue::Text(Arc::from("c"))],
            vec![SqlValue::Integer(2), SqlValue::Text(Arc::from("q"))],
        ]
    );
}

#[test]
fn distinct_on_multiple_cols_groups_by_combination() {
    let (_d, c) = open();
    c.execute("CREATE TABLE v (g1 int, g2 text, x int)")
        .expect("create");
    for (g1, g2, x) in [(1, "x", 1), (1, "x", 2), (1, "y", 3), (2, "x", 4)] {
        c.execute(&format!("INSERT INTO v VALUES ({g1}, '{g2}', {x})"))
            .expect("insert");
    }
    let r = rows(
        &c,
        "SELECT DISTINCT ON (g1, g2) g1, g2, x FROM v ORDER BY g1, g2, x DESC",
    );
    assert_eq!(
        r,
        vec![
            vec![
                SqlValue::Integer(1),
                SqlValue::Text(Arc::from("x")),
                SqlValue::Integer(2)
            ],
            vec![
                SqlValue::Integer(1),
                SqlValue::Text(Arc::from("y")),
                SqlValue::Integer(3)
            ],
            vec![
                SqlValue::Integer(2),
                SqlValue::Text(Arc::from("x")),
                SqlValue::Integer(4)
            ],
        ]
    );
}

// ── MERGE ───────────────────────────────────────────────────────────────────

#[test]
fn merge_update_and_insert_into_existing_rows() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bp_merge_t (id int primary key, v text)")
        .expect("create");
    c.execute("CREATE TABLE bp_merge_s (id int, v text)")
        .expect("create");
    c.execute("INSERT INTO bp_merge_t VALUES (1, 'a')")
        .expect("insert");
    c.execute("INSERT INTO bp_merge_s VALUES (1, 'A'), (2, 'B')")
        .expect("insert");
    c.execute(
        "MERGE INTO bp_merge_t t USING bp_merge_s s ON t.id = s.id \
         WHEN MATCHED THEN UPDATE SET v = s.v \
         WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)",
    )
    .expect("merge");
    let r = rows(&c, "SELECT id, v FROM bp_merge_t ORDER BY id");
    assert_eq!(
        r,
        vec![
            vec![SqlValue::Integer(1), SqlValue::Text(Arc::from("A"))],
            vec![SqlValue::Integer(2), SqlValue::Text(Arc::from("B"))],
        ]
    );
}

#[test]
fn merge_matched_delete_removes_target_rows() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bp_merge_d_t (id int primary key, v int)")
        .expect("create");
    c.execute("CREATE TABLE bp_merge_d_s (id int)").expect("create");
    c.execute("INSERT INTO bp_merge_d_t VALUES (1,10),(2,20),(3,30)")
        .expect("insert t");
    c.execute("INSERT INTO bp_merge_d_s VALUES (2)").expect("insert s");
    c.execute(
        "MERGE INTO bp_merge_d_t t USING bp_merge_d_s s ON t.id = s.id \
         WHEN MATCHED THEN DELETE",
    )
    .expect("merge");
    let r = rows(&c, "SELECT id, v FROM bp_merge_d_t ORDER BY id");
    assert_eq!(
        r,
        vec![
            vec![SqlValue::Integer(1), SqlValue::Integer(10)],
            vec![SqlValue::Integer(3), SqlValue::Integer(30)],
        ]
    );
}

#[test]
fn merge_conditional_branches_dispatch_on_when_predicate() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bp_merge_c_t (id int primary key, v int)")
        .expect("create");
    c.execute("CREATE TABLE bp_merge_c_s (id int, v int, op text)")
        .expect("create");
    c.execute("INSERT INTO bp_merge_c_t VALUES (1,100),(2,200),(3,300)")
        .expect("insert t");
    c.execute("INSERT INTO bp_merge_c_s VALUES (1, NULL, 'del'),(2, 999, 'upd'),(4, 400, 'ins')")
        .expect("insert s");
    c.execute(
        "MERGE INTO bp_merge_c_t t USING bp_merge_c_s s ON t.id = s.id \
         WHEN MATCHED AND s.op = 'del' THEN DELETE \
         WHEN MATCHED AND s.op = 'upd' THEN UPDATE SET v = s.v \
         WHEN NOT MATCHED AND s.op = 'ins' THEN INSERT (id, v) VALUES (s.id, s.v)",
    )
    .expect("merge");
    let r = rows(&c, "SELECT id, v FROM bp_merge_c_t ORDER BY id");
    assert_eq!(
        r,
        vec![
            vec![SqlValue::Integer(2), SqlValue::Integer(999)],
            vec![SqlValue::Integer(3), SqlValue::Integer(300)],
            vec![SqlValue::Integer(4), SqlValue::Integer(400)],
        ]
    );
}
