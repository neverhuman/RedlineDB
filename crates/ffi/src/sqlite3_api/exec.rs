use std::ffi::c_char;
use std::os::raw::{c_int, c_void};
use std::sync::atomic::Ordering;

use crate::rldb_exec;
use crate::types::{RLDB_OK, sqlite3};
use crate::util::record_status;

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
        // out-pointer); for now record the generic code only.
        // SAFETY: `db` non-null (checked); per redlinedb.h:175 from
        // sqlite3_open not yet closed; touches atomic last_code only.
        unsafe {
            (*db).last_code.store(rc, Ordering::Relaxed);
        }
    }
    rc
}
