use std::time::SystemTime;

use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPoolOptions, row::Row};

#[tokio::test]
async fn datetime_text_round_trips_through_sqlx() {
    install_default_drivers();

    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("datetime.db");
    let url = format!("redline://{}", db_path.display());

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");

    sqlx::query::query("CREATE TABLE events(id INTEGER PRIMARY KEY, ts TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create");

    let ts_text = "2024-05-18 12:34:56.123456+00:00";
    sqlx::query::query("INSERT INTO events(id, ts) VALUES (?, ?)")
        .bind(1_i64)
        .bind(ts_text)
        .execute(&pool)
        .await
        .expect("insert");

    let row = sqlx::query::query("SELECT ts FROM events WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("select");

    let fetched: String = row.try_get(0).expect("text");
    assert_eq!(fetched, ts_text);

    let expected = SystemTime::try_from(&redlinedb::Value::from(ts_text)).expect("expected");
    let parsed = SystemTime::try_from(&redlinedb::Value::from(fetched)).expect("parsed");
    assert_eq!(parsed, expected);
}
