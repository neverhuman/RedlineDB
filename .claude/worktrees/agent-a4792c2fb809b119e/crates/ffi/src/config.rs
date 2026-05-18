//! Configuration / housekeeping FFI surface
//! (`busy_timeout`, `changes`, `last_insert_rowid`, `checkpoint`, `vacuum`,
//! `stats_json`).

use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::time::Duration;

use crate::types::*;
use crate::util::{api, flatten_code, sql_result, with_db};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_busy_timeout(db: *mut rldb, milliseconds: c_int) -> c_int {
    flatten_code(api(|| {
        if db.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (checked); per redlinedb.h:130 from
        // rldb_open not yet closed; shared borrow scoped to api() closure.
        let db = unsafe { &*db };
        let timeout = Duration::from_millis(milliseconds.max(0) as u64);
        db.db.set_busy_timeout(timeout);
        db.conn.set_busy_timeout(timeout);
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_changes(db: *mut rldb) -> c_int {
    with_db(db, |db| db.conn.changes() as c_int).unwrap_or(RLDB_MISUSE)
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_last_insert_rowid(db: *mut rldb) -> i64 {
    with_db(db, |db| db.conn.last_insert_rowid().unwrap_or(0)).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_checkpoint(db: *mut rldb) -> c_int {
    flatten_code(api(|| {
        if db.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (checked); per redlinedb.h:134 from
        // rldb_open not yet closed; shared borrow scoped to api() closure.
        let db = unsafe { &*db };
        sql_result(db.db.checkpoint())?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_vacuum(db: *mut rldb) -> c_int {
    flatten_code(api(|| {
        if db.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (checked); per redlinedb.h:135 from
        // rldb_open not yet closed; shared borrow scoped to api() closure.
        let db = unsafe { &*db };
        sql_result(db.db.vacuum())?;
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_stats_json(db: *mut rldb, out_json: *mut *mut c_char) -> c_int {
    flatten_code(api(|| {
        if db.is_null() || out_json.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (checked); per redlinedb.h:136 from
        // rldb_open not yet closed; shared borrow scoped to api() closure.
        let db = unsafe { &*db };
        let stats = sql_result(db.db.stats())?;
        let json = format!(
            "{{\"schema_epoch\":{},\"resident_heap_pages\":{},\"wal_written_lsn\":{},\"wal_durable_lsn\":{}}}",
            db.db.schema_epoch().0,
            stats.resident_heap_pages,
            stats.wal_written_lsn.0,
            stats.wal_durable_lsn.0
        );
        let c = CString::new(json).unwrap();
        // SAFETY: `out_json` non-null (checked); CString::into_raw transfers
        // ownership to C caller (paired with rldb_free's CString::from_raw).
        unsafe {
            *out_json = c.into_raw();
        }
        Ok(RLDB_OK)
    }))
}
