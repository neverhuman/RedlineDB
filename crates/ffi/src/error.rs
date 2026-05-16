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
        // SAFETY: matching constructor at crates/ffi/src/util.rs:292 (errmsg_to_c_string / set_errmsg return CString::into_raw); this library is the only producer of pointers handed to rldb_free per redlinedb.h:128; double-free guarded by caller obligation to NULL the handle after free; proof: crates/ffi/tests/safety_invariants.rs::rldb_free_null_is_noop and ::exec_callback_failure_round_trips_errmsg_ownership.
        unsafe {
            drop(CString::from_raw(ptr as *mut c_char));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_interrupt(db: *mut rldb) {
    let _ = with_db(db, |db| db.interrupted.store(true, Ordering::Relaxed));
}
