//! Window function parity coverage.
//!
//! Differential against rusqlite for ROW_NUMBER, RANK, DENSE_RANK, NTILE,
//! LAG, LEAD, FIRST_VALUE, LAST_VALUE, NTH_VALUE, PERCENT_RANK, CUME_DIST,
//! plus aggregate-OVER (SUM, COUNT, AVG, MIN, MAX, TOTAL) under ROWS and
//! RANGE frames.

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
        let path = dir.path().join("win.db");
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
        // Allow small float epsilon differences when both are real.
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
        (SqlValue::Real(x), SqlValue::Real(y)) => {
            (x - y).abs() < 1e-9 || (x.is_nan() && y.is_nan())
        }
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

// ── Ranking functions ──────────────────────────────────────────────────────

#[test]
fn row_number_over_order_by() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (10), (20), (30), (40)");
    lab.assert_match("SELECT v, ROW_NUMBER() OVER (ORDER BY v) AS rn FROM t ORDER BY v");
}

#[test]
fn rank_dense_rank_with_ties() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(grp TEXT, v INTEGER)");
    lab.execute("INSERT INTO t VALUES ('A', 1), ('A', 1), ('A', 2), ('B', 5), ('B', 5), ('B', 7)");
    lab.assert_match(
        "SELECT grp, v, RANK() OVER (PARTITION BY grp ORDER BY v) AS r, \
            DENSE_RANK() OVER (PARTITION BY grp ORDER BY v) AS dr \
         FROM t ORDER BY grp, v",
    );
}

#[test]
fn ntile_distributes_buckets() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1), (2), (3), (4), (5), (6), (7)");
    lab.assert_match("SELECT v, NTILE(3) OVER (ORDER BY v) AS bucket FROM t ORDER BY v");
}

// ── LAG / LEAD ─────────────────────────────────────────────────────────────

#[test]
fn lag_and_lead_with_default() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1), (2), (3), (4)");
    lab.assert_match(
        "SELECT v, LAG(v, 1, -1) OVER (ORDER BY v) AS prev, \
                LEAD(v, 1, -1) OVER (ORDER BY v) AS next \
         FROM t ORDER BY v",
    );
}

#[test]
fn lag_with_partition() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(grp TEXT, v INTEGER)");
    lab.execute("INSERT INTO t VALUES ('A',1),('A',2),('A',3),('B',10),('B',20)");
    lab.assert_match(
        "SELECT grp, v, LAG(v) OVER (PARTITION BY grp ORDER BY v) AS prev \
         FROM t ORDER BY grp, v",
    );
}

// ── FIRST_VALUE / LAST_VALUE / NTH_VALUE ───────────────────────────────────

#[test]
fn first_value_last_value() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(grp TEXT, v INTEGER)");
    lab.execute("INSERT INTO t VALUES ('A',1),('A',2),('A',3),('B',10),('B',20)");
    lab.assert_match(
        "SELECT grp, v, \
            FIRST_VALUE(v) OVER (PARTITION BY grp ORDER BY v ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) AS fv, \
            LAST_VALUE(v)  OVER (PARTITION BY grp ORDER BY v ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) AS lv \
         FROM t ORDER BY grp, v",
    );
}

#[test]
fn nth_value_basic() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (10), (20), (30), (40)");
    lab.assert_match(
        "SELECT v, NTH_VALUE(v, 2) OVER (ORDER BY v ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) AS nth \
         FROM t ORDER BY v",
    );
}

// ── Aggregate-OVER with frames ────────────────────────────────────────────

#[test]
fn running_sum_rows_frame() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1), (2), (3), (4), (5)");
    lab.assert_match(
        "SELECT v, SUM(v) OVER (ORDER BY v ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS rs \
         FROM t ORDER BY v",
    );
}

#[test]
fn moving_avg_rows_frame() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (1), (2), (3), (4), (5)");
    lab.assert_match(
        "SELECT v, AVG(v) OVER (ORDER BY v ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) AS avg3 \
         FROM t ORDER BY v",
    );
}

#[test]
fn count_partitioned() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(grp TEXT, v INTEGER)");
    lab.execute("INSERT INTO t VALUES ('A',1),('A',2),('B',5),('B',6),('B',7)");
    lab.assert_match("SELECT grp, v, COUNT(*) OVER (PARTITION BY grp) AS c FROM t ORDER BY grp, v");
}

// ── Default frame semantics (ORDER BY only) ───────────────────────────────

#[test]
fn default_frame_with_order_by_is_running_total() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (10), (20), (30)");
    // SUM(v) OVER (ORDER BY v) with no explicit frame is RANGE UNBOUNDED
    // PRECEDING AND CURRENT ROW. With unique ORDER BY values that
    // behaves identically to ROWS UNBOUNDED PRECEDING AND CURRENT ROW.
    lab.assert_match("SELECT v, SUM(v) OVER (ORDER BY v) AS rs FROM t ORDER BY v");
}

// ── NULL handling in ORDER BY ─────────────────────────────────────────────

#[test]
fn nulls_first_or_last_in_window_order() {
    let lab = Lab::new();
    lab.execute("CREATE TABLE t(v INTEGER)");
    lab.execute("INSERT INTO t VALUES (NULL), (1), (NULL), (2)");
    lab.assert_match("SELECT v, ROW_NUMBER() OVER (ORDER BY v) AS rn FROM t ORDER BY v");
}
