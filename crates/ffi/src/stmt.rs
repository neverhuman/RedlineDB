//! Prepared statement lifecycle (prepare/step/reset/finalize/clear_bindings).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::atomic::Ordering;

use redlinedb_sql::Step;

use crate::types::*;
use crate::util::{
    api, caller_buffer, flatten_code, map_error, record_status_with_message, refresh_text_cache,
    sql_result,
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
        // SAFETY: `db` non-null (checked); per redlinedb.h:95 from rldb_open
        // not yet closed; shared borrow scoped to api() closure (we bump
        // active_statements below which gates close).
        let db_ref = unsafe { &*db };
        // SAFETY: `sql` non-null (checked); per redlinedb.h:95 it is a
        // NUL-terminated C string (nbytes<0) or byte buffer (nbytes>=0);
        // CStr only reads leading bytes, we copy what we need below.
        let sql_cstr = unsafe { CStr::from_ptr(sql) };
        let sql_text = if nbytes < 0 {
            sql_cstr.to_str().map_err(|_| RLDB_MISMATCH)?.to_owned()
        } else {
            // SAFETY: `sql` non-null (checked); per sqlite3_prepare_v2 contract when nbytes>=0 it is the explicit byte length of the caller-owned buffer; delegate to centralised helper crates/ffi/src/util.rs::caller_buffer (see its `# Safety` doc); slice copied into owned String below.
            let bytes = unsafe { caller_buffer(sql_cstr.as_ptr() as *const u8, nbytes as usize) };
            std::str::from_utf8(bytes)
                .map_err(|_| RLDB_MISMATCH)?
                .to_owned()
        };
        // sqlite3_prepare_v2 contract: parse only the FIRST statement in
        // `sql`, set `tail` to the byte after that statement (or to the NUL
        // terminator if it was the last). We route through
        // Connection::prepare_v2 which returns the unconsumed remainder.
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
        // SAFETY: `tail` may be NULL (optional per C ABI); when non-null,
        // caller guarantees writable *const c_char and the pointer
        // arithmetic stays in-allocation (sql_cstr.as_ptr() + in-range).
        unsafe {
            if !tail.is_null() {
                *tail = sql_cstr.as_ptr().wrapping_add(consumed_bytes);
            }
        }
        let Some(stmt) = stmt_opt else {
            // SAFETY: `out_stmt` non-null (checked at top); per C ABI it is
            // a writable rldb_stmt**; storing NULL for empty/comment-only.
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
        // SAFETY: `out_stmt` non-null (checked at top); per C ABI it is a
        // writable rldb_stmt**; Box::into_raw transfers ownership to caller
        // (paired with rldb_finalize's Box::from_raw).
        unsafe {
            *out_stmt = Box::into_raw(boxed);
        }
        Ok(RLDB_OK)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rldb_step(stmt: *mut rldb_stmt) -> c_int {
    flatten_code(api(|| {
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:97 from
        // rldb_prepare_v2 not yet finalized; single-thread ownership.
        let stmt_ref = unsafe { &mut *stmt };
        let db = stmt_ref.db;
        // SAFETY: stmt_ref.db recorded at prepare time and lives at least
        // as long as the statement (prepare bumped active_statements which
        // blocks rldb_close until finalize); reads atomic flag only.
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
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:98 from
        // rldb_prepare_v2 not yet finalized; single-thread ownership.
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
        // SAFETY: matching constructor/destructor pair — `stmt` originates from
        // Box::into_raw(boxed) at rldb_prepare_v2 (crates/ffi/src/stmt.rs:100);
        // ownership invariant: the C caller may not free this pointer directly
        // per redlinedb.h:99; exclusive access because rldb_stmt is documented
        // as single-thread-owned in redlinedb.h:99; double-finalize guarded by
        // the null check above (caller must NULL stmt after rldb_finalize per
        // redlinedb.h:99); ledgered at agent/unsafe-ledger.toml
        // (file=crates/ffi/src/stmt.rs, line=169, detector=rust.unsafe.raw-parts);
        // proof: crates/ffi/tests/safety_invariants.rs::oversize_sql_is_rejected_gracefully
        // and ::parameter_index_out_of_range_returns_range.
        let boxed = unsafe { Box::from_raw(stmt) };
        // SAFETY: boxed.db is the *mut rldb recorded at prepare time;
        // rldb_close waits for active_statements==0 so the parent db is
        // still alive when we decrement here.
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
        if stmt.is_null() {
            return Err(RLDB_MISUSE);
        }
        // SAFETY: `stmt` non-null (checked); per redlinedb.h:100 from
        // rldb_prepare_v2 not yet finalized; single-thread ownership.
        let stmt = unsafe { &mut *stmt };
        stmt.stmt.clear_bindings();
        Ok(RLDB_OK)
    }))
}
