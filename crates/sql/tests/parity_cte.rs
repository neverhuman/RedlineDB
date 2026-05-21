//! CTE (Common Table Expression) parity coverage.
//!
//! Differential against rusqlite to ensure non-recursive and recursive
//! CTE execution matches SQLite for graph walks, factorial-style numeric
//! recursion, hierarchy traversal, and Fibonacci.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

struct Lab {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Lab {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("cte.db");
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        let redline = db.connect();
        let sqlite = rusqlite::Connection::open_in_memory().expect("rusqlite open");
        Self {
            _dir: dir,
            redline,
            sqlite,
        }
    }
    fn execute(&self, sql: &str) {
        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|err| panic!("sqlite setup failed for {sql:?}: {err}"));
        self.redline
            .execute(sql)
            .unwrap_or_else(|err| panic!("redline setup failed for {sql:?}: {err:?}"));
    }
    fn query(&self, sql: &str) -> (Vec<Vec<SqlValue>>, Vec<Vec<SqlValue>>) {
        let ru = query_sqlite(&self.sqlite, sql);
        let rl = query_redline(&self.redline, sql);
        (ru, rl)
    }
    fn assert_match(&self, sql: &str) {
        let (ru, rl) = self.query(sql);
        assert_eq!(
            rl, ru,
            "CTE mismatch on {sql:?}\n  sqlite={ru:?}\n  redline={rl:?}"
        );
    }
}

fn query_sqlite(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|err| panic!("sqlite prepare failed for {sql:?}: {err}"));
    let cols = stmt.column_count();
    let mut rows = stmt.query([]).expect("sqlite query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("sqlite next") {
        let mut current = Vec::with_capacity(cols);
        for i in 0..cols {
            let v: RuValue = row.get(i).expect("sqlite get");
            current.push(to_sql_value(v));
        }
        out.push(current);
    }
    out
}

fn query_redline(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|err| panic!("redline prepare failed for {sql:?}: {err:?}"));
    let cols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let mut current = Vec::with_capacity(cols);
        for i in 0..cols {
            current.push(stmt.column_value(i).expect("col").clone());
        }
        out.push(current);
    }
    out
}

fn to_sql_value(v: RuValue) -> SqlValue {
    match v {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(n) => SqlValue::Integer(n),
        RuValue::Real(r) => SqlValue::Real(r),
        RuValue::Text(s) => SqlValue::Text(Arc::from(s)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

// ── Non-recursive CTE ─────────────────────────────────────────────────────────

#[test]
fn non_recursive_cte_basic() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 10), (2, 20), (3, 30)");

    lab.assert_match(
        "WITH high AS (SELECT id, v FROM t WHERE v >= 20) SELECT id, v FROM high ORDER BY id",
    );
    lab.assert_match("WITH every AS (SELECT id, v FROM t) SELECT v FROM every ORDER BY v DESC");
}

#[test]
fn non_recursive_cte_with_aggregate() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(g INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 1), (1, 2), (2, 5), (2, 6)");

    lab.assert_match(
        "WITH agg AS (SELECT g, SUM(v) AS s FROM t GROUP BY g) SELECT g, s FROM agg ORDER BY g",
    );
}

#[test]
fn values_statement_matches_sqlite() {
    let lab = Lab::new();
    lab.assert_match("VALUES(1,'a'),(2,'b')");
}

#[test]
fn cte_values_body_uses_declared_column_names() {
    let lab = Lab::new();
    lab.assert_match(
        "WITH t(x, label) AS (VALUES(2, 'b'), (1, 'a')) \
         SELECT x, label FROM t ORDER BY x",
    );
}

#[test]
fn recursive_cte_values_anchor() {
    let lab = Lab::new();
    lab.assert_match(
        "WITH RECURSIVE seq(x) AS ( \
             VALUES(1) \
             UNION ALL \
             SELECT x + 1 FROM seq WHERE x < 4 \
         ) SELECT x FROM seq ORDER BY x",
    );
}

#[test]
fn non_recursive_cte_materialized_hint_is_accepted() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 10), (2, 20), (3, 30)");

    lab.assert_match(
        "WITH filtered AS MATERIALIZED (SELECT id, v FROM t WHERE v >= 20) \
         SELECT id, v FROM filtered ORDER BY id",
    );
}

#[test]
fn non_recursive_cte_not_materialized_hint_is_accepted() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 10), (2, 20), (3, 30)");

    lab.assert_match(
        "WITH filtered AS NOT MATERIALIZED (SELECT id, v FROM t WHERE v < 30) \
         SELECT id, v FROM filtered ORDER BY id",
    );
}

// ── Recursive CTE: numeric ───────────────────────────────────────────────────

#[test]
fn recursive_cte_factorial_like_sequence() {
    let lab = Lab::new();
    // Numbers 1..=5
    lab.assert_match(
        "WITH RECURSIVE n(x) AS ( \
             SELECT 1 \
             UNION ALL \
             SELECT x + 1 FROM n WHERE x < 5 \
         ) SELECT x FROM n ORDER BY x",
    );
}

#[test]
fn recursive_cte_fibonacci_first_eight() {
    let lab = Lab::new();
    lab.assert_match(
        "WITH RECURSIVE fib(i, a, b) AS ( \
             SELECT 1, 0, 1 \
             UNION ALL \
             SELECT i+1, b, a+b FROM fib WHERE i < 8 \
         ) SELECT i, a FROM fib ORDER BY i",
    );
}

// ── Recursive CTE: graph walk (ancestors) ────────────────────────────────────

#[test]
fn recursive_cte_ancestors_walk() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE edge(parent INTEGER, child INTEGER)");
    lab.execute("INSERT INTO edge(parent, child) VALUES (1,2), (2,3), (3,4), (1,5), (5,6), (6,7)");

    // Descendants of 1
    lab.assert_match(
        "WITH RECURSIVE descendants(node) AS ( \
             SELECT child FROM edge WHERE parent = 1 \
             UNION \
             SELECT e.child FROM edge e JOIN descendants d ON e.parent = d.node \
         ) SELECT node FROM descendants ORDER BY node",
    );
}

#[test]
fn recursive_cte_employee_hierarchy() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE emp(id INTEGER PRIMARY KEY, name TEXT, manager INTEGER)");
    lab.execute(
        "INSERT INTO emp VALUES (1, 'CEO', NULL), \
            (2, 'VPSales', 1), (3, 'VPEng', 1), \
            (4, 'SalesM', 2), (5, 'EngM', 3), \
            (6, 'IC1', 4), (7, 'IC2', 5)",
    );

    lab.assert_match(
        "WITH RECURSIVE reports(id, name, depth) AS ( \
             SELECT id, name, 0 FROM emp WHERE manager IS NULL \
             UNION ALL \
             SELECT e.id, e.name, r.depth + 1 \
               FROM emp e JOIN reports r ON e.manager = r.id \
         ) SELECT depth, id, name FROM reports ORDER BY depth, id",
    );
}

// ── Cycle detection ──────────────────────────────────────────────────────────

#[test]
fn recursive_cte_cycle_dedups_with_union() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE node(id INTEGER, next INTEGER)");
    // 1 -> 2 -> 3 -> 1 (cycle)
    lab.execute("INSERT INTO node VALUES (1, 2), (2, 3), (3, 1)");

    // UNION (no ALL) deduplicates, so the walk terminates after each
    // node is seen exactly once.
    lab.assert_match(
        "WITH RECURSIVE walk(id) AS ( \
             SELECT 1 \
             UNION \
             SELECT n.next FROM node n JOIN walk w ON n.id = w.id \
         ) SELECT id FROM walk ORDER BY id",
    );
}

// ── CTE column-list aliasing ────────────────────────────────────────────────

#[test]
fn cte_explicit_column_list() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(a INTEGER, b INTEGER)");
    lab.execute("INSERT INTO t VALUES (1, 10), (2, 20)");

    lab.assert_match(
        "WITH renamed(x, y) AS (SELECT a, b FROM t) SELECT x + y AS sum_xy FROM renamed ORDER BY x",
    );
}
