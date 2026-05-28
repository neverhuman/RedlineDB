//! WS-A8: window engine linearization.
//!
//! Differential tests against rusqlite for the three new fast paths
//! added by Phase 5 WS-A8:
//!   * whole-partition aggregate (UNBOUNDED PRECEDING -> UNBOUNDED
//!     FOLLOWING) — broadcast a single accumulator value per partition.
//!   * running aggregate (ROWS UNBOUNDED PRECEDING -> CURRENT ROW) —
//!     prefix-array accumulator.
//!   * bounded sliding aggregate (ROWS BETWEEN n PRECEDING AND m
//!     FOLLOWING) — invertible incremental accumulator.
//!
//! Targets the WINDOW_PARTITION_SUM_027 / 047 parity case family.

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
        let path = dir.path().join("ws_a8.db");
        let db = Database::create(&path, DbOptions::default()).expect("create db");
        Self {
            _dir: dir,
            redline: db.connect(),
            sqlite: rusqlite::Connection::open_in_memory().expect("open in memory"),
        }
    }

    fn execute(&self, sql: &str) {
        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|e| panic!("sqlite setup failed for {sql:?}: {e}"));
        self.redline
            .execute(sql)
            .unwrap_or_else(|e| panic!("redline setup failed for {sql:?}: {e:?}"));
    }

    fn assert_match(&self, sql: &str) {
        let ru = query_sqlite(&self.sqlite, sql);
        let rl = query_redline(&self.redline, sql);
        if !approx_eq(&ru, &rl) {
            panic!("window mismatch on {sql:?}\n  sqlite={ru:?}\n  redline={rl:?}");
        }
    }
}

fn approx_eq(a: &[Vec<SqlValue>], b: &[Vec<SqlValue>]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (ra, rb) in a.iter().zip(b.iter()) {
        if ra.len() != rb.len() {
            return false;
        }
        for (va, vb) in ra.iter().zip(rb.iter()) {
            if !approx_eq_one(va, vb) {
                return false;
            }
        }
    }
    true
}

fn approx_eq_one(a: &SqlValue, b: &SqlValue) -> bool {
    match (a, b) {
        (SqlValue::Real(x), SqlValue::Real(y)) => (x - y).abs() < 1e-9,
        (SqlValue::Real(x), SqlValue::Integer(y)) => (x - *y as f64).abs() < 1e-9,
        (SqlValue::Integer(x), SqlValue::Real(y)) => (*x as f64 - y).abs() < 1e-9,
        _ => a == b,
    }
}

fn query_sqlite(c: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = c.prepare(sql).expect("prepare");
    let cols = stmt.column_count();
    let mut rows = stmt.query([]).expect("query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("next") {
        let mut current = Vec::with_capacity(cols);
        for i in 0..cols {
            let v: RuValue = row.get(i).expect("get");
            current.push(to_sql(v));
        }
        out.push(current);
    }
    out
}

fn query_redline(c: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = c
        .prepare(sql)
        .unwrap_or_else(|e| panic!("redline prepare failed for {sql:?}: {e:?}"));
    let cols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let mut row = Vec::with_capacity(cols);
        for i in 0..cols {
            row.push(stmt.column_value(i).expect("col").clone());
        }
        out.push(row);
    }
    out
}

fn to_sql(v: RuValue) -> SqlValue {
    match v {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(n) => SqlValue::Integer(n),
        RuValue::Real(r) => SqlValue::Real(r),
        RuValue::Text(s) => SqlValue::Text(Arc::from(s)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

// ── Whole-partition aggregate fast path ──────────────────────────────

#[test]
fn whole_partition_sum() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(k INTEGER, v INTEGER)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1, 10), (1, 20), (1, 30), \
         (2, 100), (2, 200), \
         (3, 7)",
    );
    lab.assert_match("SELECT k, v, SUM(v) OVER (PARTITION BY k) AS s FROM t ORDER BY k, v");
}

#[test]
fn whole_partition_count_star() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(grp TEXT, v INTEGER)");
    lab.execute("INSERT INTO t VALUES ('A',1),('A',2),('A',3),('B',10),('B',20)");
    lab.assert_match("SELECT grp, v, COUNT(*) OVER (PARTITION BY grp) AS c FROM t ORDER BY grp, v");
}

#[test]
fn whole_partition_avg_min_max() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(p INTEGER, v REAL)");
    lab.execute("INSERT INTO t VALUES (1,1.5),(1,2.5),(1,3.5),(2,10.0),(2,20.0)");
    lab.assert_match(
        "SELECT p, v, \
            AVG(v) OVER (PARTITION BY p), \
            MIN(v) OVER (PARTITION BY p), \
            MAX(v) OVER (PARTITION BY p) \
         FROM t ORDER BY p, v",
    );
}

#[test]
fn whole_partition_with_nulls() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(p INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1,1),(1,NULL),(1,3),(2,NULL),(2,NULL)");
    lab.assert_match(
        "SELECT p, SUM(v) OVER (PARTITION BY p), COUNT(v) OVER (PARTITION BY p) \
         FROM t ORDER BY p",
    );
}

#[test]
fn whole_partition_single_partition() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1),(2),(3),(4),(5)");
    lab.assert_match("SELECT v, SUM(v) OVER () AS s FROM t ORDER BY v");
}

#[test]
fn ordered_default_range_keeps_peer_frame() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(p INTEGER, k INTEGER, v INTEGER)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1,1,10),(1,1,20),(1,2,30),(1,3,40), \
         (2,1,100),(2,2,200)",
    );
    lab.assert_match(
        "SELECT p, k, v, SUM(v) OVER (PARTITION BY p ORDER BY k) \
         FROM t ORDER BY p, k, v",
    );
}

// ── Running-sum prefix path (UNBOUNDED PRECEDING → CURRENT ROW) ─────

#[test]
fn running_sum_unbounded_to_current() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1,10),(2,20),(3,30),(4,40),(5,50)");
    lab.assert_match(
        "SELECT id, v, SUM(v) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) \
         FROM t ORDER BY id",
    );
}

#[test]
fn running_sum_partitioned() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(part INTEGER, val INTEGER)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1,48),(2,49),(0,50),(1,51),(2,52),(0,53),(1,54),(2,55),(0,56)",
    );
    // Matches WINDOW_PARTITION_SUM_047 parity case shape.
    lab.assert_match(
        "SELECT part, val, \
            row_number() OVER (PARTITION BY part ORDER BY val), \
            sum(val) OVER (PARTITION BY part ORDER BY val \
                           ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) \
         FROM t ORDER BY part, val",
    );
}

#[test]
fn running_min_max_text_prefix() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(part INTEGER, seq INTEGER, label TEXT)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1,1,'delta'),(1,2,'alpha'),(1,3,'charlie'), \
         (2,1,'bravo'),(2,2,'echo')",
    );
    lab.assert_match(
        "SELECT part, seq, label, \
            MIN(label) OVER (PARTITION BY part ORDER BY seq \
                             ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW), \
            MAX(label) OVER (PARTITION BY part ORDER BY seq \
                             ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) \
         FROM t ORDER BY part, seq",
    );
}

// ── Bounded sliding-window path (n PRECEDING AND m FOLLOWING) ───────

#[test]
fn sliding_sum_one_each_side() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1,1),(2,2),(3,3),(4,4),(5,5)");
    lab.assert_match(
        "SELECT id, v, SUM(v) OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) \
         FROM t ORDER BY id",
    );
}

#[test]
fn sliding_avg_two_each_side() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v REAL)");
    lab.execute("INSERT INTO t VALUES (1,1.0),(2,2.0),(3,3.0),(4,4.0),(5,5.0),(6,6.0)");
    lab.assert_match(
        "SELECT id, v, AVG(v) OVER (ORDER BY id ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) \
         FROM t ORDER BY id",
    );
}

#[test]
fn sliding_count_partitioned() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(p INTEGER, id INTEGER, v INTEGER)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1,1,10),(1,2,20),(1,3,30),(1,4,40), \
         (2,1,100),(2,2,200),(2,3,300)",
    );
    lab.assert_match(
        "SELECT p, id, v, \
            COUNT(*) OVER (PARTITION BY p ORDER BY id \
                           ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) \
         FROM t ORDER BY p, id",
    );
}

#[test]
fn sliding_sum_asymmetric_bounds() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1,1),(2,2),(3,3),(4,4),(5,5),(6,6),(7,7)");
    lab.assert_match(
        "SELECT id, v, SUM(v) OVER (ORDER BY id ROWS BETWEEN 2 PRECEDING AND 1 FOLLOWING) \
         FROM t ORDER BY id",
    );
}

#[test]
fn sliding_sum_with_nulls() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(id INTEGER, v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1,1),(2,NULL),(3,3),(4,NULL),(5,5)");
    lab.assert_match(
        "SELECT id, v, SUM(v) OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING), \
                       COUNT(v) OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) \
         FROM t ORDER BY id",
    );
}

// ── Mixed-frame regression coverage ─────────────────────────────────

#[test]
fn whole_partition_with_running_sibling() {
    // Two window calls in the same projection — one hits the whole-
    // partition fast path, the other hits the prefix path. Both must
    // produce identical results to SQLite.
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(p INTEGER, id INTEGER, v INTEGER)");
    lab.execute(
        "INSERT INTO t VALUES \
         (1,1,10),(1,2,20),(1,3,30), \
         (2,1,100),(2,2,200)",
    );
    lab.assert_match(
        "SELECT p, id, v, \
            SUM(v) OVER (PARTITION BY p) AS total, \
            SUM(v) OVER (PARTITION BY p ORDER BY id \
                         ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS running \
         FROM t ORDER BY p, id",
    );
}
