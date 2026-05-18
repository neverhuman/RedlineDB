//! Parameter binding FFI surface.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use crate::types::*;
use crate::util::{api, flatten_code, sql_result};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_null(stmt: *mut rldb_stmt, index: c_int) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:105 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; C ABI requires single-thread ownership.
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.bind_null(index as usize))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_int64(stmt: *mut rldb_stmt, index: c_int, value: i64) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:106 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; C ABI requires single-thread ownership.
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.bind_i64(index as usize, value))?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_double(stmt: *mut rldb_stmt, index: c_int, value: f64) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:107 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; C ABI requires single-thread ownership.
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
        if stmt.is_null() || value.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:108 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; single-thread ownership required by C ABI.
        let stmt = unsafe { &mut *stmt };
        let bytes = if nbytes < 0 {
            // SAFETY: `value` non-null (checked); nbytes<0 means it is a
            // NUL-terminated C string per redlinedb.h:108; bytes copied
            // into owned Vec so the borrow does not outlive caller buffer.
            unsafe { CStr::from_ptr(value) }.to_bytes().to_vec()
        } else {
            // SAFETY: `value` non-null (checked); nbytes is the explicit
            // byte length of the caller-owned buffer per redlinedb.h:108;
            // slice copied into owned Vec immediately.
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
        if stmt.is_null() || value.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:109 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; single-thread ownership required by C ABI.
        let stmt = unsafe { &mut *stmt };
        // SAFETY: `value` non-null (checked); nbytes is byte length of the
        // caller-owned blob buffer per redlinedb.h:109; slice copied into
        // owned Vec immediately so borrow does not outlive caller buffer.
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
    // SAFETY: `stmt` non-null (checked); per redlinedb.h:111 from
    // rldb_prepare_v2 not yet finalized; reads Copy integer only.
    unsafe { (*stmt).stmt.parameter_count() as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_bind_parameter_index(stmt: *mut rldb_stmt, name: *const c_char) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() || name.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:112 `stmt` non-null from rldb_prepare_v2
        // not yet finalized; single-thread ownership required by C ABI.
        let stmt = unsafe { &mut *stmt };
        // SAFETY: `name` non-null (checked); NUL-terminated C string per
        // redlinedb.h:112; &str borrow consumed within this call.
        let name = unsafe { CStr::from_ptr(name) }
            .to_str()
            .map_err(|_| RLDB_MISMATCH)?;
        Ok(stmt.stmt.parameter_index(name).unwrap_or(0) as c_int)
    }))
}
