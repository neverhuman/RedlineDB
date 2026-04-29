//! Database open/close FFI surface.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::Ordering;

use crate::types::*;
use crate::util::{api, flatten_code, open_handle};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_open(path: *const c_char, out_db: *mut *mut rldb) -> c_int {
    flatten_code(api(|| {
        if path.is_null() || out_db.is_null() {
            return Err(RLDB_MISUSE);
        }
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, true)?;
        unsafe {
            *out_db = handle;
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_open_v2(
    path: *const c_char,
    config: *const rldb_config,
    out_db: *mut *mut rldb,
) -> c_int {
    flatten_code(api(|| {
        if path.is_null() || out_db.is_null() {
            return Err(RLDB_MISUSE);
        }
        let config = if config.is_null() {
            None
        } else {
            Some(unsafe { &*config })
        };
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, config, true)?;
        unsafe {
            *out_db = handle;
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_close(db: *mut rldb) -> c_int {
    flatten_code(api(|| {
        let db_ref = unsafe { &*db };
        if db_ref.active_statements.load(Ordering::Relaxed) != 0 {
            return Err(RLDB_BUSY);
        }
        unsafe {
            drop(Box::from_raw(db));
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_close_v2(db: *mut rldb) -> c_int {
    rldb_close(db)
}
