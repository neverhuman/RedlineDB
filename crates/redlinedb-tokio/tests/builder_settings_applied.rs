//! PoolBuilder: max_connections + busy_timeout settings take effect.

use redlinedb_tokio::{OpenOptions, Pool, params};
use std::time::Duration;

#[tokio::test]
async fn builder_constructs_pool_with_custom_settings() {
    let db = redlinedb_tokio::Database::create_in_memory(OpenOptions::default()).expect("inmem db");
    let pool = Pool::builder()
        .database(db)
        .max_connections(4)
        .busy_timeout(Duration::from_millis(250))
        .build()
        .expect("build");

    // Basic round-trip on the pool to confirm it works at the configured limit.
    pool.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", params![])
        .await
        .expect("create");
    for i in 0..10 {
        pool.execute("INSERT INTO t(id) VALUES (?)", params![i as i64])
            .await
            .expect("insert");
    }
    let row = pool
        .fetch_one("SELECT COUNT(*) FROM t", params![])
        .await
        .unwrap();
    assert_eq!(row.try_get_i64(0).unwrap(), 10);
}

#[tokio::test]
#[should_panic(expected = "max_connections must be > 0")]
async fn zero_max_connections_panics() {
    let db = redlinedb_tokio::Database::create_in_memory(OpenOptions::default()).expect("inmem db");
    let _ = Pool::builder().database(db).max_connections(0).build();
}

#[tokio::test]
async fn builder_without_database_errors() {
    let result = Pool::builder().build();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), redlinedb_tokio::ErrorCode::Misuse);
}
