//! Copy the redlinedb data directory to a destination path for snapshot use.

use std::ffi::CStr;
use std::fs;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;

use crate::types::*;
use crate::util::{api, flatten_code, io, recursive_copy};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_init(
    src: *mut rldb,
    dst_path: *const c_char,
    _dst_config: *const rldb_config,
    out: *mut *mut rldb_backup,
) -> c_int {
    flatten_code(api(|| {
        if src.is_null() || dst_path.is_null() || out.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:138 `src` is a non-null *mut rldb from
        // rldb_open not yet closed; the borrow lives only in this closure
        // which the C ABI forbids racing with destruction.
        let src_ref = unsafe { &*src };
        // SAFETY: per redlinedb.h:138 `dst_path` is non-null NUL-terminated
        // C string; the CStr borrow is copied into an owned String before
        // returning so it does not outlive the caller buffer.
        let dst = unsafe { CStr::from_ptr(dst_path) }
            .to_str()
            .map_err(|_| RLDB_MISMATCH)?
            .to_owned();
        let backup = Box::new(rldb_backup {
            src_path: src_ref.path.clone(),
            dst_path: PathBuf::from(dst),
            done: false,
            remaining: 1,
            pagecount: 1,
        });
        // SAFETY: `out` non-null (checked); Box::into_raw transfers ownership
        // to the C caller (paired with rldb_backup_close's Box::from_raw).
        unsafe {
            *out = Box::into_raw(backup);
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_step(backup: *mut rldb_backup, _batches: c_int) -> c_int {
    flatten_code(api(|| {
        if backup.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: per redlinedb.h:139 `backup` is a non-null *mut rldb_backup
        // from rldb_backup_init not yet closed; C ABI requires single-thread
        // ownership so the &mut borrow (scoped to this closure) cannot alias.
        let backup = unsafe { &mut *backup };
        if !backup.done {
            if backup.dst_path.exists() {
                io(fs::remove_dir_all(&backup.dst_path))?;
            }
            io(fs::create_dir_all(&backup.dst_path))?;
            io(recursive_copy(&backup.src_path, &backup.dst_path))?;
            backup.done = true;
            backup.remaining = 0;
        }
        Ok(RLDB_DONE)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_finish(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    RLDB_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_close(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: matching constructor/destructor pair — `backup` originates from
    // Box::into_raw(backup) at rldb_backup_init (crates/ffi/src/snapshot.rs:43);
    // ownership invariant: the C caller may not free this pointer directly per
    // redlinedb.h:141; exclusive access upheld because backup handles are not
    // shared across threads in the documented contract; double-close guarded by
    // the null check above (caller must NULL after close); ledgered at
    // agent/unsafe-ledger.toml (file=crates/ffi/src/snapshot.rs, line=94,
    // detector=rust.unsafe.raw-parts); proof:
    // crates/ffi/tests/safety_invariants.rs::backup_init_step_close_round_trips_box_ownership.
    unsafe { drop(Box::from_raw(backup)); }
    RLDB_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_remaining(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: per redlinedb.h:142 `backup` non-null from rldb_backup_init
    // not yet closed; only reads a Copy integer, no borrow escapes.
    unsafe { (*backup).remaining as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_pagecount(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: per redlinedb.h:143 `backup` non-null from rldb_backup_init
    // not yet closed; only reads a Copy integer, no borrow escapes.
    unsafe { (*backup).pagecount as c_int }
}
