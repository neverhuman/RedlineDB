use std::os::raw::{c_char, c_int, c_void};

use crate::types::sqlite3_stmt;
use crate::util::record_status;
use crate::{
    rldb_bind_blob, rldb_bind_double, rldb_bind_int64, rldb_bind_null, rldb_bind_parameter_index,
    rldb_bind_text, rldb_parameter_count,
};

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    let rc = rldb_bind_null(stmt, index);
    if !stmt.is_null() {
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:157 from
        // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, index: c_int, value: i64) -> c_int {
    let rc = rldb_bind_int64(stmt, index, value);
    if !stmt.is_null() {
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:158 from
        // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_double(stmt: *mut sqlite3_stmt, index: c_int, value: f64) -> c_int {
    let rc = rldb_bind_double(stmt, index, value);
    if !stmt.is_null() {
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:159 from
        // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
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
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:160 from
        // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
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
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:161 from
        // sqlite3_prepare_v2 not yet finalized; reads Copy db field only.
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
