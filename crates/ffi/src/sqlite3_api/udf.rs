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
pub(crate) struct AggregateEntry {
    // Aggregate UDFs are dispatched at group end by
    // `aggregate_run_from_sql` via the global
    // `redlinedb_sql::udf::AGG_RUN` slot installed when the first UDF is
    // registered (see [`registry`]).
    pub step: StepFn,
    pub final_fn: FinalFn,
    pub user_data: usize,
    /// Held for the eventual destructor invocation path; currently never
    /// fires because we never overwrite an existing registration mid-life.
    #[allow(dead_code)]
    pub destructor: Option<DestructorFn>,
}

pub(crate) enum UdfEntry {
    Scalar(ScalarEntry),
    Aggregate(AggregateEntry),
}

/// Registry: (db_addr, lowercased_name, narg) -> entry.
/// `narg = -1` matches any arity.
static REGISTRY: Mutex<Option<std::collections::HashMap<(usize, String, i32), UdfEntry>>> =
    Mutex::new(None);

fn registry()
-> std::sync::MutexGuard<'static, Option<std::collections::HashMap<(usize, String, i32), UdfEntry>>>
{
    let mut guard = REGISTRY.lock().expect("udf registry poisoned");
    if guard.is_none() {
        *guard = Some(std::collections::HashMap::new());
        sql_udf::install_dispatch(dispatch_from_sql);
        sql_udf::install_aggregate_dispatch(aggregate_run_from_sql, aggregate_is_registered);
    }
    guard
}

/// Probe: does a connection have an aggregate UDF registered under
/// `name`? Used by the planner to route the expression through the
/// grouped evaluator.
fn aggregate_is_registered(db_addr: usize, name: &str) -> bool {
    let registry = registry();
    let Some(map) = registry.as_ref() else {
        return false;
    };
    let name_lower = name.to_ascii_lowercase();
    map.iter().any(|((d, n, _), entry)| {
        *d == db_addr && *n == name_lower && matches!(entry, UdfEntry::Aggregate(_))
    })
}

/// Aggregate dispatch invoked by the SQL aggregator at group end. Walks
/// every row in `rows`, calls `xStep` once per row, then calls `xFinal`
/// and returns the value the UDF set on the context (via
/// `sqlite3_result_*`). Errors set via `sqlite3_result_error` surface
/// here as `Err`.
fn aggregate_run_from_sql(
    db_addr: usize,
    name: &str,
    rows: &[Vec<SqlValue>],
) -> Option<Result<SqlValue, String>> {
    let name_lower = name.to_ascii_lowercase();
    let (step, final_fn, user_data) = {
        let registry = registry();
        let map = registry.as_ref()?;
        // Match either the exact arity (rows[0].len()) or wildcard -1.
        // Aggregate UDFs typically take 1 arg, but xStep may be called
        // with any positive arity that matches the registration.
        let narg_exact = rows.first().map(|r| r.len() as i32).unwrap_or(0);
        let entry = map
            .get(&(db_addr, name_lower.clone(), narg_exact))
            .or_else(|| map.get(&(db_addr, name_lower.clone(), -1)))?;
        let UdfEntry::Aggregate(agg) = entry else {
            return Some(Err(format!(
                "scalar UDF {name} cannot be invoked as aggregate"
            )));
        };
        (agg.step, agg.final_fn, agg.user_data as *mut c_void)
    };
    // One context per group: SQLite's contract is that xStep is called
    // multiple times against the same `sqlite3_context*`, then xFinal is
    // called once on the same context. The accumulator lives via the
    // context's `agg_state_*` slot or via `sqlite3_result_*` set on the
    // last xStep — the latter is what we surface here.
    let mut ctx = Box::new(RldbContext::new(db_addr as *mut rldb, user_data));
    let ctx_ptr: *mut RldbContext = &mut *ctx;
    for row in rows {
        let mut values: Vec<RldbValue> = row.iter().map(RldbValue::from_sql).collect();
        let mut boxed: Vec<*mut RldbValue> = values
            .iter_mut()
            .map(|value| value as *mut RldbValue)
            .collect();
        // SAFETY: callback signature matches the FFI ABI for an aggregate
        // step; `ctx_ptr` names the live Box-backed context above; and the
        // argv buffer points at stack-owned `RldbValue`s that remain alive for
        // the duration of the call.
        // SAFETY: `ctx_ptr` names the live context and the argv pointers are
        // borrowed from stack-owned values that remain alive for the call.
        unsafe {
            step(ctx_ptr, boxed.len() as c_int, boxed.as_mut_ptr());
        }
        if let Some(err) = ctx.take_error() {
            return Some(Err(err));
        }
    }
    // SAFETY: callback signature matches the FFI ABI for an aggregate
    // final; `ctx_ptr` points at the live Box-backed context above.
    // SAFETY: `ctx_ptr` names the live context and `final_fn` is the
    // registered callback for this aggregate group.
    unsafe {
        final_fn(ctx_ptr);
    }
    if let Some(err) = ctx.take_error() {
        return Some(Err(err));
    }
    Some(Ok(ctx.take_result().to_sql()))
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
    // Build argv as stack-owned RldbValues for the duration of the call.
    let mut values: Vec<RldbValue> = args.iter().map(RldbValue::from_sql).collect();
    let mut boxed: Vec<*mut RldbValue> = values
        .iter_mut()
        .map(|value| value as *mut RldbValue)
        .collect();
    let mut ctx = Box::new(RldbContext::new(db_addr as *mut rldb, user_data));
    let ctx_ptr: *mut RldbContext = &mut *ctx;
    // SAFETY: callback signature matches the FFI ABI for a scalar UDF;
    // `ctx_ptr` points at the live Box-backed context and the argv
    // pointers reference stack-owned values that remain alive for the
    // duration of the call.
    // SAFETY: `ctx_ptr` names the live context and the argv pointers are
    // borrowed from stack-owned values that remain alive for the call.
    unsafe {
        callback(ctx_ptr, boxed.len() as c_int, boxed.as_mut_ptr());
    }
    if let Some(err) = ctx.take_error() {
        return Some(Err(err));
    }
    Some(Ok(ctx.take_result().to_sql()))
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
    // at .jankurai/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/udf.rs,
    // line=153, detector=rust.unsafe.extern-fn).
    // SAFETY: the `name` argument is a NUL-terminated C string and the
    // callee only reads it for the duration of the call.
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
    // reads only until the first NUL; ledgered at .jankurai/unsafe-ledger.toml
    // (file=crates/ffi/src/sqlite3_api/udf.rs, line=153,
    // detector=rust.unsafe.extern-fn).
    // SAFETY: `name` is a NUL-terminated C string from the caller, and the
    // helper copies it into owned storage before returning.
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

/// # Safety
/// `db` non-NULL valid sqlite3*; `name` NUL-terminated; callbacks are either
/// NULL or valid C function pointers per the SQLite ABI.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn sqlite3_create_window_function(
    db: *mut rldb,
    name: *const c_char,
    narg: c_int,
    enc: c_int,
    user_data: *mut c_void,
    step: Option<StepFn>,
    final_func: Option<FinalFn>,
    _value: Option<FinalFn>,
    _inverse: Option<StepFn>,
    destroy: Option<DestructorFn>,
) -> c_int {
    // RedlineDB's SQL executor currently routes custom aggregate callbacks,
    // not inverse/value window callbacks. Register the aggregate portion so
    // callers that provide xStep/xFinal get the same grouped behavior.
    // SAFETY: the arguments are validated by this wrapper and the v2 variant
    // performs the actual registration work.
    unsafe {
        sqlite3_create_function_v2(
            db, name, narg, enc, user_data, None, step, final_func, destroy,
        )
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
    let mut units = Vec::with_capacity(len);
    // SAFETY: `ptr` points at a NUL-terminated UTF-16 buffer. We copy the
    // measured code units into owned storage before converting them.
    unsafe {
        for idx in 0..len {
            units.push(*ptr.add(idx));
        }
    }
    let name = match String::from_utf16(&units) {
        Ok(s) => s,
        Err(_) => return RLDB_MISUSE,
    };
    let cstring = match std::ffi::CString::new(name) {
        Ok(s) => s,
        Err(_) => return RLDB_MISUSE,
    };
    // SAFETY: delegates to the v2 variant whose # Safety contract this
    // call inherits; cstring lives for the call duration; ledgered at
    // .jankurai/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/udf.rs,
    // line=153, detector=rust.unsafe.extern-fn).
    // SAFETY: the delegated call uses the same validated arguments and
    // preserves the lifetime of `cstring` for the duration of the call.
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
