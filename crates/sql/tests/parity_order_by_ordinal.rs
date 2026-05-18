//! ORDER BY / positional column reference parity against the rusqlite
//! oracle. SQLite treats a bare positive integer at the top of an ORDER BY
//! term as a 1-based reference to the N-th output column. Constant
//! expressions (`1+1`, `1.5`, parenthesised `(1)`) are NOT positional and
//! must be evaluated as ordinary expressions.

#[path = "parity_oracle/harness.rs"]
mod harness;

const T: &str = "
    CREATE TABLE t(a INTEGER, b INTEGER);
    INSERT INTO t VALUES (3, 30), (1, 10), (2, 20);
";

const T1_T2: &str = "
    CREATE TABLE t1(id INTEGER, x INTEGER);
    CREATE TABLE t2(id INTEGER, price INTEGER);
    INSERT INTO t1 VALUES (5, 50), (1, 10), (3, 30);
    INSERT INTO t2 VALUES (4, 4), (2, 2), (6, 6);
";

#[test]
fn single_branch_order_by_position_asc() {
    harness::assert_parity(&format!("{T} SELECT a, b FROM t ORDER BY 1"));
}

#[test]
fn single_branch_order_by_position_desc() {
    harness::assert_parity(&format!("{T} SELECT a, b FROM t ORDER BY 2 DESC"));
}

#[test]
fn single_branch_order_by_two_positions() {
    let sql = "
        CREATE TABLE t(a INTEGER, b INTEGER);
        INSERT INTO t VALUES (1, 30), (1, 10), (2, 20), (1, 20);
        SELECT a, b FROM t ORDER BY 1, 2";
    harness::assert_parity(sql);
}

#[test]
fn compound_union_all_order_by_position() {
    // Matches the fuzz divergence shape from
    // target/proof/sqlite-full-parity/fuzz-divergence.txt (seed=7 iter=110).
    harness::assert_parity(&format!(
        "{T1_T2} SELECT id FROM t1 WHERE x > 6 \
         UNION ALL SELECT id FROM t2 WHERE price < 6 ORDER BY 1"
    ));
}

#[test]
fn compound_union_distinct_order_by_position() {
    harness::assert_parity(&format!(
        "{T1_T2} SELECT id FROM t1 UNION SELECT id FROM t2 ORDER BY 1"
    ));
}

#[test]
fn compound_intersect_order_by_position() {
    let sql = "
        CREATE TABLE a(v INTEGER);
        CREATE TABLE b(v INTEGER);
        INSERT INTO a VALUES (3), (1), (2);
        INSERT INTO b VALUES (2), (3), (4);
        SELECT v FROM a INTERSECT SELECT v FROM b ORDER BY 1";
    harness::assert_parity(sql);
}

#[test]
fn order_by_expression_not_literal_unchanged() {
    // ORDER BY a + 1 must keep expression semantics; the rewrite is
    // top-level-only and must not descend into BinaryOp.
    harness::assert_parity(&format!("{T} SELECT a, b FROM t ORDER BY a + 1"));
}

#[test]
fn order_by_constant_expression_does_not_reorder_as_ordinal() {
    // SQLite treats `ORDER BY 1 + 1` as the constant 2 (not ordinal 2). If
    // the rewrite incorrectly fired on the inner literal, output would be
    // sorted by column b. Verify the negative: results must NOT be sorted
    // by b ascending in either engine. (We don't compare engine-to-engine
    // because both engines preserve different underlying row orders when
    // every sort key is equal — that's a separate harness limitation, not
    // a rewrite bug.)
    let sql = format!("{T} SELECT a, b FROM t ORDER BY 1 + 1");
    let redline = harness::run_redline(&sql);
    let bs: Vec<i64> = redline
        .rows
        .iter()
        .map(|r| match &r[1] {
            redlinedb_sql::SqlValue::Integer(v) => *v,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect();
    assert_ne!(
        bs,
        vec![10_i64, 20, 30],
        "ORDER BY 1+1 must not be rewritten as ORDER BY 2"
    );
}
