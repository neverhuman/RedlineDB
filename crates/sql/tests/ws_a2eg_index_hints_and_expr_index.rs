//! Phase 5 WS-A2e + WS-A2g integration tests:
//!   * `NOT INDEXED` forces the planner off any index path (TableScan).
//!   * `INDEXED BY <name>` restricts the planner to the named index.
//!   * Expression-index keys (e.g. `CREATE INDEX ... ON t(lower(name))`)
//!     match `WHERE lower(name) = ?` and pick the index for a
//!     point-lookup.
//!
//! Plan-level assertions ride on `EXPLAIN QUERY PLAN`; result-correctness
//! assertions follow each plan-check so the test catches a planner
//! regression that silently switches to a wrong path.

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};

fn open_db() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ws-a2eg.db");
    let db = Database::create(&path, DbOptions::default()).expect("create");
    let conn = db.connect();
    (dir, conn)
}

fn explain_text(conn: &Arc<Connection>, sql: &str) -> String {
    let prepared = format!("EXPLAIN QUERY PLAN {sql}");
    let mut stmt = conn.prepare(&prepared).expect("prepare explain");
    let mut out = String::new();
    while let Step::Row = stmt.step().expect("step explain") {
        out.push_str(stmt.column_text(3).expect("detail"));
        out.push('\n');
    }
    out
}

fn collect_ints(conn: &Arc<Connection>, sql: &str) -> Vec<i64> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        out.push(stmt.column_i64(0).expect("col"));
    }
    out
}

#[allow(dead_code)]
fn collect_texts(conn: &Arc<Connection>, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        out.push(stmt.column_text(0).expect("col").to_string());
    }
    out
}

// --------------------------------------------------------------------
// WS-A2e: NOT INDEXED forces TableScan
// --------------------------------------------------------------------

#[test]
fn not_indexed_forces_table_scan() {
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(x INTEGER, y INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_x_idx ON t(x)")
        .expect("create index");
    for v in 1..=5 {
        conn.execute(&format!("INSERT INTO t VALUES ({v}, {v}0)"))
            .expect("insert");
    }

    // Sanity: without the hint, the planner picks the index.
    let plan_default = explain_text(&conn, "SELECT y FROM t WHERE x = 1");
    assert!(
        plan_default.contains("USING INDEX t_x_idx"),
        "baseline expected to use index, got:\n{plan_default}"
    );

    // With NOT INDEXED, the planner must fall back to a heap scan.
    let plan_hinted = explain_text(&conn, "SELECT y FROM t NOT INDEXED WHERE x = 1");
    assert!(
        plan_hinted.contains("SCAN TABLE t"),
        "NOT INDEXED expected SCAN TABLE, got:\n{plan_hinted}"
    );
    assert!(
        !plan_hinted.contains("USING INDEX"),
        "NOT INDEXED must not advertise an index path, got:\n{plan_hinted}"
    );

    // Result rows must still match the unhinted query.
    let rows = collect_ints(&conn, "SELECT y FROM t NOT INDEXED WHERE x = 1");
    assert_eq!(rows, vec![10]);
}

#[test]
fn not_indexed_disables_rowid_pk_shortcut() {
    // SQLite parity: the rowid PK alias path is also an "index" path;
    // NOT INDEXED disables it too.
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (1, 100), (2, 200), (3, 300)")
        .expect("insert");

    let plan = explain_text(&conn, "SELECT v FROM t NOT INDEXED WHERE id = 2");
    assert!(
        plan.contains("SCAN TABLE t"),
        "NOT INDEXED on rowid PK expected SCAN TABLE, got:\n{plan}"
    );

    let rows = collect_ints(&conn, "SELECT v FROM t NOT INDEXED WHERE id = 2");
    assert_eq!(rows, vec![200]);
}

#[test]
fn indexed_by_restricts_to_named_index() {
    // Two equality-eligible indexes; INDEXED BY must pick the named one
    // even when the other would also satisfy the predicate.
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER, c INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_a ON t(a)").expect("idx a");
    conn.execute("CREATE INDEX t_b ON t(b)").expect("idx b");
    conn.execute("INSERT INTO t VALUES (1,1,10),(1,2,20),(2,1,30)")
        .expect("insert");

    let plan = explain_text(
        &conn,
        "SELECT c FROM t INDEXED BY t_b WHERE a = 1 AND b = 1",
    );
    assert!(
        plan.contains("USING INDEX t_b"),
        "INDEXED BY t_b expected USING INDEX t_b, got:\n{plan}"
    );
    assert!(
        !plan.contains("USING INDEX t_a"),
        "INDEXED BY t_b must not pick t_a, got:\n{plan}"
    );

    // Result correctness: predicate semantics are unchanged.
    let rows = collect_ints(
        &conn,
        "SELECT c FROM t INDEXED BY t_b WHERE a = 1 AND b = 1",
    );
    assert_eq!(rows, vec![10]);
}

#[test]
fn indexed_by_unrelated_index_falls_back_to_scan() {
    // SQLite parity: the planner is permissive when the named index
    // cannot satisfy the predicate (no leading-column equality on the
    // named index) — it falls back to a TableScan rather than erroring.
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX t_a ON t(a)").expect("idx a");
    conn.execute("CREATE INDEX t_b ON t(b)").expect("idx b");
    conn.execute("INSERT INTO t VALUES (1,1),(2,2)")
        .expect("insert");

    // INDEXED BY t_b but predicate only constrains `a` — the planner
    // must NOT use t_a (we asked for t_b) and must NOT use t_b (it has
    // no leading-column equality on `b`); falls back to TableScan.
    let plan = explain_text(&conn, "SELECT a FROM t INDEXED BY t_b WHERE a = 1");
    assert!(
        plan.contains("SCAN TABLE t"),
        "INDEXED BY mismatch expected SCAN TABLE, got:\n{plan}"
    );
    assert!(
        !plan.contains("USING INDEX t_a"),
        "must not silently fall through to t_a, got:\n{plan}"
    );
}

// --------------------------------------------------------------------
// WS-A2g: expression-index equality matching
// --------------------------------------------------------------------

#[test]
fn expression_index_lower_equality_uses_index() {
    // WS-A2g asserts the planner-side capability: when
    // `CREATE INDEX i ON t(lower(name))` exists and the WHERE contains
    // `lower(name) = ?`, the planner CAN pick the expression index.
    //
    // Expression-index DML maintenance and CREATE INDEX backfill are
    // wired, so the unhinted planner may use the single-key expression
    // index. Multi-key expression indexes stay disabled in the planner
    // until their residual semantics are proven.
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER, name TEXT)")
        .expect("create");
    conn.execute("CREATE INDEX t_lname ON t(lower(name))")
        .expect("create expr index");
    conn.execute("INSERT INTO t VALUES (1, 'Foo'), (2, 'BAR'), (3, 'foo')")
        .expect("insert");

    // Unhinted: planner can use the expression index now that DML and
    // backfill keep it populated.
    let plan_unhinted = explain_text(&conn, "SELECT id FROM t WHERE lower(name) = 'foo'");
    assert!(
        plan_unhinted.contains("USING INDEX t_lname"),
        "unhinted expression-index query expected USING INDEX, got:\n{plan_unhinted}"
    );
    let rows_unhinted = collect_ints(
        &conn,
        "SELECT id FROM t WHERE lower(name) = 'foo' ORDER BY id",
    );
    assert_eq!(rows_unhinted, vec![1, 3]);

    // Hinted: planner picks the named expression index.
    let plan_hinted = explain_text(
        &conn,
        "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'foo'",
    );
    assert!(
        plan_hinted.contains("USING INDEX t_lname"),
        "INDEXED BY t_lname expected USING INDEX, got:\n{plan_hinted}"
    );
    let rows_hinted = collect_ints(
        &conn,
        "SELECT id FROM t INDEXED BY t_lname WHERE lower(name) = 'foo' ORDER BY id",
    );
    assert_eq!(rows_hinted, vec![1, 3]);
}

#[test]
fn expression_index_only_matches_normalized_text() {
    // The textual match folds case and collapses whitespace, so
    // `LOWER(name)`, `lower(name)`, and `lower( name )` all bind. A
    // structurally different predicate (`lower(name || '')` etc.) must
    // NOT match (no SQL semantic equivalence — only textual).
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER, name TEXT)")
        .expect("create");
    conn.execute("CREATE INDEX t_lname ON t(lower(name))")
        .expect("create expr index");
    conn.execute("INSERT INTO t VALUES (1, 'Foo'), (2, 'BAR')")
        .expect("insert");

    // Case-folded form picks the index.
    let plan_upper = explain_text(&conn, "SELECT id FROM t WHERE LOWER(name) = 'foo'");
    assert!(
        plan_upper.contains("USING INDEX t_lname"),
        "LOWER(name) normalization expected USING INDEX, got:\n{plan_upper}"
    );

    // Whitespace-normalized form also matches.
    let plan_ws = explain_text(&conn, "SELECT id FROM t WHERE lower( name ) = 'foo'");
    assert!(
        plan_ws.contains("USING INDEX t_lname"),
        "whitespace-normalized form expected USING INDEX, got:\n{plan_ws}"
    );

    // A structurally different predicate must NOT match the named
    // expression index — the planner falls back to TableScan because
    // no other index applies.
    let plan_diff = explain_text(
        &conn,
        "SELECT id FROM t INDEXED BY t_lname WHERE lower(name || '') = 'foo'",
    );
    assert!(
        plan_diff.contains("SCAN TABLE t"),
        "structurally different expr must not match index, got:\n{plan_diff}"
    );
}

#[test]
fn not_indexed_overrides_expression_index() {
    // NOT INDEXED still wins even for expression-index matches.
    let (_dir, conn) = open_db();
    conn.execute("CREATE TABLE t(id INTEGER, name TEXT)")
        .expect("create");
    conn.execute("CREATE INDEX t_lname ON t(lower(name))")
        .expect("create expr index");
    conn.execute("INSERT INTO t VALUES (1, 'Foo')")
        .expect("insert");

    let plan = explain_text(
        &conn,
        "SELECT id FROM t NOT INDEXED WHERE lower(name) = 'foo'",
    );
    assert!(
        plan.contains("SCAN TABLE t"),
        "NOT INDEXED + expression-index expected SCAN TABLE, got:\n{plan}"
    );

    // Heap scan re-evaluates the expression against every row, so the
    // row is returned without consulting the expression-index leaf.
    let rows = collect_ints(
        &conn,
        "SELECT id FROM t NOT INDEXED WHERE lower(name) = 'foo'",
    );
    assert_eq!(rows, vec![1]);
}
