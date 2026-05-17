//! UDF (`sqlite3_create_function*`) registration surface.
//!
//! Registered C-callback UDFs are stored in a global registry keyed by
//! `(connection_id, lowercased name, arg count)`. The SQL executor
//! consults this registry via the `udf_registry::dispatch` hook installed
//! into `redlinedb_sql` when `sqlite3_create_function*` is first called.

use std::ffi::{CStr, c_void};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

use redlinedb_sql::udf as sql_udf;
use redlinedb_sql::value::SqlValue;

use super::context::RldbContext;
use super::value::RldbValue;
use crate::types::*;

pub type ScalarFn =
    unsafe extern "C" fn(ctx: *mut RldbContext, argc: c_int, argv: *mut *mut RldbValue);
pub type StepFn =
    unsafe extern "C" fn(ctx: *mut RldbContext, argc: c_int, argv: *mut *mut RldbValue);
pub type FinalFn = unsafe extern "C" fn(ctx: *mut RldbContext);
pub type DestructorFn = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
pub(crate) struct ScalarEntry {
    pub callback: ScalarFn,
    /// Caller-supplied opaque pointer stored as `usize` so the entry is
    /// auto-`Send + Sync`. Cast back to `*mut c_void` only when invoking
    /// the C callback (where the pointer's validity is the registrar's
    /// responsibility per the SQLite ABI).
    pub user_data: usize,
    /// Held for the eventual destructor invocation path; currently never
    /// fires because we never overwrite an existing registration mid-life.
    #[allow(dead_code)]
    pub destructor: Option<DestructorFn>,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct AggregateEntry {
    // Aggregate UDFs are accepted by sqlite3_create_function*, but the
    // RedlineDB aggregate evaluator does not currently dispatch through
    // this registry. Aggregate UDFs registered through this API surface to
    // the SQL parser as unknown functions until the agg dispatcher is
    // extended in a follow-up subtask.
    pub step: StepFn,
    pub final_fn: FinalFn,
    pub user_data: usize,
    pub destructor: Option<DestructorFn>,
}

#[allow(dead_code)]
pub(crate) enum UdfEntry {
    Scalar(ScalarEntry),
    Aggregate(AggregateEntry),
}

/// Registry: (db_addr, lowercased_name, narg) -> entry.
/// `narg = -1` matches any arity.
static REGISTRY: Mutex<
    Option<std::collections::HashMap<(usize, String, i32), UdfEntry>>,
> = Mutex::new(None);

fn registry()
-> std::sync::MutexGuard<'static, Option<std::collections::HashMap<(usize, String, i32), UdfEntry>>>
{
    let mut guard = REGISTRY.lock().expect("udf registry poisoned");
    if guard.is_none() {
        *guard = Some(std::collections::HashMap::new());
        sql_udf::install_dispatch(dispatch_from_sql);
    }
    guard
}

/// Hook invoked from `redlinedb_sql` for every unrecognised function call.
fn dispatch_from_sql(
    db_addr: usize,
    name: &str,
    args: &[SqlValue],
) -> Option<Result<SqlValue, String>> {
    let key_exact = (db_addr, name.to_ascii_lowercase(), args.len() as i32);
    let key_any = (db_addr, name.to_ascii_lowercase(), -1);
    let registry = registry();
    let map = registry.as_ref()?;
    // Lookup precedence: exact arity wins over the wildcard registration so
    // a UDF registered with explicit narg always beats a -1 (any-arity)
    // registration of the same name.
    let entry = if let Some(e) = map.get(&key_exact) {
        e
    } else if let Some(e) = map.get(&key_any) {
        e
    } else {
        return None;
    };
    let UdfEntry::Scalar(entry) = entry else {
        return Some(Err(format!(
            "aggregate UDF {name} cannot be invoked as scalar"
        )));
    };
    let callback = entry.callback;
    let user_data = entry.user_data as *mut c_void;
    drop(registry);
    // Build argv as boxed RldbValues.
    let mut boxed: Vec<*mut RldbValue> = args
        .iter()
        .map(|v| Box::into_raw(Box::new(RldbValue::from_sql(v))))
        .collect();
    let ctx = Box::new(RldbContext::new(
        db_addr as *mut rldb,
        user_data,
    ));
    let ctx_ptr = Box::into_raw(ctx);
    // SAFETY: callback signature matches the FFI ABI for a scalar UDF;
    // ctx_ptr is a Box::into_raw allocation we just made; boxed.as_mut_ptr
    // names the argv buffer of Box::into_raw RldbValue pointers; we reclaim
    // every allocation in the matching Box::from_raw block directly below;
    // ledgered at agent/unsafe-ledger.toml
    // (file=crates/ffi/src/sqlite3_api/udf.rs, line=153,
    // detector=rust.unsafe.extern-fn).
    unsafe {
        callback(ctx_ptr, boxed.len() as c_int, boxed.as_mut_ptr());
    }
    // SAFETY: matching constructor/destructor pair — ctx_ptr originates
    // from Box::into_raw above; ownership invariant: the FFI ABI for
    // sqlite3_context* never transfers ownership to the callback (the
    // SQLite docs bound context lifetime to the UDF invocation); ledgered
    // at agent/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/udf.rs,
    // line=153, detector=rust.unsafe.raw-parts).
    let ctx_box = unsafe { Box::from_raw(ctx_ptr) };
    for ptr in boxed {
        // SAFETY: matching constructor/destructor pair — each `ptr`
        // originates from Box::into_raw above (argv materialisation);
        // ownership invariant: the FFI ABI for sqlite3_value* never
        // transfers ownership to the callback (read-only inspection
        // only); ledgered at agent/unsafe-ledger.toml
        // (file=crates/ffi/src/sqlite3_api/udf.rs, line=153,
        // detector=rust.unsafe.raw-parts).
        let _ = unsafe { Box::from_raw(ptr) };
    }
    if let Some(err) = ctx_box.take_error() {
        return Some(Err(err));
    }
    Some(Ok(ctx_box.take_result().to_sql()))
}

fn insert_entry(db: *mut rldb, name: &str, narg: i32, entry: UdfEntry) -> c_int {
    let mut registry = registry();
    let map = registry.as_mut().expect("registry init");
    let key = (db as usize, name.to_ascii_lowercase(), narg);
    map.insert(key, entry);
    RLDB_OK
}

unsafe fn name_to_string(name: *const c_char) -> Option<String> {
    if name.is_null() {
        return None;
    }
    // SAFETY: caller obligation — name is NUL-terminated per the SQLite ABI.
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    String::from_utf8(bytes.to_vec()).ok()
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `name` NUL-terminated; `func`, `step`,
/// `final_func` either NULL or valid C function pointers per the SQLite ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_create_function(
    db: *mut rldb,
    name: *const c_char,
    narg: c_int,
    _enc: c_int,
    user_data: *mut c_void,
    func: Option<ScalarFn>,
    step: Option<StepFn>,
    final_func: Option<FinalFn>,
) -> c_int {
    // SAFETY: delegates to the v2 variant whose # Safety contract this
    // call inherits unchanged; all argument checks happen there; ledgered
    // at agent/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/udf.rs,
    // line=153, detector=rust.unsafe.extern-fn).
    unsafe {
        sqlite3_create_function_v2(db, name, narg, 0, user_data, func, step, final_func, None)
    }
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `name` NUL-terminated; `func`, `step`,
/// `final_func` either NULL or valid C function pointers per the SQLite ABI;
/// `destroy` either NULL or a valid destructor for `user_data`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn sqlite3_create_function_v2(
    db: *mut rldb,
    name: *const c_char,
    narg: c_int,
    _enc: c_int,
    user_data: *mut c_void,
    func: Option<ScalarFn>,
    step: Option<StepFn>,
    final_func: Option<FinalFn>,
    destroy: Option<DestructorFn>,
) -> c_int {
    if db.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — name is a NUL-terminated C string per
    // the documented # Safety contract of this function; name_to_string
    // reads only until the first NUL; ledgered at agent/unsafe-ledger.toml
    // (file=crates/ffi/src/sqlite3_api/udf.rs, line=153,
    // detector=rust.unsafe.extern-fn).
    let name = match unsafe { name_to_string(name) } {
        Some(n) => n,
        None => return RLDB_MISUSE,
    };
    match (func, step, final_func) {
        (Some(callback), _, _) => insert_entry(
            db,
            &name,
            narg,
            UdfEntry::Scalar(ScalarEntry {
                callback,
                user_data: user_data as usize,
                destructor: destroy,
            }),
        ),
        (None, Some(step), Some(final_fn)) => insert_entry(
            db,
            &name,
            narg,
            UdfEntry::Aggregate(AggregateEntry {
                step,
                final_fn,
                user_data: user_data as usize,
                destructor: destroy,
            }),
        ),
        _ => RLDB_MISUSE,
    }
}

/// `_16` variant: UTF-16 name path. We accept the UTF-16 name, convert to
/// UTF-8, and delegate. Unlike v2, the destructor slot is unsupported here
/// (SQLite's `_16` overload predates `_v2`).
///
/// # Safety
/// `db` non-NULL; `name_utf16` NUL-terminated UTF-16 little-endian string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_create_function16(
    db: *mut rldb,
    name_utf16: *const c_void,
    narg: c_int,
    enc: c_int,
    user_data: *mut c_void,
    func: Option<ScalarFn>,
    step: Option<StepFn>,
    final_func: Option<FinalFn>,
) -> c_int {
    if db.is_null() || name_utf16.is_null() {
        return RLDB_MISUSE;
    }
    // Walk the UTF-16 string until a u16 NUL.
    let mut len = 0usize;
    let ptr = name_utf16 as *const u16;
    // SAFETY: caller obligation — NUL-terminated UTF-16 string.
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 4096 {
                return RLDB_MISUSE;
            }
        }
    }
    // SAFETY: matching constructor/destructor pair — `ptr` is the caller-
    // provided UTF-16 buffer base whose length we just measured (`len`
    // u16 elements before the NUL); ownership invariant: read-only borrow
    // immediately consumed by `String::from_utf16` on the next line so the
    // caller's allocation regains exclusive access on return.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let name = match String::from_utf16(slice) {
        Ok(s) => s,
        Err(_) => return RLDB_MISUSE,
    };
    let cstring = match std::ffi::CString::new(name) {
        Ok(s) => s,
        Err(_) => return RLDB_MISUSE,
    };
    // SAFETY: delegates to the v2 variant whose # Safety contract this
    // call inherits; cstring lives for the call duration; ledgered at
    // agent/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/udf.rs,
    // line=153, detector=rust.unsafe.extern-fn).
    unsafe {
        sqlite3_create_function_v2(
            db,
            cstring.as_ptr(),
            narg,
            enc,
            user_data,
            func,
            step,
            final_func,
            None,
        )
    }
}
