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
        // SAFETY: matching constructor/destructor pair — every pointer handed to
        // rldb_free originates from CString::new(...).into_raw() in
        // errmsg_to_c_string (crates/ffi/src/util.rs:316), routed through
        // set_errmsg (crates/ffi/src/util.rs:342); ownership invariant: this
        // library has exclusive access as the sole producer of such pointers per
        // redlinedb.h:128; double-free guarded by the caller's obligation to
        // NULL the handle after rldb_free (redlinedb.h:128); ledgered at
        // agent/unsafe-ledger.toml (file=crates/ffi/src/error.rs, line=42,
        // detector=rust.unsafe.raw-parts); proof:
        // crates/ffi/tests/safety_invariants.rs::rldb_free_null_is_noop and
        // ::exec_callback_failure_round_trips_errmsg_ownership.
        unsafe { drop(CString::from_raw(ptr as *mut c_char)); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_interrupt(db: *mut rldb) {
    let _ = with_db(db, |db| db.interrupted.store(true, Ordering::Relaxed));
}
