use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPool, row::Row};

#[tokio::test]
async fn repeated_install_is_harmless() {
    install_default_drivers();
    install_default_drivers();

    let pool = AnyPool::connect("redline:///:memory:")
        .await
        .expect("connect");

    let row = sqlx::query::query("SELECT 1 AS n")
        .fetch_one(&pool)
        .await
        .expect("query");

    assert_eq!(row.try_get::<i64, _>("n").unwrap(), 1);
}
