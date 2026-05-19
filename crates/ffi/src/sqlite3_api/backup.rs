//! SQLite-compatible online-backup symbol wrappers.

use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::types::{sqlite3, sqlite3_backup};
use crate::util::{record_status, with_db};
use crate::{
    rldb_backup_close, rldb_backup_finish, rldb_backup_init, rldb_backup_pagecount,
    rldb_backup_remaining, rldb_backup_step,
};

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_backup_init(
    dst: *mut sqlite3,
    _dst_name: *const c_char,
    src: *mut sqlite3,
    _src_name: *const c_char,
) -> *mut sqlite3_backup {
    if dst.is_null() || src.is_null() {
        return ptr::null_mut();
    }
    let dst_path = match with_db(dst, |db| db.path_text.as_ptr()) {
        Ok(path) => path,
        Err(code) => {
            record_status(dst, code);
            return ptr::null_mut();
        }
    };
    let mut backup = ptr::null_mut();
    let rc = rldb_backup_init(src, dst_path, ptr::null(), &mut backup);
    record_status(dst, rc);
    if rc == 0 { backup } else { ptr::null_mut() }
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_backup_step(backup: *mut sqlite3_backup, pages: c_int) -> c_int {
    rldb_backup_step(backup, pages)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_backup_finish(backup: *mut sqlite3_backup) -> c_int {
    let rc = rldb_backup_finish(backup);
    if rc != 0 {
        return rc;
    }
    rldb_backup_close(backup)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_backup_remaining(backup: *mut sqlite3_backup) -> c_int {
    rldb_backup_remaining(backup)
}

#[unsafe(no_mangle)]
pub extern "C" fn sqlite3_backup_pagecount(backup: *mut sqlite3_backup) -> c_int {
    rldb_backup_pagecount(backup)
}
