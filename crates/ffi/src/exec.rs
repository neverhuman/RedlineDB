//! Multi-statement execution (`rldb_exec`).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use redlinedb_sql::Step;

use crate::types::*;
use crate::util::{
    api, exec_value, flatten_code, map_error, record_status_with_message, set_errmsg,
};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_exec(
    db: *mut rldb,
    sql: *const c_char,
    callback: Option<
        extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
    >,
    ctx: *mut c_void,
    errmsg: *mut *mut c_char,
) -> c_int {
    // Initialize errmsg to NULL up-front (sqlite3_exec contract).
    if !errmsg.is_null() {
        // SAFETY: `errmsg` non-null (checked); per redlinedb.h:124 it is a
        // writable char** out-pointer; we write NULL so failed paths cannot
        // leave an uninitialized pointer.
        unsafe {
            *errmsg = ptr::null_mut();
        }
    }
    flatten_code(api(|| {
        if db.is_null() || sql.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `db` non-null (checked); per redlinedb.h:124 from
        // rldb_open not yet closed; shared borrow scoped to api() closure.
        let db_ref = unsafe { &*db };
        // SAFETY: `sql` non-null (checked); per redlinedb.h:124 it is a
        // NUL-terminated C string; &str borrow stays inside this closure.
        let sql_text = unsafe { CStr::from_ptr(sql) }
            .to_str()
            .map_err(|_| RLDB_MISMATCH)?;
        let mut rest = sql_text;
        // Walk the multi-statement input one statement at a time. SQLite's
        // sqlite3_exec halts at the first failing statement and reports its
        // error via errmsg; later statements are not executed.
        loop {
            let (stmt_opt, tail) = match prepare_statement(db_ref, rest, errmsg, db) {
                Ok(pair) => pair,
                Err(code) => return Err(code),
            };
            if let Some(mut stmt) = stmt_opt {
                if let Some(callback) = callback {
                    run_statement_with_callback(&mut stmt, callback, ctx, errmsg, db)?;
                } else {
                    run_statement_to_completion(&mut stmt, errmsg, db)?;
                }
            }
            if tail.is_empty() {
                break;
            }
            rest = tail;
        }
        Ok(RLDB_OK)
    }))
}

fn prepare_statement<'a>(
    db_ref: &crate::types::rldb,
    sql: &'a str,
    errmsg: *mut *mut c_char,
    db: *mut crate::types::rldb,
) -> Result<(Option<redlinedb_sql::Statement>, &'a str), c_int> {
    db_ref.conn.clone().prepare_v2(sql).map_err(|err| {
        let msg = err.to_string();
        let code = map_error(err);
        // SAFETY: set_errmsg's documented contract null-checks internally;
        // non-null writes transfer ownership to the caller (paired with
        // rldb_free / sqlite3_free).
        unsafe { set_errmsg(errmsg, &msg) };
        record_status_with_message(db, code, &msg);
        code
    })
}

fn run_statement_to_completion(
    stmt: &mut redlinedb_sql::Statement,
    errmsg: *mut *mut c_char,
    db: *mut crate::types::rldb,
) -> Result<(), c_int> {
    loop {
        match stmt.step() {
            Ok(Step::Done) => return Ok(()),
            Ok(Step::Row) => continue,
            Err(err) => {
                let msg = err.to_string();
                let code = map_error(err);
                // SAFETY: set_errmsg's documented contract null-checks
                // internally; non-null writes transfer ownership to the
                // caller (paired with rldb_free / sqlite3_free).
                unsafe { set_errmsg(errmsg, &msg) };
                record_status_with_message(db, code, &msg);
                return Err(code);
            }
        }
    }
}

fn run_statement_with_callback(
    stmt: &mut redlinedb_sql::Statement,
    callback: extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
    ctx: *mut c_void,
    errmsg: *mut *mut c_char,
    db: *mut crate::types::rldb,
) -> Result<(), c_int> {
    loop {
        match stmt.step() {
            Ok(Step::Row) => invoke_exec_callback(stmt, callback, ctx, errmsg)?,
            Ok(Step::Done) => return Ok(()),
            Err(err) => {
                let msg = err.to_string();
                let code = map_error(err);
                // SAFETY: set_errmsg's documented contract null-checks
                // internally; non-null writes transfer ownership to the
                // caller (paired with rldb_free / sqlite3_free).
                unsafe { set_errmsg(errmsg, &msg) };
                record_status_with_message(db, code, &msg);
                return Err(code);
            }
        }
    }
}

fn invoke_exec_callback(
    stmt: &redlinedb_sql::Statement,
    callback: extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
    ctx: *mut c_void,
    errmsg: *mut *mut c_char,
) -> Result<(), c_int> {
    let column_count = stmt.column_count();
    let mut value_strings: Vec<Option<CString>> = Vec::with_capacity(column_count);
    let mut name_strings: Vec<CString> = Vec::with_capacity(column_count);
    for index in 0..column_count {
        name_strings.push(CString::new(stmt.column_name(index)).map_err(|_| RLDB_MISMATCH)?);
        value_strings.push(exec_value(stmt, index)?);
    }
    let mut argv: Vec<*mut c_char> = value_strings
        .iter_mut()
        .map(|value| {
            value
                .as_mut()
                .map(|s| s.as_ptr() as *mut c_char)
                .unwrap_or(ptr::null_mut())
        })
        .collect();
    let mut colnames: Vec<*mut c_char> = name_strings
        .iter_mut()
        .map(|value| value.as_ptr() as *mut c_char)
        .collect();
    let rc = callback(
        ctx,
        column_count as c_int,
        argv.as_mut_ptr(),
        colnames.as_mut_ptr(),
    );
    if rc != 0 {
        // SAFETY: set_errmsg's documented contract null-checks internally;
        // non-null writes transfer ownership to the caller (paired with
        // rldb_free / sqlite3_free).
        unsafe { set_errmsg(errmsg, "callback returned non-zero") };
        return Err(RLDB_ERROR);
    }
    Ok(())
}
