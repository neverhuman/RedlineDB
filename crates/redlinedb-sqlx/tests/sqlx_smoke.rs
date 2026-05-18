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

#[tokio::test]
async fn mode_rwc_accepts_existing_empty_file_path() {
    install_default_drivers();

    let tempfile = tempfile::NamedTempFile::new().expect("tempfile");
    let url = format!("redline:{}?mode=rwc", tempfile.path().display());

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect existing empty file path");

    sqlx::query::query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create");

    sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
        .bind(7_i64)
        .bind("Grace")
        .execute(&pool)
        .await
        .expect("insert");

    let row = sqlx::query::query("SELECT name FROM items WHERE id = ?")
        .bind(7_i64)
        .fetch_one(&pool)
        .await
        .expect("select");

    assert_eq!(row.try_get::<String, _>(0).unwrap(), "Grace");
}

#[tokio::test]
async fn parameterized_recursive_cte_uses_bound_values() {
    install_default_drivers();

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("redline:///:memory:")
        .await
        .expect("connect");

    sqlx::query::query(
        "CREATE TABLE cache_verdicts (
            id INTEGER PRIMARY KEY,
            object_hash TEXT NOT NULL,
            inputs_hash TEXT NOT NULL,
            verdict TEXT NOT NULL,
            tier TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("create");
    sqlx::query::query(
        "INSERT INTO cache_verdicts (object_hash, inputs_hash, verdict, tier) VALUES
            ('child-hash-1', 'root-hash', 'hit', 'untrusted'),
            ('grandchild-hash-1', 'child-hash-1', 'hit', 'untrusted')",
    )
    .execute(&pool)
    .await
    .expect("insert graph");

    let rows = sqlx::query::query(
        "WITH RECURSIVE taint_tree AS (
            SELECT object_hash FROM cache_verdicts WHERE inputs_hash = ?
            UNION
            SELECT cv.object_hash FROM cache_verdicts cv
            JOIN taint_tree tt ON cv.inputs_hash = tt.object_hash
        )
        SELECT object_hash FROM taint_tree ORDER BY object_hash",
    )
    .bind("root-hash")
    .fetch_all(&pool)
    .await
    .expect("recursive cte");

    let hashes = rows
        .into_iter()
        .map(|row| row.try_get::<String, _>(0).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(hashes, vec!["child-hash-1", "grandchild-hash-1"]);
}
