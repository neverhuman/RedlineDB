//! SQLite-compatible C ABI surface.
//!
//! These entry points preserve the standard `sqlite3_*` symbol names so
//! callers linked against libsqlite3 can swap in libredlinedb at runtime.
//! They delegate to the corresponding `rldb_*` implementation, layering on
//! the status-recording semantics expected by the SQLite ABI.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_void};
use std::ptr;
use std::sync::atomic::Ordering;

use crate::bind::*;
use crate::column::*;
use crate::config::*;
use crate::error::*;
use crate::exec::*;
use crate::lifecycle::*;
use crate::stmt::*;
use crate::types::*;
use crate::util::{
    api, flatten_code, open_handle, record_status, sqlite_errstr, sqlite_sourceid_cstr,
    sqlite_version_cstr,
};

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_open(path: *const c_char, out_db: *mut *mut sqlite3) -> c_int {
    rldb_open(path, out_db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_libversion() -> *const c_char {
    sqlite_version_cstr().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_libversion_number() -> c_int {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .unwrap_or(0);
    major * 1_000_000 + minor * 1_000 + patch
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_sourceid() -> *const c_char {
    sqlite_sourceid_cstr().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_threadsafe() -> c_int {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_errstr(code: c_int) -> *const c_char {
    sqlite_errstr(code).as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_open_v2(
    path: *const c_char,
    out_db: *mut *mut sqlite3,
    flags: c_int,
    _vfs: *const c_char,
) -> c_int {
    flatten_code(api(|| {
        if path.is_null() || out_db.is_null() {
            return Err(RLDB_MISUSE);
        }
        if flags & SQLITE_OPEN_READONLY != 0 {
            return Err(RLDB_READONLY);
        }
        if flags & SQLITE_OPEN_READWRITE == 0 {
            return Err(RLDB_MISUSE);
        }
        let create_if_missing = flags & SQLITE_OPEN_CREATE != 0;
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, create_if_missing)?;
        unsafe {
            *out_db = handle;
        }
        record_status(handle, RLDB_OK);
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_prepare_v3(
    db: *mut sqlite3,
    sql: *const c_char,
    nbytes: c_int,
    out_stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
    _flags: c_int,
) -> c_int {
    sqlite3_prepare_v2(db, sql, nbytes, out_stmt, tail)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_stmt_readonly(stmt: *mut sqlite3_stmt) -> c_int {
    if stmt.is_null() {
        return 0;
    }
    unsafe { (*stmt).stmt.is_readonly() as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_stmt_busy(stmt: *mut sqlite3_stmt) -> c_int {
    if stmt.is_null() {
        return 0;
    }
    unsafe { (*stmt).stmt.is_busy() as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_sql(stmt: *mut sqlite3_stmt) -> *const c_char {
    if stmt.is_null() {
        return ptr::null();
    }
    unsafe { (*stmt).sql_text.as_ptr() }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_close(db: *mut sqlite3) -> c_int {
    rldb_close(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_close_v2(db: *mut sqlite3) -> c_int {
    rldb_close_v2(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_prepare_v2(
    db: *mut sqlite3,
    sql: *const c_char,
    nbytes: c_int,
    out_stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
) -> c_int {
    let rc = rldb_prepare_v2(db, sql, nbytes, out_stmt, tail);
    // Only overwrite last_message on success — on failure, rldb_prepare_v2
    // has already stashed the enriched parser/binder error string, so
    // replacing it with the generic "ok"/"error" status text would drop
    // the actionable detail (`sqlite3_errmsg` consumers rely on it).
    if rc == RLDB_OK {
        record_status(db, rc);
    } else if !db.is_null() {
        // Still record the code so `sqlite3_errcode` is consistent.
        unsafe {
            (*db).last_code.store(rc, Ordering::Relaxed);
        }
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int {
    let rc = rldb_step(stmt);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        // rldb_step already recorded an enriched error message on failure;
        // only update the generic "ok" message on success/row/done.
        if rc == RLDB_OK || rc == RLDB_ROW || rc == RLDB_DONE {
            record_status(db, rc);
        } else if !db.is_null() {
            unsafe {
                (*db).last_code.store(rc, Ordering::Relaxed);
            }
        }
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_reset(stmt: *mut sqlite3_stmt) -> c_int {
    let rc = rldb_reset(stmt);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int {
    rldb_finalize(stmt)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_clear_bindings(stmt: *mut sqlite3_stmt) -> c_int {
    let rc = rldb_clear_bindings(stmt);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    let rc = rldb_bind_null(stmt, index);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, index: c_int, value: i64) -> c_int {
    let rc = rldb_bind_int64(stmt, index, value);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_double(stmt: *mut sqlite3_stmt, index: c_int, value: f64) -> c_int {
    let rc = rldb_bind_double(stmt, index, value);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_text(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    value: *const c_char,
    nbytes: c_int,
    _destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) -> c_int {
    let rc = rldb_bind_text(stmt, index, value, nbytes);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_blob(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    value: *const c_void,
    nbytes: c_int,
    _destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) -> c_int {
    let rc = rldb_bind_blob(stmt, index, value, nbytes);
    if !stmt.is_null() {
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_parameter_count(stmt: *mut sqlite3_stmt) -> c_int {
    rldb_parameter_count(stmt)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_parameter_index(
    stmt: *mut sqlite3_stmt,
    name: *const c_char,
) -> c_int {
    rldb_bind_parameter_index(stmt, name)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_count(stmt: *mut sqlite3_stmt) -> c_int {
    rldb_column_count(stmt)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_name(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_char {
    rldb_column_name(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_type(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    rldb_column_type(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, index: c_int) -> i64 {
    rldb_column_int64(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_double(stmt: *mut sqlite3_stmt, index: c_int) -> f64 {
    rldb_column_double(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_text(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_uchar {
    rldb_column_text(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_blob(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_void {
    rldb_column_blob(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_column_bytes(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    rldb_column_bytes(stmt, index)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_exec(
    db: *mut sqlite3,
    sql: *const c_char,
    callback: Option<
        extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
    >,
    ctx: *mut c_void,
    errmsg: *mut *mut c_char,
) -> c_int {
    let rc = rldb_exec(db, sql, callback, ctx, errmsg);
    if rc == RLDB_OK {
        record_status(db, rc);
    } else if !db.is_null() {
        // Mirror the errmsg into last_message so sqlite3_errmsg(db) returns
        // the same explanation. We cannot read from `errmsg` (it's a caller
        // out-pointer) so duplicate the message via the success path of
        // rldb_exec — for now record the generic code only.
        unsafe {
            (*db).last_code.store(rc, Ordering::Relaxed);
        }
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_errcode(db: *mut sqlite3) -> c_int {
    rldb_errcode(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char {
    rldb_errmsg(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_free(ptr: *mut c_void) {
    rldb_free(ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_interrupt(db: *mut sqlite3) {
    rldb_interrupt(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_busy_timeout(db: *mut sqlite3, milliseconds: c_int) -> c_int {
    let rc = rldb_busy_timeout(db, milliseconds);
    record_status(db, rc);
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_extended_result_codes(_db: *mut sqlite3, _onoff: c_int) -> c_int {
    RLDB_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_changes(db: *mut sqlite3) -> c_int {
    rldb_changes(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_changes64(db: *mut sqlite3) -> i64 {
    rldb_changes(db) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_total_changes(db: *mut sqlite3) -> c_int {
    use crate::util::with_db;
    with_db(db, |db| db.conn.total_changes() as c_int).unwrap_or(RLDB_MISUSE)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_total_changes64(db: *mut sqlite3) -> i64 {
    use crate::util::with_db;
    with_db(db, |db| db.conn.total_changes() as i64).unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_get_autocommit(db: *mut sqlite3) -> c_int {
    use crate::util::with_db;
    with_db(db, |db| (!db.conn.in_transaction()) as c_int).unwrap_or(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_last_insert_rowid(db: *mut sqlite3) -> i64 {
    rldb_last_insert_rowid(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_db_handle(stmt: *mut sqlite3_stmt) -> *mut sqlite3 {
    if stmt.is_null() {
        return ptr::null_mut();
    }
    unsafe { (*stmt).db }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_db_filename(db: *mut sqlite3, name: *const c_char) -> *const c_char {
    if db.is_null() || name.is_null() {
        return ptr::null();
    }
    unsafe {
        if CStr::from_ptr(name).to_bytes() == b"main" {
            (*db).path_text.as_ptr()
        } else {
            ptr::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_db_readonly(_db: *mut sqlite3, _name: *const c_char) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_checkpoint(db: *mut sqlite3) -> c_int {
    let rc = rldb_checkpoint(db);
    record_status(db, rc);
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_vacuum(db: *mut sqlite3) -> c_int {
    let rc = rldb_vacuum(db);
    record_status(db, rc);
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_stats_json(db: *mut sqlite3, out_json: *mut *mut c_char) -> c_int {
    let rc = rldb_stats_json(db, out_json);
    record_status(db, rc);
    rc
}
