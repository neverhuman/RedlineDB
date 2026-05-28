//! SELECT-surface smoke tests: projections, WHERE, ORDER BY, LIMIT, GROUP
//! BY, HAVING, joins, subqueries, set operations, expressions, scalar
//! functions, and the SELECT-side planner / index-read paths (Lane C and
//! related Lane KH regressions).
//!
//! Split off from the original `tests/sql_smoke.rs` (Phase 11 Wave 0). Each
//! `#[test] fn` here is verbatim from the source file.

mod common;

use common::open_database;
use redlinedb_sql::{BeginMode, SqlValue, Step};

#[test]
fn create_insert_select_round_trip() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert row");
    conn.execute("INSERT INTO t VALUES (2, 'two')")
        .expect("insert row");

    let mut stmt = conn
        .prepare("SELECT a, b FROM t ORDER BY a")
        .expect("prepare select");

    assert_eq!(stmt.step().expect("first step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 1);
    assert_eq!(stmt.column_text(1).expect("b"), "one");

    assert_eq!(stmt.step().expect("second step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("a"), 2);
    assert_eq!(stmt.column_text(1).expect("b"), "two");

    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn nested_select_reuses_enclosing_transaction_snapshot() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create table");
    conn.begin(BeginMode::Deferred).expect("begin");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert uncommitted row");

    let mut stmt = conn
        .prepare(
            "SELECT (SELECT COUNT(*) FROM t), \
                    EXISTS(SELECT 1 FROM t WHERE id = outer_t.id) \
             FROM t AS outer_t",
        )
        .expect("prepare nested select");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("count"), 1);
    assert_eq!(stmt.column_i64(1).expect("exists"), 1);
    assert_eq!(stmt.step().expect("done"), Step::Done);
    drop(stmt);

    conn.rollback().expect("rollback");
}

#[test]
fn scalar_subquery_uses_first_row_and_empty_returns_null() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(v INTEGER)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (3), (1), (2)")
        .expect("insert rows");

    let mut stmt = conn
        .prepare(
            "SELECT \
                (SELECT v FROM t ORDER BY v), \
                (SELECT v FROM t WHERE v > 10)",
        )
        .expect("prepare scalar subquery");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("first row"), 1);
    assert_eq!(
        stmt.column_value(1).expect("empty subquery"),
        &SqlValue::Null
    );
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn select_distinct_deduplicates_rows() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(v TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES ('a')")
        .expect("insert a");
    conn.execute("INSERT INTO t VALUES ('a')")
        .expect("insert duplicate a");
    conn.execute("INSERT INTO t VALUES ('b')")
        .expect("insert b");

    let mut stmt = conn
        .prepare("SELECT DISTINCT v FROM t ORDER BY v")
        .expect("prepare distinct");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step distinct") {
        rows.push(stmt.column_text(0).expect("v").to_owned());
    }

    assert_eq!(rows, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn select_all_preserves_duplicates() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(v TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES ('a')")
        .expect("insert a");
    conn.execute("INSERT INTO t VALUES ('a')")
        .expect("insert duplicate a");

    let mut stmt = conn
        .prepare("SELECT ALL v FROM t ORDER BY rowid")
        .expect("prepare select all");
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step select all") {
        rows.push(stmt.column_text(0).expect("v").to_owned());
    }

    assert_eq!(rows, vec!["a".to_owned(), "a".to_owned()]);
}

#[test]
fn sqlite_expressions_cover_case_like_and_blob_literals() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(x TEXT, b BLOB)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES ('Alpha', x'4142')")
        .expect("insert");

    let mut stmt = conn
        .prepare(
            "SELECT CASE WHEN x LIKE 'a%' THEN 'yes' ELSE 'no' END, \
             x IS DISTINCT FROM 'beta', \
             b = x'4142' \
             FROM t",
        )
        .expect("prepare select");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("case"), "yes");
    assert_eq!(stmt.column_i64(1).expect("distinct"), 1);
    assert_eq!(stmt.column_i64(2).expect("blob"), 1);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn sqlite_core_functions_cover_round_hex_quote_random_and_glob() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(x TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES ('alpha')")
        .expect("insert");

    let mut stmt = conn
        .prepare(
            "SELECT round(1.25, 1), hex(x'4142'), quote('O''Reilly'), \
             likely(1), unlikely(0), likelihood(1, 0.25), random(), \
             glob('a*', 'alpha'), glob('b*', 'alpha') FROM t",
        )
        .expect("prepare select");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_f64(0).expect("round"), 1.3);
    assert_eq!(stmt.column_text(1).expect("hex"), "4142");
    assert_eq!(stmt.column_text(2).expect("quote"), "'O''Reilly'");
    assert_eq!(stmt.column_i64(3).expect("likely"), 1);
    assert_eq!(stmt.column_i64(4).expect("unlikely"), 0);
    assert_eq!(stmt.column_i64(5).expect("likelihood"), 1);
    let _ = stmt.column_i64(6).expect("random");
    assert_eq!(stmt.column_i64(7).expect("glob true"), 1);
    assert_eq!(stmt.column_i64(8).expect("glob false"), 0);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn sqlite_null_and_zero_arithmetic_semantics_match_core_behavior() {
    let (_dir, conn) = open_database();
    let mut stmt = conn
        .prepare("SELECT 1 / 0, 5 % 0, 'a' || NULL, NULL || 'b'")
        .expect("prepare arithmetic");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    for idx in 0..4 {
        assert!(matches!(
            stmt.column_value(idx).expect("value"),
            redlinedb_sql::SqlValue::Null
        ));
    }
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn union_all_concatenates_rows() {
    let (_dir, conn) = open_database();

    let mut stmt = conn
        .prepare("SELECT 1 AS v UNION ALL SELECT 2 UNION ALL SELECT 3")
        .expect("prepare union all");

    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step union all") {
        rows.push(stmt.column_i64(0).expect("v"));
    }

    assert_eq!(rows, vec![1, 2, 3]);
}

#[test]
fn exists_and_in_subqueries_follow_membership_rules() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(x INTEGER)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1)")
        .expect("insert row 1");
    conn.execute("INSERT INTO t VALUES (3)")
        .expect("insert row 3");

    let mut exists = conn
        .prepare("SELECT EXISTS(SELECT 1 FROM t WHERE x = 3), EXISTS(SELECT 1 FROM t WHERE x = 9)")
        .expect("prepare exists");
    assert_eq!(exists.step().expect("step"), Step::Row);
    assert_eq!(exists.column_i64(0).expect("exists true"), 1);
    assert_eq!(exists.column_i64(1).expect("exists false"), 0);
    assert_eq!(exists.step().expect("done"), Step::Done);

    let mut membership = conn
        .prepare("SELECT 3 IN (SELECT x FROM t), 9 IN (SELECT x FROM t)")
        .expect("prepare in subquery");
    assert_eq!(membership.step().expect("step"), Step::Row);
    assert_eq!(membership.column_i64(0).expect("in true"), 1);
    assert_eq!(membership.column_i64(1).expect("in false"), 0);
    assert_eq!(membership.step().expect("done"), Step::Done);
}

#[test]
fn left_join_null_extends_missing_rows() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE parent(id INTEGER PRIMARY KEY, name TEXT)")
        .expect("create parent");
    conn.execute("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER, note TEXT)")
        .expect("create child");
    conn.execute("INSERT INTO parent VALUES (1, 'one')")
        .expect("insert parent 1");
    conn.execute("INSERT INTO parent VALUES (2, 'two')")
        .expect("insert parent 2");
    conn.execute("INSERT INTO child VALUES (10, 1, 'matched')")
        .expect("insert matched child");

    let mut stmt = conn
        .prepare(
            "SELECT parent.id, child.note \
             FROM parent LEFT JOIN child ON parent.id = child.parent_id \
             ORDER BY parent.id",
        )
        .expect("prepare left join");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("parent id"), 1);
    assert_eq!(stmt.column_text(1).expect("child note"), "matched");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("parent id"), 2);
    assert_eq!(
        stmt.column_value(1).expect("child note"),
        &redlinedb_sql::SqlValue::Null
    );

    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn inner_join_and_grouped_aggregate_work() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE parent(id INTEGER PRIMARY KEY, name TEXT)")
        .expect("create parent");
    conn.execute("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER, value TEXT)")
        .expect("create child");

    conn.execute("INSERT INTO parent VALUES (1, 'one')")
        .expect("insert parent");
    conn.execute("INSERT INTO parent VALUES (2, 'two')")
        .expect("insert parent");
    conn.execute("INSERT INTO child VALUES (10, 1, 'alpha')")
        .expect("insert child");
    conn.execute("INSERT INTO child VALUES (11, 1, 'beta')")
        .expect("insert child");
    conn.execute("INSERT INTO child VALUES (12, 2, 'gamma')")
        .expect("insert child");

    let mut join = conn
        .prepare(
            "SELECT parent.id, child.value \
             FROM parent INNER JOIN child ON parent.id = child.parent_id \
             ORDER BY parent.id, child.id",
        )
        .expect("prepare join");
    let mut join_rows = Vec::new();
    while let Step::Row = join.step().expect("join step") {
        join_rows.push((
            join.column_i64(0).expect("parent id"),
            join.column_text(1).expect("value").to_owned(),
        ));
    }
    assert_eq!(
        join_rows,
        vec![
            (1, "alpha".to_owned()),
            (1, "beta".to_owned()),
            (2, "gamma".to_owned())
        ]
    );

    let mut grouped = conn
        .prepare(
            "SELECT parent_id, COUNT(*), SUM(id) FROM child GROUP BY parent_id HAVING COUNT(*) > 1",
        )
        .expect("prepare grouped");
    assert_eq!(grouped.step().expect("grouped step"), Step::Row);
    assert_eq!(grouped.column_i64(0).expect("parent_id"), 1);
    assert_eq!(grouped.column_i64(1).expect("count"), 2);
    assert_eq!(grouped.column_i64(2).expect("sum"), 21);
    assert_eq!(grouped.step().expect("grouped done"), Step::Done);
}

// ---------------------------------------------------------------------------
// Lane C: SQL Index Reads And Planner.
//
// Lane C wires SELECT to consume the kernel B-tree indexes that Lane B
// keeps in sync with DML. These tests assert two invariants:
//   1. EXPLAIN names the physical access path the executor actually
//      takes (`IndexPointLookup`, `IndexRangeScan`, or `TableScan`),
//      and only advertises an index path when one is consumable.
//   2. Index-driven SELECT results match what a TableScan would have
//      produced, end-to-end across the heap and the index.
// Covering indexes and multi-index AND/OR remain disabled until later
// waves; the last two tests assert that fact.
// ---------------------------------------------------------------------------

mod lane_c {
    use super::open_database;
    use redlinedb_sql::Step;

    /// Run `EXPLAIN QUERY PLAN <sql>` and concatenate the detail
    /// column for every plan row. The detail format is
    /// `SEARCH TABLE <name> USING INDEX <idx>: <Probe>` for index
    /// paths and `SCAN TABLE <name>` for full scans, so substring
    /// matching is reliable.
    fn explain_text(conn: &std::sync::Arc<redlinedb_sql::Connection>, sql: &str) -> String {
        let prepared = format!("EXPLAIN QUERY PLAN {sql}");
        let mut stmt = conn.prepare(&prepared).expect("prepare explain");
        let mut out = String::new();
        while let Step::Row = stmt.step().expect("step explain") {
            // Column 3 is the textual detail (id, parent, notused,
            // detail) — see `planner::explain_rows`.
            out.push_str(stmt.column_text(3).expect("detail"));
            out.push('\n');
        }
        out
    }

    fn collect_select_ints(
        conn: &std::sync::Arc<redlinedb_sql::Connection>,
        sql: &str,
    ) -> Vec<i64> {
        let mut stmt = conn.prepare(sql).expect("prepare select");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step select") {
            rows.push(stmt.column_i64(0).expect("col"));
        }
        rows
    }

    #[test]
    fn select_by_pk_uses_index_point_lookup() {
        let (_dir, conn) = open_database();
        // CREATE TABLE PRIMARY KEY indexes are autoindexes that the
        // catalog records without a `meta_page_id` (no physical pages
        // are allocated until CREATE INDEX runs). Lane KH P1 #5 made
        // the planner skip indexes without a live handle, so we issue
        // CREATE INDEX explicitly here to exercise the point-lookup
        // path through a real B-tree.
        conn.execute("CREATE TABLE t(k TEXT, v INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_k_idx ON t(k)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES ('a', 1)")
            .expect("insert a");
        conn.execute("INSERT INTO t VALUES ('b', 2)")
            .expect("insert b");

        let plan = explain_text(&conn, "SELECT v FROM t WHERE k = 'a'");
        assert!(
            plan.contains("USING INDEX") && plan.contains("PointLookup"),
            "expected IndexPointLookup, got plan:\n{plan}"
        );
        assert!(
            !plan.contains("SCAN TABLE t"),
            "did not expect a full SCAN TABLE under an index path:\n{plan}"
        );
    }

    #[test]
    fn select_indexed_range_uses_index_range_scan() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        for v in 1..=5 {
            conn.execute(&format!("INSERT INTO t VALUES ({v}, 'v{v}')"))
                .expect("insert");
        }

        let plan = explain_text(&conn, "SELECT b FROM t WHERE a BETWEEN 2 AND 4");
        assert!(
            plan.contains("USING INDEX t_a_idx") && plan.contains("RangeScan"),
            "expected IndexRangeScan on t_a_idx, got plan:\n{plan}"
        );
    }

    #[test]
    fn unsupported_predicate_falls_back_to_table_scan() {
        let (_dir, conn) = open_database();
        // Index is on `a`; the predicate constrains only `b`, which
        // is the non-leading and indeed unindexed column. The
        // planner must not advertise an index path.
        conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 100)")
            .expect("insert");
        conn.execute("INSERT INTO t VALUES (2, 200)")
            .expect("insert");

        let plan = explain_text(&conn, "SELECT a FROM t WHERE b = 100");
        assert!(
            plan.contains("SCAN TABLE t"),
            "expected TableScan (no leading-column predicate), got plan:\n{plan}"
        );
        assert!(
            !plan.contains("USING INDEX"),
            "must not advertise an index path here:\n{plan}"
        );
    }

    #[test]
    fn index_point_lookup_returns_correct_rows() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(k TEXT PRIMARY KEY, v INTEGER)")
            .expect("create");
        for (k, v) in [("a", 1i64), ("b", 2), ("c", 3)] {
            conn.execute(&format!("INSERT INTO t VALUES ('{k}', {v})"))
                .expect("insert");
        }
        // Index path: WHERE k = 'b' (this is the planner-advertised
        // IndexPointLookup case).
        let via_index = collect_select_ints(&conn, "SELECT v FROM t WHERE k = 'b'");
        // Reference: table scan with a residual filter (we re-issue
        // the same query; the planner would still pick the index,
        // but the result must equal the logical answer regardless).
        assert_eq!(via_index, vec![2]);
        // Confirm a miss returns an empty set (the index returns no
        // rows; no fallback to a heap scan happens silently).
        let miss = collect_select_ints(&conn, "SELECT v FROM t WHERE k = 'zzz'");
        assert!(
            miss.is_empty(),
            "missing key must yield no rows, got {miss:?}"
        );
    }

    #[test]
    fn index_range_scan_returns_correct_rows() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        for v in 1..=5 {
            conn.execute(&format!("INSERT INTO t VALUES ({v}, 'v{v}')"))
                .expect("insert");
        }
        // BETWEEN 2 AND 4 -> indexed range scan
        let mut stmt = conn
            .prepare("SELECT a FROM t WHERE a BETWEEN 2 AND 4 ORDER BY a")
            .expect("prepare");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_i64(0).expect("a"));
        }
        assert_eq!(rows, vec![2, 3, 4]);

        // Open-ended range: a > 3
        let mut stmt = conn
            .prepare("SELECT a FROM t WHERE a > 3 ORDER BY a")
            .expect("prepare");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_i64(0).expect("a"));
        }
        assert_eq!(rows, vec![4, 5]);
    }

    #[test]
    fn planner_does_not_advertise_covering_index() {
        // Even when every projected column is a leading key of the
        // index (a true covering candidate), Lane C must NOT emit
        // `CoveringIndexScan` — that optimization stays disabled
        // until a later wave wires the executor for it. The
        // physical plan should still pick an index path
        // (IndexRangeScan), but render WITHOUT "COVERING".
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 10)")
            .expect("insert");
        conn.execute("INSERT INTO t VALUES (2, 20)")
            .expect("insert");

        // SELECT a FROM t WHERE a = 1 — `a` is the only projected
        // column AND the leading key, so this is a textbook covering
        // candidate. Assert that the plan reports the regular index
        // path (PointLookup), NOT a "COVERING INDEX" line.
        let plan = explain_text(&conn, "SELECT a FROM t WHERE a = 1");
        assert!(
            !plan.contains("COVERING INDEX"),
            "covering-index optimization must stay off:\n{plan}"
        );
        assert!(
            plan.contains("USING INDEX") && plan.contains("PointLookup"),
            "expected a regular IndexPointLookup, got plan:\n{plan}"
        );
    }

    #[test]
    fn planner_does_not_advertise_multi_index_and_or() {
        // Two single-column indexes plus a predicate that touches
        // BOTH (`a = 1 OR b = 10`). A multi-index OR planner could
        // theoretically union the two probe sets, but Lane C keeps
        // that optimization disabled. The plan must therefore fall
        // back to a TableScan rather than emitting `MULTI-INDEX
        // SCAN`.
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("CREATE INDEX t_b_idx ON t(b)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 10)")
            .expect("insert");
        conn.execute("INSERT INTO t VALUES (2, 20)")
            .expect("insert");

        let plan_or = explain_text(&conn, "SELECT a FROM t WHERE a = 1 OR b = 20");
        assert!(
            !plan_or.contains("MULTI-INDEX"),
            "multi-index OR must stay off:\n{plan_or}"
        );
        // We accept either TableScan or — if a single-index path is
        // somehow extracted from one side of the OR — the plain
        // index path. What we MUST NOT see is a multi-index union.
        // (Today the planner walks only top-level AND chains, so an
        // OR pins us to TableScan; this assertion preserves that.)
        assert!(
            plan_or.contains("SCAN TABLE t"),
            "expected fallback to TableScan for OR, got plan:\n{plan_or}"
        );

        // Same for AND-of-two-indexes: only the leading conjunct
        // gets used. We never emit MULTI-INDEX AND.
        let plan_and = explain_text(&conn, "SELECT a FROM t WHERE a = 1 AND b = 10");
        assert!(
            !plan_and.contains("MULTI-INDEX"),
            "multi-index AND must stay off:\n{plan_and}"
        );
    }

    /// Regression: composite (a, b) index with WHERE a = ? must surface every
    /// row that shares the leading-key value. The previous upper-bound for the
    /// half-open range was `prefix || 0x00`, which sorts BEFORE every full
    /// composite key (because the next part starts with a non-zero type tag),
    /// so the range returned an empty set.
    #[test]
    fn composite_index_leading_prefix_returns_all_rows() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_ab ON t(a, b)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 'x')")
            .expect("insert 1x");
        conn.execute("INSERT INTO t VALUES (1, 'y')")
            .expect("insert 1y");
        conn.execute("INSERT INTO t VALUES (1, 'z')")
            .expect("insert 1z");
        conn.execute("INSERT INTO t VALUES (2, 'x')")
            .expect("insert 2x");

        // The planner must pick the (a, b) composite index for `WHERE a = 1`.
        let plan = explain_text(&conn, "SELECT b FROM t WHERE a = 1");
        assert!(
            plan.contains("USING INDEX t_ab"),
            "expected (a,b) index path for leading-only equality, got plan:\n{plan}"
        );

        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1 ORDER BY b")
            .expect("prepare");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_text(0).expect("b").to_owned());
        }
        assert_eq!(
            rows,
            vec!["x".to_owned(), "y".to_owned(), "z".to_owned()],
            "leading-prefix range must surface every (a=1, *) row"
        );
    }

    /// Regression: composite (a, b) index with WHERE a = ? AND b = ? is a
    /// full-key point lookup; the upper bound must be tight enough to NOT
    /// surface rows for other `b` values, and lax enough to include the
    /// requested one.
    #[test]
    fn composite_index_leading_prefix_with_explicit_b() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_ab ON t(a, b)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 'x')")
            .expect("insert 1x");
        conn.execute("INSERT INTO t VALUES (1, 'y')")
            .expect("insert 1y");
        conn.execute("INSERT INTO t VALUES (1, 'z')")
            .expect("insert 1z");
        conn.execute("INSERT INTO t VALUES (2, 'x')")
            .expect("insert 2x");

        // Full-key equality should resolve to a point lookup.
        let plan = explain_text(&conn, "SELECT b FROM t WHERE a = 1 AND b = 'y'");
        assert!(
            plan.contains("USING INDEX t_ab") && plan.contains("PointLookup"),
            "expected (a,b) IndexPointLookup, got plan:\n{plan}"
        );

        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1 AND b = 'y'")
            .expect("prepare");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push(stmt.column_text(0).expect("b").to_owned());
        }
        assert_eq!(rows, vec!["y".to_owned()]);
    }
}

// ----- Lane KH (Wave 7) regressions -----

/// P1 #5: the planner must skip indexes whose catalog entry has no
/// `meta_page_id` even when the engine could otherwise satisfy the
/// predicate. CREATE TABLE PRIMARY KEY autoindexes are exactly this
/// case — they live in the snapshot but no physical B-tree is
/// allocated until CREATE INDEX runs. Before the fix, EXPLAIN
/// reported `IndexPointLookup` while the executor silently fell back
/// to a TableScan.
#[test]
fn planner_does_not_advertise_index_without_handle() {
    let (_dir, conn) = open_database();
    // CREATE TABLE PRIMARY KEY records an autoindex with
    // `meta_page_id=None` and never registers an engine handle for it.
    // (CREATE INDEX is the only path that allocates the physical
    // pages.) The planner must observe that absence and pick TableScan.
    conn.execute("CREATE TABLE t(k TEXT PRIMARY KEY, v INTEGER)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES ('a', 1)")
        .expect("insert a");
    conn.execute("INSERT INTO t VALUES ('b', 2)")
        .expect("insert b");

    let prepared = "EXPLAIN QUERY PLAN SELECT v FROM t WHERE k = 'a'";
    let mut stmt = conn.prepare(prepared).expect("prepare explain");
    let mut detail = String::new();
    while let Step::Row = stmt.step().expect("step explain") {
        detail.push_str(stmt.column_text(3).expect("detail"));
        detail.push('\n');
    }
    assert!(
        detail.contains("SCAN TABLE t"),
        "expected TableScan, got plan:\n{detail}"
    );
    assert!(
        !detail.contains("USING INDEX"),
        "must not advertise an index without a live handle:\n{detail}"
    );

    // Sanity: the executor must still satisfy the predicate via the
    // fallback path so end-user behavior matches the EXPLAIN output.
    let mut stmt = conn
        .prepare("SELECT v FROM t WHERE k = 'a'")
        .expect("prepare select");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("v"), 1);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}
