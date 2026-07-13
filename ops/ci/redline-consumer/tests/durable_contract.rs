use redlinedb::Database;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "jain-redline-consumer-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated consumer-test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn jain_model_and_release_state_survives_transaction_checkpoint_and_reopen() {
    let directory = TestDirectory::new();
    let path = directory.path().join("jain-consumer.redline");

    {
        let database = Database::create(&path).expect("create Redline database");
        let mut connection = database.connect().expect("connect to Redline database");

        connection
            .execute(
                "CREATE TABLE model_runs(\
                    id TEXT PRIMARY KEY,\
                    tenant TEXT NOT NULL,\
                    status TEXT NOT NULL,\
                    version INTEGER NOT NULL\
                )",
                (),
            )
            .expect("create Jain model-run table");
        connection
            .execute(
                "CREATE TABLE release_receipts(\
                    id TEXT PRIMARY KEY,\
                    tenant TEXT NOT NULL,\
                    subject TEXT NOT NULL,\
                    payload TEXT NOT NULL\
                )",
                (),
            )
            .expect("create Jain release-receipt table");
        connection
            .execute(
                "INSERT INTO model_runs(id, tenant, status, version) VALUES (?, ?, ?, ?)",
                ("JAIN-800", "tenant-a", "ready", 1_i64),
            )
            .expect("insert model run");

        connection
            .execute("BEGIN IMMEDIATE", ())
            .expect("begin durable release update");
        connection
            .execute(
                "UPDATE model_runs SET status = ?, version = ? WHERE id = ? AND tenant = ?",
                ("released", 2_i64, "JAIN-800", "tenant-a"),
            )
            .expect("update model run");
        connection
            .execute(
                "INSERT INTO release_receipts(id, tenant, subject, payload) VALUES (?, ?, ?, ?)",
                (
                    "receipt-800",
                    "tenant-a",
                    "JAIN-800",
                    "{\"status\":\"released\",\"version\":2}",
                ),
            )
            .expect("insert release receipt");
        connection
            .execute("COMMIT", ())
            .expect("commit release update");

        connection
            .execute("BEGIN IMMEDIATE", ())
            .expect("begin rollback probe");
        connection
            .execute(
                "UPDATE model_runs SET status = ? WHERE id = ?",
                ("corrupted", "JAIN-800"),
            )
            .expect("stage rollback probe");
        connection.execute("ROLLBACK", ()).expect("rollback probe");

        let status: String = connection
            .query_row(
                "SELECT status FROM model_runs WHERE id = ? AND tenant = ?",
                ("JAIN-800", "tenant-a"),
            )
            .expect("read committed status");
        assert_eq!(status, "released");

        database
            .checkpoint()
            .expect("checkpoint durable Jain consumer state");
    }

    let reopened = Database::open(&path).expect("reopen Redline database");
    let mut connection = reopened.connect().expect("connect after reopen");
    let version: i64 = connection
        .query_row(
            "SELECT version FROM model_runs WHERE id = ? AND tenant = ?",
            ("JAIN-800", "tenant-a"),
        )
        .expect("read durable model run");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM release_receipts WHERE subject = ? AND tenant = ?",
            ("JAIN-800", "tenant-a"),
        )
        .expect("read durable release receipt");

    assert_eq!(version, 2);
    assert_eq!(receipt_count, 1);
}
