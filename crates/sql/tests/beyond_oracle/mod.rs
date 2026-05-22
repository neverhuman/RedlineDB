#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};

pub struct RedlineHarness {
    _root: Option<tempfile::TempDir>,
    conn: Arc<Connection>,
}

impl RedlineHarness {
    pub fn in_memory() -> Self {
        let db = Database::create_in_memory(DbOptions::default()).expect("redline in-memory db");
        Self {
            _root: None,
            conn: db.connect(),
        }
    }

    pub fn file_backed(name: &str) -> Self {
        let root = preferred_redline_root();
        let db_path = root.path().join(format!("{name}.db"));
        let db = Database::create(&db_path, DbOptions::default()).expect("redline file db");
        Self {
            _root: Some(root),
            conn: db.connect(),
        }
    }

    pub fn execute(&self, sql: &str) {
        self.conn.execute(sql).expect("redline execute");
    }

    pub fn execute_error(&self, sql: &str) -> String {
        match self.conn.execute(sql) {
            Ok(_) => panic!("redline unexpectedly accepted: {sql}"),
            Err(err) => err.to_string(),
        }
    }

    pub fn query_text_rows(&self, sql: &str) -> Vec<Vec<String>> {
        let mut stmt = self.conn.prepare(sql).expect("redline prepare");
        let columns = stmt.column_count();
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("redline step") {
            rows.push(
                (0..columns)
                    .map(|index| value_to_text(stmt.column_value(index).expect("redline value")))
                    .collect(),
            );
        }
        rows
    }
}

pub struct SqliteHarness {
    conn: rusqlite::Connection,
}

impl SqliteHarness {
    pub fn in_memory() -> Self {
        Self {
            conn: rusqlite::Connection::open_in_memory().expect("sqlite in-memory db"),
        }
    }

    pub fn execute(&self, sql: &str) {
        self.conn.execute_batch(sql).expect("sqlite execute");
    }

    pub fn prepare_error(&self, sql: &str) -> String {
        match self.conn.prepare(sql) {
            Ok(_) => panic!("sqlite unexpectedly prepared: {sql}"),
            Err(err) => err.to_string(),
        }
    }
}

pub struct PostgresHarness {
    client: postgres::Client,
    schema: String,
}

impl PostgresHarness {
    pub fn try_connect_from_env() -> Option<Self> {
        let url = match std::env::var("REDLINEDB_POSTGRES_URL") {
            Ok(url) => url,
            Err(_) => return None,
        };
        Some(Self::connect(&url))
    }

    pub fn connect_from_env() -> Self {
        let url = std::env::var("REDLINEDB_POSTGRES_URL")
            .expect("REDLINEDB_POSTGRES_URL is required for beyond Postgres reference tests");
        Self::connect(&url)
    }

    fn connect(url: &str) -> Self {
        let mut client = postgres::Client::connect(&url, postgres::NoTls)
            .expect("connect to REDLINEDB_POSTGRES_URL");
        let schema = format!(
            "redlinedb_beyond_{}_{}",
            std::process::id(),
            next_schema_id()
        );
        client
            .batch_execute(&format!(
                "CREATE SCHEMA {schema}; SET search_path TO {schema};"
            ))
            .expect("create isolated postgres schema");
        Self { client, schema }
    }

    pub fn execute(&mut self, sql: &str) {
        self.client.batch_execute(sql).expect("postgres execute");
    }

    pub fn execute_error(&mut self, sql: &str) -> String {
        match self.client.batch_execute(sql) {
            Ok(_) => panic!("postgres unexpectedly accepted: {sql}"),
            Err(err) => err.to_string(),
        }
    }

    pub fn query_text_rows(&mut self, sql: &str) -> Vec<Vec<String>> {
        self.client
            .query(sql, &[])
            .expect("postgres query")
            .into_iter()
            .map(|row| {
                (0..row.len())
                    .map(|index| row.get::<usize, String>(index))
                    .collect()
            })
            .collect()
    }
}

impl Drop for PostgresHarness {
    fn drop(&mut self) {
        let _ = self
            .client
            .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE;", self.schema));
    }
}

fn preferred_redline_root() -> tempfile::TempDir {
    let shm = Path::new("/dev/shm/redlinedb-beyond-sqlite");
    if std::fs::create_dir_all(shm).is_ok()
        && let Ok(dir) = tempfile::Builder::new()
            .prefix("redlinedb-beyond.")
            .tempdir_in(shm)
    {
        return dir;
    }
    tempfile::Builder::new()
        .prefix("redlinedb-beyond.")
        .tempdir()
        .expect("tempdir")
}

fn value_to_text(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "NULL".to_owned(),
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Real(value) => value.to_string(),
        SqlValue::Text(value) => value.to_string(),
        SqlValue::Blob(value) => format!("{value:x?}"),
    }
}

fn next_schema_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

pub fn both_engines_reject(redline: &RedlineHarness, postgres: &mut PostgresHarness, sql: &str) {
    let redline_err = redline.execute_error(sql);
    let postgres_err = postgres.execute_error(sql);
    assert!(
        !redline_err.trim().is_empty(),
        "redline rejection message was empty for {sql}"
    );
    assert!(
        !postgres_err.trim().is_empty(),
        "postgres rejection message was empty for {sql}"
    );
}
