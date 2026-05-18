//! SQLite-compatible `sqlite3_value_*` family.
//!
//! `RldbValue` is the opaque type behind `sqlite3_value*`. It is heap
//! allocated and owned by the FFI layer (typically constructed by the UDF
//! dispatcher in `udf.rs` when materialising a SQL row of arguments), then
//! handed to C as `*mut RldbValue` for read-only inspection.

use std::ffi::CString;
use std::os::raw::{c_int, c_uchar, c_uint, c_void};
use std::ptr;
use std::sync::{Arc, Mutex};

use redlinedb_sql::value::SqlValue;

use crate::types::*;

static DUP_VALUES: Mutex<Vec<Box<RldbValue>>> = Mutex::new(Vec::new());

/// Opaque value type backing the C `sqlite3_value*` opaque pointer.
///
/// The variant order matches SQLite's type-code mapping (`SQLITE_*`).
#[allow(non_camel_case_types)]
pub struct RldbValue {
    pub(crate) inner: RldbValueInner,
    /// Lazily populated NUL-terminated cache for `sqlite3_value_text` so the
    /// pointer returned to C remains valid for the lifetime of the value.
    pub(crate) text_cache: std::cell::RefCell<Option<CString>>,
}

#[derive(Clone)]
pub(crate) enum RldbValueInner {
    Null,
    Integer(i64),
    Real(f64),
    Text(Arc<str>),
    Blob(Arc<[u8]>),
}

impl RldbValue {
    pub fn from_sql(value: &SqlValue) -> Self {
        let inner = match value {
            SqlValue::Null => RldbValueInner::Null,
            SqlValue::Integer(i) => RldbValueInner::Integer(*i),
            SqlValue::Real(f) => RldbValueInner::Real(*f),
            SqlValue::Text(t) => RldbValueInner::Text(Arc::clone(t)),
            SqlValue::Blob(b) => RldbValueInner::Blob(Arc::clone(b)),
        };
        Self {
            inner,
            text_cache: std::cell::RefCell::new(None),
        }
    }

    pub fn to_sql(&self) -> SqlValue {
        match &self.inner {
            RldbValueInner::Null => SqlValue::Null,
            RldbValueInner::Integer(i) => SqlValue::Integer(*i),
            RldbValueInner::Real(f) => SqlValue::Real(*f),
            RldbValueInner::Text(t) => SqlValue::Text(Arc::clone(t)),
            RldbValueInner::Blob(b) => SqlValue::Blob(Arc::clone(b)),
        }
    }

    pub fn duplicate(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            text_cache: std::cell::RefCell::new(None),
        }
    }
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` originating from a
/// `Box::into_raw` in this crate, valid for shared read for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_type(value: *mut RldbValue) -> c_int {
    if value.is_null() {
        return RLDB_NULL;
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract;
    // shared borrow valid for the call duration only.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Null => RLDB_NULL,
        RldbValueInner::Integer(_) => RLDB_INTEGER,
        RldbValueInner::Real(_) => RLDB_REAL,
        RldbValueInner::Text(_) => RLDB_TEXT,
        RldbValueInner::Blob(_) => RLDB_BLOB,
    }
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_int(value: *mut RldbValue) -> c_int {
    // SAFETY: delegates to int64 then casts.
    let v = unsafe { sqlite3_value_int64(value) };
    v as c_int
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_int64(value: *mut RldbValue) -> i64 {
    if value.is_null() {
        return 0;
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Integer(i) => *i,
        RldbValueInner::Real(f) => *f as i64,
        RldbValueInner::Text(t) => t.trim().parse::<i64>().unwrap_or(0),
        RldbValueInner::Blob(b) => std::str::from_utf8(b)
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0),
        RldbValueInner::Null => 0,
    }
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_double(value: *mut RldbValue) -> f64 {
    if value.is_null() {
        return 0.0;
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Integer(i) => *i as f64,
        RldbValueInner::Real(f) => *f,
        RldbValueInner::Text(t) => t.trim().parse::<f64>().unwrap_or(0.0),
        RldbValueInner::Blob(b) => std::str::from_utf8(b)
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0),
        RldbValueInner::Null => 0.0,
    }
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate. The
/// returned pointer is valid until the value is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_text(value: *mut RldbValue) -> *const c_uchar {
    if value.is_null() {
        return ptr::null();
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    let text = match &v.inner {
        RldbValueInner::Null => return ptr::null(),
        RldbValueInner::Text(t) => t.to_string(),
        RldbValueInner::Integer(i) => i.to_string(),
        RldbValueInner::Real(f) => f.to_string(),
        RldbValueInner::Blob(b) => match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            Err(_) => return ptr::null(),
        },
    };
    let cstring = match CString::new(text) {
        Ok(c) => c,
        Err(_) => return ptr::null(),
    };
    let mut cache = v.text_cache.borrow_mut();
    *cache = Some(cstring);
    cache
        .as_ref()
        .map(|c| c.as_ptr() as *const c_uchar)
        .unwrap_or(ptr::null())
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_blob(value: *mut RldbValue) -> *const c_void {
    if value.is_null() {
        return ptr::null();
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Blob(b) => b.as_ptr() as *const c_void,
        RldbValueInner::Text(t) => t.as_ptr() as *const c_void,
        _ => ptr::null(),
    }
}

/// # Safety
/// `value` must be a non-NULL `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_bytes(value: *mut RldbValue) -> c_int {
    if value.is_null() {
        return 0;
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Blob(b) => b.len() as c_int,
        RldbValueInner::Text(t) => t.len() as c_int,
        RldbValueInner::Integer(i) => i.to_string().len() as c_int,
        RldbValueInner::Real(f) => f.to_string().len() as c_int,
        RldbValueInner::Null => 0,
    }
}

/// # Safety
/// `value` must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_dup(value: *mut RldbValue) -> *mut RldbValue {
    if value.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller obligation; non-null checked above.
    let value = unsafe { &*value };
    let mut duplicate = Box::new(value.duplicate());
    let duplicate_ptr = duplicate.as_mut() as *mut RldbValue;
    match DUP_VALUES.lock() {
        Ok(mut values) => {
            values.push(duplicate);
            duplicate_ptr
        }
        Err(_) => ptr::null_mut(),
    }
}

/// # Safety
/// `value` must be NULL or a pointer returned by `sqlite3_value_dup`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_free(value: *mut RldbValue) {
    if value.is_null() {
        return;
    }
    let target = value as usize;
    if let Ok(mut values) = DUP_VALUES.lock() {
        if let Some(index) = values
            .iter()
            .position(|stored| stored.as_ref() as *const RldbValue as usize == target)
        {
            values.swap_remove(index);
        }
    }
}

/// # Safety
/// `value` must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_numeric_type(value: *mut RldbValue) -> c_int {
    if value.is_null() {
        return RLDB_NULL;
    }
    // SAFETY: caller obligation; non-null checked above.
    let v = unsafe { &*value };
    match &v.inner {
        RldbValueInner::Integer(_) => RLDB_INTEGER,
        RldbValueInner::Real(_) => RLDB_REAL,
        RldbValueInner::Text(t) => {
            let trimmed = t.trim();
            if trimmed.parse::<i64>().is_ok() {
                RLDB_INTEGER
            } else if trimmed.parse::<f64>().is_ok() {
                RLDB_REAL
            } else {
                RLDB_TEXT
            }
        }
        RldbValueInner::Blob(_) => RLDB_BLOB,
        RldbValueInner::Null => RLDB_NULL,
    }
}

/// # Safety
/// `value` must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_subtype(_value: *mut RldbValue) -> c_uint {
    0
}

/// # Safety
/// `value` must be NULL or a valid `*mut RldbValue` from this crate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_value_frombind(_value: *mut RldbValue) -> c_int {
    0
}
