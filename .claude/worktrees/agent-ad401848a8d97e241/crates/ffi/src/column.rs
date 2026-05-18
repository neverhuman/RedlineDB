//! Column accessor FFI surface.

use std::os::raw::{c_char, c_int, c_uchar, c_void};
use std::ptr;

use crate::types::*;
use crate::util::{api, flatten_code};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_count(stmt: *mut rldb_stmt) -> c_int {
    if stmt.is_null() {
        return RLDB_MISUSE;
    }
    unsafe { (*stmt).stmt.column_count() as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_name(stmt: *mut rldb_stmt, index: c_int) -> *const c_char {
    if stmt.is_null() {
        return ptr::null();
    }
    unsafe {
        let stmt = &*stmt;
        stmt.column_names
            .get(index as usize)
            .map(|value| value.as_ptr())
            .unwrap_or(ptr::null())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_type(stmt: *mut rldb_stmt, index: c_int) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        if stmt.stmt.column_text(index as usize).is_ok() {
            Ok(RLDB_TEXT)
        } else if stmt.stmt.column_blob(index as usize).is_ok() {
            Ok(RLDB_BLOB)
        } else if stmt.stmt.column_i64(index as usize).is_ok() {
            Ok(RLDB_INTEGER)
        } else if stmt.stmt.column_f64(index as usize).is_ok() {
            Ok(RLDB_REAL)
        } else {
            Ok(RLDB_NULL)
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_int64(stmt: *mut rldb_stmt, index: c_int) -> i64 {
    if stmt.is_null() {
        return 0;
    }
    unsafe { (*stmt).stmt.column_i64(index as usize).unwrap_or(0) }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_double(stmt: *mut rldb_stmt, index: c_int) -> f64 {
    if stmt.is_null() {
        return 0.0;
    }
    unsafe { (*stmt).stmt.column_f64(index as usize).unwrap_or(0.0) }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_text(stmt: *mut rldb_stmt, index: c_int) -> *const c_uchar {
    if stmt.is_null() {
        return ptr::null();
    }
    unsafe {
        let stmt = &*stmt;
        stmt.text_cache
            .get(index as usize)
            .map(|value| value.as_ptr() as *const c_uchar)
            .unwrap_or(ptr::null())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_blob(stmt: *mut rldb_stmt, index: c_int) -> *const c_void {
    if stmt.is_null() {
        return ptr::null();
    }
    unsafe {
        match (*stmt).stmt.column_blob(index as usize) {
            Ok(blob) => blob.as_ptr() as *const c_void,
            Err(_) => ptr::null(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_column_bytes(stmt: *mut rldb_stmt, index: c_int) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        if let Ok(text) = stmt.stmt.column_text(index as usize) {
            Ok(text.len() as c_int)
        } else if let Ok(blob) = stmt.stmt.column_blob(index as usize) {
            Ok(blob.len() as c_int)
        } else {
            Ok(8)
        }
    }))
}
