use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPoolOptions, row::Row};

#[tokio::test]
async fn creates_inserts_and_reads_rows() {
    install_default_drivers();

    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("smoke.db");
    let url = format!("redline://{}", db_path.display());

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");

    sqlx::query::query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create");

    sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
        .bind(42_i64)
        .bind("Ada")
        .execute(&pool)
        .await
        .expect("insert");

    let row = sqlx::query::query("SELECT id, name FROM items WHERE id = ?")
        .bind(42_i64)
        .fetch_one(&pool)
        .await
        .expect("select");

    assert_eq!(row.try_get::<i64, _>(0).unwrap(), 42);
    assert_eq!(row.try_get::<String, _>(1).unwrap(), "Ada");
}
