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

    let db_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/jeryu/autonomy.redlineDB");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("create target/jeryu test dir");
    }
    let _ = fs::remove_file(&db_path);

    assert_file_backed_round_trip(&format!("redline://{}", db_path.display()), 1, "kill-bell")
        .await;
    assert_file_backed_round_trip(&format!("redlinedb://{}", db_path.display()), 2, "status").await;

    let alias_url = format!("redlineDB://{}", db_path.display());
    assert_file_backed_round_trip(&alias_url, 3, "validate").await;
}

async fn assert_file_backed_round_trip(url: &str, id: i64, kind: &str) {
    let pool = AnyPool::connect(url)
        .await
        .unwrap_or_else(|err| panic!("connect {url}: {err}"));

    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS ledger_events(
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap_or_else(|err| panic!("create ledger_events via {url}: {err}"));

    sqlx::query::query("INSERT INTO ledger_events(id, kind) VALUES (?, ?)")
        .bind(id)
        .bind(kind)
        .execute(&pool)
        .await
        .unwrap_or_else(|err| panic!("insert ledger row via {url}: {err}"));

    let row = sqlx::query::query("SELECT kind FROM ledger_events WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|err| panic!("select ledger row via {url}: {err}"));

    assert_eq!(row.try_get::<String, _>(0).unwrap(), kind);
}
