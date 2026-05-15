//! Phase 11 Wave 1b Worker C/D/E acceptance tests.
//!
//! W1-C — streaming index-access with batched cursor consumption +
//!         per-heap-page rechecks + early-stop on `LIMIT n`.
//! W1-D — index-aware `ORDER BY k LIMIT n` over a leading column.
//! W1-E — `IndexCountRange` for `SELECT COUNT(*) FROM t WHERE k
//!         BETWEEN ? AND ?` and simple covering scans for `SELECT k, v
//!         FROM t WHERE k BETWEEN ? AND ?` against an `(k, v)` index.
//!
//! These suites assert observable end-to-end behaviour rather than
//! probing the bench harness directly — the fast paths must produce
//! identical answers to the legacy heap path, regardless of which
//! plan node services them.

mod common;

use std::sync::Arc;

use common::open_database;
use redlinedb_sql::Step;

fn insert_kv(
    conn: &Arc<redlinedb_sql::Connection>,
    n: i64,
    tenant_mod: i64,
    val_fn: impl Fn(i64) -> i64,
) {
    for i in 0..n {
        let mut stmt = conn
            .prepare("INSERT INTO kv(k, tenant, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, i).unwrap();
        stmt.bind_i64(2, i % tenant_mod).unwrap();
        stmt.bind_i64(3, val_fn(i)).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }
}

fn query_pairs(conn: &Arc<redlinedb_sql::Connection>, sql: &str, binds: &[i64]) -> Vec<(i64, i64)> {
    let mut stmt = conn.prepare(sql).expect("prep query");
    for (idx, value) in binds.iter().enumerate() {
        stmt.bind_i64(idx + 1, *value).expect("bind");
    }
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((
            stmt.column_i64(0).expect("col0"),
            stmt.column_i64(1).expect("col1"),
        ));
    }
    rows
}

fn query_single_i64(conn: &Arc<redlinedb_sql::Connection>, sql: &str, binds: &[i64]) -> Vec<i64> {
    let mut stmt = conn.prepare(sql).expect("prep query");
    for (idx, value) in binds.iter().enumerate() {
        stmt.bind_i64(idx + 1, *value).expect("bind");
    }
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push(stmt.column_i64(0).expect("col0"));
    }
    rows
}

#[test]
fn w1c_streaming_index_range_returns_visible_rows() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    insert_kv(&conn, 1000, 32, |i| i);
    let mut stmt = conn
        .prepare("SELECT k FROM kv WHERE tenant = ?1")
        .expect("prep select");
    stmt.bind_i64(1, 7).unwrap();
    let mut count = 0;
    while let Step::Row = stmt.step().expect("step") {
        let k = stmt.column_i64(0).expect("k");
        assert_eq!(k % 32, 7);
        count += 1;
    }
    // tenant 7 covers k = 7, 39, 71, ..., 999 — about 31-32 rows.
    assert!((30..=32).contains(&count), "count = {count}");
}

#[test]
fn w1c_index_range_with_limit_returns_correct_count() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    insert_kv(&conn, 2000, 32, |i| i);
    let mut stmt = conn
        .prepare("SELECT k FROM kv WHERE tenant >= ?1 ORDER BY tenant LIMIT 5")
        .expect("prep");
    stmt.bind_i64(1, 0).unwrap();
    let mut count = 0;
    while let Step::Row = stmt.step().expect("step") {
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn w1d_ordered_index_limit_emits_in_key_order() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    for i in 0..128_i64 {
        let tenant = (i * 7) % 32; // scattered insertion order
        let mut stmt = conn
            .prepare("INSERT INTO kv(k, tenant, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, i).unwrap();
        stmt.bind_i64(2, tenant).unwrap();
        stmt.bind_i64(3, i).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }
    let mut stmt = conn
        .prepare("SELECT tenant FROM kv WHERE tenant >= ?1 ORDER BY tenant LIMIT 10")
        .expect("prep");
    stmt.bind_i64(1, 0).unwrap();
    let mut tenants = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        tenants.push(stmt.column_i64(0).expect("tenant"));
    }
    assert_eq!(tenants.len(), 10);
    let mut sorted = tenants.clone();
    sorted.sort();
    assert_eq!(tenants, sorted, "rows must already be in tenant order");
}

#[test]
fn w1e_count_range_matches_table_scan() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    insert_kv(&conn, 500, 16, |i| i);
    // Index path
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2")
        .expect("prep");
    stmt.bind_i64(1, 3).unwrap();
    stmt.bind_i64(2, 7).unwrap();
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let count_index = stmt.column_i64(0).expect("c");
    // Reference: a `tenant + 0` predicate disables the index probe and
    // forces a heap scan.
    let mut stmt2 = conn
        .prepare("SELECT COUNT(*) FROM kv WHERE tenant + 0 BETWEEN ?1 AND ?2")
        .expect("prep");
    stmt2.bind_i64(1, 3).unwrap();
    stmt2.bind_i64(2, 7).unwrap();
    assert_eq!(stmt2.step().expect("step"), Step::Row);
    let count_scan = stmt2.column_i64(0).expect("c");
    assert_eq!(count_index, count_scan);
    // Sanity: 500 rows / 16 buckets, range covers buckets 3..=7 (5
    // buckets * ~31-32 rows each).
    assert!((5 * 31..=5 * 32).contains(&count_index));
}

#[test]
fn w1e_covering_scan_returns_index_columns() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE covered_kv(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX covered_kv_kv ON covered_kv(k, v)")
        .expect("create idx");
    for i in 0..300_i64 {
        let mut stmt = conn
            .prepare("INSERT INTO covered_kv(id, k, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, i).unwrap();
        stmt.bind_i64(2, i).unwrap();
        stmt.bind_i64(3, i * 2).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }
    let mut stmt = conn
        .prepare("SELECT k, v FROM covered_kv WHERE k BETWEEN ?1 AND ?2")
        .expect("prep");
    stmt.bind_i64(1, 50).unwrap();
    stmt.bind_i64(2, 60).unwrap();
    let mut rows: Vec<(i64, i64)> = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let k = stmt.column_i64(0).expect("k");
        let v = stmt.column_i64(1).expect("v");
        rows.push((k, v));
    }
    assert_eq!(rows.len(), 11); // 50, 51, ..., 60
    for (k, v) in &rows {
        assert_eq!(*v, k * 2, "covering decoder wrong: k={k} v={v}");
    }
    let mut sorted = rows.clone();
    sorted.sort_by_key(|(k, _)| *k);
    assert_eq!(rows, sorted, "covered scan walks ascending k");

    let fast = rows;
    let mut slow = query_pairs(
        &conn,
        "SELECT k, v FROM covered_kv WHERE k + 0 BETWEEN ?1 AND ?2",
        &[50, 60],
    );
    let mut fast_sorted = fast.clone();
    fast_sorted.sort_by_key(|(k, _)| *k);
    slow.sort_by_key(|(k, _)| *k);
    assert_eq!(
        fast_sorted, slow,
        "covering fast path must match forced fallback"
    );
}

#[test]
fn w1e_count_star_zero_for_empty_range() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2")
        .expect("prep");
    stmt.bind_i64(1, 0).unwrap();
    stmt.bind_i64(2, 99).unwrap();
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("c"), 0);
}

#[test]
fn w1e_covering_with_order_and_limit() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE covered_kv(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX covered_kv_kv ON covered_kv(k, v)")
        .expect("create idx");
    for i in 0..50_i64 {
        let mut stmt = conn
            .prepare("INSERT INTO covered_kv(id, k, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, i).unwrap();
        stmt.bind_i64(2, i).unwrap();
        stmt.bind_i64(3, i + 100).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }
    let mut stmt = conn
        .prepare("SELECT k, v FROM covered_kv WHERE k BETWEEN ?1 AND ?2 ORDER BY k LIMIT 3")
        .expect("prep");
    stmt.bind_i64(1, 10).unwrap();
    stmt.bind_i64(2, 49).unwrap();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push((stmt.column_i64(0).unwrap(), stmt.column_i64(1).unwrap()));
    }
    assert_eq!(rows, vec![(10, 110), (11, 111), (12, 112)]);

    let slow = query_pairs(
        &conn,
        "SELECT k, v FROM covered_kv WHERE k + 0 BETWEEN ?1 AND ?2 ORDER BY k LIMIT 3",
        &[10, 49],
    );
    assert_eq!(rows, slow, "ordered covering fast path must match fallback");
}

#[test]
fn w1d_secondary_index_read_matches_forced_fallback() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    for &(k, tenant, v) in &[
        (42_i64, 7_i64, 10_i64),
        (4, 7, 11),
        (19, 7, 12),
        (61, 3, 13),
    ] {
        let mut stmt = conn
            .prepare("INSERT INTO kv(k, tenant, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, k).unwrap();
        stmt.bind_i64(2, tenant).unwrap();
        stmt.bind_i64(3, v).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }

    let fast = query_single_i64(
        &conn,
        "SELECT k FROM kv WHERE tenant = ?1 ORDER BY k LIMIT 1",
        &[7],
    );
    let slow = query_single_i64(
        &conn,
        "SELECT k FROM kv WHERE tenant + 0 = ?1 ORDER BY k LIMIT 1",
        &[7],
    );
    assert_eq!(
        fast, slow,
        "secondary-index-read fast path must match fallback"
    );
    assert_eq!(fast, vec![4], "ordered limit should stop at the smallest k");
}

#[test]
fn w1d_ordered_limit_matches_forced_fallback() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    for i in 0..128_i64 {
        let tenant = i;
        let mut stmt = conn
            .prepare("INSERT INTO kv(k, tenant, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, i * 3 + 1).unwrap();
        stmt.bind_i64(2, tenant).unwrap();
        stmt.bind_i64(3, i).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }

    let fast = query_pairs(
        &conn,
        "SELECT tenant, k FROM kv WHERE tenant >= ?1 ORDER BY tenant LIMIT 5",
        &[7],
    );
    let slow = query_pairs(
        &conn,
        "SELECT tenant, k FROM kv WHERE tenant + 0 >= ?1 ORDER BY tenant LIMIT 5",
        &[7],
    );
    assert_eq!(fast, slow, "ordered limit fast path must match fallback");
    assert_eq!(fast.len(), 5);
    let mut sorted = fast.clone();
    sorted.sort_by_key(|(tenant, _)| *tenant);
    assert_eq!(fast, sorted, "fast path must preserve tenant order");
}

#[test]
fn w1d_rowid_limit_uses_rowid_suffix_and_matches_fallback() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");
    for &(k, tenant, v) in &[
        (42_i64, 7_i64, 10_i64),
        (4, 7, 11),
        (19, 7, 12),
        (61, 3, 13),
    ] {
        let mut stmt = conn
            .prepare("INSERT INTO kv(k, tenant, v) VALUES (?1, ?2, ?3)")
            .expect("prep insert");
        stmt.bind_i64(1, k).unwrap();
        stmt.bind_i64(2, tenant).unwrap();
        stmt.bind_i64(3, v).unwrap();
        assert_eq!(stmt.step().unwrap(), Step::Done);
    }

    let fast = query_single_i64(
        &conn,
        "SELECT rowid FROM kv WHERE tenant = ?1 ORDER BY rowid LIMIT 1",
        &[7],
    );
    let slow = query_single_i64(
        &conn,
        "SELECT rowid FROM kv WHERE tenant + 0 = ?1 ORDER BY rowid LIMIT 1",
        &[7],
    );
    assert_eq!(fast, slow, "rowid-limit fast path must match fallback");
    assert_eq!(
        fast,
        vec![4],
        "rowid order should stop at the smallest rowid"
    );
}
