//! Parameter binding FFI surface.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use crate::types::*;
use crate::util::{api, flatten_code, sql_result};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_null(stmt: *mut rldb_stmt, index: c_int) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.bind_null(index as usize))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_int64(stmt: *mut rldb_stmt, index: c_int, value: i64) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.bind_i64(index as usize, value))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_double(stmt: *mut rldb_stmt, index: c_int, value: f64) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.bind_f64(index as usize, value))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_text(
    stmt: *mut rldb_stmt,
    index: c_int,
    value: *const c_char,
    nbytes: c_int,
) -> c_int {
    flatten_code(api(|| {
        if value.is_null() {
            return Err(RLDB_MISUSE);
        }
        let stmt = unsafe { &mut *stmt };
        let bytes = if nbytes < 0 {
            unsafe { CStr::from_ptr(value) }.to_bytes().to_vec()
        } else {
            unsafe { std::slice::from_raw_parts(value as *const u8, nbytes as usize) }.to_vec()
        };
        let text = String::from_utf8(bytes).map_err(|_| RLDB_MISMATCH)?;
        sql_result(stmt.stmt.bind_text(index as usize, text))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_blob(
    stmt: *mut rldb_stmt,
    index: c_int,
    value: *const c_void,
    nbytes: c_int,
) -> c_int {
    flatten_code(api(|| {
        if value.is_null() {
            return Err(RLDB_MISUSE);
        }
        let stmt = unsafe { &mut *stmt };
        let slice = unsafe { std::slice::from_raw_parts(value as *const u8, nbytes as usize) };
        sql_result(stmt.stmt.bind_blob(index as usize, slice.to_vec()))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_parameter_count(stmt: *mut rldb_stmt) -> c_int {
    if stmt.is_null() {
        return RLDB_MISUSE;
    }
    unsafe { (*stmt).stmt.parameter_count() as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_parameter_index(stmt: *mut rldb_stmt, name: *const c_char) -> c_int {
    flatten_code(api(|| {
        if name.is_null() {
            return Err(RLDB_MISUSE);
        }
        let stmt = unsafe { &mut *stmt };
        let name = unsafe { CStr::from_ptr(name) }
            .to_str()
            .map_err(|_| RLDB_MISMATCH)?;
        Ok(stmt.stmt.parameter_index(name).unwrap_or(0) as c_int)
    }))
}
