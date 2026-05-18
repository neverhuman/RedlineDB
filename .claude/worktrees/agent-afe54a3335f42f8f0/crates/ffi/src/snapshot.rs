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
        let src_ref = unsafe { &*src };
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
    unsafe {
        drop(Box::from_raw(backup));
    }
    RLDB_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_remaining(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    unsafe { (*backup).remaining as c_int }
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_backup_pagecount(backup: *mut rldb_backup) -> c_int {
    if backup.is_null() {
        return RLDB_MISUSE;
    }
    unsafe { (*backup).pagecount as c_int }
}
