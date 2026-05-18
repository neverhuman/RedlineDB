mod common;

use std::sync::Arc;

use common::open_database;
use redlinedb_sql::Step;

fn insert_pair(conn: &Arc<redlinedb_sql::Connection>, table: &str, id: i64, tenant: i64, v: i64) {
    let sql = format!("INSERT INTO {table}(id, tenant, v) VALUES (?1, ?2, ?3)");
    let mut stmt = conn.prepare(&sql).expect("prep insert");
    stmt.bind_i64(1, id).unwrap();
    stmt.bind_i64(2, tenant).unwrap();
    stmt.bind_i64(3, v).unwrap();
    assert_eq!(stmt.step().unwrap(), Step::Done);
}

fn query_i64(conn: &Arc<redlinedb_sql::Connection>, sql: &str, binds: &[i64]) -> Vec<i64> {
    let mut stmt = conn.prepare(sql).expect("prep query");
    for (idx, value) in binds.iter().enumerate() {
        stmt.bind_i64(idx + 1, *value).unwrap();
    }
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        rows.push(stmt.column_i64(0).expect("col0"));
    }
    rows
}

#[test]
fn indexed_point_and_range_results_match_reference_table() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE kv(id INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create kv");
    conn.execute("CREATE TABLE kv_ref(id INTEGER PRIMARY KEY, tenant INTEGER, v INTEGER)")
        .expect("create ref");
    conn.execute("CREATE INDEX kv_tenant ON kv(tenant)")
        .expect("create idx");

    for i in 0..512_i64 {
        let tenant = (i * 17) % 41;
        let v = i * 3;
        insert_pair(&conn, "kv", i, tenant, v);
        insert_pair(&conn, "kv_ref", i, tenant, v);
    }

    let indexed_point = query_i64(
        &conn,
        "SELECT id FROM kv WHERE tenant = ?1 ORDER BY id",
        &[7],
    );
    let reference_point = query_i64(
        &conn,
        "SELECT id FROM kv_ref WHERE tenant = ?1 ORDER BY id",
        &[7],
    );
    assert_eq!(indexed_point, reference_point);

    let indexed_range = query_i64(
        &conn,
        "SELECT id FROM kv WHERE tenant >= ?1 AND tenant <= ?2 ORDER BY tenant, id",
        &[5, 12],
    );
    let reference_range = query_i64(
        &conn,
        "SELECT id FROM kv_ref WHERE tenant >= ?1 AND tenant <= ?2 ORDER BY tenant, id",
        &[5, 12],
    );
    assert_eq!(indexed_range, reference_range);
}
