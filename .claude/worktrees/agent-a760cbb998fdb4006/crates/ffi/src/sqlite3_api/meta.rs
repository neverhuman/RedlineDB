use std::ffi::{CStr, c_char};
use std::os::raw::{c_int, c_void};
use std::ptr;

use crate::types::{RLDB_MISUSE, RLDB_OK, sqlite3, sqlite3_stmt};
use crate::util::record_status;
use crate::{
    rldb_busy_timeout, rldb_changes, rldb_checkpoint, rldb_errcode, rldb_errmsg, rldb_free,
    rldb_interrupt, rldb_last_insert_rowid, rldb_stats_json, rldb_vacuum,
};

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
    // SAFETY: `stmt` non-null (checked); per redlinedb.h:189 from
    // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
    unsafe { (*stmt).db }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_db_filename(db: *mut sqlite3, name: *const c_char) -> *const c_char {
    if db.is_null() || name.is_null() {
        return ptr::null();
    }
    // SAFETY: both `db` and `name` non-null (checked); per redlinedb.h:190
    // `db` from sqlite3_open not yet closed, `name` NUL-terminated C
    // string; returned pointer into db.path_text, valid for connection life.
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
