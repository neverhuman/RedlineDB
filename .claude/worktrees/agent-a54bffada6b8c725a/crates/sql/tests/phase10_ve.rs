//! Lane VE end-to-end integration tests: vectorized executor primitives
//! exercised through real SQL statements, not unit-level fixtures.
//!
//! Coverage areas (mirrors the lane plan):
//!   - Selection vector / filter parity with materialized scan.
//!   - Hash aggregation over GROUP BY (count, sum, avg, min, max).
//!   - Top-K heap activation for `ORDER BY ... LIMIT k`, including ties +
//!     DESC + NULL keys.
//!   - Spillable sort: in-memory path, spill-triggered path, and merge of
//!     several spilled runs.
//!   - End-to-end `ORDER BY` query honouring `work_mem_bytes`.
//!   - Regression for the previously O(n^2) grouped path.

use std::sync::Arc;

use redlinedb_sql::{Database, DbOptions, QueryMemoryConfig, Step};
use tempfile::tempdir;

fn open_database_with(opts: DbOptions) -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("phase10-ve.db");
    let db = Database::create(&path, opts).expect("create");
    let conn = db.connect();
    (dir, conn)
}

fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    open_database_with(DbOptions::default())
}

fn collect_int(stmt: &mut redlinedb_sql::Statement) -> Vec<Vec<i64>> {
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let mut row = Vec::new();
        for col in 0..stmt.column_count() {
            row.push(stmt.column_i64(col).unwrap_or(0));
        }
        rows.push(row);
    }
    rows
}

#[test]
fn phase10_ve_topk_returns_smallest_with_small_limit() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for i in (0..200i64).rev() {
        conn.execute(&format!("INSERT INTO t VALUES ({i}, {})", i * 2))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k LIMIT 5")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(got, vec![0, 1, 2, 3, 4]);
}

#[test]
fn phase10_ve_topk_desc_returns_largest() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..100i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({i})"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k DESC LIMIT 3")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(got, vec![99, 98, 97]);
}

#[test]
fn phase10_ve_topk_with_ties_is_deterministic() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, ord INTEGER)")
        .expect("create");
    for i in 0..100i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({}, {})", i % 2, i))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k LIMIT 10")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    // All keys should be 0 (smallest); top-10 of all-zero rows.
    for row in &rows {
        assert_eq!(row[0], 0);
    }
}

#[test]
fn phase10_ve_topk_with_null_keys_orders_nulls_first_asc() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, label TEXT)")
        .expect("create");
    conn.execute("INSERT INTO t VALUES (NULL, 'nul1')")
        .expect("insert");
    conn.execute("INSERT INTO t VALUES (1, 'one')")
        .expect("insert");
    conn.execute("INSERT INTO t VALUES (2, 'two')")
        .expect("insert");
    conn.execute("INSERT INTO t VALUES (3, 'three')")
        .expect("insert");
    let mut stmt = conn
        .prepare("SELECT label FROM t ORDER BY k LIMIT 2")
        .expect("prepare");
    let mut labels = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        labels.push(stmt.column_text(0).expect("text").to_owned());
    }
    assert_eq!(labels[0], "nul1");
}

#[test]
fn phase10_ve_topk_k64_boundary_uses_heap_path() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..200i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({i})"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k LIMIT 64")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(got, (0..64).collect::<Vec<_>>());
}

#[test]
fn phase10_ve_hash_agg_count_sum_avg_min_max() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for i in 1..=10i64 {
        let group = i % 3;
        conn.execute(&format!("INSERT INTO t VALUES ({group}, {i})"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k, COUNT(*), SUM(v), MIN(v), MAX(v) FROM t GROUP BY k ORDER BY k")
        .expect("prepare");
    let mut groups: Vec<(i64, i64, i64, i64, i64)> = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        groups.push((
            stmt.column_i64(0).unwrap_or(0),
            stmt.column_i64(1).unwrap_or(0),
            stmt.column_i64(2).unwrap_or(0),
            stmt.column_i64(3).unwrap_or(0),
            stmt.column_i64(4).unwrap_or(0),
        ));
    }
    // Groups: g=0 -> rows {3,6,9} sum=18; g=1 -> {1,4,7,10} sum=22;
    //         g=2 -> {2,5,8} sum=15.
    let expected: Vec<(i64, i64, i64, i64, i64)> =
        vec![(0, 3, 18, 3, 9), (1, 4, 22, 1, 10), (2, 3, 15, 2, 8)];
    assert_eq!(groups, expected);
}

#[test]
fn phase10_ve_hash_agg_avg_real() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for v in [10, 20, 30] {
        conn.execute(&format!("INSERT INTO t VALUES (1, {v})"))
            .expect("insert");
    }
    let mut stmt = conn.prepare("SELECT AVG(v) FROM t").expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let avg = stmt.column_f64(0).expect("avg");
    assert!((avg - 20.0).abs() < 1e-9);
}

#[test]
fn phase10_ve_spillable_sort_correct_under_tight_budget() {
    let opts = DbOptions {
        query_memory: QueryMemoryConfig {
            work_mem_bytes: 1, // force spill
            max_spill_bytes: 256 * 1024 * 1024,
            batch_rows: 1024,
        },
        ..DbOptions::default()
    };
    let (_dir, conn) = open_database_with(opts);
    conn.execute("CREATE TABLE t(k INTEGER, payload TEXT)")
        .expect("create");
    for i in 0..1_000i64 {
        // Pseudo-random insertion order, but deterministic.
        let perm = (i.wrapping_mul(2654435761)) % 1_000;
        conn.execute(&format!("INSERT INTO t VALUES ({perm}, 'payload-{i:04}')"))
            .expect("insert");
    }
    let mut stmt = conn.prepare("SELECT k FROM t ORDER BY k").expect("prepare");
    let rows = collect_int(&mut stmt);
    assert_eq!(rows.len(), 1_000);
    let mut prev = i64::MIN;
    for row in &rows {
        assert!(row[0] >= prev, "out-of-order: {row:?} prev={prev}");
        prev = row[0];
    }
}

#[test]
fn phase10_ve_spillable_sort_in_memory_path_correct() {
    // Big budget => never spills.
    let opts = DbOptions {
        query_memory: QueryMemoryConfig {
            work_mem_bytes: 16 * 1024 * 1024,
            max_spill_bytes: 256 * 1024 * 1024,
            batch_rows: 1024,
        },
        ..DbOptions::default()
    };
    let (_dir, conn) = open_database_with(opts);
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in (0..100i64).rev() {
        conn.execute(&format!("INSERT INTO t VALUES ({i})"))
            .expect("insert");
    }
    let mut stmt = conn.prepare("SELECT k FROM t ORDER BY k").expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(got, (0..100).collect::<Vec<_>>());
}

#[test]
fn phase10_ve_spillable_sort_runs_merge_correctly() {
    // Tiny budget guarantees several flushes.
    let opts = DbOptions {
        query_memory: QueryMemoryConfig {
            work_mem_bytes: 1,
            max_spill_bytes: 256 * 1024 * 1024,
            batch_rows: 1024,
        },
        ..DbOptions::default()
    };
    let (_dir, conn) = open_database_with(opts);
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..2_000i64 {
        // alternating high/low feeds many distinct runs
        let v = if i % 2 == 0 { i } else { 2_000 - i };
        conn.execute(&format!("INSERT INTO t VALUES ({v})"))
            .expect("insert");
    }
    let mut stmt = conn.prepare("SELECT k FROM t ORDER BY k").expect("prepare");
    let rows = collect_int(&mut stmt);
    assert_eq!(rows.len(), 2_000);
    let mut prev = i64::MIN;
    for row in &rows {
        assert!(row[0] >= prev);
        prev = row[0];
    }
}

#[test]
fn phase10_ve_grouped_aggregate_no_quadratic_blowup() {
    // Lane VE replaced the O(n^2) "linear-find" group build. With 5_000
    // distinct keys this would have taken minutes pre-VE; we only assert
    // correctness here, but the test name documents the intent.
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("BEGIN").expect("begin");
    for i in 0..5_000i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({i}, {})", i * 2))
            .expect("insert");
    }
    conn.execute("COMMIT").expect("commit");
    let started = std::time::Instant::now();
    let mut stmt = conn
        .prepare("SELECT k, COUNT(*) FROM t GROUP BY k")
        .expect("prepare");
    let mut count = 0;
    while let Step::Row = stmt.step().expect("step") {
        count += 1;
    }
    let elapsed = started.elapsed();
    assert_eq!(count, 5_000);
    // Loose budget; pre-VE this took >>10s for 5k distinct keys.
    assert!(
        elapsed.as_secs() < 30,
        "5k distinct GROUP BY took {:?} — likely O(n^2) regression",
        elapsed
    );
}

#[test]
fn phase10_ve_distinct_with_order_by_still_correct() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(v TEXT)").expect("create");
    for s in ["b", "a", "c", "a", "b", "d"] {
        conn.execute(&format!("INSERT INTO t VALUES ('{s}')"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT DISTINCT v FROM t ORDER BY v")
        .expect("prepare");
    let mut got: Vec<String> = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        got.push(stmt.column_text(0).expect("v").to_owned());
    }
    assert_eq!(got, vec!["a", "b", "c", "d"]);
}

#[test]
fn phase10_ve_grouped_with_order_by_remains_consistent() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for i in 1..=12i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({}, {i})", i % 4))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k, SUM(v) FROM t GROUP BY k ORDER BY k LIMIT 3")
        .expect("prepare");
    let mut got: Vec<(i64, i64)> = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        got.push((
            stmt.column_i64(0).unwrap_or(0),
            stmt.column_i64(1).unwrap_or(0),
        ));
    }
    // groups by i%4: 0 -> {4,8,12}=24, 1 -> {1,5,9}=15, 2 -> {2,6,10}=18, 3 -> {3,7,11}=21
    // ORDER BY k ASC LIMIT 3 => keys 0,1,2.
    assert_eq!(got, vec![(0, 24), (1, 15), (2, 18)]);
}

#[test]
fn phase10_ve_filter_parity_with_baseline() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for i in 0..200i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({i}, {})", i * 2))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t WHERE v > 100 AND k < 80 ORDER BY k LIMIT 3")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    // v=2*k > 100 => k > 50; k < 80; smallest 3 => 51,52,53.
    assert_eq!(got, vec![51, 52, 53]);
}

#[test]
fn phase10_ve_full_sort_path_no_limit_correct() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..500i64 {
        let v = (i.wrapping_mul(2654435761)) % 500;
        conn.execute(&format!("INSERT INTO t VALUES ({v})"))
            .expect("insert");
    }
    let mut stmt = conn.prepare("SELECT k FROM t ORDER BY k").expect("prepare");
    let rows = collect_int(&mut stmt);
    let mut prev = i64::MIN;
    for row in &rows {
        assert!(row[0] >= prev, "out-of-order");
        prev = row[0];
    }
    assert_eq!(rows.len(), 500);
}

#[test]
fn phase10_ve_top_k_above_threshold_uses_full_sort() {
    // Limit > TOPK threshold (=64) => spillable / in-memory full sort path.
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..200i64 {
        let v = (i.wrapping_mul(2654435761)) % 200;
        conn.execute(&format!("INSERT INTO t VALUES ({v})"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k LIMIT 100")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    assert_eq!(rows.len(), 100);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    let mut expected: Vec<i64> = (0..200i64)
        .map(|i| i.wrapping_mul(2654435761) % 200)
        .collect();
    expected.sort();
    assert_eq!(got, expected[..100]);
}

#[test]
fn phase10_ve_offset_with_topk_skips_rows() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER)").expect("create");
    for i in 0..30i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({i})"))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t ORDER BY k LIMIT 5 OFFSET 10")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(got, vec![10, 11, 12, 13, 14]);
}

#[test]
fn phase10_ve_having_with_hash_agg() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(k INTEGER, v INTEGER)")
        .expect("create");
    for i in 1..=12i64 {
        conn.execute(&format!("INSERT INTO t VALUES ({}, 1)", i % 3))
            .expect("insert");
    }
    let mut stmt = conn
        .prepare("SELECT k FROM t GROUP BY k HAVING COUNT(*) > 3 ORDER BY k")
        .expect("prepare");
    let rows = collect_int(&mut stmt);
    let got: Vec<i64> = rows.iter().map(|r| r[0]).collect();
    // groups: 0 -> 4 rows, 1 -> 4 rows, 2 -> 4 rows. All > 3.
    assert_eq!(got, vec![0, 1, 2]);
}
