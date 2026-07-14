use std::path::Path;
use std::time::{Duration, Instant};

use redlinedb::{Database, ErrorCode};
use redlinedb_kernel::format::{DEFAULT_PAGE_SIZE, PAGE_HEADER_LEN, SLOT_LEN, TUPLE_HEADER_LEN};

// A fresh one-column user table has TableId 10_000. Its v1 SQL record stores the format byte,
// column count, two serial types, the two-byte table id, and a three-byte text-length varint.
const SINGLE_TEXT_SQL_RECORD_OVERHEAD: usize = 8;
const MAX_PUBLIC_TEXT_BYTES: usize = DEFAULT_PAGE_SIZE
    - PAGE_HEADER_LEN
    - SLOT_LEN
    - TUPLE_HEADER_LEN
    - SINGLE_TEXT_SQL_RECORD_OVERHEAD;
const OVERSIZED_TEXT_BYTES: usize = 64 * 1024;
const REJECTIONS_PER_MUTATION: usize = 128;

fn data_file_len(path: &Path) -> u64 {
    std::fs::metadata(path.join("data.redline"))
        .expect("data.redline metadata")
        .len()
}

#[test]
fn public_text_row_boundary_accepts_max_minus_one_and_max_but_rejects_max_plus_one() {
    for (body_len, should_fit) in [
        (MAX_PUBLIC_TEXT_BYTES - 1, true),
        (MAX_PUBLIC_TEXT_BYTES, true),
        (MAX_PUBLIC_TEXT_BYTES + 1, false),
    ] {
        let root = tempfile::tempdir().expect("database root");
        let path = root.path().join(format!("boundary-{body_len}.redline"));
        let database = Database::create(&path).expect("create database");
        let mut connection = database.connect().expect("connect");
        connection
            .execute("CREATE TABLE payloads(body TEXT NOT NULL)", ())
            .expect("create table");

        let result = connection.execute(
            "INSERT INTO payloads(body) VALUES(?1)",
            ("x".repeat(body_len),),
        );
        if should_fit {
            result.expect("max-1 and max public rows must fit");
            let stored_len: i64 = connection
                .query_row("SELECT length(body) FROM payloads", ())
                .expect("read accepted body length");
            assert_eq!(stored_len as usize, body_len);
        } else {
            let error = result.expect_err("max+1 public row must fail");
            assert_eq!(error.code(), ErrorCode::TooBig);
        }
    }
}

#[test]
fn repeated_oversized_inserts_and_updates_leave_storage_and_wal_unchanged_after_reopen() {
    let root = tempfile::tempdir().expect("database root");
    let path = root.path().join("oversized-mutations.redline");
    let oversized = "x".repeat(OVERSIZED_TEXT_BYTES);

    {
        let database = Database::create(&path).expect("create database");
        let mut connection = database.connect().expect("connect");
        connection
            .execute(
                "CREATE TABLE payloads(id INTEGER PRIMARY KEY, body TEXT NOT NULL)",
                (),
            )
            .expect("create table");
        connection
            .execute("INSERT INTO payloads VALUES(1, 'small')", ())
            .expect("seed row");

        let before_checkpoint = database.checkpoint().expect("checkpoint before rejection");
        let before = database.benchmark_stats().expect("stats before rejection");
        let before_file_len = data_file_len(&path);
        let started = Instant::now();

        for _ in 0..REJECTIONS_PER_MUTATION {
            let insert_error = connection
                .execute(
                    "INSERT INTO payloads(id, body) VALUES(2, ?1)",
                    (oversized.as_str(),),
                )
                .expect_err("oversized insert must fail");
            assert_eq!(insert_error.code(), ErrorCode::TooBig);

            let update_error = connection
                .execute(
                    "UPDATE payloads SET body = ?1 WHERE id = 1",
                    (oversized.as_str(),),
                )
                .expect_err("oversized update must fail");
            assert_eq!(update_error.code(), ErrorCode::TooBig);
        }

        assert!(
            started.elapsed() < Duration::from_secs(5),
            "oversized mutation rejection took {:?}",
            started.elapsed()
        );
        let after_checkpoint = database.checkpoint().expect("checkpoint after rejection");
        let after = database.benchmark_stats().expect("stats after rejection");
        let after_file_len = data_file_len(&path);

        assert_eq!(after_checkpoint.page_count, before_checkpoint.page_count);
        assert_eq!(after.buffer.resident_pages, before.buffer.resident_pages);
        assert_eq!(after_file_len, before_file_len);
        assert_eq!(after.wal.written_lsn, before.wal.written_lsn);
        assert_eq!(after.wal.durable_lsn, before.wal.durable_lsn);
        let body: String = connection
            .query_row("SELECT body FROM payloads WHERE id = 1", ())
            .expect("seed row remains readable");
        assert_eq!(body, "small");
    }

    let reopened = Database::open(&path).expect("reopen database");
    let mut connection = reopened.connect().expect("connect after reopen");
    let body: String = connection
        .query_row("SELECT body FROM payloads WHERE id = 1", ())
        .expect("read seed row after reopen");
    assert_eq!(body, "small");
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", ())
        .expect("run integrity check after reopen");
    assert_eq!(integrity, "ok");
}
