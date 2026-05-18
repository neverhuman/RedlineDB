//! SQLite-compatible `sqlite3_result_*` family.
//!
//! Each setter stores a value into the `RldbContext`'s result slot; the UDF
//! dispatcher in `udf.rs` extracts that value when the C callback returns.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::Arc;

use super::context::RldbContext;
use super::value::{RldbValue, RldbValueInner};
use crate::types::{RLDB_NOMEM, RLDB_TOOBIG};
use crate::util::caller_buffer;

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_int(ctx: *mut RldbContext, value: c_int) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Integer(value as i64);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_int64(ctx: *mut RldbContext, value: i64) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Integer(value);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_double(ctx: *mut RldbContext, value: f64) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Real(value);
    }
}

/// # Safety
/// `ctx` non-NULL from this crate; `text` either NULL or a valid C string;
/// `nbytes < 0` means "until NUL", else the exact byte count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_text(
    ctx: *mut RldbContext,
    text: *const c_char,
    nbytes: c_int,
    _destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if ctx.is_null() {
        return;
    }
    if text.is_null() {
        // SAFETY: caller obligation; non-null ctx checked above.
        let ctx = unsafe { &*ctx };
        if let Ok(mut slot) = ctx.result.lock() {
            *slot = RldbValueInner::Null;
        }
        return;
    }
    let bytes: Vec<u8> = if nbytes < 0 {
        // SAFETY: caller obligation — text is NUL-terminated; we trust the
        // C ABI contract documented on this function.
        unsafe { CStr::from_ptr(text).to_bytes().to_vec() }
    } else {
        // SAFETY: caller obligation — text is valid for reads of `nbytes`
        // consecutive bytes; routed through caller_buffer per its contract.
        unsafe { caller_buffer(text as *const u8, nbytes as usize).to_vec() }
    };
    let s = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return,
    };
    // SAFETY: caller obligation; non-null ctx checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Text(Arc::from(s));
    }
}

/// # Safety
/// `ctx` non-NULL from this crate; `data` valid for reads of `nbytes` bytes
/// or NULL (then treated as zero-length blob).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_blob(
    ctx: *mut RldbContext,
    data: *const c_void,
    nbytes: c_int,
    _destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if ctx.is_null() {
        return;
    }
    let bytes: Vec<u8> = if data.is_null() || nbytes <= 0 {
        Vec::new()
    } else {
        // SAFETY: caller obligation — data is valid for reads of `nbytes`
        // consecutive bytes; routed through caller_buffer per its contract.
        unsafe { caller_buffer(data as *const u8, nbytes as usize).to_vec() }
    };
    // SAFETY: caller obligation; non-null ctx checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Blob(Arc::from(bytes.as_slice()));
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_null(ctx: *mut RldbContext) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Null;
    }
}

/// # Safety
/// `ctx` non-NULL from this crate; `msg` either NULL or NUL-terminated when
/// `nbytes < 0`, else valid for `nbytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_error(
    ctx: *mut RldbContext,
    msg: *const c_char,
    nbytes: c_int,
) {
    if ctx.is_null() {
        return;
    }
    let message = if msg.is_null() {
        "error".to_owned()
    } else if nbytes < 0 {
        // SAFETY: caller obligation — msg is NUL-terminated.
        unsafe { CStr::from_ptr(msg).to_string_lossy().into_owned() }
    } else {
        // SAFETY: caller obligation — msg valid for reads of `nbytes` bytes.
        let bytes = unsafe { caller_buffer(msg as *const u8, nbytes as usize) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    // SAFETY: caller obligation; non-null ctx checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.error.lock() {
        *slot = Some(message);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_error_code(ctx: *mut RldbContext, code: c_int) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.error_code.lock() {
        *slot = Some(code);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate and `value`
/// must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_value(ctx: *mut RldbContext, value: *mut RldbValue) {
    if ctx.is_null() {
        return;
    }
    if value.is_null() {
        // SAFETY: ctx is non-null as checked above.
        unsafe { sqlite3_result_null(ctx) };
        return;
    }
    // SAFETY: caller obligation; non-null checked above.
    let value = unsafe { &*value };
    // SAFETY: caller obligation; non-null ctx checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = match value.to_sql() {
            redlinedb_sql::value::SqlValue::Null => RldbValueInner::Null,
            redlinedb_sql::value::SqlValue::Integer(i) => RldbValueInner::Integer(i),
            redlinedb_sql::value::SqlValue::Real(f) => RldbValueInner::Real(f),
            redlinedb_sql::value::SqlValue::Text(t) => RldbValueInner::Text(t),
            redlinedb_sql::value::SqlValue::Blob(b) => RldbValueInner::Blob(b),
        };
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_zeroblob(ctx: *mut RldbContext, nbytes: c_int) {
    if ctx.is_null() || nbytes < 0 {
        return;
    }
    let bytes = vec![0; nbytes as usize];
    // SAFETY: caller obligation; non-null ctx checked above.
    let ctx = unsafe { &*ctx };
    if let Ok(mut slot) = ctx.result.lock() {
        *slot = RldbValueInner::Blob(Arc::from(bytes.into_boxed_slice()));
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_zeroblob64(ctx: *mut RldbContext, nbytes: u64) -> c_int {
    if nbytes > c_int::MAX as u64 {
        // SAFETY: delegates to the documented error-code setter.
        unsafe { sqlite3_result_error_toobig(ctx) };
        return RLDB_TOOBIG;
    }
    // SAFETY: delegates to the 32/usize-backed zeroblob implementation.
    unsafe { sqlite3_result_zeroblob(ctx, nbytes as c_int) };
    0
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_error_nomem(ctx: *mut RldbContext) {
    // SAFETY: delegates to documented result setters.
    unsafe {
        sqlite3_result_error_code(ctx, RLDB_NOMEM);
        sqlite3_result_error(ctx, c"out of memory".as_ptr(), -1);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_error_toobig(ctx: *mut RldbContext) {
    // SAFETY: delegates to documented result setters.
    unsafe {
        sqlite3_result_error_code(ctx, RLDB_TOOBIG);
        sqlite3_result_error(ctx, c"string or blob too big".as_ptr(), -1);
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_result_subtype(_ctx: *mut RldbContext, _subtype: c_uint) {}
