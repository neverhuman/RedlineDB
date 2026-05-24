//! High-level SQLite parity traceability tests.
//!
//! This suite is intentionally small. It inspects the bundled `rusqlite`
//! reference build, checks representative passing behavior against that
//! reference, and keeps major full-SQLite gaps executable until they are
//! implemented. It does not write local parity evidence artifacts; official
//! SQLite parity evidence comes only from the pinned `neverhuman/redline-testing`
//! release artifact.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

const ROW_COMPARED_REFERENCE_PRAGMAS: &[&str] = &[
    "cache_size",
    "database_list",
    "foreign_keys",
    "index_info",
    "index_list",
    "integrity_check",
    "journal_mode",
    "query_only",
    "quick_check",
    "recursive_triggers",
    "schema_version",
    "synchronous",
    "table_info",
    "table_xinfo",
    "temp_store",
    "user_version",
];

const KNOWN_GAP_REFERENCE_PRAGMAS: &[&str] = &[
    "collation_list",
    "compile_options",
    "foreign_key_list",
    "function_list",
    "index_xinfo",
    "module_list",
    "pragma_list",
    "table_list",
];

const EXPLICIT_REJECT_REFERENCE_PRAGMAS: &[&str] = &[
    "analysis_limit",
    "application_id",
    "auto_vacuum",
    "automatic_index",
    "busy_timeout",
    "cache_spill",
    "case_sensitive_like",
    "cell_size_check",
    "checkpoint_fullfsync",
    "count_changes",
    "data_version",
    "default_cache_size",
    "defer_foreign_keys",
    "empty_result_callbacks",
    "encoding",
    "foreign_key_check",
    "freelist_count",
    "full_column_names",
    "fullfsync",
    "hard_heap_limit",
    "ignore_check_constraints",
    "incremental_vacuum",
    "journal_size_limit",
    "legacy_alter_table",
    "locking_mode",
    "max_page_count",
    "mmap_size",
    "optimize",
    "page_count",
    "page_size",
    "read_uncommitted",
    "reverse_unordered_selects",
    "secure_delete",
    "short_column_names",
    "shrink_memory",
    "soft_heap_limit",
    "temp_store_directory",
    "threads",
    "trusted_schema",
    "wal_autocheckpoint",
    "wal_checkpoint",
    "writable_schema",
];

fn sqlite_pragma_list(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("PRAGMA pragma_list")
        .expect("prepare pragma_list");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("pragma_list rows");
    let mut out = rows
        .collect::<Result<Vec<_>, _>>()
        .expect("pragma_list values");
    out.sort();
    out
}

fn sqlite_reference_metadata() -> (String, Vec<String>, Vec<String>) {
    let conn = rusqlite::Connection::open_in_memory().expect("open sqlite");
    let version: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .expect("sqlite version");
    let compile_options: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA compile_options")
            .expect("compile options");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("compile option rows");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("compile option values")
    };
    let pragmas = sqlite_pragma_list(&conn);
    (version, compile_options, pragmas)
}

fn classified_reference_pragmas() -> BTreeSet<String> {
    ROW_COMPARED_REFERENCE_PRAGMAS
        .iter()
        .chain(KNOWN_GAP_REFERENCE_PRAGMAS)
        .chain(EXPLICIT_REJECT_REFERENCE_PRAGMAS)
        .map(|name| (*name).to_owned())
        .collect()
}

struct Harness {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl Harness {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let db = Database::create(dir.path().join("parity.db"), DbOptions::default())
            .expect("create redline db");
        let sqlite = rusqlite::Connection::open_in_memory().expect("open sqlite");
        Self {
            _dir: dir,
            redline: db.connect(),
            sqlite,
        }
    }

    fn execute_both(&self, sql: &str) {
        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|err| panic!("sqlite setup failed for {sql:?}: {err}"));
        self.redline
            .execute(sql)
            .unwrap_or_else(|err| panic!("redline setup failed for {sql:?}: {err:?}"));
    }

    fn assert_query_matches(&self, sql: &str) {
        let sqlite_rows = query_sqlite(&self.sqlite, sql);
        let redline_rows = query_redline(&self.redline, sql);
        assert_eq!(
            redline_rows, sqlite_rows,
            "query mismatch for {sql:?}\nsqlite={sqlite_rows:?}\nredline={redline_rows:?}"
        );
    }

    #[allow(dead_code)] // Used by parity-gap fixtures that drop in or out as the ledger flips.
    fn assert_sqlite_accepts_redline_rejects(&self, setup: &[&str], sql: &str) {
        for stmt in setup {
            self.execute_both(stmt);
        }

        self.sqlite
            .execute_batch(sql)
            .unwrap_or_else(|err| panic!("sqlite should accept {sql:?}: {err}"));

        let redline_result = redline_accepts(&self.redline, sql);
        assert!(
            redline_result.is_err(),
            "redline unexpectedly accepted known full-parity gap: {sql}"
        );
    }

    #[allow(dead_code)] // Used by parity-gap fixtures that drop in or out as the ledger flips.
    fn assert_sqlite_result_diff_or_redline_rejects(&self, setup: &[&str], sql: &str) {
        for stmt in setup {
            self.sqlite
                .execute_batch(stmt)
                .unwrap_or_else(|err| panic!("sqlite setup failed for {stmt:?}: {err}"));
            if self.redline.execute(stmt).is_err() {
                return;
            }
        }

        let sqlite_rows = query_sqlite(&self.sqlite, sql);
        let redline_rows = match try_query_redline(&self.redline, sql) {
            Ok(rows) => rows,
            Err(_) => return,
        };
        assert_ne!(
            redline_rows, sqlite_rows,
            "redline unexpectedly matched known full-parity gap: {sql}"
        );
    }
}

fn query_sqlite(conn: &rusqlite::Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|err| panic!("sqlite prepare failed for {sql:?}: {err}"));
    let columns = stmt.column_count();
    let mut rows = stmt.query([]).expect("sqlite query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("sqlite next") {
        let mut values = Vec::with_capacity(columns);
        for idx in 0..columns {
            let value: RuValue = row.get(idx).expect("sqlite value");
            values.push(to_sql_value(value));
        }
        out.push(values);
    }
    out
}

fn query_redline(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    try_query_redline(conn, sql).unwrap_or_else(|err| {
        panic!("redline query failed for {sql:?}: {err:?}");
    })
}

fn try_query_redline(
    conn: &Arc<Connection>,
    sql: &str,
) -> Result<Vec<Vec<SqlValue>>, redlinedb_sql::Error> {
    let mut stmt = conn.prepare(sql)?;
    let columns = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step()? {
        let mut values = Vec::with_capacity(columns);
        for idx in 0..columns {
            values.push(stmt.column_value(idx)?.clone());
        }
        out.push(values);
    }
    Ok(out)
}

#[allow(dead_code)] // Used by `assert_sqlite_accepts_redline_rejects` parity-gap fixtures.
fn redline_accepts(conn: &Arc<Connection>, sql: &str) -> Result<(), redlinedb_sql::Error> {
    let trimmed = sql.trim_start();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("SELECT") || upper.starts_with("WITH") {
        let mut stmt = conn.prepare(sql)?;
        while let Step::Row = stmt.step()? {}
        return Ok(());
    }
    conn.execute(sql).map(|_| ())
}

fn to_sql_value(value: RuValue) -> SqlValue {
    match value {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(value) => SqlValue::Integer(value),
        RuValue::Real(value) => SqlValue::Real(value),
        RuValue::Text(value) => SqlValue::Text(Arc::from(value)),
        RuValue::Blob(value) => SqlValue::Blob(Arc::from(value)),
    }
}

#[test]
fn reference_build_metadata_is_available() {
    let (version, compile_options, pragmas) = sqlite_reference_metadata();

    println!("rusqlite_crate_version=0.37.0");
    println!("sqlite_version={version}");
    println!("compile_options:");
    for option in &compile_options {
        println!("{option}");
    }
    println!("pragma_list:");
    for pragma in &pragmas {
        println!("{pragma}");
    }

    assert!(!version.trim().is_empty(), "empty sqlite_version()");
    assert!(
        !compile_options.is_empty(),
        "bundled SQLite reported no compile options"
    );
    assert!(
        !pragmas.is_empty(),
        "bundled SQLite reported no PRAGMA names"
    );
}

#[test]
fn reference_build_pragmas_are_fully_classified() {
    let (_, _, pragmas) = sqlite_reference_metadata();
    let reference: BTreeSet<String> = pragmas.into_iter().collect();
    let classified = classified_reference_pragmas();

    let missing: Vec<_> = reference.difference(&classified).cloned().collect();
    let retired: Vec<_> = classified.difference(&reference).cloned().collect();

    assert!(
        missing.is_empty(),
        "bundled SQLite exposes unclassified PRAGMA(s): {missing:?}"
    );
    assert!(
        retired.is_empty(),
        "PRAGMA classification contains names not emitted by bundled SQLite: {retired:?}"
    );
}

#[test]
fn reference_build_pragma_rows_match_for_supported_surfaces() {
    let harness = Harness::new();
    harness.execute_both("PRAGMA foreign_keys = ON");
    harness.execute_both("PRAGMA recursive_triggers = ON");
    harness.execute_both("PRAGMA user_version = 7");
    harness.execute_both("PRAGMA journal_mode = memory");
    harness.execute_both("PRAGMA synchronous = FULL");
    harness.execute_both("PRAGMA temp_store = MEMORY");
    harness.execute_both("PRAGMA cache_size = -256");
    harness.execute_both("PRAGMA query_only = OFF");
    harness.execute_both("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, n INTEGER)");
    harness.execute_both("CREATE TABLE x(a TEXT, b INTEGER)");
    harness.execute_both("CREATE TABLE parent(id INTEGER PRIMARY KEY, label TEXT)");
    harness.execute_both(
        "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id), label TEXT)",
    );
    harness.execute_both("CREATE INDEX t_name_idx ON t(name)");
    harness.execute_both(
        "INSERT INTO t(id, name, n) VALUES (1, ' Ada ', 10), (2, 'Grace', NULL), (3, NULL, 5)",
    );
    harness.execute_both("INSERT INTO parent(id, label) VALUES (1, 'Ada'), (2, 'Grace')");
    harness.execute_both(
        "INSERT INTO child(id, parent_id, label) VALUES (10, 1, 'alpha'), (11, NULL, 'beta')",
    );

    harness.assert_query_matches("PRAGMA foreign_keys");
    harness.assert_query_matches("PRAGMA recursive_triggers");
    harness.assert_query_matches("PRAGMA user_version");
    harness.assert_query_matches("PRAGMA journal_mode");
    harness.assert_query_matches("PRAGMA synchronous");
    harness.assert_query_matches("PRAGMA temp_store");
    harness.assert_query_matches("PRAGMA cache_size");
    harness.assert_query_matches("PRAGMA query_only");
    harness.assert_query_matches("PRAGMA integrity_check");
    harness.assert_query_matches("PRAGMA quick_check");
    harness.assert_query_matches(
        "SELECT seq, name FROM pragma_database_list() WHERE name = 'main' ORDER BY seq",
    );
    harness.assert_query_matches(
        "SELECT cid, name, type, dflt_value, pk FROM pragma_table_info('t') ORDER BY cid",
    );
    harness.assert_query_matches("PRAGMA table_xinfo('x')");
    harness.assert_query_matches(
        "SELECT name, \"unique\", origin FROM pragma_index_list('t') \
         WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY name",
    );
    harness.assert_query_matches(
        "SELECT seqno, cid, name FROM pragma_index_info('t_name_idx') ORDER BY seqno",
    );

    let sqlite_pragmas = sqlite_pragma_list(&harness.sqlite);
    for pragma in ROW_COMPARED_REFERENCE_PRAGMAS {
        assert!(
            sqlite_pragmas.iter().any(|name| name == pragma),
            "row-compared PRAGMA {pragma} is not present in bundled SQLite pragma_list"
        );
    }
}

#[test]
fn known_full_sqlite_parity_gaps_are_explicit_failures() {
    let harness = Harness::new();
    // Track C closed these PRAGMAs (auto_vacuum, page_size, encoding,
    // application_id) — they are now accepted recall-only surfaces.
    // Asserting acceptance here keeps the ledger honest: any
    // regression that re-rejects them will trip the assertion above.
    harness.execute_both("PRAGMA auto_vacuum = FULL");
    harness.execute_both("PRAGMA application_id = 42");
    // page_size and encoding accept the value but don't produce rows;
    // execute through redline directly so a stray future surface
    // change shows up here.
    harness
        .redline
        .execute("PRAGMA page_size = 4096")
        .expect("page_size accepted");
    harness
        .redline
        .execute("PRAGMA encoding = 'UTF-8'")
        .expect("encoding accepted");
    harness.assert_sqlite_result_diff_or_redline_rejects(
        &["PRAGMA journal_mode=WAL"],
        "PRAGMA wal_checkpoint(FULL)",
    );
    harness.assert_sqlite_result_diff_or_redline_rejects(
        &[
            "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)",
            "CREATE INDEX t_name_idx ON t(name)",
        ],
        "PRAGMA index_xinfo('t_name_idx')",
    );
    // pragma_foreign_key_list closed in Track C — both engines now
    // return identical rows.
    harness.execute_both("CREATE TABLE parent(id INTEGER PRIMARY KEY, label TEXT)");
    harness.execute_both(
        "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id), label TEXT)",
    );
    harness.assert_query_matches(
        "SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete, \"match\" \
         FROM pragma_foreign_key_list('child') ORDER BY id, seq",
    );
    harness.assert_sqlite_result_diff_or_redline_rejects(
        &[
            "CREATE TABLE names(name TEXT COLLATE NOCASE)",
            "INSERT INTO names(name) VALUES ('a'), ('B')",
        ],
        "SELECT name FROM names ORDER BY name",
    );
}

#[test]
fn sqlite_native_file_format_is_not_compatibility_surface() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("redline-native.db");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create redline db");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)")
            .expect("create table");
        conn.execute("INSERT INTO t(id, name) VALUES (1, 'Ada')")
            .expect("insert");
    }

    assert!(
        path.is_dir(),
        "RedlineDB root should remain a directory, not a SQLite database file"
    );
    let mut entries = fs::read_dir(&path)
        .unwrap_or_else(|err| panic!("read RedlineDB root directory {}: {err}", path.display()));
    assert!(
        entries.next().is_some(),
        "RedlineDB root should contain Redline-native files"
    );

    let sqlite = rusqlite::Connection::open(&path);
    assert!(
        sqlite.is_err(),
        "SQLite should not open a RedlineDB-native directory as a valid SQLite database"
    );
}

#[test]
fn sqlite_native_file_is_not_redline_database_root() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("sqlite-native.db");
    {
        let sqlite = rusqlite::Connection::open(&path).expect("create sqlite file");
        sqlite
            .execute_batch(
                "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO t(id, name) VALUES (1, 'Ada');",
            )
            .expect("seed sqlite file");
    }

    assert!(
        path.is_file(),
        "SQLite should create a native database file at {}",
        path.display()
    );
    let redline = Database::open(&path, DbOptions::default());
    assert!(
        redline.is_err(),
        "RedlineDB should not open a SQLite-native database file as a RedlineDB root"
    );
}
