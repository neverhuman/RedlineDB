//! Baseline smoke: open in-memory pool, create table, insert, select.

use redlinedb_tokio::{Pool, params};

#[tokio::test]
async fn round_trips_single_row() {
    let pool = Pool::open_in_memory().await.expect("pool");

    pool.execute(
        "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        params![],
    )
    .await
    .expect("create");

    let summary = pool
        .execute(
            "INSERT INTO items(id, name) VALUES (?, ?)",
            params![42_i64, "Ada"],
        )
        .await
        .expect("insert");
    assert_eq!(summary.rows_returned, 0);

    let row = pool
        .fetch_one("SELECT name FROM items WHERE id = ?", params![42_i64])
        .await
        .expect("fetch_one");
    assert_eq!(row.try_get_text(0).unwrap(), "Ada");

    let missing = pool
        .fetch_optional("SELECT name FROM items WHERE id = ?", params![999_i64])
        .await
        .expect("fetch_optional");
    assert!(missing.is_none());

    let all = pool
        .fetch_all("SELECT id, name FROM items ORDER BY id", params![])
        .await
        .expect("fetch_all");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].try_get_i64(0).unwrap(), 42);
    assert_eq!(all[0].try_get_text(1).unwrap(), "Ada");
}
