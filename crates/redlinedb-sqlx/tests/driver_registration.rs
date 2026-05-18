use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPool, row::Row};
use std::{fs, path::PathBuf};

async fn assert_select_one(url: &str) {
    let pool = AnyPool::connect(url)
        .await
        .unwrap_or_else(|err| panic!("connect {url}: {err}"));

    let row = sqlx::query::query("SELECT 1 AS n")
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|err| panic!("query {url}: {err}"));

    assert_eq!(row.try_get::<i64, _>("n").unwrap(), 1);
}

#[tokio::test]
async fn repeated_install_is_harmless() {
    install_default_drivers();
    install_default_drivers();

    assert_select_one("redline:///:memory:").await;
}

#[tokio::test]
async fn redline_memory_url_connects() {
    install_default_drivers();

    assert_select_one("redline:///:memory:").await;
}

#[tokio::test]
async fn redlinedb_memory_url_connects() {
    install_default_drivers();

    assert_select_one("redlinedb:///:memory:").await;
}

#[tokio::test]
async fn mixed_case_redlinedb_scheme_is_normalized() {
    install_default_drivers();

    assert_select_one("redlineDB:///:memory:").await;
}

#[tokio::test]
async fn file_backed_jeryu_autonomy_ledger_url_connects() {
    install_default_drivers();

    let target_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/jeryu/redlinedb-sqlx");
    fs::create_dir_all(&target_dir).expect("create target/jeryu test dir");
    let temp_dir = tempfile::Builder::new()
        .prefix("driver-registration-")
        .tempdir_in(&target_dir)
        .expect("temp dir in target/jeryu");
    let db_path = temp_dir.path().join("autonomy.redlineDB");
    let url = format!("redline://{}", db_path.display());

    let pool = AnyPool::connect(&url).await.expect("connect file ledger");
    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS ledger_events(id INTEGER PRIMARY KEY, kind TEXT)",
    )
    .execute(&pool)
    .await
    .expect("create ledger table");
    sqlx::query::query("INSERT INTO ledger_events(kind) VALUES ('kill-bell')")
        .execute(&pool)
        .await
        .expect("insert ledger event");
    let row = sqlx::query::query("SELECT count(*) AS n FROM ledger_events")
        .fetch_one(&pool)
        .await
        .expect("read ledger table");
    assert_eq!(row.try_get::<i64, _>("n").unwrap(), 1);
    pool.close().await;

    let alias_url = format!("redlineDB://{}", db_path.display());
    assert_select_one(&alias_url).await;
}
