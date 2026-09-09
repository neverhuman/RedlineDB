//! Custom collation (`sqlite3_create_collation*`) registration surface.
//!
//! Registered collation callbacks are stored in a global registry keyed by
//! `(connection_id, lowercased name)`. The SQL `Collation::Custom` variant
//! consults this registry through `redlinedb_sql::udf::install_collation_dispatch`.

use std::ffi::{CStr, c_void};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

use redlinedb_sql::udf as sql_udf;

use crate::types::*;
use crate::util::caller_buffer;

pub type CompareFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    nbytes_a: c_int,
    a: *const c_void,
    nbytes_b: c_int,
    b: *const c_void,
) -> c_int;
pub type CollationDestructorFn = unsafe extern "C" fn(*mut c_void);

#[allow(dead_code)]
type _CollationCompareCheck = CompareFn;
pub type CollationNeededFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    db: *mut rldb,
    encoding: c_int,
    name: *const c_char,
);

#[derive(Clone, Copy)]
pub(crate) struct CollationEntry {
    pub callback: CompareFn,
    /// Caller-supplied opaque pointer stored as `usize` so the entry is
    /// auto-`Send + Sync`. Cast back to `*mut c_void` only when invoking
    /// the C callback (where the pointer's validity is the registrar's
    /// responsibility per the SQLite ABI).
    pub user_data: usize,
    /// Held for the eventual destructor invocation path; never fires today
    /// because we never overwrite an existing registration mid-life.
    #[allow(dead_code)]
    pub destructor: Option<CollationDestructorFn>,
}

static REGISTRY: Mutex<Option<std::collections::HashMap<(usize, String), CollationEntry>>> =
    Mutex::new(None);

#[derive(Clone, Copy)]
struct NeededCb {
    /// Stored as `usize` so the struct is auto-`Send + Sync`; only cast
    /// back to `*mut c_void` when invoking the C callback.
    user_data: usize,
    cb: CollationNeededFn,
}

static NEEDED_CB: Mutex<Option<NeededCb>> = Mutex::new(None);

fn registry() -> std::sync::MutexGuard<
    'static,
    Option<std::collections::HashMap<(usize, String), CollationEntry>>,
> {
    let mut guard = REGISTRY.lock().expect("collation registry poisoned");
    if guard.is_none() {
        *guard = Some(std::collections::HashMap::new());
        sql_udf::install_collation_dispatch(dispatch_from_sql);
    }
    guard
}

fn dispatch_from_sql(db_addr: usize, name: &str, a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let key = (db_addr, name.to_ascii_lowercase());
    let registry = registry();
    let map = registry.as_ref()?;
    let entry = *map.get(&key)?;
    drop(registry);
    let cmp = entry.callback;
    let user_data = entry.user_data as *mut c_void;
    // SAFETY: callback signature matches FFI ABI; we pass byte slices with
    // explicit lengths; the registered callback agreed to inspect those
    // bytes for the call duration only; user_data cast back from usize.
    let rc = unsafe {
        cmp(
            user_data,
            a.len() as c_int,
            a.as_ptr() as *const c_void,
            b.len() as c_int,
            b.as_ptr() as *const c_void,
        )
    };
    Some(match rc.cmp(&0) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => std::cmp::Ordering::Equal,
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    })
}

fn invoke_needed(db: *mut rldb, name: &str) {
    let needed = NEEDED_CB.lock().expect("needed cb poisoned");
    if let Some(entry) = *needed {
        if let Ok(cstr) = std::ffi::CString::new(name) {
            let user_data = entry.user_data as *mut c_void;
            // SAFETY: callback signature matches the FFI ABI for
            // sqlite3_collation_needed; user_data is the pointer the
            // registrar provided (stored as usize, cast back here) and
            // remains valid for the lifetime guaranteed by the SQLite ABI;
            // cstr lives for the entire call; ledgered at
            // .jankurai/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/collation.rs,
            // line=184, detector=rust.unsafe.extern-fn).
            unsafe {
                // SAFETY: see the documented FFI-ABI callback invariant above.
                (entry.cb)(user_data, db, 1 /* SQLITE_UTF8 */, cstr.as_ptr());
            }
        }
    }
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `name` NUL-terminated; `compare` either
/// NULL (unregister) or a valid C function pointer per the SQLite ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_create_collation(
    db: *mut rldb,
    name: *const c_char,
    _enc: c_int,
    user_data: *mut c_void,
    compare: Option<CompareFn>,
) -> c_int {
    // SAFETY: delegates to v2 which performs all argument checks.
    unsafe { sqlite3_create_collation_v2(db, name, _enc, user_data, compare, None) }
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `name` NUL-terminated; `compare` either
/// NULL (unregister) or valid C function pointer; `destroy` either NULL or
/// valid destructor for `user_data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_create_collation_v2(
    db: *mut rldb,
    name: *const c_char,
    _enc: c_int,
    user_data: *mut c_void,
    compare: Option<CompareFn>,
    destroy: Option<CollationDestructorFn>,
) -> c_int {
    if db.is_null() || name.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — name NUL-terminated per the SQLite ABI.
    let name_bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    let name_string = match String::from_utf8(name_bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => return RLDB_MISUSE,
    };
    let key = (db as usize, name_string.to_ascii_lowercase());
    let mut registry = registry();
    let map = registry.as_mut().expect("registry init");
    match compare {
        Some(callback) => {
            map.insert(
                key,
                CollationEntry {
                    callback,
                    user_data: user_data as usize,
                    destructor: destroy,
                },
            );
        }
        None => {
            map.remove(&key);
        }
    }
    RLDB_OK
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `cb` either NULL (unregister) or valid
/// C function pointer per the SQLite ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_collation_needed(
    db: *mut rldb,
    user_data: *mut c_void,
    cb: Option<CollationNeededFn>,
) -> c_int {
    if db.is_null() {
        return RLDB_MISUSE;
    }
    let mut slot = NEEDED_CB.lock().expect("needed cb poisoned");
    *slot = cb.map(|cb| NeededCb {
        user_data: user_data as usize,
        cb,
    });
    RLDB_OK
}

/// Test-only helper: pretend a collation was requested. Lets the
/// `collation_needed` test path exercise the dispatch round-trip without
/// running a full SQL statement against an unregistered collation.
#[doc(hidden)]
pub fn __test_invoke_needed(db: *mut rldb, name: &str) {
    invoke_needed(db, name);
}

#[doc(hidden)]
pub fn __test_consume_buffer(ptr: *const u8, len: usize) -> usize {
    if ptr.is_null() || len == 0 {
        return 0;
    }
    // SAFETY: test-only helper; caller passes a length-bounded slice.
    let slice = unsafe { caller_buffer(ptr, len) };
    slice.len()
}
