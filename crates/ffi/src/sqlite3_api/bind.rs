use std::os::raw::{c_char, c_int, c_void};

use redlinedb_sql::value::SqlValue;

use crate::types::{RLDB_MISUSE, RLDB_TOOBIG, sqlite3_stmt};
use crate::util::record_status;
use crate::{
    rldb_bind_blob, rldb_bind_double, rldb_bind_int64, rldb_bind_null, rldb_bind_parameter_index,
    rldb_bind_text, rldb_parameter_count,
};

use super::value::RldbValue;

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
pub extern "C" fn sqlite3_bind_zeroblob(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    nbytes: c_int,
) -> c_int {
    if nbytes < 0 {
        return RLDB_MISUSE;
    }
    let bytes = vec![0; nbytes as usize];
    let rc = rldb_bind_blob(stmt, index, bytes.as_ptr() as *const c_void, nbytes);
    if !stmt.is_null() {
        // SAFETY: `stmt` non-null (checked); reads Copy db field only.
        let db = unsafe { (*stmt).db };
        record_status(db, rc);
    }
    rc
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_bind_zeroblob64(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    nbytes: u64,
) -> c_int {
    if nbytes > c_int::MAX as u64 {
        return RLDB_TOOBIG;
    }
    sqlite3_bind_zeroblob(stmt, index, nbytes as c_int)
}

/// # Safety
/// `value` must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_bind_value(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    value: *mut RldbValue,
) -> c_int {
    if value.is_null() {
        return sqlite3_bind_null(stmt, index);
    }
    // SAFETY: caller obligation; non-null checked above.
    let value = unsafe { &*value };
    let sql_value = value.to_sql();
    let rc = match sql_value {
        SqlValue::Null => rldb_bind_null(stmt, index),
        SqlValue::Integer(i) => rldb_bind_int64(stmt, index, i),
        SqlValue::Real(f) => rldb_bind_double(stmt, index, f),
        SqlValue::Text(text) => {
            let bytes = text.as_bytes();
            rldb_bind_text(
                stmt,
                index,
                bytes.as_ptr() as *const c_char,
                bytes.len() as c_int,
            )
        }
        SqlValue::Blob(blob) => rldb_bind_blob(
            stmt,
            index,
            blob.as_ptr() as *const c_void,
            blob.len() as c_int,
        ),
    };
    if !stmt.is_null() {
        // SAFETY: `stmt` non-null (checked); reads Copy db field only.
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
