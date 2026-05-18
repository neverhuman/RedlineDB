//! Prepared statement lifecycle (prepare/step/reset/finalize/clear_bindings).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::atomic::Ordering;

use redlinedb_sql::Step;

use crate::types::*;
use crate::util::{
    api, flatten_code, map_error, record_status_with_message, refresh_text_cache, sql_result,
};

#[unsafe(no_mangle)]
pub extern "C" fn rldb_prepare_v2(
    db: *mut rldb,
    sql: *const c_char,
    nbytes: c_int,
    out_stmt: *mut *mut rldb_stmt,
    tail: *mut *const c_char,
) -> c_int {
    flatten_code(api(|| {
        if db.is_null() || sql.is_null() || out_stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        let db_ref = unsafe { &*db };
        let sql_cstr = unsafe { CStr::from_ptr(sql) };
        let sql_text = if nbytes < 0 {
            sql_cstr.to_str().map_err(|_| RLDB_MISMATCH)?.to_owned()
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(sql_cstr.as_ptr() as *const u8, nbytes as usize)
            };
            std::str::from_utf8(bytes)
                .map_err(|_| RLDB_MISMATCH)?
                .to_owned()
        };
        // sqlite3_prepare_v2 contract: parse only the FIRST statement in
        // `sql`, set `tail` to the byte after that statement (or to the NUL
        // terminator if it was the last). We achieve this by routing through
        // `Connection::prepare_v2` which returns the unconsumed remainder.
        let (stmt_opt, remainder) = match db_ref.conn.clone().prepare_v2(&sql_text) {
            Ok(pair) => pair,
            Err(err) => {
                let msg = err.to_string();
                let code = map_error(err);
                record_status_with_message(db, code, &msg);
                return Err(code);
            }
        };
        let consumed_bytes = sql_text.len() - remainder.len();
        // Set out_stmt: NULL if input was blank/comment-only (per SQLite).
        unsafe {
            if !tail.is_null() {
                *tail = sql_cstr.as_ptr().wrapping_add(consumed_bytes);
            }
        }
        let Some(stmt) = stmt_opt else {
            unsafe {
                *out_stmt = ptr::null_mut();
            }
            return Ok(RLDB_OK);
        };
        // Preserve only the consumed prefix in `sql_text` so callers that
        // read `sqlite3_sql(stmt)` see the single statement, not the
        // multi-statement input.
        let head_text = &sql_text[..consumed_bytes];
        let mut boxed = Box::new(rldb_stmt {
            db,
            stmt,
            sql_text: CString::new(head_text).map_err(|_| RLDB_MISMATCH)?,
            column_names: Vec::new(),
            text_cache: Vec::new(),
        });
        for index in 0..boxed.stmt.column_count() {
            boxed
                .column_names
                .push(CString::new(boxed.stmt.column_name(index)).map_err(|_| RLDB_MISMATCH)?);
        }
        boxed
            .text_cache
            .resize_with(boxed.stmt.column_count(), || CString::new("").unwrap());
        db_ref.active_statements.fetch_add(1, Ordering::Relaxed);
        unsafe {
            *out_stmt = Box::into_raw(boxed);
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_step(stmt: *mut rldb_stmt) -> c_int {
    flatten_code(api(|| {
        let stmt_ref = unsafe { &mut *stmt };
        let db = stmt_ref.db;
        if unsafe { (*db).interrupted.load(Ordering::Relaxed) } {
            return Err(RLDB_INTERRUPT);
        }
        match stmt_ref.stmt.step() {
            Ok(Step::Row) => {
                refresh_text_cache(stmt_ref)?;
                Ok(RLDB_ROW)
            }
            Ok(Step::Done) => Ok(RLDB_DONE),
            Err(err) => {
                let msg = err.to_string();
                let code = map_error(err);
                record_status_with_message(db, code, &msg);
                Err(code)
            }
        }
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_reset(stmt: *mut rldb_stmt) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        sql_result(stmt.stmt.reset())?;
        stmt.text_cache.clear();
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_finalize(stmt: *mut rldb_stmt) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        let boxed = unsafe { Box::from_raw(stmt) };
        unsafe {
            (*boxed.db)
                .active_statements
                .fetch_sub(1, Ordering::Relaxed);
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_clear_bindings(stmt: *mut rldb_stmt) -> c_int {
    flatten_code(api(|| {
        let stmt = unsafe { &mut *stmt };
        stmt.stmt.clear_bindings();
        Ok(RLDB_OK)
    }))
}
