//! SQLite drop-in parity coverage tests — positive cases for constructs
//! previously untested or thinly covered.
//!
//! Covers: ALTER TABLE RENAME, DROP INDEX, RETURNING with expressions,
//! subqueries (EXISTS/NOT EXISTS/correlated), multi-join chains,
//! NULL semantics, PRAGMA integrity_check, WAL checkpoint modes,
//! nested SAVEPOINTs, and NULL IN/NOT IN edge cases.

use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use rusqlite::types::Value as RuValue;
use std::sync::Arc;
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("pos.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn query_all(conn: &Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let ncols = stmt.column_count();
    let mut out = Vec::new();
    while let Step::Row = stmt.step().expect("step") {
        let row: Vec<SqlValue> = (0..ncols)
            .map(|i| stmt.column_value(i).expect("col").clone())
            .collect();
        out.push(row);
    }
    out
}

fn q1(conn: &Arc<Connection>, sql: &str) -> SqlValue {
    query_all(conn, sql)
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next())
        .unwrap_or(SqlValue::Null)
}

// ── ALTER TABLE RENAME ────────────────────────────────────────────────────────

#[test]
fn alter_table_rename_to() {
    let (_d, c) = open();
    c.execute("CREATE TABLE old_name(id INTEGER)")
        .expect("create");
    c.execute("INSERT INTO old_name VALUES (42)")
        .expect("insert");
    c.execute("ALTER TABLE old_name RENAME TO new_name")
        .expect("rename");
    let v = q1(&c, "SELECT id FROM new_name");
    assert_eq!(v, SqlValue::Integer(42));
}

#[test]
fn alter_table_rename_column() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(old_col INTEGER)")
        .expect("create");
    c.execute("INSERT INTO t VALUES (7)").expect("insert");
    c.execute("ALTER TABLE t RENAME COLUMN old_col TO new_col")
        .expect("rename column");
    let v = q1(&c, "SELECT new_col FROM t");
    assert_eq!(v, SqlValue::Integer(7));
}

// ── CREATE TABLE AS SELECT ───────────────────────────────────────────────────

#[test]
fn create_table_as_select_executes() {
    let (_d, c) = open();
    c.execute("CREATE TABLE src(id INTEGER)").expect("create");
    c.execute("INSERT INTO src VALUES (1)").expect("insert");
    c.execute("CREATE TABLE dst AS SELECT * FROM src")
        .expect("ctas");
    let v = q1(&c, "SELECT id FROM dst");
    assert_eq!(v, SqlValue::Integer(1));
}

// ── DROP INDEX ────────────────────────────────────────────────────────────────

#[test]
fn create_and_drop_index() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    c.execute("CREATE INDEX idx_a ON t(a)")
        .expect("create index");
    c.execute("DROP INDEX idx_a").expect("drop index");
    // Verify the index is gone by re-creating it with the same name (would fail if it still existed)
    c.execute("CREATE INDEX idx_a ON t(a)")
        .expect("re-create after drop");
}

#[test]
fn reindex_executes_after_index_creation() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a TEXT COLLATE NOCASE)")
        .expect("create");
    c.execute("CREATE INDEX i_t_a ON t(a)")
        .expect("create index");
    c.execute("INSERT INTO t VALUES('A'),('b')")
        .expect("insert");
    c.execute("REINDEX").expect("reindex");
    let count = q1(&c, "SELECT count(*) FROM t");
    assert_eq!(count, SqlValue::Integer(2));
}

#[test]
fn vacuum_into_creates_copy_of_database_directory() {
    let dir = tempdir().expect("temp dir");
    let src = dir.path().join("src.db");
    let dst = dir.path().join("dst.db");
    let db = Database::create(&src, DbOptions::default()).expect("create");
    let c = db.connect();
    c.execute("CREATE TABLE t(x INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (12)").expect("insert");
    c.execute(&format!("VACUUM INTO '{}'", dst.display()))
        .expect("vacuum into");
    let copy = Database::open(&dst, DbOptions::default()).expect("open copy");
    let copy_conn = copy.connect();
    assert_eq!(q1(&copy_conn, "SELECT x FROM t"), SqlValue::Integer(12));
}

#[test]
fn drop_index_if_exists() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER)").expect("create");
    // Should not error when the index does not exist
    c.execute("DROP INDEX IF EXISTS idx_nonexistent")
        .expect("drop if exists");
}

// ── RETURNING with expressions ────────────────────────────────────────────────

#[test]
fn returning_with_arithmetic_expression() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    let rows = query_all(&c, "INSERT INTO t VALUES (3, 4) RETURNING a + b");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(7));
}

#[test]
fn returning_with_function_call() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(name TEXT)").expect("create");
    let rows = query_all(&c, "INSERT INTO t VALUES ('hello') RETURNING upper(name)");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Text(Arc::from("HELLO")));
}

#[test]
fn update_returning_with_expression() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("create");
    c.execute("INSERT INTO t VALUES (10, 3)").expect("insert");
    let rows = query_all(&c, "UPDATE t SET a = a + 1 RETURNING a * b");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(33)); // (10+1)*3
}

// ── EXISTS / NOT EXISTS ───────────────────────────────────────────────────────

#[test]
fn exists_subquery_true() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1)").expect("insert");
    let v = q1(&c, "SELECT 1 WHERE EXISTS (SELECT 1 FROM t)");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn exists_subquery_false() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    // Empty table → EXISTS is false → no rows in outer
    let rows = query_all(&c, "SELECT 1 WHERE EXISTS (SELECT 1 FROM t)");
    assert!(rows.is_empty(), "expected no rows");
}

#[test]
fn not_exists_subquery_true() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    let v = q1(&c, "SELECT 42 WHERE NOT EXISTS (SELECT 1 FROM t)");
    assert_eq!(v, SqlValue::Integer(42));
}

// ── NULL semantics ────────────────────────────────────────────────────────────

#[test]
fn null_in_empty_list() {
    let (_d, c) = open();
    // NULL IN () → NULL (or 0 by SQLite semantics — returns false/0)
    let v = q1(&c, "SELECT NULL IN (1, 2, 3)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn value_in_list_with_null() {
    let (_d, c) = open();
    // 1 IN (1, NULL) → 1 (found, ignores NULL)
    let v = q1(&c, "SELECT 1 IN (1, NULL)");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn value_not_in_list_with_null() {
    // 2 NOT IN (1, NULL) → NULL (because NULL means unknown whether 2 = NULL)
    let (_d, c) = open();
    let v = q1(&c, "SELECT 2 NOT IN (1, NULL)");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn row_value_in_subquery_matches() {
    let (_d, c) = open();
    c.execute("CREATE TABLE lhs(a INTEGER, b TEXT)")
        .expect("create lhs");
    c.execute("CREATE TABLE rhs(a INTEGER, b TEXT)")
        .expect("create rhs");
    c.execute("INSERT INTO lhs VALUES (1, 'one'), (2, 'two')")
        .expect("insert lhs");
    c.execute("INSERT INTO rhs VALUES (1, 'one')")
        .expect("insert rhs");
    let rows = query_all(
        &c,
        "SELECT (a, b) IN (SELECT a, b FROM rhs), (a, b) NOT IN (SELECT a, b FROM rhs) FROM lhs ORDER BY a",
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec![SqlValue::Integer(1), SqlValue::Integer(0)]);
    assert_eq!(rows[1], vec![SqlValue::Integer(0), SqlValue::Integer(1)]);
}

#[test]
fn unqualified_correlated_in_subquery_is_not_cached_as_uncorrelated() {
    let (_d, c) = open();
    c.execute("CREATE TABLE outer_t(a INTEGER, marker INTEGER)")
        .expect("create outer");
    c.execute("CREATE TABLE inner_t(b INTEGER)")
        .expect("create inner");
    c.execute("INSERT INTO outer_t VALUES (1, 1), (2, 0)")
        .expect("insert outer");
    c.execute("INSERT INTO inner_t VALUES (1), (2)")
        .expect("insert inner");

    let rows = query_all(
        &c,
        "SELECT a FROM outer_t WHERE marker IN (SELECT marker FROM inner_t WHERE b = a) ORDER BY a",
    );
    assert_eq!(
        rows,
        vec![vec![SqlValue::Integer(1)], vec![SqlValue::Integer(2)]]
    );
}

#[test]
fn null_comparison_is_null() {
    let (_d, c) = open();
    // NULL = NULL → NULL, not 1
    let v = q1(&c, "SELECT NULL = NULL");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn null_is_null_is_true() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT NULL IS NULL");
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn value_is_not_null() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT 1 IS NOT NULL");
    assert_eq!(v, SqlValue::Integer(1));
}

// ── PRAGMA integrity_check ────────────────────────────────────────────────────

#[test]
fn pragma_integrity_check_ok() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    c.execute("INSERT INTO t VALUES (1),(2)").expect("insert");
    let rows = query_all(&c, "PRAGMA integrity_check");
    // Should return at least one row with "ok"
    assert!(!rows.is_empty());
    let first = &rows[0][0];
    let s = match first {
        SqlValue::Text(s) => s.as_ref(),
        _ => "",
    };
    assert_eq!(s.to_lowercase(), "ok", "integrity_check returned: {s}");
}

// ── WAL checkpoint modes ──────────────────────────────────────────────────────

#[test]
fn pragma_wal_checkpoint_passive() {
    let (_d, c) = open();
    assert!(c.prepare("PRAGMA wal_checkpoint(PASSIVE)").is_ok());
}

#[test]
fn pragma_wal_checkpoint_full() {
    let (_d, c) = open();
    assert!(c.prepare("PRAGMA wal_checkpoint(FULL)").is_ok());
}

#[test]
fn pragma_wal_checkpoint_restart() {
    let (_d, c) = open();
    assert!(c.prepare("PRAGMA wal_checkpoint(RESTART)").is_ok());
}

#[test]
fn pragma_wal_checkpoint_truncate() {
    let (_d, c) = open();
    assert!(c.prepare("PRAGMA wal_checkpoint(TRUNCATE)").is_ok());
}

// ── Nested SAVEPOINT ──────────────────────────────────────────────────────────

#[test]
fn nested_savepoint_basic() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("BEGIN").expect("begin");
    c.execute("INSERT INTO t VALUES (1)").expect("ins1");
    c.execute("SAVEPOINT sp1").expect("sp1");
    c.execute("INSERT INTO t VALUES (2)").expect("ins2");
    c.execute("ROLLBACK TO sp1").expect("rollback to sp1");
    // Row 2 should be gone after rollback-to
    let count = q1(&c, "SELECT count(*) FROM t");
    assert_eq!(count, SqlValue::Integer(1));
    c.execute("COMMIT").expect("commit");
}

#[test]
fn nested_savepoint_release() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute("BEGIN").expect("begin");
    c.execute("INSERT INTO t VALUES (10)").expect("ins");
    c.execute("SAVEPOINT sp_a").expect("savepoint");
    c.execute("INSERT INTO t VALUES (20)").expect("ins2");
    c.execute("RELEASE sp_a").expect("release");
    c.execute("COMMIT").expect("commit");
    // Both rows should be committed
    let count = q1(&c, "SELECT count(*) FROM t");
    assert_eq!(count, SqlValue::Integer(2));
}

// ── PRAGMA additional coverage ────────────────────────────────────────────────

#[test]
fn pragma_auto_vacuum() {
    // RedlineDB rejects `PRAGMA auto_vacuum`; this test checks the
    // rejection boundary directly.
    let (_d, c) = open();
    assert!(c.prepare("PRAGMA auto_vacuum").is_err());
}

#[test]
fn pragma_case_sensitive_like_toggles_like_semantics() {
    let (_d, c) = open();
    c.execute("PRAGMA case_sensitive_like=ON")
        .expect("enable case_sensitive_like");
    assert_eq!(q1(&c, "SELECT 'A' LIKE 'a'"), SqlValue::Integer(0));
    c.execute("PRAGMA case_sensitive_like=OFF")
        .expect("disable case_sensitive_like");
    assert_eq!(q1(&c, "SELECT 'A' LIKE 'a'"), SqlValue::Integer(1));
}

#[test]
fn pragma_page_size_reports_database_page_size() {
    let (_d, c) = open();
    let value = q1(&c, "PRAGMA page_size");
    assert!(matches!(value, SqlValue::Integer(v) if v > 0));
}

#[test]
fn pragma_quick_check() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(id INTEGER)").expect("create");
    let rows = query_all(&c, "PRAGMA quick_check");
    assert!(!rows.is_empty());
}

// ── Large integer boundary ────────────────────────────────────────────────────

#[test]
fn i64_max_stores_and_retrieves() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute(&format!("INSERT INTO t VALUES ({})", i64::MAX))
        .expect("insert");
    let v = q1(&c, "SELECT v FROM t");
    assert_eq!(v, SqlValue::Integer(i64::MAX));
}

#[test]
fn i64_min_stores_and_retrieves() {
    let (_d, c) = open();
    c.execute("CREATE TABLE t(v INTEGER)").expect("create");
    c.execute(&format!("INSERT INTO t VALUES ({})", i64::MIN))
        .expect("insert");
    let v = q1(&c, "SELECT v FROM t");
    assert_eq!(v, SqlValue::Integer(i64::MIN));
}

// ── Multi-join chain ──────────────────────────────────────────────────────────

#[test]
fn inner_join_chain() {
    let (_d, c) = open();
    c.execute("CREATE TABLE a(id INTEGER, name TEXT)")
        .expect("create a");
    c.execute("CREATE TABLE b(aid INTEGER, val TEXT)")
        .expect("create b");
    c.execute("CREATE TABLE bb(bid INTEGER, extra TEXT)")
        .expect("create bb");
    c.execute("INSERT INTO a VALUES (1, 'alice')")
        .expect("ins a");
    c.execute("INSERT INTO b VALUES (1, 'beta')")
        .expect("ins b");
    c.execute("INSERT INTO bb VALUES (1, 'extra')")
        .expect("ins bb");
    let rows = query_all(
        &c,
        "SELECT a.name, b.val, bb.extra FROM a JOIN b ON a.id = b.aid JOIN bb ON b.aid = bb.bid",
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Text(Arc::from("alice")));
    assert_eq!(rows[0][1], SqlValue::Text(Arc::from("beta")));
    assert_eq!(rows[0][2], SqlValue::Text(Arc::from("extra")));
}

// ── CREATE TABLE AS SELECT parity ────────────────────────────────────────────

struct CtasLab {
    _dir: tempfile::TempDir,
    redline: Arc<Connection>,
    sqlite: rusqlite::Connection,
}

impl CtasLab {
    fn new() -> Self {
        let dir = tempdir().expect("temp dir");
        let redline_path = dir.path().join("ctas.db");
        let sqlite_path = dir.path().join("ctas.sqlite");
        let db = Database::create(&redline_path, DbOptions::default()).expect("create db");
        Self {
            _dir: dir,
            redline: db.connect(),
            sqlite: rusqlite::Connection::open(&sqlite_path).expect("rusqlite open"),
        }
    }

    fn execute_both(&self, sql: &str) {
        self.sqlite.execute_batch(sql).unwrap_or_else(|e| {
            panic!("sqlite failed for {sql:?}: {e}");
        });
        self.redline.execute(sql).unwrap_or_else(|e| {
            panic!("redline failed for {sql:?}: {e:?}");
        });
    }

    fn query_sqlite(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        let mut stmt = self.sqlite.prepare(sql).expect("sqlite prepare");
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).expect("sqlite query");
        let mut out = Vec::new();
        while let Some(row) = rows.next().expect("sqlite next") {
            let current: Vec<SqlValue> = (0..cols)
                .map(|i| to_sql_value(row.get::<usize, RuValue>(i).expect("sqlite get")))
                .collect();
            out.push(current);
        }
        out
    }

    fn query_redline(&self, sql: &str) -> Vec<Vec<SqlValue>> {
        query_all(&self.redline, sql)
    }

    fn assert_parity(&self, sql: &str) {
        let sqlite = self.query_sqlite(sql);
        let redline = self.query_redline(sql);
        assert_eq!(redline, sqlite, "row mismatch for {sql}");
    }
}

fn to_sql_value(v: RuValue) -> SqlValue {
    match v {
        RuValue::Null => SqlValue::Null,
        RuValue::Integer(i) => SqlValue::Integer(i),
        RuValue::Real(f) => SqlValue::Real(f),
        RuValue::Text(s) => SqlValue::Text(Arc::from(s)),
        RuValue::Blob(b) => SqlValue::Blob(Arc::from(b)),
    }
}

#[test]
fn ctas_basic_data_and_row_order_match_sqlite() {
    let lab = CtasLab::new();
    lab.execute_both("CREATE TABLE src(id INTEGER, label TEXT)");
    lab.execute_both("INSERT INTO src VALUES (2, 'two'), (1, 'one'), (3, 'three')");
    lab.execute_both("CREATE TABLE dst AS SELECT id, label FROM src ORDER BY id DESC");
    lab.assert_parity("SELECT rowid, id, label FROM dst ORDER BY rowid");
}

#[test]
fn ctas_table_info_metadata_matches_sqlite() {
    let lab = CtasLab::new();
    lab.execute_both("CREATE TABLE src(i INTEGER, r REAL, t TEXT, b BLOB)");
    lab.execute_both("INSERT INTO src VALUES (7, 7.5, 'hello', x'0102')");
    lab.execute_both(
        "CREATE TABLE dst AS SELECT i, r, t, b, CAST(i AS TEXT) AS text_i, CAST(i AS INT) AS int_i, CAST(i AS NUMERIC) AS num_i, CAST(i AS REAL) AS real_i FROM src",
    );
    lab.assert_parity("PRAGMA table_info('dst')");
}

#[test]
fn ctas_duplicate_and_aliased_names_match_sqlite() {
    let lab = CtasLab::new();
    lab.execute_both("CREATE TABLE src(a INTEGER, b INTEGER)");
    lab.execute_both("INSERT INTO src VALUES (1, 2)");
    lab.execute_both(
        "CREATE TABLE dup AS SELECT a AS x, b AS X, a, a AS \"1\", b AS \"\" FROM src",
    );
    lab.assert_parity("PRAGMA table_info('dup')");
}

#[test]
fn ctas_if_not_exists_short_circuits_before_source_bind() {
    let lab = CtasLab::new();
    lab.execute_both("CREATE TABLE existing(id INTEGER)");
    lab.execute_both("INSERT INTO existing VALUES (1)");
    lab.execute_both("CREATE TABLE IF NOT EXISTS existing AS SELECT * FROM missing_source");
    lab.assert_parity("SELECT id FROM existing");
}

#[test]
fn ctas_rolls_back_on_source_runtime_error() {
    let lab = CtasLab::new();
    let sql = "CREATE TABLE fail AS SELECT json_extract('not json', '$.')";

    let sqlite_err = lab.sqlite.execute_batch(sql).err();
    assert!(sqlite_err.is_some(), "sqlite unexpectedly accepted {sql:?}");

    let redline_err = lab.redline.execute(sql).err();
    assert!(
        redline_err.is_some(),
        "redline unexpectedly accepted {sql:?}"
    );

    lab.assert_parity("SELECT count(*) FROM sqlite_schema WHERE name = 'fail'");
}

// ── Track A scalar-functions coverage (SQL_MATH / STRING / PATTERN / BLOB) ───
//
// These exercise the SQLite math1 / string / pattern / blob helpers we added
// for the sqlite-parity sweep. Note: the rusqlite oracle is bundled with the
// stock build of SQLite (no `SQLITE_ENABLE_MATH_FUNCTIONS`), so we can't use
// the `CtasLab` parity helper for math1 calls — those are exercised against
// the reference shell via the `sqlite_parity` corpus and unit-tested in
// `parity_scalar_funcs.rs` for closed-form behaviour (NULL semantics, domain
// errors). The tests below cover what the rusqlite oracle DOES expose:
// `length(utf8)`, `octet_length`, `hex(NULL)`, GLOB operator, and `concat*`.

#[test]
fn sqlite_parity_track_a_string_helpers() {
    let lab = CtasLab::new();
    for sql in [
        // UTF-8 char length vs byte length.
        "SELECT length('héllo'), octet_length('héllo')",
        // concat / concat_ws null handling.
        "SELECT concat('a', NULL, 'b'), concat_ws(',', 'a', NULL, 'b')",
        // hex(NULL) returns "" text, not NULL.
        "SELECT hex(NULL), typeof(hex(NULL))",
    ] {
        lab.assert_parity(sql);
    }
}

#[test]
fn sqlite_parity_track_a_glob_operator_rewrite() {
    let lab = CtasLab::new();
    // GLOB operator (rewritten to glob() function call in the parser):
    // wildcard, character class, NOT, NULL propagation.
    for sql in [
        "SELECT 'abc' GLOB 'a*', 'abc' GLOB '*c'",
        "SELECT 'abc' GLOB 'a?c', 'ab' GLOB '???'",
        "SELECT 'abc' GLOB '[abc]bc', 'abc' GLOB '[^abc]bc'",
        "SELECT 'abc' NOT GLOB 'z*', 'abc' NOT GLOB 'a*'",
        "SELECT NULL GLOB 'a*', 'abc' GLOB NULL",
    ] {
        lab.assert_parity(sql);
    }
}

#[test]
fn sqlite_parity_track_a_unhex() {
    let lab = CtasLab::new();
    for sql in [
        "SELECT length(unhex('01ab')), hex(unhex('01ab')), typeof(unhex('01ab'))",
        // Invalid hex returns NULL.
        "SELECT unhex('xyz'), typeof(unhex('xyz'))",
    ] {
        lab.assert_parity(sql);
    }
}
