//! B2 scalar UDF registration + invocation through SQL.
//!
//! Verifies that a C-callback UDF registered via `sqlite3_create_function*`
//! is dispatched when the SQL evaluator encounters that function name.

use std::ffi::{CStr, CString};
use std::os::raw::{c_int, c_void};
use std::ptr;

use redlinedb::sqlite3_api::context::RldbContext;
use redlinedb::sqlite3_api::result::{sqlite3_result_int, sqlite3_result_text};
use redlinedb::sqlite3_api::value::{RldbValue, sqlite3_value_int64};
use redlinedb::types::rldb;
use redlinedb::{rldb_close, rldb_exec, rldb_open, sqlite3_create_function_v2};
use tempfile::TempDir;

fn open_db() -> (TempDir, *mut rldb) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("udf.redline");
    let c_path = CString::new(path.to_str().unwrap()).unwrap();
    let mut db: *mut rldb = ptr::null_mut();
    let rc = rldb_open(c_path.as_ptr(), &mut db);
    assert_eq!(rc, 0);
    (dir, db)
}

fn exec(db: *mut rldb, sql: &str) {
    let c_sql = CString::new(sql).unwrap();
    let mut err: *mut std::os::raw::c_char = ptr::null_mut();
    let rc = rldb_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), &mut err);
    assert_eq!(rc, 0, "exec({sql}) failed");
}

unsafe extern "C" fn times_two(
    ctx: *mut RldbContext,
    argc: c_int,
    argv: *mut *mut RldbValue,
) {
    assert_eq!(argc, 1);
    let val = unsafe { *argv };
    let n = unsafe { sqlite3_value_int64(val) };
    unsafe { sqlite3_result_int(ctx, (n * 2) as c_int) };
}

unsafe extern "C" fn echo_label(
    ctx: *mut RldbContext,
    _argc: c_int,
    _argv: *mut *mut RldbValue,
) {
    let label = CString::new("hello-udf").unwrap();
    unsafe { sqlite3_result_text(ctx, label.as_ptr(), -1, None) };
}

#[test]
fn times_two_scalar_udf_invoked_from_select() {
    let (_dir, db) = open_db();
    let name = CString::new("times_two").unwrap();
    let rc = unsafe {
        sqlite3_create_function_v2(
            db,
            name.as_ptr(),
            1,
            0,
            ptr::null_mut(),
            Some(times_two),
            None,
            None,
            None,
        )
    };
    assert_eq!(rc, 0);
    // Drive a single-row callback to read the UDF's output.
    let saw: std::sync::Arc<std::sync::Mutex<Option<i64>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let saw_ptr = std::sync::Arc::into_raw(saw.clone()) as *mut c_void;
    extern "C" fn cb(
        ctx: *mut c_void,
        ncol: c_int,
        argv: *mut *mut std::os::raw::c_char,
        _argn: *mut *mut std::os::raw::c_char,
    ) -> c_int {
        assert_eq!(ncol, 1);
        let val = unsafe { *argv };
        let cstr = unsafe { CStr::from_ptr(val) };
        let s = cstr.to_string_lossy().into_owned();
        let arc = unsafe {
            std::sync::Arc::from_raw(
                ctx as *const std::sync::Mutex<Option<i64>>,
            )
        };
        *arc.lock().unwrap() = s.parse::<i64>().ok();
        let _ = std::sync::Arc::into_raw(arc);
        0
    }
    let c_sql = CString::new("SELECT times_two(21)").unwrap();
    let rc = rldb_exec(db, c_sql.as_ptr(), Some(cb), saw_ptr, ptr::null_mut());
    assert_eq!(rc, 0);
    let _ = unsafe { std::sync::Arc::from_raw(saw_ptr as *const std::sync::Mutex<Option<i64>>) };
    assert_eq!(*saw.lock().unwrap(), Some(42));
    rldb_close(db);
}

#[test]
fn arity_any_matches_when_exact_arity_missing() {
    let (_dir, db) = open_db();
    let name = CString::new("echo_label").unwrap();
    let rc = unsafe {
        sqlite3_create_function_v2(
            db,
            name.as_ptr(),
            -1, // any arity
            0,
            ptr::null_mut(),
            Some(echo_label),
            None,
            None,
            None,
        )
    };
    assert_eq!(rc, 0);
    exec(db, "SELECT echo_label(1, 2, 3)");
    rldb_close(db);
}

#[test]
fn null_db_returns_misuse() {
    let name = CString::new("noop").unwrap();
    let rc = unsafe {
        sqlite3_create_function_v2(
            ptr::null_mut(),
            name.as_ptr(),
            1,
            0,
            ptr::null_mut(),
            Some(times_two),
            None,
            None,
            None,
        )
    };
    assert_eq!(rc, 21); // RLDB_MISUSE
}
