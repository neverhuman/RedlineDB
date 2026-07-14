use std::time::{Duration, Instant};

use redlinedb::{Database, ErrorCode};

#[test]
fn oversized_row_fails_promptly_without_growing_the_page_file() {
    let root = tempfile::tempdir().expect("database root");
    let path = root.path().join("oversized-row.redline");
    let database = Database::create(&path).expect("create database");
    let mut connection = database.connect().expect("connect");
    connection
        .execute(
            "CREATE TABLE payloads(id TEXT PRIMARY KEY, body TEXT NOT NULL)",
            (),
        )
        .expect("create table");
    let data_path = path.join("data.redline");
    let before = std::fs::metadata(&data_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let started = Instant::now();

    let error = connection
        .execute(
            "INSERT INTO payloads(id, body) VALUES(?1, ?2)",
            ("oversized", "x".repeat(64 * 1024)),
        )
        .expect_err("oversized row must fail");

    assert_eq!(error.code(), ErrorCode::TooBig);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "oversized row took {:?} to fail",
        started.elapsed()
    );
    let after = std::fs::metadata(&data_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    assert!(
        after <= before + 64 * 1024,
        "rejected row grew Redline from {before} to {after} bytes"
    );
}
