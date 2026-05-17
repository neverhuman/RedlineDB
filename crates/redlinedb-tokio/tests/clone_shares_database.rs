//! Pool::clone() produces a sibling pool that reads/writes the SAME database.

use redlinedb_tokio::{Pool, params};

#[tokio::test]
async fn clone_writes_visible_to_original() {
    let pool_a = Pool::open_in_memory().await.expect("pool");
    let pool_b = pool_a.clone();

    pool_a
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", params![])
        .await
        .expect("create on A");

    pool_b
        .execute("INSERT INTO t(id) VALUES (1)", params![])
        .await
        .expect("insert on B");

    let row = pool_a
        .fetch_one("SELECT id FROM t", params![])
        .await
        .expect("fetch on A");
    assert_eq!(row.try_get_i64(0).unwrap(), 1);
}
