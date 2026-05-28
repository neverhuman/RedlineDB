mod common;

use common::open_database;
use redlinedb_sql::{Connection, Database, DbOptions, Error, SqlValue, Step};
use std::sync::Arc;

#[test]
fn autoincrement_integer_primary_key_keeps_rowid_alias_semantics() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
        .expect("create table");
    conn.execute("INSERT INTO t(name) VALUES ('one')")
        .expect("insert omitted rowid");
    let first_id = scalar_i64(&conn, "SELECT id FROM t WHERE name = 'one'");
    assert!(first_id > 0);
    assert_eq!(conn.last_insert_rowid(), Some(first_id));
    assert_eq!(scalar_i64(&conn, "SELECT last_insert_rowid()"), first_id);

    conn.execute("INSERT INTO t(id, name) VALUES (42, 'explicit')")
        .expect("insert explicit rowid");
    assert_eq!(conn.last_insert_rowid(), Some(42));
    assert_eq!(scalar_i64(&conn, "SELECT last_insert_rowid()"), 42);

    conn.execute("INSERT INTO t(name) VALUES ('two')")
        .expect("insert second omitted rowid");
    let second_id = scalar_i64(&conn, "SELECT id FROM t WHERE name = 'two'");
    assert!(second_id > first_id);
    assert_eq!(conn.last_insert_rowid(), Some(second_id));
}

#[test]
fn autoincrement_populates_sqlite_sequence_and_keeps_it_monotonic() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
        .expect("create table");
    assert_eq!(scalar_i64(&conn, "SELECT count(*) FROM sqlite_sequence"), 0);

    conn.execute("INSERT INTO t(name) VALUES ('one')")
        .expect("insert first row");
    assert_eq!(scalar_text(&conn, "SELECT name FROM sqlite_sequence"), "t");
    let first_seq = scalar_i64(&conn, "SELECT seq FROM sqlite_sequence");
    assert!(first_seq > 0);

    conn.execute("DELETE FROM t").expect("delete all rows");
    conn.execute("INSERT INTO t(name) VALUES ('two')")
        .expect("insert second row");
    let second_seq = scalar_i64(&conn, "SELECT seq FROM sqlite_sequence");
    assert!(second_seq > first_seq, "sequence must not rewind after delete");

    conn.execute("INSERT INTO t(id, name) VALUES (42, 'explicit')")
        .expect("explicit rowid bumps sequence");
    assert_eq!(scalar_i64(&conn, "SELECT seq FROM sqlite_sequence"), 42);
}

#[test]
fn autoincrement_requires_integer_primary_key_column() {
    let (_dir, conn) = open_database();

    let err = conn
        .execute("CREATE TABLE not_integer(id TEXT PRIMARY KEY AUTOINCREMENT)")
        .expect_err("non-integer autoincrement must fail");
    assert_sqlite_autoincrement_error(err);

    let err = conn
        .execute("CREATE TABLE not_primary(id INTEGER AUTOINCREMENT)")
        .expect_err("non-primary-key autoincrement must fail");
    assert_sqlite_autoincrement_error(err);
}

#[test]
fn check_in_list_accepts_valid_values_and_rejects_invalid_values() {
    let (_dir, conn) = open_database();

    conn.execute(
        "CREATE TABLE job_state(
            id INTEGER PRIMARY KEY,
            state TEXT NOT NULL CHECK(state IN ('armed', 'paused'))
        )",
    )
    .expect("create table");
    conn.execute("INSERT INTO job_state(id, state) VALUES (1, 'armed'), (2, 'paused')")
        .expect("insert valid states");

    let err = conn
        .execute("INSERT INTO job_state(id, state) VALUES (3, 'done')")
        .expect_err("invalid state must fail");
    assert!(
        matches!(err, Error::ConstraintViolation(_)),
        "expected CHECK constraint violation, got {err:?}"
    );
}

#[test]
fn before_update_and_delete_raise_abort_blocks_append_only_mutations() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE events(id INTEGER PRIMARY KEY, payload TEXT NOT NULL)")
        .expect("create events");
    conn.execute("INSERT INTO events(id, payload) VALUES (1, 'original')")
        .expect("seed events");
    conn.execute(
        "CREATE TRIGGER events_no_update
         BEFORE UPDATE ON events
         FOR EACH ROW
         BEGIN
             SELECT RAISE(ABORT, 'events are append-only');
         END",
    )
    .expect("create update trigger");
    conn.execute(
        "CREATE TRIGGER events_no_delete
         BEFORE DELETE ON events
         FOR EACH ROW
         BEGIN
             SELECT RAISE(ABORT, 'events are append-only');
         END",
    )
    .expect("create delete trigger");

    assert_constraint_message(
        conn.execute("UPDATE events SET payload = 'changed' WHERE id = 1")
            .expect_err("update must fail"),
        "events are append-only",
    );
    assert_eq!(
        scalar_text(&conn, "SELECT payload FROM events WHERE id = 1"),
        "original"
    );

    assert_constraint_message(
        conn.execute("DELETE FROM events WHERE id = 1")
            .expect_err("delete must fail"),
        "events are append-only",
    );
    assert_eq!(scalar_i64(&conn, "SELECT count(*) FROM events"), 1);
}

#[test]
fn raise_fail_and_rollback_surface_constraint_messages() {
    let (_dir, conn) = open_database();

    for action in ["FAIL", "ROLLBACK"] {
        let table = format!("blocked_{}", action.to_ascii_lowercase());
        conn.execute(&format!("CREATE TABLE {table}(id INTEGER PRIMARY KEY)"))
            .expect("create table");
        conn.execute(&format!("INSERT INTO {table}(id) VALUES (1)"))
            .expect("seed table");
        conn.execute(&format!(
            "CREATE TRIGGER {table}_no_delete
             BEFORE DELETE ON {table}
             FOR EACH ROW
             BEGIN
                 SELECT RAISE({action}, '{action} blocked');
             END"
        ))
        .expect("create trigger");

        assert_constraint_message(
            conn.execute(&format!("DELETE FROM {table} WHERE id = 1"))
                .expect_err("delete must fail"),
            &format!("{action} blocked"),
        );
        assert_eq!(
            scalar_i64(&conn, &format!("SELECT count(*) FROM {table}")),
            1
        );
    }
}

#[test]
fn jeryu_schema_prefix_creates_indexes_after_composite_primary_key() {
    let (_dir, conn) = open_database();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS pools (
            name TEXT PRIMARY KEY,
            gitlab_runner_id INTEGER NOT NULL,
            auth_token TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '',
            executor TEXT NOT NULL DEFAULT 'docker',
            min_warm INTEGER NOT NULL DEFAULT 1,
            max_managers INTEGER NOT NULL DEFAULT 4,
            concurrent INTEGER NOT NULL DEFAULT 8,
            request_concurrency INTEGER NOT NULL DEFAULT 4,
            paused INTEGER NOT NULL DEFAULT 0,
            trust_tier TEXT NOT NULL DEFAULT 'trusted'
        )",
    )
    .expect("create pools");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS managers (
            id TEXT PRIMARY KEY,
            pool_name TEXT NOT NULL REFERENCES pools(name),
            docker_container_id TEXT NOT NULL UNIQUE,
            system_id TEXT,
            state TEXT NOT NULL DEFAULT 'starting',
            config_dir TEXT NOT NULL,
            started_at TEXT,
            last_contact_at TEXT
        )",
    )
    .expect("create managers");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS job_events (
            job_id INTEGER NOT NULL,
            project_id INTEGER NOT NULL,
            pipeline_id INTEGER,
            status TEXT NOT NULL,
            job_name TEXT,
            pool_name TEXT,
            system_id TEXT,
            queued_duration REAL,
            received_at TEXT NOT NULL,
            PRIMARY KEY (job_id, status)
        )",
    )
    .expect("create job_events");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ci_job_runs (
            job_id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL,
            pipeline_id INTEGER NOT NULL,
            root_pipeline_id INTEGER NOT NULL,
            pipeline_sha TEXT NOT NULL,
            ref_name TEXT NOT NULL,
            job_name TEXT NOT NULL,
            stage TEXT NOT NULL,
            status TEXT NOT NULL,
            runner TEXT,
            runner_pool TEXT,
            queued_duration_secs REAL,
            duration_secs REAL,
            started_at TEXT,
            finished_at TEXT,
            web_url TEXT,
            observed_at TEXT NOT NULL
        )",
    )
    .expect("create ci_job_runs");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ci_job_runs_pipeline
         ON ci_job_runs(project_id, pipeline_id)",
    )
    .expect("create ci_job_runs index");
}

#[test]
fn schema_ddl_is_visible_to_sibling_connections() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = Database::create(dir.path().join("schema.db"), DbOptions::default())
        .expect("create database");
    let writer = db.connect();
    let reader = db.connect();

    writer
        .execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .expect("create table on writer");
    assert_eq!(scalar_i64(&reader, "SELECT count(*) FROM t"), 0);

    writer
        .execute("CREATE INDEX IF NOT EXISTS t_name_idx ON t(name)")
        .expect("create index on writer");
    reader
        .execute("CREATE INDEX IF NOT EXISTS t_name_idx ON t(name)")
        .expect("duplicate index on reader is ignored");
}

fn assert_sqlite_autoincrement_error(err: Error) {
    assert!(
        err.to_string()
            .contains("AUTOINCREMENT is only allowed on an INTEGER PRIMARY KEY"),
        "unexpected AUTOINCREMENT error: {err:?}"
    );
}

fn assert_constraint_message(err: Error, expected: &str) {
    match err {
        Error::ConstraintViolation(message) => {
            assert!(
                message.contains(expected),
                "expected message containing {expected:?}, got {message:?}"
            );
        }
        other => panic!("expected constraint violation, got {other:?}"),
    }
}

fn scalar_i64(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare scalar i64");
    assert_eq!(stmt.step().expect("step row"), Step::Row);
    let value = stmt.column_i64(0).expect("read i64");
    assert_eq!(stmt.step().expect("step done"), Step::Done);
    value
}

fn scalar_text(conn: &Arc<Connection>, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect("prepare scalar text");
    assert_eq!(stmt.step().expect("step row"), Step::Row);
    let value = match stmt.column_value(0).expect("read text") {
        SqlValue::Text(value) => value.to_string(),
        other => panic!("expected text value, got {other:?}"),
    };
    assert_eq!(stmt.step().expect("step done"), Step::Done);
    value
}
