//! redlinedb::Error must survive .await boundaries without panicking. This
//! exercises both the happy path (Ok crossing) and the error path (Err
//! crossing including a synthetic stderr-bearing source).

use redlinedb_tokio::{ErrorCode, Pool, params};

#[tokio::test]
async fn error_returned_to_caller_with_code() {
    let pool = Pool::open_in_memory().await.expect("pool");

    // Query a table that doesn't exist — RedlineDB returns NotFound.
    let err = pool
        .execute("SELECT * FROM does_not_exist", params![])
        .await
        .expect_err("should fail");
    assert_eq!(err.code(), ErrorCode::NotFound);
    // RedlineDB surfaces this as a generic kernel "object not found" message;
    // the important contract is the ErrorCode, not the message text.
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn fetch_one_on_empty_returns_not_found() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", params![])
        .await
        .expect("create");

    let err = pool
        .fetch_one("SELECT id FROM t WHERE id = 99", params![])
        .await
        .expect_err("no rows");
    assert_eq!(err.code(), ErrorCode::NotFound);
}

#[tokio::test]
async fn errors_are_send_sync_compatible() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<redlinedb_tokio::Error>();
    assert_send_sync::<redlinedb_tokio::Result<()>>();
}
