//! Smoke tests for the SQLite-compatible API surface. The asserts cover
//! lifecycle, prepare-tail semantics, multi-statement exec, savepoints,
//! and ownership of the heap-allocated `errmsg` round-trip.

use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sqlite3_api::*;
use crate::types::*;

fn temp_path(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    let unique = format!(
        "redlinedb-ffi-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    path.push(unique);
    path
}

#[test]
fn sqlite3_open_v2_requires_create_for_missing_path() {
    let path = temp_path("open-v2");
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join("missing.redline");
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut sqlite3 = ptr::null_mut();

    let rc = sqlite3_open_v2(c_path.as_ptr(), &mut db, SQLITE_OPEN_READWRITE, ptr::null());

    assert_eq!(rc, RLDB_CANTOPEN);
    assert!(db.is_null());
}

#[test]
fn sqlite3_surface_executes_bind_and_query() {
    let path = temp_path("surface");
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join("surface.redline");
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut sqlite3 = ptr::null_mut();
    assert_eq!(sqlite3_open(c_path.as_ptr(), &mut db), RLDB_OK);
    assert!(!db.is_null());
    let main = CString::new("main").unwrap();
    let filename = unsafe { CStr::from_ptr(sqlite3_db_filename(db, main.as_ptr())) };
    assert_eq!(
        filename.to_str().expect("utf8"),
        db_path.to_str().expect("utf8")
    );
    assert_eq!(sqlite3_db_readonly(db, main.as_ptr()), 0);

    let create = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)").unwrap();
    assert_eq!(
        sqlite3_exec(db, create.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    let insert = CString::new("INSERT INTO t VALUES(?, ?)").unwrap();
    let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
    assert_eq!(
        sqlite3_prepare_v2(db, insert.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert_eq!(sqlite3_bind_int64(stmt, 1, 7), RLDB_OK);
    let value = CString::new("seven").unwrap();
    assert_eq!(
        sqlite3_bind_text(stmt, 2, value.as_ptr(), -1, None),
        RLDB_OK
    );
    assert_eq!(sqlite3_step(stmt), RLDB_DONE);
    assert_eq!(sqlite3_finalize(stmt), RLDB_OK);
    assert!(sqlite3_changes(db) >= 1);
    assert!(sqlite3_total_changes(db) >= 1);
    assert!(sqlite3_total_changes64(db) >= 1);

    let select = CString::new("SELECT v FROM t WHERE id = 7").unwrap();
    assert_eq!(
        sqlite3_prepare_v2(db, select.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    let sql_text = unsafe { CStr::from_ptr(sqlite3_sql(stmt)) };
    assert_eq!(
        sql_text.to_str().expect("utf8"),
        "SELECT v FROM t WHERE id = 7"
    );
    assert_eq!(sqlite3_stmt_readonly(stmt), 1);
    assert_eq!(sqlite3_stmt_busy(stmt), 0);
    assert_eq!(sqlite3_step(stmt), RLDB_ROW);
    assert_eq!(sqlite3_stmt_busy(stmt), 1);
    let text_ptr = sqlite3_column_text(stmt, 0);
    assert!(!text_ptr.is_null());
    let text = unsafe { CStr::from_ptr(text_ptr as *const c_char) };
    assert_eq!(text.to_str().expect("utf8"), "seven");
    assert_eq!(sqlite3_column_type(stmt, 0), RLDB_TEXT);
    assert_eq!(sqlite3_finalize(stmt), RLDB_OK);

    assert_eq!(sqlite3_busy_timeout(db, 50), RLDB_OK);
    assert_eq!(sqlite3_errcode(db), RLDB_OK);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_metadata_helpers_return_values() {
    let version = unsafe { CStr::from_ptr(sqlite3_libversion()) };
    let sourceid = unsafe { CStr::from_ptr(sqlite3_sourceid()) };

    assert!(!version.to_bytes().is_empty());
    assert!(!sourceid.to_bytes().is_empty());
    assert!(sqlite3_libversion_number() > 0);
    assert_eq!(sqlite3_threadsafe(), 1);
    assert!(
        !unsafe { CStr::from_ptr(sqlite3_errstr(RLDB_BUSY)) }
            .to_bytes()
            .is_empty()
    );
}

#[test]
fn sqlite3_prepare_v3_is_compatible_with_prepare_v2() {
    let path = temp_path("prepare-v3");
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join("prepare-v3.redline");
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut sqlite3 = ptr::null_mut();
    assert_eq!(sqlite3_open(c_path.as_ptr(), &mut db), RLDB_OK);

    let sql = CString::new("SELECT 1").unwrap();
    let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
    assert_eq!(
        sqlite3_prepare_v3(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut(), 0),
        RLDB_OK
    );
    assert_eq!(sqlite3_db_handle(stmt), db);
    let sql_text = unsafe { CStr::from_ptr(sqlite3_sql(stmt)) };
    assert_eq!(sql_text.to_str().expect("utf8"), "SELECT 1");
    assert_eq!(sqlite3_stmt_readonly(stmt), 1);
    assert_eq!(sqlite3_stmt_busy(stmt), 0);
    assert_eq!(sqlite3_step(stmt), RLDB_ROW);
    assert_eq!(sqlite3_stmt_busy(stmt), 1);
    assert_eq!(sqlite3_finalize(stmt), RLDB_OK);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_autocommit_tracks_transaction_state() {
    let path = temp_path("autocommit");
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join("autocommit.redline");
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut sqlite3 = ptr::null_mut();
    assert_eq!(sqlite3_open(c_path.as_ptr(), &mut db), RLDB_OK);
    assert_eq!(sqlite3_get_autocommit(db), 1);

    let begin = CString::new("BEGIN").unwrap();
    assert_eq!(
        sqlite3_exec(db, begin.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    assert_eq!(sqlite3_get_autocommit(db), 0);

    let rollback = CString::new("ROLLBACK").unwrap();
    assert_eq!(
        sqlite3_exec(
            db,
            rollback.as_ptr(),
            None,
            ptr::null_mut(),
            ptr::null_mut()
        ),
        RLDB_OK
    );
    assert_eq!(sqlite3_get_autocommit(db), 1);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

fn open_test_db(name: &str) -> *mut sqlite3 {
    let path = temp_path(name);
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join(format!("{name}.redline"));
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut sqlite3 = ptr::null_mut();
    assert_eq!(sqlite3_open(c_path.as_ptr(), &mut db), RLDB_OK);
    db
}

extern "C" fn count_callback(
    ctx: *mut c_void,
    _n: c_int,
    _argv: *mut *mut c_char,
    _names: *mut *mut c_char,
) -> c_int {
    unsafe {
        let counter = ctx as *mut usize;
        *counter += 1;
    }
    0
}

#[test]
fn sqlite3_exec_runs_multiple_statements() {
    let db = open_test_db("multi-exec");
    let sql = CString::new(
        "CREATE TABLE t(id INTEGER PRIMARY KEY); \
         INSERT INTO t VALUES(1); \
         INSERT INTO t VALUES(2); \
         INSERT INTO t VALUES(3);",
    )
    .unwrap();
    let mut errmsg: *mut c_char = ptr::null_mut();
    let rc = sqlite3_exec(db, sql.as_ptr(), None, ptr::null_mut(), &mut errmsg);
    assert_eq!(rc, RLDB_OK);
    assert!(errmsg.is_null(), "no errmsg expected on success");

    // Now run a SELECT and verify the callback fires once per row.
    let mut counter: usize = 0;
    let sel = CString::new("SELECT id FROM t").unwrap();
    let rc = sqlite3_exec(
        db,
        sel.as_ptr(),
        Some(count_callback),
        (&mut counter) as *mut usize as *mut c_void,
        ptr::null_mut(),
    );
    assert_eq!(rc, RLDB_OK);
    assert_eq!(counter, 3);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_exec_callback_fires_for_each_row_in_multi_stmt() {
    let db = open_test_db("multi-cb");
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY)").unwrap();
    assert_eq!(
        sqlite3_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    let mut counter: usize = 0;
    // Two INSERTs (no rows callback), then a SELECT that yields 2 rows.
    let sql = CString::new("INSERT INTO t VALUES(1); INSERT INTO t VALUES(2); SELECT id FROM t;")
        .unwrap();
    let rc = sqlite3_exec(
        db,
        sql.as_ptr(),
        Some(count_callback),
        (&mut counter) as *mut usize as *mut c_void,
        ptr::null_mut(),
    );
    assert_eq!(rc, RLDB_OK);
    assert_eq!(counter, 2, "callback fires once per SELECT row");
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_prepare_v2_sets_pztail_after_first_statement() {
    let db = open_test_db("prepare-tail");
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY)").unwrap();
    assert_eq!(
        sqlite3_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    let sql = CString::new("INSERT INTO t VALUES(1); INSERT INTO t VALUES(2);").unwrap();
    let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
    let mut tail: *const c_char = ptr::null();
    let rc = sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, &mut tail);
    assert_eq!(rc, RLDB_OK);
    assert!(!stmt.is_null());
    // tail must point into the original buffer, after the first ;.
    assert!(!tail.is_null());
    let consumed = unsafe { tail.offset_from(sql.as_ptr()) } as usize;
    let head_str = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(
            sql.as_ptr() as *const u8,
            consumed,
        ))
        .unwrap()
    };
    assert_eq!(head_str, "INSERT INTO t VALUES(1);");
    let tail_str = unsafe { CStr::from_ptr(tail) }.to_str().unwrap();
    assert_eq!(tail_str, " INSERT INTO t VALUES(2);");

    // Step the first statement to apply it.
    assert_eq!(sqlite3_step(stmt), RLDB_DONE);
    assert_eq!(sqlite3_finalize(stmt), RLDB_OK);

    // Prepare and step the tail.
    let mut stmt2: *mut sqlite3_stmt = ptr::null_mut();
    let mut tail2: *const c_char = ptr::null();
    let rc = sqlite3_prepare_v2(db, tail, -1, &mut stmt2, &mut tail2);
    assert_eq!(rc, RLDB_OK);
    assert!(!stmt2.is_null());
    let tail2_str = unsafe { CStr::from_ptr(tail2) }.to_str().unwrap();
    assert_eq!(tail2_str, "");
    assert_eq!(sqlite3_step(stmt2), RLDB_DONE);
    assert_eq!(sqlite3_finalize(stmt2), RLDB_OK);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_prepare_v2_blank_input_returns_null_stmt() {
    let db = open_test_db("prepare-blank");
    let sql = CString::new("  -- only a comment\n   ").unwrap();
    let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
    let mut tail: *const c_char = ptr::null();
    let rc = sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, &mut tail);
    assert_eq!(rc, RLDB_OK);
    assert!(stmt.is_null());
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_exec_stops_at_first_error_and_sets_errmsg() {
    let db = open_test_db("multi-err");
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY)").unwrap();
    assert_eq!(
        sqlite3_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    // First INSERT runs, second is invalid, third must NOT run.
    let sql = CString::new("INSERT INTO t VALUES(1); INVALID SQL HERE; INSERT INTO t VALUES(2);")
        .unwrap();
    let mut errmsg: *mut c_char = ptr::null_mut();
    let rc = sqlite3_exec(db, sql.as_ptr(), None, ptr::null_mut(), &mut errmsg);
    assert_ne!(rc, RLDB_OK, "must report the parse error");
    assert!(!errmsg.is_null(), "errmsg must be populated on failure");
    let msg = unsafe { CStr::from_ptr(errmsg) }
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        msg.to_lowercase().contains("parse")
            || msg.to_lowercase().contains("invalid")
            || msg.to_lowercase().contains("expected"),
        "unexpected message: {msg}"
    );
    // Free errmsg via sqlite3_free — must not double-free or panic.
    sqlite3_free(errmsg as *mut c_void);

    // Verify only the first INSERT applied.
    let mut counter: usize = 0;
    let sel = CString::new("SELECT id FROM t").unwrap();
    assert_eq!(
        sqlite3_exec(
            db,
            sel.as_ptr(),
            Some(count_callback),
            (&mut counter) as *mut usize as *mut c_void,
            ptr::null_mut()
        ),
        RLDB_OK
    );
    assert_eq!(counter, 1);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_exec_errmsg_is_null_terminated_and_freeable() {
    // Ownership contract: sqlite3_exec sets errmsg to a heap-allocated
    // string the caller must free with sqlite3_free. The test exercises
    // the round-trip multiple times to catch leaks during repeat use
    // (in lieu of running under ASan in this lane).
    let db = open_test_db("errmsg-own");
    for _ in 0..16 {
        let bad = CString::new("THIS IS NOT VALID SQL").unwrap();
        let mut errmsg: *mut c_char = ptr::null_mut();
        let rc = sqlite3_exec(db, bad.as_ptr(), None, ptr::null_mut(), &mut errmsg);
        assert_ne!(rc, RLDB_OK);
        assert!(!errmsg.is_null());
        let _ = unsafe { CStr::from_ptr(errmsg) }.to_str().unwrap();
        sqlite3_free(errmsg as *mut c_void);
    }
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_exec_savepoint_release_round_trip() {
    // End-to-end: a SAVEPOINT/RELEASE round-trip via sqlite3_exec must
    // succeed and propagate inserted rows to the outer state.
    let db = open_test_db("savepoint-ffi");
    let sql = CString::new(
        "CREATE TABLE t(id INTEGER PRIMARY KEY); \
         BEGIN; \
         INSERT INTO t VALUES(1); \
         SAVEPOINT sp1; \
         INSERT INTO t VALUES(2); \
         RELEASE sp1; \
         COMMIT;",
    )
    .unwrap();
    let rc = sqlite3_exec(db, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    assert_eq!(rc, RLDB_OK);

    let mut counter: usize = 0;
    let sel = CString::new("SELECT id FROM t").unwrap();
    assert_eq!(
        sqlite3_exec(
            db,
            sel.as_ptr(),
            Some(count_callback),
            (&mut counter) as *mut usize as *mut c_void,
            ptr::null_mut()
        ),
        RLDB_OK
    );
    assert_eq!(counter, 2);
    assert_eq!(sqlite3_close(db), RLDB_OK);
}

#[test]
fn sqlite3_exec_savepoint_rollback_to_drops_postsaved_rows() {
    let db = open_test_db("savepoint-rb-ffi");
    let sql = CString::new(
        "CREATE TABLE t(id INTEGER PRIMARY KEY); \
         BEGIN; \
         INSERT INTO t VALUES(1); \
         SAVEPOINT sp1; \
         INSERT INTO t VALUES(2); \
         INSERT INTO t VALUES(3); \
         ROLLBACK TO sp1; \
         RELEASE sp1; \
         COMMIT;",
    )
    .unwrap();
    let rc = sqlite3_exec(db, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    assert_eq!(rc, RLDB_OK);

    let mut counter: usize = 0;
    let sel = CString::new("SELECT id FROM t").unwrap();
    assert_eq!(
        sqlite3_exec(
            db,
            sel.as_ptr(),
            Some(count_callback),
            (&mut counter) as *mut usize as *mut c_void,
            ptr::null_mut()
        ),
        RLDB_OK
    );
    assert_eq!(counter, 1, "only pre-savepoint row survives");
    assert_eq!(sqlite3_close(db), RLDB_OK);
}
