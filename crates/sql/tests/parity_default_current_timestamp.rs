use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::ValueRef;
use std::sync::Arc;
use tempfile::tempdir;

const JANSU_CLUSTER_DDL: &str = "
    CREATE TABLE IF NOT EXISTS cluster (
        id integer primary key autoincrement,
        name text not null unique,
        last_updated datetime default current_timestamp not null,
        created_at datetime default current_timestamp not null
    )
";

#[test]
fn jansu_cluster_defaults_match_sqlite_observable_contract() {
    let (_dir, redline) = open_redline();
    let sqlite = rusqlite::Connection::open_in_memory().expect("open sqlite");

    redline.execute(JANSU_CLUSTER_DDL).expect("redline create");
    sqlite
        .execute_batch(JANSU_CLUSTER_DDL)
        .expect("sqlite create");

    redline
        .execute("INSERT INTO cluster(name) VALUES ('main')")
        .expect("redline insert");
    sqlite
        .execute("INSERT INTO cluster(name) VALUES ('main')", [])
        .expect("sqlite insert");

    let sql = "
        SELECT
            id,
            typeof(created_at),
            length(created_at),
            typeof(last_updated),
            length(last_updated)
        FROM cluster
    ";
    assert_eq!(redline_rows(&redline, sql), sqlite_rows(&sqlite, sql));

    let created_at = redline_text(&redline, "SELECT created_at FROM cluster");
    assert_sqlite_date_time_shape(&created_at);
}

#[test]
fn current_date_time_and_timestamp_defaults_follow_sqlite_shapes() {
    let (_dir, redline) = open_redline();
    let sqlite = rusqlite::Connection::open_in_memory().expect("open sqlite");
    let ddl = "
        CREATE TABLE clock_defaults (
            id integer primary key autoincrement,
            d text default current_date,
            t text default current_time,
            ts text default current_timestamp not null,
            explicit_ts text default current_timestamp
        )
    ";

    redline.execute(ddl).expect("redline create");
    sqlite.execute_batch(ddl).expect("sqlite create");
    for sql in [
        "INSERT INTO clock_defaults DEFAULT VALUES",
        "INSERT INTO clock_defaults(explicit_ts) VALUES (NULL)",
    ] {
        redline.execute(sql).expect("redline insert");
        sqlite.execute(sql, []).expect("sqlite insert");
    }

    let sql = "
        SELECT
            id,
            typeof(d),
            length(d),
            typeof(t),
            length(t),
            typeof(ts),
            length(ts),
            explicit_ts IS NULL
        FROM clock_defaults
        ORDER BY id
    ";
    assert_eq!(redline_rows(&redline, sql), sqlite_rows(&sqlite, sql));

    assert_sqlite_date_shape(&redline_text(
        &redline,
        "SELECT d FROM clock_defaults WHERE id = 1",
    ));
    assert_sqlite_time_shape(&redline_text(
        &redline,
        "SELECT t FROM clock_defaults WHERE id = 1",
    ));
    assert_sqlite_date_time_shape(&redline_text(
        &redline,
        "SELECT ts FROM clock_defaults WHERE id = 1",
    ));
}

#[test]
fn current_timestamp_default_survives_catalog_reopen() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("defaults.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create redline");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(id integer primary key, ts text default current_timestamp)")
            .expect("create table");
    }
    {
        let db = Database::open(&path, DbOptions::default()).expect("reopen redline");
        let conn = db.connect();
        conn.execute("INSERT INTO t(id) VALUES (1)")
            .expect("insert default");
        assert_sqlite_date_time_shape(&redline_text(&conn, "SELECT ts FROM t WHERE id = 1"));
    }
}

fn open_redline() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("redline.db");
    let db = Database::create(path, DbOptions::default()).expect("create redline");
    (dir, db.connect())
}

fn redline_rows(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare redline");
    let col_count = stmt.column_count();
    let mut rows = Vec::new();
    while let Step::Row = stmt.step().expect("step redline") {
        rows.push(
            (0..col_count)
                .map(|idx| stmt.column_value(idx).expect("redline value").clone())
                .collect(),
        );
    }
    rows
}

fn sqlite_rows(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare sqlite");
    let col_count = stmt.column_count();
    let mut rows = stmt.query([]).expect("query sqlite");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("next sqlite") {
        out.push(
            (0..col_count)
                .map(|idx| sqlite_value(row.get_ref(idx).expect("sqlite value")))
                .collect(),
        );
    }
    out
}

fn sqlite_value(value: ValueRef<'_>) -> SqlValue {
    match value {
        ValueRef::Null => SqlValue::Null,
        ValueRef::Integer(value) => SqlValue::Integer(value),
        ValueRef::Real(value) => SqlValue::Real(value),
        ValueRef::Text(value) => SqlValue::Text(Arc::from(
            std::str::from_utf8(value).expect("sqlite text utf8"),
        )),
        ValueRef::Blob(value) => SqlValue::Blob(Arc::from(value)),
    }
}

fn redline_text(conn: &Arc<Connection>, sql: &str) -> String {
    match redline_rows(conn, sql).remove(0).remove(0) {
        SqlValue::Text(value) => value.to_string(),
        other => panic!("expected text, got {other:?}"),
    }
}

fn assert_sqlite_date_time_shape(value: &str) {
    assert_eq!(value.len(), 19, "timestamp length: {value}");
    assert_eq!(
        value.as_bytes()[4],
        b'-',
        "timestamp year separator: {value}"
    );
    assert_eq!(
        value.as_bytes()[7],
        b'-',
        "timestamp month separator: {value}"
    );
    assert_eq!(
        value.as_bytes()[10],
        b' ',
        "timestamp date/time separator: {value}"
    );
    assert_eq!(
        value.as_bytes()[13],
        b':',
        "timestamp hour separator: {value}"
    );
    assert_eq!(
        value.as_bytes()[16],
        b':',
        "timestamp minute separator: {value}"
    );
}

fn assert_sqlite_date_shape(value: &str) {
    assert_eq!(value.len(), 10, "date length: {value}");
    assert_eq!(value.as_bytes()[4], b'-', "date year separator: {value}");
    assert_eq!(value.as_bytes()[7], b'-', "date month separator: {value}");
}

fn assert_sqlite_time_shape(value: &str) {
    assert_eq!(value.len(), 8, "time length: {value}");
    assert_eq!(value.as_bytes()[2], b':', "time hour separator: {value}");
    assert_eq!(value.as_bytes()[5], b':', "time minute separator: {value}");
}
