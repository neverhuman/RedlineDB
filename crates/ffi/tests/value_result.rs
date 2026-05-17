//! B1 value/result/context family round-trips.
//!
//! These tests exercise the new `sqlite3_value_*` / `sqlite3_result_*` /
//! `sqlite3_context_db_handle` / `sqlite3_user_data` shims by constructing
//! `RldbValue` / `RldbContext` directly and round-tripping each variant.

use std::ffi::{CStr, CString};
use std::os::raw::{c_int, c_void};
use std::ptr;

use redlinedb::sqlite3_api::context::{RldbContext, sqlite3_context_db_handle, sqlite3_user_data};
use redlinedb::sqlite3_api::result::{
    sqlite3_result_blob, sqlite3_result_double, sqlite3_result_error,
    sqlite3_result_error_code, sqlite3_result_int, sqlite3_result_int64,
    sqlite3_result_null, sqlite3_result_text,
};
use redlinedb::sqlite3_api::value::{
    RldbValue, sqlite3_value_blob, sqlite3_value_bytes, sqlite3_value_double, sqlite3_value_int,
    sqlite3_value_int64, sqlite3_value_text, sqlite3_value_type,
};
use redlinedb::types::rldb;
use redlinedb_sql::value::SqlValue;

fn make_value(value: SqlValue) -> *mut RldbValue {
    Box::into_raw(Box::new(RldbValue::from_sql(&value)))
}

fn free_value(ptr: *mut RldbValue) {
    if !ptr.is_null() {
        // SAFETY: matching constructor/destructor pair — pointer originates from
        // make_value's Box::into_raw above; not freed by FFI.
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[test]
fn null_value_reports_null_type_and_zero_accessors() {
    let v = make_value(SqlValue::Null);
    unsafe {
        assert_eq!(sqlite3_value_type(v), 0); // SQLITE_NULL
        assert_eq!(sqlite3_value_int(v), 0);
        assert_eq!(sqlite3_value_int64(v), 0);
        assert_eq!(sqlite3_value_double(v), 0.0);
        assert!(sqlite3_value_text(v).is_null());
        assert!(sqlite3_value_blob(v).is_null());
        assert_eq!(sqlite3_value_bytes(v), 0);
    }
    free_value(v);
}

#[test]
fn integer_value_round_trips_and_promotes_to_double() {
    let v = make_value(SqlValue::Integer(42));
    unsafe {
        assert_eq!(sqlite3_value_type(v), 1);
        assert_eq!(sqlite3_value_int(v), 42);
        assert_eq!(sqlite3_value_int64(v), 42);
        assert_eq!(sqlite3_value_double(v), 42.0);
    }
    free_value(v);
}

#[test]
fn text_value_returns_nul_terminated_pointer() {
    let v = make_value(SqlValue::Text(std::sync::Arc::from("hello")));
    unsafe {
        assert_eq!(sqlite3_value_type(v), 3);
        let ptr = sqlite3_value_text(v);
        assert!(!ptr.is_null());
        let cstr = CStr::from_ptr(ptr as *const i8);
        assert_eq!(cstr.to_str().unwrap(), "hello");
        assert_eq!(sqlite3_value_bytes(v), 5);
    }
    free_value(v);
}

#[test]
fn blob_value_returns_pointer_and_length() {
    let v = make_value(SqlValue::Blob(std::sync::Arc::from(b"\x00\x01\x02".as_slice())));
    unsafe {
        assert_eq!(sqlite3_value_type(v), 4);
        let ptr = sqlite3_value_blob(v) as *const u8;
        assert!(!ptr.is_null());
        let slice = std::slice::from_raw_parts(ptr, 3);
        assert_eq!(slice, &[0u8, 1, 2]);
        assert_eq!(sqlite3_value_bytes(v), 3);
    }
    free_value(v);
}

#[test]
fn result_setters_populate_context_slot() {
    let user_data: *mut c_void = 0xdead_beefusize as *mut c_void;
    let db: *mut rldb = ptr::null_mut();
    let ctx = Box::into_raw(Box::new(RldbContext::new(db, user_data)));
    unsafe {
        assert_eq!(sqlite3_user_data(ctx), user_data);
        assert!(sqlite3_context_db_handle(ctx).is_null());

        sqlite3_result_int(ctx, 7);
        let result = (*ctx).take_result();
        assert_eq!(result.to_sql(), SqlValue::Integer(7));

        sqlite3_result_int64(ctx, 1_000_000_000_000);
        let result = (*ctx).take_result();
        assert_eq!(result.to_sql(), SqlValue::Integer(1_000_000_000_000));

        sqlite3_result_double(ctx, 3.14);
        let result = (*ctx).take_result();
        assert_eq!(result.to_sql(), SqlValue::Real(3.14));

        let text = CString::new("rldb").unwrap();
        sqlite3_result_text(ctx, text.as_ptr(), -1, None);
        let result = (*ctx).take_result();
        assert!(matches!(result.to_sql(), SqlValue::Text(s) if s.as_ref() == "rldb"));

        let bytes: [u8; 3] = [0xAA, 0xBB, 0xCC];
        sqlite3_result_blob(ctx, bytes.as_ptr() as *const c_void, 3, None);
        let result = (*ctx).take_result();
        assert!(matches!(result.to_sql(), SqlValue::Blob(b) if b.as_ref() == bytes));

        sqlite3_result_null(ctx);
        let result = (*ctx).take_result();
        assert_eq!(result.to_sql(), SqlValue::Null);

        // Error path.
        let msg = CString::new("nope").unwrap();
        sqlite3_result_error(ctx, msg.as_ptr(), -1);
        sqlite3_result_error_code(ctx, 19); // SQLITE_CONSTRAINT
        assert_eq!((*ctx).take_error().as_deref(), Some("nope"));
        assert_eq!((*ctx).take_error_code(), Some(19));

        drop(Box::from_raw(ctx));
    }
}

#[test]
fn null_pointers_to_value_accessors_are_safe() {
    unsafe {
        let null: *mut RldbValue = ptr::null_mut();
        assert_eq!(sqlite3_value_type(null), 0);
        assert_eq!(sqlite3_value_int(null), 0);
        assert_eq!(sqlite3_value_int64(null), 0);
        assert_eq!(sqlite3_value_double(null), 0.0);
        assert!(sqlite3_value_text(null).is_null());
        assert!(sqlite3_value_blob(null).is_null());
        assert_eq!(sqlite3_value_bytes(null), 0);
    }
}

#[test]
fn null_pointers_to_result_setters_are_no_op() {
    unsafe {
        let null: *mut RldbContext = ptr::null_mut();
        sqlite3_result_int(null, 1);
        sqlite3_result_int64(null, 1);
        sqlite3_result_double(null, 1.0);
        sqlite3_result_text(null, ptr::null(), -1, None);
        sqlite3_result_blob(null, ptr::null(), 0, None);
        sqlite3_result_null(null);
        sqlite3_result_error(null, ptr::null(), -1);
        sqlite3_result_error_code(null, 0);
        assert!(sqlite3_user_data(null).is_null());
        assert!(sqlite3_context_db_handle(null).is_null());
    }
    let _ = unused_silencer(0);
}

fn unused_silencer(x: c_int) -> c_int {
    x
}
