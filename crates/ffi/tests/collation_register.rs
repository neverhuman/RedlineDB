//! B2 custom collation registration + ORDER BY round-trip.
//!
//! Registers `REVERSE_NOCASE`: a case-insensitive compare that reverses
//! lexicographic order (so 'a' > 'b'). Verifies ORDER BY uses it.

use std::ffi::CString;
use std::os::raw::{c_int, c_void};
use std::ptr;

use redlinedb::types::rldb;
use redlinedb::{
    rldb_close, rldb_exec, rldb_open, sqlite3_collation_needed, sqlite3_create_collation_v2,
};
use tempfile::TempDir;

fn open_db() -> (TempDir, *mut rldb) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("col.redline");
    let c_path = CString::new(path.to_str().unwrap()).unwrap();
    let mut db: *mut rldb = ptr::null_mut();
    let rc = rldb_open(c_path.as_ptr(), &mut db);
    assert_eq!(rc, 0);
    (dir, db)
}

unsafe extern "C" fn reverse_nocase(
    _user: *mut c_void,
    na: c_int,
    a: *const c_void,
    nb: c_int,
    b: *const c_void,
) -> c_int {
    let a_bytes = unsafe { std::slice::from_raw_parts(a as *const u8, na as usize) };
    let b_bytes = unsafe { std::slice::from_raw_parts(b as *const u8, nb as usize) };
    let lower_a: String = a_bytes
        .iter()
        .map(|b| (*b as char).to_ascii_lowercase())
        .collect();
    let lower_b: String = b_bytes
        .iter()
        .map(|b| (*b as char).to_ascii_lowercase())
        .collect();
    // Reverse the cmp result.
    match lower_a.cmp(&lower_b) {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Greater => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

unsafe extern "C" fn needed_cb(
    user: *mut c_void,
    db: *mut rldb,
    _encoding: c_int,
    name: *const std::os::raw::c_char,
) {
    let cstr = unsafe { std::ffi::CStr::from_ptr(name) };
    // Caller signalled it needs `cstr`; the test stashes the name into the
    // user_data pointer (cast as Mutex<String>).
    let arc = unsafe { std::sync::Arc::from_raw(user as *const std::sync::Mutex<String>) };
    {
        let mut s = arc.lock().unwrap();
        *s = cstr.to_string_lossy().into_owned();
    }
    let _ = std::sync::Arc::into_raw(arc);
    let _ = db;
}

fn exec(db: *mut rldb, sql: &str) {
    let c_sql = CString::new(sql).unwrap();
    let rc = rldb_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    assert_eq!(rc, 0, "exec({sql})");
}

#[test]
fn reverse_nocase_orders_descending() {
    let (_dir, db) = open_db();
    let name = CString::new("REVERSE_NOCASE").unwrap();
    let rc = unsafe {
        sqlite3_create_collation_v2(
            db,
            name.as_ptr(),
            0,
            ptr::null_mut(),
            Some(reverse_nocase),
            None,
        )
    };
    assert_eq!(rc, 0);
    exec(db, "CREATE TABLE t(s TEXT)");
    exec(db, "INSERT INTO t VALUES ('Banana'), ('apple'), ('cherry')");

    let collected: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = std::sync::Arc::into_raw(collected.clone()) as *mut c_void;
    extern "C" fn cb(
        ctx: *mut c_void,
        _ncol: c_int,
        argv: *mut *mut std::os::raw::c_char,
        _argn: *mut *mut std::os::raw::c_char,
    ) -> c_int {
        let val = unsafe { *argv };
        let s = unsafe { std::ffi::CStr::from_ptr(val) }
            .to_string_lossy()
            .into_owned();
        let arc = unsafe { std::sync::Arc::from_raw(ctx as *const std::sync::Mutex<Vec<String>>) };
        arc.lock().unwrap().push(s);
        let _ = std::sync::Arc::into_raw(arc);
        0
    }
    let c_sql = CString::new("SELECT s FROM t ORDER BY s COLLATE REVERSE_NOCASE").unwrap();
    let rc = rldb_exec(db, c_sql.as_ptr(), Some(cb), ctx, ptr::null_mut());
    assert_eq!(rc, 0);
    let _ = unsafe { std::sync::Arc::from_raw(ctx as *const std::sync::Mutex<Vec<String>>) };
    let got = collected.lock().unwrap().clone();
    // Reverse alphabetical (case-insensitive).
    assert_eq!(got, vec!["cherry", "Banana", "apple"]);
    rldb_close(db);
}

#[test]
fn collation_needed_fires_when_invoked() {
    use redlinedb::sqlite3_api::collation::__test_invoke_needed;
    let (_dir, db) = open_db();
    let captured: std::sync::Arc<std::sync::Mutex<String>> =
        std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let user = std::sync::Arc::into_raw(captured.clone()) as *mut c_void;
    let rc = unsafe { sqlite3_collation_needed(db, user, Some(needed_cb)) };
    assert_eq!(rc, 0);
    __test_invoke_needed(db, "FOO");
    let _ = unsafe { std::sync::Arc::from_raw(user as *const std::sync::Mutex<String>) };
    assert_eq!(*captured.lock().unwrap(), "FOO");
    rldb_close(db);
}

#[test]
fn null_db_to_create_collation_returns_misuse() {
    let name = CString::new("X").unwrap();
    let rc = unsafe {
        sqlite3_create_collation_v2(
            ptr::null_mut(),
            name.as_ptr(),
            0,
            ptr::null_mut(),
            Some(reverse_nocase),
            None,
        )
    };
    assert_eq!(rc, 21); // RLDB_MISUSE
}
