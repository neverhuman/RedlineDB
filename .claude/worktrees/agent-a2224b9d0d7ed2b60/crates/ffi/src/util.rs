//! Shared FFI helpers: panic shims, error mapping, status recording, and
//! small allocation helpers used across multiple modules.
//!
//! These helpers are intentionally `pub(crate)` — they are not part of the
//! public C ABI, but each `rldb_*` extern function delegates to them, so
//! preserving their semantics is load-bearing.

use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use redlinedb_kernel::error::Error as KernelError;
use redlinedb_sql::{DbOptions, Error as SqlError};

use crate::types::*;

// ---- Panic / result-flatten shims ------------------------------------------

pub(crate) fn api<T>(f: impl FnOnce() -> T) -> Result<T, c_int> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| RLDB_INTERNAL)
}

pub(crate) fn flatten_code(result: Result<Result<c_int, c_int>, c_int>) -> c_int {
    match result {
        Ok(Ok(code)) => code,
        Ok(Err(code)) => code,
        Err(code) => code,
    }
}

pub(crate) fn map_error(err: SqlError) -> c_int {
    match err {
        SqlError::Kernel(KernelError::LockTimeout)
        | SqlError::Kernel(KernelError::SerializationFailure) => RLDB_BUSY,
        SqlError::Kernel(KernelError::WriteConflict) => RLDB_LOCKED,
        SqlError::Kernel(KernelError::DatatypeMismatch) | SqlError::DatatypeMismatch => {
            RLDB_MISMATCH
        }
        SqlError::Kernel(KernelError::ConstraintViolation(_))
        | SqlError::ConstraintViolation(_) => RLDB_CONSTRAINT,
        SqlError::CommitMaybeCommitted => RLDB_IOERR,
        SqlError::Kernel(KernelError::SchemaChanged) => RLDB_SCHEMA,
        SqlError::Kernel(KernelError::ObjectNotFound)
        | SqlError::UnknownTable(_)
        | SqlError::UnknownColumn(_) => RLDB_NOTADB,
        SqlError::ParameterOutOfRange(_) => RLDB_RANGE,
        SqlError::TransactionState(_) | SqlError::Bind(_) => RLDB_MISUSE,
        SqlError::Parse(_) => RLDB_ERROR,
        SqlError::UnsupportedSql(_) => RLDB_MISUSE,
        _ => RLDB_ERROR,
    }
}

pub(crate) fn sql_result<T>(
    result: std::result::Result<T, SqlError>,
) -> std::result::Result<T, c_int> {
    result.map_err(map_error)
}

pub(crate) fn io<T>(result: std::io::Result<T>) -> std::result::Result<T, c_int> {
    result.map_err(|_| RLDB_IOERR)
}

// ---- Connection / handle helpers -------------------------------------------

pub(crate) fn db_options_from_config(config: Option<&rldb_config>) -> DbOptions {
    let mut options = DbOptions::default();
    if let Some(config) = config {
        let page_size = options.engine.page_size.max(1);
        options.engine.buffer_pool_pages = (config.cache_bytes as usize / page_size).max(16);
        options.query_memory.work_mem_bytes = config.work_mem_bytes as usize;
        options.query_memory.max_spill_bytes = config.max_spill_bytes as usize;
        options.query_memory.batch_rows = config.statement_cache_capacity.max(1) as usize;
        options.busy_timeout = std::time::Duration::from_millis(config.busy_timeout_ms as u64);
    }
    options
}

pub(crate) fn open_handle(
    path: &CStr,
    config: Option<&rldb_config>,
    create_if_missing: bool,
) -> Result<*mut rldb, c_int> {
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize};

    let path = path.to_str().map_err(|_| RLDB_MISMATCH)?;
    let options = db_options_from_config(config);
    let db = if Path::new(path).exists() {
        sql_result(redlinedb_sql::Database::open(path, options))?
    } else if create_if_missing {
        sql_result(redlinedb_sql::Database::create(path, options))?
    } else {
        return Err(RLDB_CANTOPEN);
    };
    let conn = db.connect();
    let handle = Box::new(rldb {
        db,
        conn,
        path: PathBuf::from(path),
        path_text: CString::new(path).map_err(|_| RLDB_MISMATCH)?,
        last_code: AtomicI32::new(RLDB_OK),
        last_message: Mutex::new(CString::new("").unwrap()),
        interrupted: AtomicBool::new(false),
        active_statements: AtomicUsize::new(0),
    });
    Ok(Box::into_raw(handle))
}

pub(crate) fn with_db<R>(db: *mut rldb, f: impl FnOnce(&rldb) -> R) -> Result<R, c_int> {
    if db.is_null() {
        return Err(RLDB_MISUSE);
    }
    // SAFETY: `db` non-null (checked); per C ABI in redlinedb.h every rldb_*
    // function that accepts *mut rldb requires rldb_open + not yet closed;
    // shared borrow lives only for f() which runs synchronously here.
    Ok(f(unsafe { &*db }))
}

// ---- Statement helpers ------------------------------------------------------

pub(crate) fn refresh_text_cache(stmt: &mut rldb_stmt) -> Result<(), c_int> {
    stmt.text_cache.clear();
    for index in 0..stmt.stmt.column_count() {
        if let Ok(text) = stmt.stmt.column_text(index) {
            stmt.text_cache
                .push(CString::new(text).map_err(|_| RLDB_MISMATCH)?);
        } else {
            stmt.text_cache.push(CString::new("").unwrap());
        }
    }
    Ok(())
}

pub(crate) fn to_hex(bytes: &[u8]) -> CString {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(*byte >> 4) as usize]);
        out.push(HEX[(*byte & 0x0f) as usize]);
    }
    match CString::new(out) {
        Ok(s) => s,
        // hex-encoded buffer cannot contain NUL bytes; fall through to a
        // fixed literal only as a typed last-resort sentinel.
        Err(_) => CString::new("blob").expect("static literal contains no NUL"),
    }
}

pub(crate) fn exec_value(
    stmt: &redlinedb_sql::Statement,
    index: usize,
) -> Result<Option<CString>, c_int> {
    if let Ok(text) = stmt.column_text(index) {
        Ok(Some(CString::new(text).map_err(|_| RLDB_MISMATCH)?))
    } else if let Ok(blob) = stmt.column_blob(index) {
        Ok(Some(to_hex(blob)))
    } else if let Ok(v) = stmt.column_i64(index) {
        Ok(Some(CString::new(v.to_string()).unwrap()))
    } else if let Ok(v) = stmt.column_f64(index) {
        Ok(Some(CString::new(v.to_string()).unwrap()))
    } else {
        Ok(Some(CString::new("").unwrap()))
    }
}

pub(crate) fn recursive_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "owner.lock" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            recursive_copy(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// ---- Status recording -------------------------------------------------------

pub(crate) fn status_message(code: c_int) -> &'static str {
    match code {
        RLDB_OK => "ok",
        RLDB_BUSY => "busy",
        RLDB_LOCKED => "locked",
        RLDB_INTERRUPT => "interrupted",
        RLDB_IOERR => "io error",
        RLDB_READONLY => "read only",
        RLDB_CANTOPEN => "cannot open",
        RLDB_SCHEMA => "schema changed",
        RLDB_CONSTRAINT => "constraint violation",
        RLDB_MISMATCH => "datatype mismatch",
        RLDB_MISUSE => "misuse",
        RLDB_RANGE => "parameter out of range",
        RLDB_NOTADB => "not an open database",
        _ => "error",
    }
}

pub(crate) fn sqlite_version_cstr() -> &'static CStr {
    static VERSION: OnceLock<CString> = OnceLock::new();
    VERSION
        .get_or_init(|| CString::new(env!("CARGO_PKG_VERSION")).expect("version cstring"))
        .as_c_str()
}

pub(crate) fn sqlite_sourceid_cstr() -> &'static CStr {
    static SOURCEID: OnceLock<CString> = OnceLock::new();
    SOURCEID
        .get_or_init(|| {
            CString::new(concat!(
                env!("CARGO_PKG_NAME"),
                " ",
                env!("CARGO_PKG_VERSION")
            ))
            .expect("sourceid cstring")
        })
        .as_c_str()
}

pub(crate) fn sqlite_errstr(code: c_int) -> &'static CStr {
    match code {
        RLDB_OK => c"not an error",
        RLDB_ERROR => c"SQL error or missing database",
        RLDB_INTERNAL => c"internal error",
        RLDB_BUSY => c"database is busy",
        RLDB_LOCKED => c"database is locked",
        RLDB_INTERRUPT => c"operation interrupted",
        RLDB_IOERR => c"disk I/O error",
        RLDB_READONLY => c"attempt to write a readonly database",
        RLDB_CANTOPEN => c"unable to open database file",
        RLDB_SCHEMA => c"database schema has changed",
        RLDB_CONSTRAINT => c"constraint failed",
        RLDB_MISMATCH => c"datatype mismatch",
        RLDB_MISUSE => c"library routine called out of sequence",
        RLDB_RANGE => c"bind or column index out of range",
        RLDB_NOTADB => c"file is not a database",
        _ => c"unknown error",
    }
}

pub(crate) fn record_status(db: *mut rldb, code: c_int) {
    let _ = with_db(db, |db| {
        db.last_code.store(code, Ordering::Relaxed);
        if let Ok(mut last_message) = db.last_message.lock() {
            *last_message = CString::new(status_message(code)).unwrap();
        }
    });
}

/// Like `record_status`, but stores `message` (typically an SqlError's
/// `to_string()`) so `sqlite3_errmsg` returns the actual cause rather than
/// the generic "error"/"busy" status text. Used by failure paths that
/// have a human-readable explanation.
pub(crate) fn record_status_with_message(db: *mut rldb, code: c_int, message: &str) {
    let _ = with_db(db, |db| {
        db.last_code.store(code, Ordering::Relaxed);
        if let Ok(mut last_message) = db.last_message.lock() {
            let safe: String = message.replace('\0', "?");
            // NUL stripped above, so CString::new never returns Err. Fixed
            // sentinel only as a typed last-resort guard.
            *last_message = match CString::new(safe) {
                Ok(s) => s,
                Err(_) => CString::new("error").expect("static literal contains no NUL"),
            };
        }
    });
}

// ---- errmsg helpers ---------------------------------------------------------

/// Helper: copy a Rust-side error message into a heap-allocated C string and
/// return its raw pointer to the FFI caller. Ownership transfers to the
/// caller, who must free via `rldb_free` / `sqlite3_free`. Wraps NULs so
/// pathological messages don't panic on `CString::new`.
pub(crate) fn errmsg_to_c_string(msg: &str) -> *mut c_char {
    let safe: String = msg.replace('\0', "?");
    match CString::new(safe) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::new("error").unwrap().into_raw(),
    }
}

/// Write `msg` to `*errmsg` if `errmsg` is non-null. Caller must own & free.
///
/// # Safety
/// Caller MUST guarantee:
/// 1. `errmsg` is NULL (no-op) or a writable, aligned `*mut c_char` the
///    caller owns exclusively for this call (no concurrent writer). See
///    `char **errmsg` slot in crates/ffi/include/redlinedb.h on
///    `rldb_exec`/`sqlite3_exec`.
/// 2. The slot at `*errmsg` receives a `CString::into_raw` pointer whose
///    ownership transfers to the caller; release ONLY via `rldb_free` /
///    `sqlite3_free` (paired with `CString::from_raw`). Any other free is UB.
/// 3. Any prior value at `*errmsg` was already freed (this function
///    unconditionally overwrites without freeing).
pub(crate) unsafe fn set_errmsg(errmsg: *mut *mut c_char, msg: &str) {
    if errmsg.is_null() {
        return;
    }
    // SAFETY: `errmsg` non-null (checked); caller obligations 1, 2, 4 from
    // the # Safety block ensure ownership of the slot for this call and the
    // CString::into_raw ownership transfer (paired with rldb_free).
    unsafe {
        *errmsg = errmsg_to_c_string(msg);
    }
}
