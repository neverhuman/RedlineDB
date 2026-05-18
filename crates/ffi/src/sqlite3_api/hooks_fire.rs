//! Hook firing helpers — invoked from the FFI surface layer at well-defined
//! sites (statement step, `sqlite3_exec`, blob write). Kept in a sibling
//! file so `hooks.rs` stays focused on the registration ABI.

use std::ffi::CString;
use std::os::raw::{c_int, c_void};
use std::sync::Once;

use redlinedb_sql::udf as sql_udf;

use crate::types::*;

/// Ensure the SQL-side mutation/authorizer dispatchers are wired exactly
/// once. Called from `sqlite3_update_hook` / `sqlite3_set_authorizer` so
/// the SQL executor learns where to fire its hooks.
pub(crate) fn ensure_sql_dispatchers_installed() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        sql_udf::install_mutation_dispatch(mutation_from_sql);
        sql_udf::install_authorizer_dispatch(authorizer_from_sql);
    });
}

fn mutation_from_sql(db_addr: usize, op: i32, table: &str, rowid: i64) {
    fire_update(db_addr as *mut rldb, op, table, rowid);
}

fn authorizer_from_sql(db_addr: usize, action: i32, arg3: Option<&str>, arg4: Option<&str>) -> i32 {
    fire_authorizer(db_addr as *mut rldb, action, arg3, arg4)
}

pub(crate) enum CommitDecision {
    Proceed,
    Veto,
}

fn validate_db(db: *mut rldb) -> Option<&'static rldb> {
    if db.is_null() {
        return None;
    }
    // SAFETY: caller obligation — db is a non-null rldb* from sqlite3_open
    // that has not yet been closed; we hold only a shared borrow.
    Some(unsafe { &*(db as *const rldb) })
}

/// Inspect `sql` for top-level transaction keywords and fire the appropriate
/// hook before/after exec. Returns Veto when the commit hook returned a
/// non-zero value (caller should rollback the txn).
pub(crate) fn fire_for_sql(db: *mut rldb, sql: &str) -> CommitDecision {
    let trimmed = sql.trim_start().to_ascii_uppercase();
    if trimmed.starts_with("COMMIT") || trimmed.starts_with("END") {
        if let Some(handle) = validate_db(db) {
            let slot = handle.hooks.commit.lock().expect("commit hook poisoned");
            if let Some((cb, user_data_addr)) = *slot {
                let user_data = user_data_addr as *mut c_void;
                // SAFETY: cb signature matches the documented C ABI; user_data
                // is the pointer the registrar provided and remains valid per
                // the standard hook contract.
                let rc = unsafe { cb(user_data) };
                if rc != 0 {
                    return CommitDecision::Veto;
                }
            }
        }
    } else if trimmed.starts_with("ROLLBACK") {
        if let Some(handle) = validate_db(db) {
            let slot = handle
                .hooks
                .rollback
                .lock()
                .expect("rollback hook poisoned");
            if let Some((cb, user_data_addr)) = *slot {
                let user_data = user_data_addr as *mut c_void;
                // SAFETY: cb signature matches the documented C ABI; user_data
                // remains valid per the standard hook contract.
                unsafe { cb(user_data) };
            }
        }
    }
    CommitDecision::Proceed
}

/// Fire the trace callback if installed. SQL must be UTF-8.
pub(crate) fn fire_trace(db: *mut rldb, sql: &str) {
    if let Some(handle) = validate_db(db) {
        let slot = handle.hooks.trace.lock().expect("trace hook poisoned");
        if let Some((cb, user_data_addr)) = *slot {
            let user_data = user_data_addr as *mut c_void;
            if let Ok(cstr) = CString::new(sql) {
                // SAFETY: cb signature matches the documented C ABI; the
                // CString lives for the entire `cb` call.
                unsafe { cb(user_data, cstr.as_ptr()) };
            }
        }
    }
}

/// Fire the profile callback if installed.
pub(crate) fn fire_profile(db: *mut rldb, sql: &str, nanos: u64) {
    if let Some(handle) = validate_db(db) {
        let slot = handle.hooks.profile.lock().expect("profile hook poisoned");
        if let Some((cb, user_data_addr)) = *slot {
            let user_data = user_data_addr as *mut c_void;
            if let Ok(cstr) = CString::new(sql) {
                // SAFETY: cb signature matches the documented C ABI; CString
                // lives for the call.
                unsafe { cb(user_data, cstr.as_ptr(), nanos) };
            }
        }
    }
}

/// Fire the update callback for an INSERT/UPDATE/DELETE row.
///
/// `op` follows SQLite's convention: 18 = INSERT, 23 = UPDATE, 9 = DELETE.
pub(crate) fn fire_update(db: *mut rldb, op: c_int, table: &str, rowid: i64) {
    if let Some(handle) = validate_db(db) {
        let slot = handle.hooks.update.lock().expect("update hook poisoned");
        if let Some((cb, user_data_addr)) = *slot {
            let user_data = user_data_addr as *mut c_void;
            let main_db = CString::new("main").expect("static literal");
            let table_c = match CString::new(table) {
                Ok(c) => c,
                Err(_) => return,
            };
            // SAFETY: cb signature matches the documented C ABI; both
            // CStrings live for the call.
            unsafe { cb(user_data, op, main_db.as_ptr(), table_c.as_ptr(), rowid) };
        }
    }
}

/// Consult the authorizer for an access decision.
///
/// Returns one of `SQLITE_OK (0)`, `SQLITE_DENY (1)`, `SQLITE_IGNORE (2)`.
pub(crate) fn fire_authorizer(
    db: *mut rldb,
    action: c_int,
    arg3: Option<&str>,
    arg4: Option<&str>,
) -> c_int {
    let Some(handle) = validate_db(db) else {
        return 0;
    };
    let slot = handle.hooks.authorizer.lock().expect("authorizer poisoned");
    let Some((cb, user_data_addr)) = *slot else {
        return 0;
    };
    let user_data = user_data_addr as *mut c_void;
    let a3 = arg3.and_then(|s| CString::new(s).ok());
    let a4 = arg4.and_then(|s| CString::new(s).ok());
    let a3p = a3.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
    let a4p = a4.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
    // SAFETY: cb signature matches the documented C ABI; CStrings live for
    // the duration of the call.
    unsafe {
        cb(
            user_data,
            action,
            a3p,
            a4p,
            std::ptr::null(),
            std::ptr::null(),
        )
    }
}

/// Consult the busy handler for retry decisions. Returns true to retry,
/// false to surface BUSY to the caller.
pub(crate) fn fire_busy(db: *mut rldb, attempts: c_int) -> bool {
    let Some(handle) = validate_db(db) else {
        return false;
    };
    let slot = handle.hooks.busy.lock().expect("busy hook poisoned");
    if let Some((cb, user_data_addr)) = *slot {
        let user_data = user_data_addr as *mut c_void;
        // SAFETY: cb signature matches the documented C ABI; user_data
        // remains valid per the standard hook contract.
        let rc = unsafe { cb(user_data, attempts) };
        return rc != 0;
    }
    false
}

// ---- Test-only helpers used by crates/ffi/tests/hooks.rs ------------------

#[doc(hidden)]
pub fn __test_fire_commit(db: *mut rldb) -> bool {
    matches!(fire_for_sql(db, "COMMIT"), CommitDecision::Veto)
}

#[doc(hidden)]
pub fn __test_fire_rollback(db: *mut rldb) {
    let _ = fire_for_sql(db, "ROLLBACK");
}

#[doc(hidden)]
pub fn __test_fire_update(db: *mut rldb, op: c_int, table: &str, rowid: i64) {
    fire_update(db, op, table, rowid);
}

#[doc(hidden)]
pub fn __test_fire_trace(db: *mut rldb, sql: &str) {
    fire_trace(db, sql);
}

#[doc(hidden)]
pub fn __test_fire_profile(db: *mut rldb, sql: &str, nanos: u64) {
    fire_profile(db, sql, nanos);
}

#[doc(hidden)]
pub fn __test_fire_authorizer(db: *mut rldb, action: c_int, table: Option<&str>) -> c_int {
    fire_authorizer(db, action, table, None)
}

#[doc(hidden)]
pub fn __test_fire_busy(db: *mut rldb, attempts: c_int) -> bool {
    fire_busy(db, attempts)
}
