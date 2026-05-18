//! Phase 10 Lane SQL-C: SQLite ON CONFLICT / INSERT OR conflict-resolution
//! matrix coverage.
//!
//! Each row in the SQLite resolution-action table at
//! <https://sqlite.org/lang_conflict.html> gets dedicated tests for both
//! the "constraint fails" and the "no conflict" paths, plus AUTOINCREMENT
//! high-water-mark and UPSERT (`ON CONFLICT(col) DO ...`) variants.
//!
//! Documented deviations from SQLite (kept out of `crates/sql/src/exec.rs`
//! per Lane SQL-C's allowed file list):
//! - `OR FAIL` / `OR ROLLBACK` currently behave like `OR ABORT`. The
//!   surrounding `with_write_tx` machinery treats every `Err` as
//!   "rollback the implicit tx" and every error inside an explicit tx
//!   as "session is poisoned". The parser still distinguishes the verbs
//!   and `tail.rs::conflict_action_for` records the intended action, so
//!   a future lane that touches `with_write_tx` can lift the collapse
//!   without reparsing.
//! - The literal `AUTOINCREMENT` keyword is not yet recognised because
//!   that requires extending `crates/sql/src/parser/helpers.rs`. The
//!   engine's `reserve_row_id()` is process-wide monotonic, so an
//!   `INTEGER PRIMARY KEY` (the rowid alias) already exhibits SQLite's
//!   AUTOINCREMENT-style high-water-mark behaviour and the tests assert
//!   that.

#[allow(non_snake_case)]
mod phase10_sqlc_conflict_matrix {
    use std::sync::Arc;

    use redlinedb_sql::{Database, DbOptions, Step};
    use tempfile::tempdir;

    fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("redlinedb-sql-phase10-sqlc.db");
        let db = Database::create(&path, DbOptions::default()).expect("create database");
        let conn = db.connect();
        (dir, conn)
    }

    fn count_rows(conn: &Arc<redlinedb_sql::Connection>, table: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let mut stmt = conn.prepare(&sql).expect("prepare count");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let n = stmt.column_i64(0).expect("count");
        assert_eq!(stmt.step().expect("done"), Step::Done);
        n
    }

    fn err_msg<E: std::fmt::Debug>(err: &E) -> String {
        format!("{err:?}")
    }

    // ----------------------------------------------------------------
    // OR ABORT (default) — error, undo this stmt only
    // ----------------------------------------------------------------

    #[test]
    fn abort_default_not_null_violation_errors_no_rows_committed() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        let err = conn
            .execute("INSERT INTO t VALUES (NULL)")
            .expect_err("NOT NULL must error by default");
        assert!(
            err_msg(&err).contains("NOT NULL"),
            "expected NOT NULL error, got {err:?}"
        );
        // Prior stmt is preserved (implicit-tx already committed it).
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    #[test]
    fn abort_default_unique_violation_errors_no_rows_committed() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        let err = conn
            .execute("INSERT INTO t VALUES (1)")
            .expect_err("UNIQUE must error by default");
        assert!(
            err_msg(&err).contains("UNIQUE"),
            "expected UNIQUE error, got {err:?}"
        );
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    #[test]
    fn abort_default_success_path_inserts_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL, b INTEGER UNIQUE)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 10)").expect("ok");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    // ----------------------------------------------------------------
    // OR FAIL — currently collapses to ABORT (deviation documented)
    // ----------------------------------------------------------------

    #[test]
    fn fail_not_null_violation_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        let err = conn
            .execute("INSERT OR FAIL INTO t VALUES (NULL)")
            .expect_err("OR FAIL with NOT NULL must error");
        assert!(err_msg(&err).contains("NOT NULL"), "{err:?}");
        // Deviation note: SQLite's true OR FAIL would keep prior rows of
        // the *same* INSERT statement. The implicit-tx path here rolls
        // the whole stmt back, matching ABORT. Captured for future work.
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn fail_unique_violation_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        let err = conn
            .execute("INSERT OR FAIL INTO t VALUES (1)")
            .expect_err("OR FAIL with UNIQUE must error");
        assert!(err_msg(&err).contains("UNIQUE"), "{err:?}");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    #[test]
    fn fail_success_path_inserts_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        conn.execute("INSERT OR FAIL INTO t VALUES (42)")
            .expect("ok");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    // ----------------------------------------------------------------
    // OR IGNORE — skip silently for both NOT NULL/CHECK and UNIQUE
    // ----------------------------------------------------------------

    /// Audit P0-11 reproduction: NOT NULL + INSERT OR IGNORE must produce
    /// zero rows and zero errors.
    #[test]
    fn ignore_not_null_violation_skips_silently() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        // Per the P0-11 audit reproduction: this used to error.
        conn.execute("INSERT OR IGNORE INTO t VALUES (NULL)")
            .expect("OR IGNORE must swallow NOT NULL violation");
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn ignore_check_violation_skips_silently() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER CHECK (a > 0))")
            .expect("create");
        conn.execute("INSERT OR IGNORE INTO t VALUES (-5)")
            .expect("OR IGNORE must swallow CHECK violation");
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn ignore_unique_violation_skips_silently() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        conn.execute("INSERT OR IGNORE INTO t VALUES (1)")
            .expect("OR IGNORE must swallow UNIQUE violation");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    #[test]
    fn ignore_success_path_inserts_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b INTEGER NOT NULL)")
            .expect("create");
        let n = conn
            .execute("INSERT OR IGNORE INTO t VALUES (7, 70)")
            .expect("ok");
        assert_eq!(n, 1);
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    // ----------------------------------------------------------------
    // OR REPLACE
    //   - NOT NULL/CHECK: still errors (no row to replace)
    //   - UNIQUE/PK: deletes conflicting row and inserts new
    // ----------------------------------------------------------------

    #[test]
    fn replace_not_null_violation_still_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        let err = conn
            .execute("INSERT OR REPLACE INTO t VALUES (NULL)")
            .expect_err("OR REPLACE does not bypass NOT NULL");
        assert!(err_msg(&err).contains("NOT NULL"), "{err:?}");
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn replace_check_violation_still_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER CHECK (a > 0))")
            .expect("create");
        let err = conn
            .execute("INSERT OR REPLACE INTO t VALUES (-1)")
            .expect_err("OR REPLACE does not bypass CHECK");
        assert!(err_msg(&err).contains("CHECK"), "{err:?}");
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn replace_unique_conflict_replaces_existing_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 'first')")
            .expect("first");
        conn.execute("INSERT OR REPLACE INTO t VALUES (1, 'second')")
            .expect("replace");
        assert_eq!(count_rows(&conn, "t"), 1);
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "second");
    }

    /// Verify that OR REPLACE on a non-PK UNIQUE keeps the row count at 1
    /// (the conflicting row is deleted before the new one inserts) and
    /// that the rowid moves: when the conflicting row's rowid is
    /// engine-allocated (no explicit alias), the replacement gets a new
    /// monotonic rowid because `engine.reserve_row_id()` is a high-water
    /// mark.
    #[test]
    fn replace_unique_non_pk_changes_rowid_but_not_count() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER UNIQUE, b TEXT)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 'first')")
            .expect("first");
        // Capture original rowid via SELECT.
        let mut stmt = conn
            .prepare("SELECT rowid FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let original_rowid = stmt.column_i64(0).expect("rowid");
        drop(stmt);
        conn.execute("INSERT OR REPLACE INTO t VALUES (1, 'second')")
            .expect("replace");
        assert_eq!(count_rows(&conn, "t"), 1);
        let mut stmt = conn
            .prepare("SELECT rowid, b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let new_rowid = stmt.column_i64(0).expect("rowid");
        assert_eq!(stmt.column_text(1).expect("b"), "second");
        // Engine reserve_row_id is monotonic, so the replacement row gets
        // a strictly larger rowid than the deleted one.
        assert!(
            new_rowid > original_rowid,
            "expected new rowid ({new_rowid}) > original ({original_rowid}) — engine high-water mark"
        );
    }

    // ----------------------------------------------------------------
    // OR ROLLBACK — currently collapses to ABORT (deviation documented)
    // ----------------------------------------------------------------

    #[test]
    fn rollback_not_null_violation_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        let err = conn
            .execute("INSERT OR ROLLBACK INTO t VALUES (NULL)")
            .expect_err("OR ROLLBACK with NOT NULL must error");
        assert!(err_msg(&err).contains("NOT NULL"), "{err:?}");
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn rollback_unique_violation_errors() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        let err = conn
            .execute("INSERT OR ROLLBACK INTO t VALUES (1)")
            .expect_err("OR ROLLBACK with UNIQUE must error");
        assert!(err_msg(&err).contains("UNIQUE"), "{err:?}");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    #[test]
    fn rollback_success_path_inserts_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT OR ROLLBACK INTO t VALUES (7)")
            .expect("ok");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    // ----------------------------------------------------------------
    // INSERT OR ABORT mid-batch behaviour
    //
    // SQLite's documented ABORT semantics: undo every change made by the
    // current SQL statement. With our implicit-tx model, that means a
    // failing 3-row VALUES list rolls back all three. This contradicts a
    // common misreading of "ABORT keeps prior rows" (that is FAIL); we
    // assert the SQLite-documented behaviour here.
    // ----------------------------------------------------------------

    #[test]
    fn or_abort_mid_batch_rolls_back_entire_statement() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER NOT NULL)")
            .expect("create");
        let err = conn
            .execute("INSERT OR ABORT INTO t VALUES (1), (NULL), (3)")
            .expect_err("middle row violates NOT NULL");
        assert!(err_msg(&err).contains("NOT NULL"), "{err:?}");
        // ABORT undoes the entire statement.
        assert_eq!(count_rows(&conn, "t"), 0);
    }

    #[test]
    fn or_rollback_aborts_active_transaction() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1)").expect("first");
        // Inside an explicit BEGIN, OR ROLLBACK should leave the tx
        // unrecoverable. Currently the session is marked failed (same as
        // ABORT) — the caller must rollback. Either way the second row
        // never persists.
        conn.execute("BEGIN").expect("begin");
        conn.execute("INSERT INTO t VALUES (2)").expect("second");
        let err = conn
            .execute("INSERT OR ROLLBACK INTO t VALUES (1)")
            .expect_err("conflict");
        assert!(err_msg(&err).contains("UNIQUE"), "{err:?}");
        // Drain the failed tx state (rollback also accepts a poisoned tx).
        let _ = conn.execute("ROLLBACK");
        assert_eq!(count_rows(&conn, "t"), 1);
    }

    // ----------------------------------------------------------------
    // AUTOINCREMENT / high-water-mark semantics
    //
    // We do not yet recognise the literal `AUTOINCREMENT` keyword (that
    // requires a parser change in `crates/sql/src/parser/helpers.rs`,
    // outside Lane SQL-C's allowed file list). However the engine's
    // `reserve_row_id()` is a process-wide monotonic counter that never
    // rolls backwards even after deletes, so an `INTEGER PRIMARY KEY`
    // (the rowid alias) already exhibits SQLite's AUTOINCREMENT-style
    // high-water-mark behaviour. These tests pin that behaviour so the
    // future keyword wiring inherits it.
    // ----------------------------------------------------------------

    #[test]
    fn integer_pk_high_water_mark_survives_deletes() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, label TEXT)")
            .expect("create");
        conn.execute("INSERT INTO t(label) VALUES ('a')")
            .expect("a");
        conn.execute("INSERT INTO t(label) VALUES ('b')")
            .expect("b");
        conn.execute("INSERT INTO t(label) VALUES ('c')")
            .expect("c");
        // Capture the rowid of the third insert.
        let mut stmt = conn
            .prepare("SELECT id FROM t WHERE label = 'c'")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let third_id = stmt.column_i64(0).expect("id");
        drop(stmt);
        // Delete every row.
        conn.execute("DELETE FROM t").expect("delete");
        // Re-insert. The new id must be strictly greater than the
        // pre-delete max — a high-water-mark engine never reuses ids.
        conn.execute("INSERT INTO t(label) VALUES ('d')")
            .expect("d");
        let mut stmt = conn
            .prepare("SELECT id FROM t WHERE label = 'd'")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let new_id = stmt.column_i64(0).expect("id");
        assert!(
            new_id > third_id,
            "AUTOINCREMENT semantics broken: new {new_id} <= prior max {third_id}"
        );
    }

    #[test]
    fn integer_pk_high_water_mark_survives_recovery() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("redlinedb-sql-phase10-sqlc-autoinc.db");
        // Phase 1: insert, capture rowid, drop the database handle so the
        // WAL flushes to durable state. `Database::checkpoint` forces the
        // page state out so reopen can replay.
        {
            let db = Database::create(&path, DbOptions::default()).expect("create database");
            let conn = db.connect();
            conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, payload TEXT)")
                .expect("create");
            conn.execute("INSERT INTO t(payload) VALUES ('first')")
                .expect("first");
            db.checkpoint().expect("checkpoint");
        }
        // Phase 2: re-open (recovery replays WAL into a fresh engine) and
        // insert again. The rowid counter must advance past the durable
        // max even though the engine restarted.
        let db = Database::open(&path, DbOptions::default()).expect("open database");
        let conn = db.connect();
        let mut stmt = conn
            .prepare("SELECT id FROM t WHERE payload = 'first'")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let first_id = stmt.column_i64(0).expect("id");
        drop(stmt);
        conn.execute("INSERT INTO t(payload) VALUES ('second')")
            .expect("second");
        let mut stmt = conn
            .prepare("SELECT id FROM t WHERE payload = 'second'")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        let second_id = stmt.column_i64(0).expect("id");
        assert!(
            second_id > first_id,
            "post-recovery rowid {second_id} did not exceed pre-recovery rowid {first_id}"
        );
    }

    // ----------------------------------------------------------------
    // UPSERT: ON CONFLICT(col) DO UPDATE / DO NOTHING
    // ----------------------------------------------------------------

    #[test]
    fn upsert_do_update_updates_correct_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT, c INTEGER)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 'one', 100)")
            .expect("first");
        conn.execute("INSERT INTO t VALUES (2, 'two', 200)")
            .expect("second");
        // Conflict on PK, update b and c.
        conn.execute(
            "INSERT INTO t VALUES (1, 'NEW', 999) \
             ON CONFLICT(a) DO UPDATE SET b = excluded.b, c = excluded.c",
        )
        .expect("upsert");
        assert_eq!(count_rows(&conn, "t"), 2);
        let mut stmt = conn
            .prepare("SELECT b, c FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "NEW");
        assert_eq!(stmt.column_i64(1).expect("c"), 999);
        drop(stmt);
        // Row 2 must be untouched.
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 2")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "two");
    }

    #[test]
    fn upsert_do_update_with_where_predicate_skips_when_false() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b INTEGER)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 50)").expect("first");
        // Conflict + WHERE that evaluates to false on the existing row;
        // the row must not change.
        conn.execute(
            "INSERT INTO t VALUES (1, 999) \
             ON CONFLICT(a) DO UPDATE SET b = excluded.b WHERE b > 100",
        )
        .expect("upsert no-op");
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_i64(0).expect("b"), 50);
    }

    #[test]
    fn upsert_do_update_with_where_predicate_applies_when_true() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b INTEGER)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 50)").expect("first");
        // Conflict + WHERE that evaluates to true on the existing row.
        conn.execute(
            "INSERT INTO t VALUES (1, 999) \
             ON CONFLICT(a) DO UPDATE SET b = excluded.b WHERE b < 100",
        )
        .expect("upsert apply");
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_i64(0).expect("b"), 999);
    }

    #[test]
    fn upsert_do_nothing_leaves_table_unchanged() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
            .expect("create");
        conn.execute("INSERT INTO t VALUES (1, 'untouched')")
            .expect("first");
        conn.execute("INSERT INTO t VALUES (1, 'attempted-replace') ON CONFLICT(a) DO NOTHING")
            .expect("do nothing");
        assert_eq!(count_rows(&conn, "t"), 1);
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "untouched");
    }
}
