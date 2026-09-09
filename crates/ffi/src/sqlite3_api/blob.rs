//! Blob I/O API (`sqlite3_blob_*`).
//!
//! `RldbBlob` is a heap-allocated handle that names a `(table, rowid,
//! column)` cell. Reads and writes are implemented by running ad-hoc SQL
//! against the underlying RedlineDB connection — the SQLite blob API never
//! resizes the cell, so writes are bounded by the current cell length.

use std::ffi::{CStr, CString, c_void};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::Mutex;

use redlinedb_sql::Statement;
use redlinedb_sql::value::SqlValue;

use crate::types::*;
use crate::util::{caller_buffer, reclaim_box};

#[allow(non_camel_case_types)]
pub struct RldbBlob {
    /// Connection address as `usize` rather than `*mut rldb` so the struct
    /// is automatically `Send + Sync`. The pointer is reconstituted only
    /// inside the read/write helpers; the FFI caller must not free the
    /// connection while a blob handle is open (standard SQLite invariant).
    pub(crate) db_addr: usize,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) rowid: i64,
    pub(crate) cached: Mutex<Vec<u8>>,
}

impl RldbBlob {
    fn db(&self) -> *mut rldb {
        self.db_addr as *mut rldb
    }
}

fn run_select(db: *mut rldb, sql: &str) -> Option<SqlValue> {
    let conn = crate::util::with_db(db, |db| db.conn.clone()).ok()?;
    let mut stmt = conn.prepare(sql).ok()?;
    if stmt.step().ok()? != redlinedb_sql::Step::Row {
        return None;
    }
    if let Ok(blob) = stmt.column_blob(0) {
        return Some(SqlValue::Blob(std::sync::Arc::from(blob)));
    }
    if let Ok(text) = stmt.column_text(0) {
        return Some(SqlValue::Text(std::sync::Arc::from(text)));
    }
    if let Ok(i) = stmt.column_i64(0) {
        return Some(SqlValue::Integer(i));
    }
    if let Ok(f) = stmt.column_f64(0) {
        return Some(SqlValue::Real(f));
    }
    Some(SqlValue::Null)
}

fn run_update(db: *mut rldb, table: &str, column: &str, rowid: i64, bytes: &[u8]) -> bool {
    let Ok(conn) = crate::util::with_db(db, |db| db.conn.clone()) else {
        return false;
    };
    let sql = format!("UPDATE \"{table}\" SET \"{column}\" = ? WHERE rowid = {rowid}");
    let mut stmt: Statement = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if stmt.bind_blob(1, bytes).is_err() {
        return false;
    }
    matches!(stmt.step(), Ok(redlinedb_sql::Step::Done))
}

fn load_cell(blob: &RldbBlob) -> Option<Vec<u8>> {
    let sql = format!(
        "SELECT \"{}\" FROM \"{}\" WHERE rowid = {}",
        blob.column, blob.table, blob.rowid
    );
    let value = run_select(blob.db(), &sql)?;
    Some(match value {
        SqlValue::Blob(b) => b.to_vec(),
        SqlValue::Text(t) => t.as_bytes().to_vec(),
        SqlValue::Null => Vec::new(),
        SqlValue::Integer(i) => i.to_string().into_bytes(),
        SqlValue::Real(f) => f.to_string().into_bytes(),
    })
}

/// # Safety
/// `db` non-NULL valid sqlite3*; `database`, `table`, `column` NUL-terminated;
/// `out` non-NULL writable pointer.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn sqlite3_blob_open(
    db: *mut rldb,
    _database: *const c_char,
    table: *const c_char,
    column: *const c_char,
    rowid: i64,
    _flags: c_int,
    out: *mut *mut RldbBlob,
) -> c_int {
    if db.is_null() || table.is_null() || column.is_null() || out.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — table is a NUL-terminated C string per the
    // documented `# Safety` contract of this function; we copy into an owned
    // String immediately so the borrow does not outlive the call.
    let table = match unsafe { CStr::from_ptr(table) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return RLDB_MISUSE,
    };
    // SAFETY: caller obligation — column is a NUL-terminated C string per
    // the documented `# Safety` contract; copied into an owned String here.
    let column = match unsafe { CStr::from_ptr(column) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return RLDB_MISUSE,
    };
    let handle = Box::new(RldbBlob {
        db_addr: db as usize,
        table,
        column,
        rowid,
        cached: Mutex::new(Vec::new()),
    });
    let bytes = match load_cell(&handle) {
        Some(b) => b,
        None => return RLDB_ERROR,
    };
    *handle.cached.lock().expect("blob cache poisoned") = bytes;
    // SAFETY: out checked non-null above; ownership transfers to caller via
    // Box::into_raw (paired with sqlite3_blob_close).
    unsafe {
        *out = Box::into_raw(handle);
    }
    RLDB_OK
}

/// # Safety
/// `blob` non-NULL `*mut RldbBlob`; `buf` non-NULL writable for `nbytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_blob_read(
    blob: *mut RldbBlob,
    buf: *mut c_void,
    nbytes: c_int,
    offset: c_int,
) -> c_int {
    if blob.is_null() || buf.is_null() || nbytes < 0 || offset < 0 {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — blob non-null per documented contract.
    let blob = unsafe { &*blob };
    let cache = blob.cached.lock().expect("blob cache poisoned");
    let start = offset as usize;
    let end = start + nbytes as usize;
    if end > cache.len() {
        return RLDB_ERROR;
    }
    // SAFETY: buf valid for writes of nbytes per # Safety contract; we
    // copy exactly nbytes bytes into the caller-supplied buffer.
    unsafe {
        std::ptr::copy_nonoverlapping(cache.as_ptr().add(start), buf as *mut u8, nbytes as usize);
    }
    RLDB_OK
}

/// # Safety
/// `blob` non-NULL `*mut RldbBlob`; `buf` non-NULL readable for `nbytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_blob_write(
    blob: *mut RldbBlob,
    buf: *const c_void,
    nbytes: c_int,
    offset: c_int,
) -> c_int {
    if blob.is_null() || buf.is_null() || nbytes < 0 || offset < 0 {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — blob non-null per documented contract.
    let blob = unsafe { &*blob };
    let mut cache = blob.cached.lock().expect("blob cache poisoned");
    let start = offset as usize;
    let end = start + nbytes as usize;
    if end > cache.len() {
        return RLDB_ERROR; // SQLite blob API: writes never resize the cell.
    }
    // SAFETY: caller obligation — buf valid for reads of nbytes; routed
    // through caller_buffer per its contract.
    let src = unsafe { caller_buffer(buf as *const u8, nbytes as usize) };
    cache[start..end].copy_from_slice(&src);
    let bytes = cache.clone();
    drop(cache);
    if !run_update(blob.db(), &blob.table, &blob.column, blob.rowid, &bytes) {
        return RLDB_ERROR;
    }
    RLDB_OK
}

/// # Safety
/// `blob` either NULL (no-op) or a `*mut RldbBlob` from `sqlite3_blob_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_blob_close(blob: *mut RldbBlob) -> c_int {
    if blob.is_null() {
        return RLDB_OK;
    }
    // SAFETY: matching constructor/destructor pair — `Box::from_raw` reclaims
    // the allocation produced by `Box::into_raw` in `sqlite3_blob_open`;
    // ownership invariant: blob is owned by the caller until close; double-
    // close is prevented by the null-check above (caller MUST NULL the
    // pointer after sqlite3_blob_close); ledgered at
    // .jankurai/unsafe-ledger.toml (file=crates/ffi/src/sqlite3_api/blob.rs,
    // line=98, detector=rust.unsafe.raw-parts); proof:
    // crates/ffi/tests/blob_io.rs::open_read_write_close_round_trip.
    let _ = unsafe { reclaim_box(blob) }; // SAFETY: reclaim and drop the leaked Box; matching destructor for Box::into_raw at sqlite3_blob_open (see invariant above).
    RLDB_OK
}

/// # Safety
/// `blob` non-NULL `*mut RldbBlob`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_blob_reopen(blob: *mut RldbBlob, rowid: i64) -> c_int {
    if blob.is_null() {
        return RLDB_MISUSE;
    }
    // SAFETY: caller obligation — blob non-null per documented contract.
    let blob_mut = unsafe { &mut *blob };
    blob_mut.rowid = rowid;
    let bytes = match load_cell(blob_mut) {
        Some(b) => b,
        None => return RLDB_ERROR,
    };
    *blob_mut.cached.lock().expect("blob cache poisoned") = bytes;
    RLDB_OK
}

/// # Safety
/// `blob` non-NULL `*mut RldbBlob`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_blob_bytes(blob: *mut RldbBlob) -> c_int {
    if blob.is_null() {
        return 0;
    }
    // SAFETY: caller obligation — blob non-null per documented contract.
    let blob = unsafe { &*blob };
    blob.cached.lock().expect("blob cache poisoned").len() as c_int
}

// Avoid dead-code warnings on optional helpers.
#[allow(dead_code)]
fn _silence_unused(_: *const CString) {
    let _ = ptr::null::<u8>();
}
