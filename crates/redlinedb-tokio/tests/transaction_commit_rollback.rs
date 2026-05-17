//! Pool::transaction commits on Ok, rolls back on Err.

use redlinedb_tokio::{Pool, params};

#[tokio::test]
async fn commits_on_ok() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", params![])
        .await
        .expect("create");

    pool.transaction(|conn| {
        conn.execute("INSERT INTO t(id) VALUES (1)", ())?;
        conn.execute("INSERT INTO t(id) VALUES (2)", ())?;
        Ok(())
    })
    .await
    .expect("tx ok");

    let row = pool
        .fetch_one("SELECT COUNT(*) FROM t", params![])
        .await
        .unwrap();
    assert_eq!(row.try_get_i64(0).unwrap(), 2);
}

#[tokio::test]
async fn rolls_back_on_err() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", params![])
        .await
        .expect("create");
    pool.execute("INSERT INTO t(id) VALUES (1)", params![])
        .await
        .expect("seed");

    let result: Result<(), _> = pool
        .transaction(|conn| {
            conn.execute("INSERT INTO t(id) VALUES (2)", ())?;
            // Force an error inside the transaction; expect rollback.
            Err(redlinedb_tokio::Error::new(
                redlinedb_tokio::ErrorCode::Abort,
                "intentional",
            ))
        })
        .await;
    assert!(result.is_err());

    // Row 2 must NOT exist — transaction rolled back.
    let row = pool
        .fetch_one("SELECT COUNT(*) FROM t", params![])
        .await
        .unwrap();
    assert_eq!(row.try_get_i64(0).unwrap(), 1);

    let two = pool
        .fetch_optional("SELECT id FROM t WHERE id = 2", params![])
        .await
        .unwrap();
    assert!(two.is_none());
}

#[tokio::test]
async fn rolls_back_on_inner_db_error() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute(
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT UNIQUE)",
        params![],
    )
    .await
    .expect("create");
    pool.execute("INSERT INTO t(id, v) VALUES (1, 'one')", params![])
        .await
        .expect("seed");

    let result: Result<(), _> = pool
        .transaction(|conn| {
            conn.execute("INSERT INTO t(id, v) VALUES (2, 'two')", ())?;
            // Triggers a UNIQUE constraint violation.
            conn.execute("INSERT INTO t(id, v) VALUES (3, 'one')", ())?;
            Ok(())
        })
        .await;
    assert!(result.is_err());

    // After rollback only the seed row 1 should remain.
    let row = pool
        .fetch_one("SELECT COUNT(*) FROM t", params![])
        .await
        .unwrap();
    assert_eq!(row.try_get_i64(0).unwrap(), 1);
}
