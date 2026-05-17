//! Bind every Value variant and round-trip it through the pool.

use redlinedb_tokio::{Pool, Value, params};

#[tokio::test]
async fn round_trips_all_value_variants() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute(
        "CREATE TABLE kv(
            id INTEGER PRIMARY KEY,
            i INTEGER,
            r REAL,
            t TEXT,
            b BLOB
        )",
        params![],
    )
    .await
    .expect("create");

    let blob = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    pool.execute(
        "INSERT INTO kv(id, i, r, t, b) VALUES (?, ?, ?, ?, ?)",
        params![1_i64, 42_i64, std::f64::consts::PI, "hello", blob.clone()],
    )
    .await
    .expect("insert");

    let row = pool
        .fetch_one("SELECT i, r, t, b FROM kv WHERE id = 1", params![])
        .await
        .expect("fetch");
    assert_eq!(row.try_get_i64(0).unwrap(), 42);
    assert!((row.try_get_f64(1).unwrap() - std::f64::consts::PI).abs() < 1e-12);
    assert_eq!(row.try_get_text(2).unwrap(), "hello");
    assert_eq!(row.try_get_blob(3).unwrap(), blob.as_slice());

    // Null variant
    pool.execute(
        "INSERT INTO kv(id, i, r, t, b) VALUES (?, ?, ?, ?, ?)",
        vec![
            Value::Integer(2),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
        ],
    )
    .await
    .expect("insert nulls");

    let row = pool
        .fetch_one("SELECT i, t FROM kv WHERE id = 2", params![])
        .await
        .expect("fetch null row");
    assert_eq!(row.try_get_optional_i64(0).unwrap(), None);
    assert_eq!(row.try_get_optional_text(1).unwrap(), None);
}
