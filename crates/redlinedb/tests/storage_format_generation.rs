use std::fs;

use redlinedb::{BackupOptions, Database, ErrorCode, OpenOptions, Step};

const STORAGE_FORMAT_FILE: &str = "STORAGE_FORMAT";
const GENERATION_ONE: &str = "redlinedb-storage-format/v1\ngeneration=1\n";

fn scalar_i64(db: &Database, sql: &str) -> i64 {
    let mut connection = db.connect().expect("connect");
    let mut statement = connection.prepare(sql).expect("prepare");
    match statement.step().expect("step") {
        Step::Row(row) => row.get::<i64>(0).expect("integer result"),
        Step::Done => panic!("query returned no row"),
    }
}

#[test]
fn create_persists_generation_and_reopen_enforces_it() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("generation.redline");
    let database = Database::create(&path).expect("create");
    database
        .connect()
        .expect("connect")
        .execute_batch("CREATE TABLE t(value INT); INSERT INTO t VALUES (41);")
        .expect("seed");
    drop(database);

    assert_eq!(
        fs::read_to_string(path.join(STORAGE_FORMAT_FILE)).expect("storage format marker"),
        GENERATION_ONE
    );
    let reopened = Database::open_with_options(&path, OpenOptions::default().with_create(false))
        .expect("reopen");
    assert_eq!(scalar_i64(&reopened, "SELECT value + 1 FROM t"), 42);
}

#[test]
fn legacy_generation_one_is_read_and_migrated_without_data_loss() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("legacy.redline");
    let database = Database::create(&path).expect("create");
    database
        .connect()
        .expect("connect")
        .execute_batch("CREATE TABLE t(value INT); INSERT INTO t VALUES (7);")
        .expect("seed");
    drop(database);
    fs::remove_file(path.join(STORAGE_FORMAT_FILE)).expect("simulate exact 4.1 image");

    let read_only = Database::open_with_options(
        &path,
        OpenOptions::default()
            .with_create(false)
            .with_read_only(true),
    )
    .expect("read legacy image");
    assert_eq!(scalar_i64(&read_only, "SELECT value FROM t"), 7);
    drop(read_only);
    assert!(!path.join(STORAGE_FORMAT_FILE).exists());

    let migrated = Database::open_with_options(&path, OpenOptions::default().with_create(false))
        .expect("migrate legacy image");
    assert_eq!(scalar_i64(&migrated, "SELECT value FROM t"), 7);
    assert_eq!(
        fs::read_to_string(path.join(STORAGE_FORMAT_FILE)).expect("migrated marker"),
        GENERATION_ONE
    );
}

#[test]
fn future_and_malformed_generations_fail_before_open() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("future.redline");
    drop(Database::create(&path).expect("create"));

    fs::write(
        path.join(STORAGE_FORMAT_FILE),
        "redlinedb-storage-format/v1\ngeneration=2\n",
    )
    .expect("future marker");
    let future = Database::open_with_options(&path, OpenOptions::default().with_create(false))
        .err()
        .expect("future generation must fail closed");
    assert_eq!(future.code(), ErrorCode::Unsupported);

    fs::write(path.join(STORAGE_FORMAT_FILE), "generation=1\n").expect("malformed marker");
    let malformed = Database::open_with_options(&path, OpenOptions::default().with_create(false))
        .err()
        .expect("malformed generation must fail closed");
    assert_eq!(malformed.code(), ErrorCode::Corrupt);
}

#[test]
fn public_backup_rejects_invalid_live_generation_without_destination_mutation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("live.redline");
    let destination = temp.path().join("backup.redline");
    let database = Database::create(&path).expect("create");
    database
        .connect()
        .expect("connect")
        .execute_batch("CREATE TABLE t(value INT); INSERT INTO t VALUES (11);")
        .expect("seed");
    fs::create_dir(&destination).expect("destination");
    fs::write(destination.join("sentinel"), "preserve\n").expect("destination sentinel");

    for (marker, expected_code) in [
        (
            "redlinedb-storage-format/v1\ngeneration=2\n",
            ErrorCode::Unsupported,
        ),
        ("generation=1\n", ErrorCode::Corrupt),
    ] {
        fs::write(path.join(STORAGE_FORMAT_FILE), marker).expect("replace live marker");
        let error = database
            .backup_to_path(&destination, BackupOptions::default())
            .expect_err("invalid live storage generation must fail before backup");
        assert_eq!(error.code(), expected_code);
        assert_eq!(
            fs::read_to_string(destination.join("sentinel")).expect("preserved sentinel"),
            "preserve\n"
        );
        assert_eq!(
            fs::read_dir(&destination)
                .expect("destination listing")
                .count(),
            1,
            "failed backup mutated the destination"
        );
    }
}

#[test]
fn public_backup_remains_a_reopenable_physical_copy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("source.redline");
    let destination = temp.path().join("backup.redline");
    let database = Database::create(&path).expect("create");
    database
        .connect()
        .expect("connect")
        .execute_batch("CREATE TABLE t(value INT); INSERT INTO t VALUES (29);")
        .expect("seed");

    database
        .backup_to_path(&destination, BackupOptions::default())
        .expect("physical backup");
    drop(database);

    let backup =
        Database::open_with_options(&destination, OpenOptions::default().with_create(false))
            .expect("reopen physical backup");
    assert_eq!(scalar_i64(&backup, "SELECT value FROM t"), 29);
}
