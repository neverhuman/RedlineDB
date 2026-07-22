use std::time::Duration;

use redlinedb::{BeginMode, Database, ErrorCode, TransactionIsolationLevel, TransactionOptions};

fn read_i64(conn: &mut redlinedb::Connection, sql: &str) -> i64 {
    conn.query_row(sql, ()).expect("read scalar")
}

fn begin_at(
    conn: &mut redlinedb::Connection,
    isolation: TransactionIsolationLevel,
) -> redlinedb::Result<()> {
    conn.begin_with_options(
        TransactionOptions::default()
            .with_mode(BeginMode::Deferred)
            .with_isolation(isolation),
    )
}

#[test]
fn read_committed_refreshes_visibility_at_each_statement() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("read-committed.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::ReadCommitted).expect("begin reader");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    writer
        .execute("UPDATE t SET value = 20 WHERE id = 1", ())
        .expect("committed update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        20
    );
    reader.commit().expect("commit reader");
}

#[test]
fn repeatable_read_preserves_the_existing_snapshot_behavior() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("repeatable-read.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::RepeatableRead).expect("begin reader");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    writer
        .execute("UPDATE t SET value = 20 WHERE id = 1", ())
        .expect("committed update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    reader.commit().expect("commit reader");
}

#[test]
fn repeatable_read_prevents_dirty_reads_and_phantoms() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("snapshot-schedules.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::RepeatableRead).expect("begin reader");
    assert_eq!(read_i64(&mut reader, "SELECT COUNT(*) FROM t"), 1);

    begin_at(&mut writer, TransactionIsolationLevel::RepeatableRead).expect("begin writer");
    writer
        .execute("UPDATE t SET value = 99 WHERE id = 1", ())
        .expect("uncommitted update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    writer.rollback().expect("rollback dirty write");

    writer
        .execute("INSERT INTO t VALUES (2, 20)", ())
        .expect("committed insert");
    assert_eq!(read_i64(&mut reader, "SELECT COUNT(*) FROM t"), 1);
    reader.commit().expect("commit reader");
}

#[test]
fn same_row_writers_conflict_instead_of_losing_an_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("same-row.redline")).expect("database");
    let mut first = db.connect().expect("first");
    let mut second = db.connect().expect("second");
    first.set_busy_timeout(Duration::from_millis(10));
    second.set_busy_timeout(Duration::from_millis(10));
    first
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    first
        .execute("INSERT INTO t VALUES (1, 0)", ())
        .expect("seed");

    begin_at(&mut first, TransactionIsolationLevel::RepeatableRead).expect("first begin");
    begin_at(&mut second, TransactionIsolationLevel::RepeatableRead).expect("second begin");
    first
        .execute("UPDATE t SET value = value + 1 WHERE id = 1", ())
        .expect("first update");
    let error = second
        .execute("UPDATE t SET value = value + 1 WHERE id = 1", ())
        .expect_err("second writer must conflict");
    assert!(matches!(error.code(), ErrorCode::Busy | ErrorCode::Locked));
    second.rollback().expect("second rollback");
    first.commit().expect("first commit");

    let mut check = db.connect().expect("check");
    assert_eq!(read_i64(&mut check, "SELECT value FROM t WHERE id = 1"), 1);
}

#[test]
fn serializable_is_rejected_before_a_transaction_opens_or_a_label_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("serializable.redline")).expect("database");
    let mut conn = db.connect().expect("connection");

    let error = begin_at(&mut conn, TransactionIsolationLevel::Serializable)
        .expect_err("SSI is not implemented");
    assert_eq!(error.message(), "unsupported isolation level");
    assert!(!conn.in_transaction());

    conn.execute("BEGIN", ()).expect("begin SQL transaction");
    let error = conn
        .execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE", ())
        .expect_err("SQL Serializable must fail");
    assert_eq!(error.message(), "unsupported isolation level");
    let shown: String = conn
        .query_row("SHOW transaction_isolation", ())
        .expect("show actual isolation");
    assert_eq!(shown, "repeatable read");
    conn.rollback().expect("rollback");
}

#[test]
fn set_transaction_changes_the_active_kernel_behavior() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("sql-set-isolation.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    reader.execute("BEGIN", ()).expect("begin");
    reader
        .execute("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", ())
        .expect("set read committed");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    writer
        .execute("UPDATE t SET value = 20 WHERE id = 1", ())
        .expect("update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        20
    );
    let shown: String = reader
        .query_row("SHOW transaction_isolation", ())
        .expect("show actual isolation");
    assert_eq!(shown, "read committed");
    reader.commit().expect("commit");
}

#[test]
fn access_modes_leave_the_active_isolation_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("access-mode-isolation.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::RepeatableRead).expect("begin reader");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    reader
        .execute("SET TRANSACTION READ ONLY", ())
        .expect("accept read-only mode");
    reader
        .execute("SET TRANSACTION READ WRITE", ())
        .expect("accept read-write mode");
    writer
        .execute("UPDATE t SET value = 20 WHERE id = 1", ())
        .expect("committed update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    let shown: String = reader
        .query_row("SHOW transaction_isolation", ())
        .expect("show unchanged isolation");
    assert_eq!(shown, "repeatable read");
    reader.commit().expect("commit reader");
}

#[test]
fn snapshot_sql_maps_to_repeatable_read_behavior() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("snapshot-alias.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    reader.execute("BEGIN", ()).expect("begin reader");
    reader
        .execute("SET TRANSACTION ISOLATION LEVEL SNAPSHOT", ())
        .expect("snapshot alias");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    writer
        .execute("UPDATE t SET value = 20 WHERE id = 1", ())
        .expect("committed update");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM t WHERE id = 1"),
        10
    );
    let shown: String = reader
        .query_row("SHOW transaction_isolation", ())
        .expect("show snapshot mapping");
    assert_eq!(shown, "repeatable read");
    reader.commit().expect("commit reader");
}

#[test]
fn set_transaction_outside_a_transaction_is_accepted_without_changing_the_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("set-before-begin.redline")).expect("database");
    let mut conn = db.connect().expect("connection");

    conn.execute("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", ())
        .expect("outside-transaction SET is accepted as a no-op");
    conn.execute("BEGIN", ())
        .expect("begin with unchanged default");
    let shown: String = conn
        .query_row("SHOW transaction_isolation", ())
        .expect("show transaction isolation");
    assert_eq!(shown, "repeatable read");
    conn.rollback().expect("rollback");
}

#[test]
fn session_characteristics_select_the_next_transaction_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db =
        Database::create(dir.path().join("session-characteristics.redline")).expect("database");
    let mut conn = db.connect().expect("connection");

    conn.execute(
        "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL READ COMMITTED",
        (),
    )
    .expect("set session default");
    conn.execute("BEGIN", ())
        .expect("begin with session default");
    let shown: String = conn
        .query_row("SHOW transaction_isolation", ())
        .expect("show session default");
    assert_eq!(shown, "read committed");
    conn.rollback().expect("rollback");

    conn.execute("BEGIN", ()).expect("begin at read committed");
    conn.execute(
        "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL REPEATABLE READ",
        (),
    )
    .expect("change only the later default");
    let shown: String = conn
        .query_row("SHOW transaction_isolation", ())
        .expect("show active transaction isolation");
    assert_eq!(shown, "read committed");
    conn.rollback().expect("rollback current transaction");

    conn.execute("BEGIN", ())
        .expect("begin with updated default");
    let shown: String = conn
        .query_row("SHOW transaction_isolation", ())
        .expect("show updated session default");
    assert_eq!(shown, "repeatable read");
    conn.rollback().expect("rollback");
}

#[test]
fn repeatable_read_prevents_cross_row_read_skew() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("read-skew.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute(
            "CREATE TABLE balances(id INTEGER PRIMARY KEY, value INTEGER)",
            (),
        )
        .expect("create");
    writer
        .execute("INSERT INTO balances VALUES (1, 50), (2, 50)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::RepeatableRead).expect("reader begin");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM balances WHERE id=1"),
        50
    );
    begin_at(&mut writer, TransactionIsolationLevel::RepeatableRead).expect("writer begin");
    writer
        .execute("UPDATE balances SET value=40 WHERE id=1", ())
        .expect("debit");
    writer
        .execute("UPDATE balances SET value=60 WHERE id=2", ())
        .expect("credit");
    writer.commit().expect("atomic writer commit");
    assert_eq!(
        read_i64(&mut reader, "SELECT value FROM balances WHERE id=2"),
        50
    );
    reader.commit().expect("reader commit");
}

#[test]
fn savepoint_rollback_preserves_read_committed_statement_refresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("savepoint-isolation.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
        .expect("create");
    writer
        .execute("INSERT INTO t VALUES (1, 10)", ())
        .expect("seed");

    begin_at(&mut reader, TransactionIsolationLevel::ReadCommitted).expect("begin reader");
    reader
        .execute("SAVEPOINT before_local", ())
        .expect("savepoint");
    reader
        .execute("UPDATE t SET value=11 WHERE id=1", ())
        .expect("local update");
    reader
        .execute("ROLLBACK TO before_local", ())
        .expect("rollback to savepoint");
    reader.execute("RELEASE before_local", ()).expect("release");
    reader.commit().expect("release writer ownership");

    writer
        .execute("UPDATE t SET value=20 WHERE id=1", ())
        .expect("committed writer update");
    begin_at(&mut reader, TransactionIsolationLevel::ReadCommitted).expect("restart reader");
    assert_eq!(read_i64(&mut reader, "SELECT value FROM t WHERE id=1"), 20);
    reader.commit().expect("commit reader");
}

#[test]
fn ddl_catalog_visibility_is_explicitly_current_not_snapshot_versioned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("ddl-visibility.redline")).expect("database");
    let mut reader = db.connect().expect("reader");
    let mut writer = db.connect().expect("writer");
    writer
        .execute("CREATE TABLE original(id INTEGER)", ())
        .expect("create original");

    begin_at(&mut reader, TransactionIsolationLevel::RepeatableRead).expect("reader begin");
    assert_eq!(
        read_i64(
            &mut reader,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'"
        ),
        1
    );
    writer
        .execute("CREATE TABLE later(id INTEGER)", ())
        .expect("create later");
    // Row heaps are MVCC-snapshotted, but catalog definitions are not yet
    // versioned. Keep this boundary executable instead of silently claiming
    // transactional-DDL snapshot semantics.
    assert_eq!(
        read_i64(
            &mut reader,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'"
        ),
        2
    );
    reader.commit().expect("reader commit");
    assert_eq!(
        read_i64(
            &mut reader,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'"
        ),
        2
    );
}

#[test]
fn committed_state_survives_reopen_and_uncommitted_state_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("recovery-boundary.redline");
    {
        let db = Database::create(&path).expect("create database");
        let mut committed = db.connect().expect("committed connection");
        committed
            .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, value INTEGER)", ())
            .expect("create");
        committed
            .execute("INSERT INTO t VALUES (1, 10)", ())
            .expect("commit row");
        let mut aborted = db.connect().expect("aborted connection");
        begin_at(&mut aborted, TransactionIsolationLevel::RepeatableRead).expect("begin aborted");
        aborted
            .execute("INSERT INTO t VALUES (2, 20)", ())
            .expect("uncommitted row");
        aborted.rollback().expect("rollback");
    }

    let reopened = Database::open(&path).expect("reopen");
    let mut connection = reopened.connect().expect("connection");
    assert_eq!(read_i64(&mut connection, "SELECT COUNT(*) FROM t"), 1);
    assert_eq!(
        read_i64(&mut connection, "SELECT value FROM t WHERE id=1"),
        10
    );
}
