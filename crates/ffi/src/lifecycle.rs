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
        // SAFETY: `path` non-null (checked); per redlinedb.h:85 it is a
        // NUL-terminated C string; open_handle copies it into owned PathBuf.
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, true)?;
        // SAFETY: `out_db` non-null (checked); per redlinedb.h:85 it is a
        // writable rldb**; open_handle returned a Box::into_raw pointer
        // whose ownership transfers to the C caller (paired with rldb_close).
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
            // SAFETY: `config` non-null (just checked); per redlinedb.h:86
            // caller owns rldb_config for this call; borrow consumed inside
            // open_handle which copies fields into owned DbOptions.
            Some(unsafe { &*config })
        };
        // SAFETY: `path` non-null (checked); per redlinedb.h:86 it is a
        // NUL-terminated C string; open_handle copies it into owned PathBuf.
        let handle = open_handle(unsafe { CStr::from_ptr(path) }, config, true)?;
        // SAFETY: `out_db` non-null (checked); per redlinedb.h:86 it is a
        // writable rldb**; open_handle returned a Box::into_raw pointer
        // whose ownership transfers to the C caller (paired with rldb_close).
        unsafe {
            *out_db = handle;
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_close(db: *mut rldb) -> c_int {
    flatten_code(api(|| {
        if db.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (just checked); per redlinedb.h:87 from
        // rldb_open not yet closed; only reads active_statements count.
        let db_ref = unsafe { &*db };
        if db_ref.active_statements.load(Ordering::Relaxed) != 0 {
            return Err(RLDB_BUSY);
        }
        // SAFETY: matching constructor/destructor pair — `db` originates from
        // Box::into_raw(handle) at open_handle (crates/ffi/src/util.rs:112);
        // ownership invariant: only rldb_close / rldb_close_v2 consume it
        // (caller never frees directly per redlinedb.h:87); exclusive access
        // upheld by the active_statements==0 check above; double-close guarded
        // by the null check above (caller must NULL the handle after close);
        // ledgered at .jankurai/unsafe-ledger.toml (file=crates/ffi/src/lifecycle.rs,
        // line=81, detector=rust.unsafe.raw-parts); proof:
        // crates/ffi/tests/safety_invariants.rs::double_close_via_null_after_close_is_safe.
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
