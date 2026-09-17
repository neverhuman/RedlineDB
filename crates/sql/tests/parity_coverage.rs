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
    // SQLite-parity surface: `PRAGMA auto_vacuum` is accepted and
    // returns the recall-only session bit. Previously we rejected it
    // because the RedlineDB storage engine doesn't track free-page
    // lists; that diverged from the SQLite behaviour callers probe for
    // at connection open time.
    let (_d, c) = open();
    assert_eq!(q1(&c, "PRAGMA auto_vacuum"), SqlValue::Integer(0));
    c.execute("PRAGMA auto_vacuum=NONE").expect("set none");
    assert_eq!(q1(&c, "PRAGMA auto_vacuum"), SqlValue::Integer(0));
    c.execute("PRAGMA auto_vacuum=FULL").expect("set full");
    assert_eq!(q1(&c, "PRAGMA auto_vacuum"), SqlValue::Integer(1));
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
fn sqlite_cast_numeric_returns_numeric_storage_class() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT typeof(CAST(5 AS NUMERIC))"),
        SqlValue::Text(Arc::from("integer"))
    );
    assert_eq!(q1(&c, "SELECT CAST(5 AS NUMERIC)"), SqlValue::Integer(5));
    assert_eq!(
        q1(&c, "SELECT typeof(CAST(3.14 AS NUMERIC))"),
        SqlValue::Text(Arc::from("real"))
    );
    assert_eq!(q1(&c, "SELECT CAST(3.14 AS NUMERIC)"), SqlValue::Real(3.14));
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

// ── JSONB operator and function surface (Track F) ────────────────────────────
//
// These tests cover the Postgres `jsonb` operator and `jsonb_*` function
// shapes RedlineDB now implements on top of its text-stored JSON. The
// reference oracle is the redline-testing beyond-SQLite corpus
// (`BEYOND_JSONB_INDEXING`); each test here pins one of the closed cases
// so a regression surfaces during local `cargo test`.

fn text(s: &str) -> SqlValue {
    SqlValue::Text(Arc::from(s))
}

#[test]
fn jsonb_at_arrow_object_containment() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' @> '{\"a\":1}'"),
        text("t")
    );
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' @> '{\"a\":2}'"),
        text("f")
    );
}

#[test]
fn jsonb_arrow_at_contained_by() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1}' <@ '{\"a\":1,\"b\":2}'"),
        text("t")
    );
}

#[test]
fn jsonb_question_key_exists() {
    let (_d, c) = open();
    assert_eq!(q1(&c, "SELECT '{\"a\":1,\"b\":2}' ? 'a'"), text("t"));
    assert_eq!(q1(&c, "SELECT '{\"a\":1,\"b\":2}' ? 'z'"), text("f"));
}

#[test]
fn jsonb_question_any_exists() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' ?| ARRAY['z','b']"),
        text("t")
    );
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' ?| ARRAY['x','y']"),
        text("f")
    );
}

#[test]
fn jsonb_question_all_exists() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' ?& ARRAY['a','b']"),
        text("t")
    );
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' ?& ARRAY['a','z']"),
        text("f")
    );
}

#[test]
fn jsonb_hash_arrow_path_extract() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":{\"b\":1}}' #> '{a}'"),
        text("{\"b\": 1}")
    );
    assert_eq!(q1(&c, "SELECT '{\"a\":{\"b\":1}}' #>> '{a,b}'"), text("1"));
}

#[test]
fn jsonb_hash_minus_path_delete() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":{\"b\":1,\"c\":2}}' #- '{a,b}'"),
        text("{\"a\": {\"c\": 2}}")
    );
}

#[test]
fn jsonb_concat_object_and_array() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1}' || '{\"b\":2}'"),
        text("{\"a\": 1, \"b\": 2}")
    );
    assert_eq!(q1(&c, "SELECT '[1,2]' || '[3,4]'"), text("[1, 2, 3, 4]"));
}

#[test]
fn jsonb_minus_text_removes_object_key() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT '{\"a\":1,\"b\":2}' - 'a'"),
        text("{\"b\": 2}")
    );
}

#[test]
fn jsonb_set_and_insert_pg_semantics() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT jsonb_set('{\"a\":1,\"b\":2}', '{a}', '99')"),
        text("{\"a\": 99, \"b\": 2}")
    );
    assert_eq!(
        q1(&c, "SELECT jsonb_set('{\"a\":1}', '{b}', '7', true)"),
        text("{\"a\": 1, \"b\": 7}")
    );
    assert_eq!(
        q1(&c, "SELECT jsonb_insert('[1,2,3]', '{1}', '99')"),
        text("[1, 99, 2, 3]")
    );
    assert_eq!(
        q1(&c, "SELECT jsonb_insert('[1,2,3]', '{1}', '99', true)"),
        text("[1, 2, 99, 3]")
    );
}

#[test]
fn jsonb_strip_nulls_drops_nested_nulls() {
    let (_d, c) = open();
    assert_eq!(
        q1(
            &c,
            "SELECT jsonb_strip_nulls('{\"a\":1,\"b\":null,\"c\":{\"d\":null,\"e\":2}}')"
        ),
        text("{\"a\": 1, \"c\": {\"e\": 2}}")
    );
}

#[test]
fn jsonb_pretty_indents_four_spaces() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT jsonb_pretty('{\"a\":1,\"b\":[1,2,3]}')");
    let SqlValue::Text(s) = v else {
        panic!("expected text output");
    };
    assert!(s.contains("    \"a\": 1"), "expected indented `a` row");
    assert!(s.contains("        1,"), "expected nested array indent");
}

#[test]
fn jsonb_path_exists_and_at_at_predicate() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT jsonb_path_exists('{\"a\":{\"b\":1}}', '$.a.b')"),
        text("t")
    );
    assert_eq!(q1(&c, "SELECT '{\"a\":5}' @@ '$.a > 3'"), text("t"));
    assert_eq!(q1(&c, "SELECT '{\"a\":5}' @@ '$.a > 9'"), text("f"));
}

#[test]
fn jsonb_path_query_table_valued() {
    let (_d, c) = open();
    let rows = query_all(
        &c,
        "SELECT * FROM jsonb_path_query('[10,20,30]', '$[*]') ORDER BY jsonb_path_query::text",
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0], text("10"));
    assert_eq!(rows[1][0], text("20"));
    assert_eq!(rows[2][0], text("30"));
}

#[test]
fn jsonb_array_elements_table_valued() {
    let (_d, c) = open();
    let rows = query_all(&c, "SELECT * FROM jsonb_array_elements('[10,20,30]')");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0], text("10"));
    let text_rows = query_all(
        &c,
        "SELECT * FROM jsonb_array_elements_text('[\"x\",\"y\"]') ORDER BY 1",
    );
    assert_eq!(text_rows.len(), 2);
    assert_eq!(text_rows[0][0], text("x"));
    assert_eq!(text_rows[1][0], text("y"));
}

#[test]
fn jsonb_to_record_projects_columns() {
    let (_d, c) = open();
    let rows = query_all(
        &c,
        "SELECT * FROM jsonb_to_record('{\"a\":5,\"b\":\"x\"}') AS t(a int, b text)",
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], text("5"));
    assert_eq!(rows[0][1], text("x"));
}

#[test]
fn jsonb_where_clause_truthy_pg_bool() {
    // PG boolean tokens (`t`/`f`) emitted by @> must be honoured by
    // WHERE / CASE truthiness without breaking SQLite text semantics
    // for unrelated cells.
    let (_d, c) = open();
    c.execute("CREATE TABLE bsp_doc(id INTEGER, doc TEXT)")
        .expect("create");
    c.execute("INSERT INTO bsp_doc VALUES (1, '{\"name\":\"alice\"}'), (2, '{\"name\":\"bob\"}')")
        .expect("insert");
    let rows = query_all(
        &c,
        "SELECT id FROM bsp_doc WHERE doc @> '{\"name\":\"bob\"}' ORDER BY id",
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(2));
}

#[test]
fn create_index_using_gin_is_parseable() {
    let (_d, c) = open();
    c.execute("CREATE TABLE bsp_doc(id INTEGER PRIMARY KEY, doc TEXT)")
        .expect("create");
    c.execute("INSERT INTO bsp_doc VALUES (1, '{\"x\":1}')")
        .expect("insert");
    // `USING GIN` and the `jsonb_path_ops` opclass marker are stripped
    // pre-parse; the index itself is created via the regular btree path.
    c.execute("CREATE INDEX bsp_doc_gin ON bsp_doc USING GIN (doc jsonb_path_ops)")
        .expect("create index using gin");
    let rows = query_all(&c, "SELECT id FROM bsp_doc");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], SqlValue::Integer(1));
}

// ── Track G: BEYOND_COLLATIONS_ILIKE ──────────────────────────────────────────
//
// Coverage for the closable beyond-Postgres parity cases in this category:
// ILIKE rendering, multibyte LOWER/UPPER, the four Postgres POSIX regex
// operators (`~`, `~*`, `!~`, `!~*`), SIMILAR TO, POSITION, and
// octet_length. Cases that the curator deferred (citext, named ICU
// collations, nondeterministic collations) are intentionally not exercised.

#[test]
fn ilike_basic_mixed_case_matches_case_insensitively() {
    let (_d, c) = open();
    // Mirrors BEYOND-CASE-20031.
    assert_eq!(q1(&c, "SELECT 'ABC' ILIKE 'abc'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'ABC' ILIKE 'AbC'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'ABC' ILIKE 'xyz'"), SqlValue::Integer(0));
}

#[test]
fn ilike_supports_wildcards_and_escape_clause() {
    let (_d, c) = open();
    // Wildcard coverage (BEYOND-CASE-20032, 20033).
    assert_eq!(
        q1(&c, "SELECT 'Hello World' ILIKE '%world%'"),
        SqlValue::Integer(1)
    );
    assert_eq!(q1(&c, "SELECT 'ABC' ILIKE 'a_c'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'ABCDE' ILIKE 'a_c'"), SqlValue::Integer(0));
    // ESCAPE clause (BEYOND-CASE-20034) — `!` escapes the literal `%`.
    assert_eq!(
        q1(&c, "SELECT '50%' ILIKE '50!%' ESCAPE '!'"),
        SqlValue::Integer(1)
    );
    assert_eq!(
        q1(&c, "SELECT '50' ILIKE '50!%' ESCAPE '!'"),
        SqlValue::Integer(0)
    );
}

// ---------------------------------------------------------------------------
// Track H — beyond-SQLite (Postgres parity) coverage.
// ---------------------------------------------------------------------------

#[test]
fn pg_date_trunc_month_floors_to_first_of_month() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT date_trunc('month', '2025-04-17 13:25:00')");
    assert_eq!(v, SqlValue::Text(Arc::from("2025-04-01 00:00:00")));
}

#[test]
fn pg_date_trunc_year_floors_to_jan_1() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT date_trunc('year', '2025-04-17 13:25:00')");
    assert_eq!(v, SqlValue::Text(Arc::from("2025-01-01 00:00:00")));
}

#[test]
fn pg_date_trunc_hour_clears_smaller_units() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT date_trunc('hour', '2025-04-17 13:25:42')");
    assert_eq!(v, SqlValue::Text(Arc::from("2025-04-17 13:00:00")));
}

#[test]
fn pg_date_trunc_quarter_floors_to_quarter_start() {
    let (_d, c) = open();
    // April → Q2, which starts in April. 2025-04-17 → 2025-04-01.
    let v = q1(&c, "SELECT date_trunc('quarter', '2025-04-17 13:25:00')");
    assert_eq!(v, SqlValue::Text(Arc::from("2025-04-01 00:00:00")));
    // July → Q3, which starts in July. 2025-07-15 → 2025-07-01.
    let v = q1(&c, "SELECT date_trunc('quarter', '2025-07-15 06:00:00')");
    assert_eq!(v, SqlValue::Text(Arc::from("2025-07-01 00:00:00")));
}

#[test]
fn pg_date_trunc_unknown_field_returns_null() {
    let (_d, c) = open();
    let v = q1(&c, "SELECT date_trunc('nope', '2025-04-17 13:25:00')");
    assert_eq!(v, SqlValue::Null);
}

#[test]
fn pg_gen_random_uuid_renders_36_char_canonical() {
    let (_d, c) = open();
    // Canonical 8-4-4-4-12 with hyphens is exactly 36 chars.
    let v = q1(&c, "SELECT length(gen_random_uuid())");
    assert_eq!(v, SqlValue::Integer(36));
    // Two calls produce distinct outputs (probabilistically certain at 122 bits).
    let v = q1(&c, "SELECT gen_random_uuid() = gen_random_uuid()");
    assert_eq!(v, SqlValue::Integer(0));
}

#[test]
fn pg_gen_random_uuid_has_v4_variant_bits() {
    let (_d, c) = open();
    // Position 15 (1-based) of the canonical string is the version nibble;
    // it must be '4' for a v4 UUID.
    let v = q1(&c, "SELECT substr(gen_random_uuid(), 15, 1)");
    assert_eq!(v, SqlValue::Text(Arc::from("4")));
    // Position 20 (1-based) is the variant nibble; for RFC 4122 it must be
    // one of 8, 9, a, b (binary 10xx). GLOB '[89ab]' returns 1 when so.
    // Parens around the substr call sidestep a parser limitation where
    // `func() GLOB ...` is otherwise rejected by the bundled sqlparser.
    let v = q1(
        &c,
        "SELECT (substr(gen_random_uuid(), 20, 1)) GLOB '[89ab]'",
    );
    assert_eq!(v, SqlValue::Integer(1));
}

#[test]
fn pg_boolean_cast_from_text_aliases() {
    let (_d, c) = open();
    // PG-style truthy / falsy text aliases collapse to Integer(0|1) so
    // SQLite-shape arithmetic and rendering keep working.
    let r = query_all(
        &c,
        "SELECT 'yes'::bool, 'no'::bool, '1'::bool, '0'::bool, 't'::bool, 'f'::bool",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0],
        vec![
            SqlValue::Integer(1),
            SqlValue::Integer(0),
            SqlValue::Integer(1),
            SqlValue::Integer(0),
            SqlValue::Integer(1),
            SqlValue::Integer(0),
        ]
    );
}

#[test]
fn not_ilike_propagates_null() {
    let (_d, c) = open();
    // BEYOND-CASE-20035.
    assert_eq!(q1(&c, "SELECT 'ABC' NOT ILIKE 'abc'"), SqlValue::Integer(0));
    assert_eq!(q1(&c, "SELECT 'XYZ' NOT ILIKE 'abc'"), SqlValue::Integer(1));
    assert!(matches!(
        q1(&c, "SELECT NULL NOT ILIKE 'x'"),
        SqlValue::Null
    ));
}

#[test]
fn ilike_unicode_folds_caf_e_to_caf_e_acute() {
    let (_d, c) = open();
    // BEYOND-CASE-20058: PG's ILIKE applies Unicode folding via libc
    // locale, so `'CAFÉ' ILIKE 'café'` is true. The ASCII-only `cafe`
    // form remains false because É and e are different chars.
    assert_eq!(q1(&c, "SELECT 'CAFÉ' ILIKE 'café'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'CAFÉ' ILIKE 'cafe'"), SqlValue::Integer(0));
}

#[test]
fn pg_uuid_cast_normalises_to_canonical_lowercase() {
    let (_d, c) = open();
    let r = q1(&c, "SELECT '00000000000000000000000000000001'::uuid");
    assert_eq!(
        r,
        SqlValue::Text(Arc::from("00000000-0000-0000-0000-000000000001"))
    );
    let r = q1(&c, "SELECT '{ABCDEF01-2345-6789-ABCD-EF0123456789}'::uuid");
    assert_eq!(
        r,
        SqlValue::Text(Arc::from("abcdef01-2345-6789-abcd-ef0123456789"))
    );
}

#[test]
fn like_is_case_sensitive_with_pragma() {
    let (_d, c) = open();
    // BEYOND-CASE-20059: With `case_sensitive_like = ON`, LIKE diverges
    // from ILIKE on mixed-case input. The beyond-PG oracle enables the
    // pragma in its preamble so redlinedb's LIKE matches Postgres'
    // SQL-standard semantics.
    c.execute("PRAGMA case_sensitive_like = 1").expect("pragma");
    assert_eq!(q1(&c, "SELECT 'AbCdEf' LIKE '%cd%'"), SqlValue::Integer(0));
    assert_eq!(q1(&c, "SELECT 'AbCdEf' ILIKE '%cd%'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'AbCdEf' LIKE '%CD%'"), SqlValue::Integer(0));
}

#[test]
fn pg_array_literal_rewrites_to_json_array() {
    let (_d, c) = open();
    let r = q1(&c, "SELECT ARRAY[10,20,30]");
    assert_eq!(r, SqlValue::Text(Arc::from("[10,20,30]")));
}

#[test]
fn pg_array_length_rewrites_to_json_array_length() {
    let (_d, c) = open();
    let r = q1(&c, "SELECT array_length(ARRAY[10,20,30], 1)");
    assert_eq!(r, SqlValue::Integer(3));
}

#[test]
fn pg_array_index_is_one_based() {
    let (_d, c) = open();
    let r = query_all(
        &c,
        "SELECT (ARRAY['a','b','c'])[1], (ARRAY['a','b','c'])[3]",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0],
        vec![
            SqlValue::Text(Arc::from("a")),
            SqlValue::Text(Arc::from("c")),
        ]
    );
}

#[test]
fn lower_upper_are_ascii_only() {
    let (_d, c) = open();
    assert_eq!(
        q1(&c, "SELECT lower('Hello')"),
        SqlValue::Text(Arc::from("hello"))
    );
    assert_eq!(
        q1(&c, "SELECT lower('Ｈｅｌｌｏ')"),
        SqlValue::Text(Arc::from("Ｈｅｌｌｏ"))
    );
    assert_eq!(
        q1(&c, "SELECT lower('ÉCOLE')"),
        SqlValue::Text(Arc::from("École"))
    );
    assert_eq!(
        q1(&c, "SELECT upper('Hello')"),
        SqlValue::Text(Arc::from("HELLO"))
    );
    assert_eq!(
        q1(&c, "SELECT upper('Ｈｅｌｌｏ')"),
        SqlValue::Text(Arc::from("Ｈｅｌｌｏ"))
    );
}

#[test]
fn upper_leaves_non_ascii_codepoints() {
    let (_d, c) = open();
    // SQLite `upper()` folds ASCII letters only, leaving non-ASCII code
    // points untouched. `ß` and `σ` stay as-is.
    assert_eq!(
        q1(&c, "SELECT upper('straße')"),
        SqlValue::Text(Arc::from("STRAßE"))
    );
}

#[test]
fn pg_regex_match_operators_dispatch() {
    let (_d, c) = open();
    // BEYOND-CASE-20051: case-sensitive `~`.
    assert_eq!(q1(&c, "SELECT 'Hello' ~ 'ello'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'Hello' ~ 'ELLO'"), SqlValue::Integer(0));
    assert_eq!(q1(&c, "SELECT 'Hello' ~ '^H'"), SqlValue::Integer(1));
    // BEYOND-CASE-20052: case-insensitive `~*`.
    assert_eq!(q1(&c, "SELECT 'Hello' ~* 'ELLO'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'Hello' !~* 'XYZ'"), SqlValue::Integer(1));
    // BEYOND-CASE-20053: negated forms.
    assert_eq!(q1(&c, "SELECT 'Hello' !~ 'XYZ'"), SqlValue::Integer(1));
    assert_eq!(q1(&c, "SELECT 'Hello' !~* 'ELLO'"), SqlValue::Integer(0));
    assert_eq!(q1(&c, "SELECT 'Hello' !~ '^H'"), SqlValue::Integer(0));
}

#[test]
fn pg_regex_match_propagates_null() {
    let (_d, c) = open();
    assert!(matches!(q1(&c, "SELECT NULL ~ 'abc'"), SqlValue::Null));
    assert!(matches!(q1(&c, "SELECT 'abc' ~ NULL"), SqlValue::Null));
}

#[test]
fn similar_to_basic_anchored_matches() {
    let (_d, c) = open();
    // BEYOND-CASE-20054.
    assert_eq!(q1(&c, "SELECT 'abc' SIMILAR TO 'a%'"), SqlValue::Integer(1));
    assert_eq!(
        q1(&c, "SELECT 'abc' SIMILAR TO '(a|b)bc'"),
        SqlValue::Integer(1)
    );
    assert_eq!(
        q1(&c, "SELECT 'abc' SIMILAR TO 'a_c'"),
        SqlValue::Integer(1)
    );
}

#[test]
fn similar_to_character_classes_pass_through() {
    let (_d, c) = open();
    // BEYOND-CASE-20055: `[a-z]+` is a POSIX class; SIMILAR TO uses the
    // same bracket syntax so it passes through to the regex engine.
    assert_eq!(
        q1(&c, "SELECT 'abc1' SIMILAR TO '[a-z]+[0-9]+'"),
        SqlValue::Integer(1)
    );
    assert_eq!(
        q1(&c, "SELECT 'ABC' SIMILAR TO '[a-z]+'"),
        SqlValue::Integer(0)
    );
    assert_eq!(
        q1(&c, "SELECT 'ABC' SIMILAR TO '[A-Z]+'"),
        SqlValue::Integer(1)
    );
}

#[test]
fn similar_to_null_propagation_and_anchoring() {
    let (_d, c) = open();
    assert!(matches!(
        q1(&c, "SELECT NULL SIMILAR TO 'a%'"),
        SqlValue::Null
    ));
    assert!(matches!(
        q1(&c, "SELECT 'abc' SIMILAR TO NULL"),
        SqlValue::Null
    ));
    // Anchored at both ends — partial matches must NOT pass.
    assert_eq!(
        q1(&c, "SELECT 'abcdef' SIMILAR TO 'abc'"),
        SqlValue::Integer(0)
    );
    assert_eq!(
        q1(&c, "SELECT 'abcdef' SIMILAR TO 'abc%'"),
        SqlValue::Integer(1)
    );
}

#[test]
fn position_returns_one_indexed_char_offset() {
    let (_d, c) = open();
    // BEYOND-CASE-20056: char-indexed POSITION matches Postgres on
    // multibyte input. `'é'` is one Unicode char (two UTF-8 bytes); the
    // expected index is 4 in `'café'` (after c, a, f).
    assert_eq!(
        q1(&c, "SELECT position('é' IN 'café')"),
        SqlValue::Integer(4)
    );
    assert_eq!(
        q1(&c, "SELECT position('e' IN 'école')"),
        SqlValue::Integer(5)
    );
    // Empty needle returns 1 (PG / SQLite convention).
    assert_eq!(q1(&c, "SELECT position('' IN 'abc')"), SqlValue::Integer(1));
    // Missing needle returns 0.
    assert_eq!(
        q1(&c, "SELECT position('z' IN 'abc')"),
        SqlValue::Integer(0)
    );
    // NULL propagation.
    assert!(matches!(
        q1(&c, "SELECT position(NULL IN 'abc')"),
        SqlValue::Null
    ));
    assert!(matches!(
        q1(&c, "SELECT position('a' IN NULL)"),
        SqlValue::Null
    ));
}

#[test]
fn length_vs_octet_length_disagree_on_multibyte() {
    let (_d, c) = open();
    // BEYOND-CASE-20057.
    assert_eq!(q1(&c, "SELECT length('café')"), SqlValue::Integer(4));
    assert_eq!(q1(&c, "SELECT octet_length('café')"), SqlValue::Integer(5));
    assert_eq!(q1(&c, "SELECT length('αβγ')"), SqlValue::Integer(3));
    assert_eq!(q1(&c, "SELECT octet_length('αβγ')"), SqlValue::Integer(6));
}

#[test]
fn pg_array_overlap_returns_zero_or_one() {
    let (_d, c) = open();
    let r = query_all(
        &c,
        "SELECT ARRAY[1,2,3] && ARRAY[3,4,5], ARRAY[1,2] && ARRAY[10,11]",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0], vec![SqlValue::Integer(1), SqlValue::Integer(0)]);
}

#[test]
fn pg_decimal_arith_preserves_precision() {
    let (_d, c) = open();
    // The classic `0.1 + 0.2 = 0.3` torture test — needs TEXT-shaped
    // decimal arithmetic to avoid the f64 rounding to 0.30000000000000004.
    let r = q1(&c, "SELECT 0.1::numeric + 0.2::numeric");
    assert_eq!(r, SqlValue::Text(Arc::from("0.3")));
    // Equality between the sum and the literal `0.3::numeric`.
    let r = q1(&c, "SELECT 0.1::numeric + 0.2::numeric = 0.3::numeric");
    assert_eq!(r, SqlValue::Integer(1));
    // Multiplication keeps precision and the explicit (10,2) cast pads
    // the result to exactly two fractional digits.
    let r = q1(&c, "SELECT (1.5::numeric * 3)::numeric(10,2)");
    assert_eq!(r, SqlValue::Text(Arc::from("4.50")));
}

#[test]
fn pg_decimal_division_renders_16_fractional_digits() {
    let (_d, c) = open();
    let r = q1(&c, "SELECT 10::numeric / 3::numeric");
    assert_eq!(r, SqlValue::Text(Arc::from("3.3333333333333333")));
}

#[test]
fn pg_interval_add_to_date_via_modifier() {
    let (_d, c) = open();
    let r = q1(&c, "SELECT '2025-01-01'::date + INTERVAL '5 days'");
    assert_eq!(r, SqlValue::Text(Arc::from("2025-01-06 00:00:00")));
}

#[test]
fn pg_timestamptz_at_time_zone_is_tz_naive() {
    let (_d, c) = open();
    let r = q1(
        &c,
        "SELECT '2025-01-15 12:00:00+00'::timestamptz AT TIME ZONE 'UTC'",
    );
    assert_eq!(r, SqlValue::Text(Arc::from("2025-01-15 12:00:00")));
}

#[test]
fn pg_array_agg_with_order_by_keeps_ordering() {
    let (_d, c) = open();
    // array_agg(x ORDER BY x DESC) over (3,2,1)/(2,1,3) shapes — the
    // rewriter swaps the function name and preserves the in-aggregate
    // ORDER BY so the result is `[3,2,1]`.
    let r = q1(
        &c,
        "WITH v(x) AS (VALUES (1),(2),(3)) SELECT array_agg(x ORDER BY x DESC) FROM v",
    );
    assert_eq!(r, SqlValue::Text(Arc::from("[3,2,1]")));
}
