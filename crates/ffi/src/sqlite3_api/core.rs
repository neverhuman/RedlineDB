use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use crate::types::{
    RLDB_MISUSE, RLDB_OK, RLDB_READONLY, SQLITE_OPEN_CREATE, SQLITE_OPEN_READONLY,
    SQLITE_OPEN_READWRITE, sqlite3, sqlite3_stmt,
};
use crate::util::{
    api, flatten_code, open_handle, record_status, sqlite_errstr, sqlite_sourceid_cstr,
    sqlite_version_cstr,
};
use crate::{rldb_close, rldb_close_v2, rldb_open};

use super::stmt::sqlite3_prepare_v2;

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
        // SAFETY: `path` non-null (checked); per redlinedb.h:147 it is a
        // NUL-terminated C string; open_handle copies it into owned PathBuf.
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, create_if_missing)?;
        // SAFETY: `out_db` non-null (checked); per redlinedb.h:147 it is a
        // writable sqlite3**; open_handle returned a Box::into_raw pointer
        // whose ownership transfers to the C caller (paired with sqlite3_close).
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
pub extern "C" fn sqlite3_close(db: *mut sqlite3) -> c_int {
    rldb_close(db)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_close_v2(db: *mut sqlite3) -> c_int {
    rldb_close_v2(db)
}
