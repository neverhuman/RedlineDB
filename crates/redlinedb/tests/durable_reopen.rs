use redlinedb::Database;

#[test]
fn committed_autocommit_row_survives_checkpoint_and_reopen() {
    let root = tempfile::tempdir().expect("database root");
    let path = root.path().join("autocommit.redline");

    {
        let database = Database::create(&path).expect("create database");
        let mut connection = database.connect().expect("connect");
        connection
            .execute(
                "CREATE TABLE work_items(id TEXT PRIMARY KEY, version INTEGER)",
                (),
            )
            .expect("create table");
        connection
            .execute("INSERT INTO work_items VALUES (?, ?)", ("JRY-800", 1_i64))
            .expect("insert row");
        let version: i64 = connection
            .query_row("SELECT version FROM work_items WHERE id = ?", ("JRY-800",))
            .expect("read row before reopen");
        assert_eq!(version, 1);
        database.checkpoint().expect("checkpoint");
    }

    let reopened = Database::open(&path).expect("reopen database");
    let mut connection = reopened.connect().expect("connect after reopen");
    let full_scan_version: i64 = connection
        .query_row("SELECT version FROM work_items", ())
        .expect("read durable row with a full scan");
    assert_eq!(full_scan_version, 1);
    let version: i64 = connection
        .query_row("SELECT version FROM work_items WHERE id = ?", ("JRY-800",))
        .expect("read durable row");
    assert_eq!(version, 1);
}

#[test]
fn committed_transaction_survives_rollback_checkpoint_and_reopen() {
    let root = tempfile::tempdir().expect("database root");
    let path = root.path().join("transaction.redline");

    {
        let database = Database::create(&path).expect("create database");
        let mut connection = database.connect().expect("connect");
        connection
            .execute(
                "CREATE TABLE work_items(id TEXT PRIMARY KEY, version INTEGER)",
                (),
            )
            .expect("create table");
        connection.execute("BEGIN IMMEDIATE", ()).expect("begin");
        connection
            .execute("INSERT INTO work_items VALUES (?, ?)", ("JRY-800", 2_i64))
            .expect("insert row");
        connection.execute("COMMIT", ()).expect("commit");
        connection
            .execute("BEGIN IMMEDIATE", ())
            .expect("begin rollback probe");
        connection
            .execute(
                "UPDATE work_items SET version = 3 WHERE id = ?",
                ("JRY-800",),
            )
            .expect("update row");
        connection.execute("ROLLBACK", ()).expect("rollback");
        let version: i64 = connection
            .query_row("SELECT version FROM work_items WHERE id = ?", ("JRY-800",))
            .expect("read committed row before reopen");
        assert_eq!(version, 2);
        database.checkpoint().expect("checkpoint");
    }

    let reopened = Database::open(&path).expect("reopen database");
    let mut connection = reopened.connect().expect("connect after reopen");
    let full_scan_version: i64 = connection
        .query_row("SELECT version FROM work_items", ())
        .expect("read durable row with a full scan");
    assert_eq!(full_scan_version, 2);
    let version: i64 = connection
        .query_row("SELECT version FROM work_items WHERE id = ?", ("JRY-800",))
        .expect("read durable row");
    assert_eq!(version, 2);
}
