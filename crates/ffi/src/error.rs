//! Error reporting and ancillary FFI surface (`errcode`, `errmsg`,
//! `interrupt`, `free`).

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::Ordering;

use crate::types::*;
use crate::util::with_db;

#[unsafe(no_mangle)]
pub extern "C" fn rldb_errcode(db: *mut rldb) -> c_int {
    with_db(db, |db| db.last_code.load(Ordering::Relaxed)).unwrap_or(RLDB_MISUSE)
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_errmsg(db: *mut rldb) -> *const c_char {
    with_db(db, |db| {
        db.last_message
            .lock()
            .map(|value| value.as_ptr())
            .unwrap_or(ptr::null())
    })
    .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        // SAFETY: `ptr` non-null (checked); per redlinedb.h:128 it was
        // returned by this library via CString::into_raw (rldb_stats_json,
        // errmsg_to_c_string, set_errmsg); from_raw reclaims ownership.
        unsafe {
            drop(CString::from_raw(ptr as *mut c_char));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_interrupt(db: *mut rldb) {
    let _ = with_db(db, |db| db.interrupted.store(true, Ordering::Relaxed));
}
