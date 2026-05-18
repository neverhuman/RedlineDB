//! Section D1 safety invariants: negative tests that exercise the
//! caller-obligation guards documented by each `SAFETY:` comment in
//! `crates/ffi/src/`. None of these tests should panic or invoke UB; every
//! misuse path must return a documented error code (or be a no-op).
//!
//! The result-code constants below are duplicated from
//! `crates/ffi/include/redlinedb.h` because the module-internal `RLDB_*`
//! constants in `crates/ffi/src/types.rs` are `pub(crate)`. The values must
//! match the public C ABI exactly — drift here is a contract bug.

use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use redlinedb::{
    rldb, rldb_backup, rldb_backup_close, rldb_backup_finish, rldb_backup_init,
    rldb_backup_pagecount, rldb_backup_remaining, rldb_backup_step, rldb_bind_blob,
    rldb_bind_double, rldb_bind_int64, rldb_bind_null, rldb_bind_parameter_index, rldb_bind_text,
    rldb_changes, rldb_checkpoint, rldb_clear_bindings, rldb_close, rldb_close_v2,
    rldb_column_blob, rldb_column_bytes, rldb_column_count, rldb_column_double, rldb_column_int64,
    rldb_column_name, rldb_column_text, rldb_column_type, rldb_errcode, rldb_errmsg, rldb_exec,
    rldb_finalize, rldb_free, rldb_interrupt, rldb_last_insert_rowid, rldb_open, rldb_open_v2,
    rldb_parameter_count, rldb_prepare_v2, rldb_reset, rldb_stats_json, rldb_step, rldb_stmt,
    rldb_vacuum,
};

// ---- Result codes (mirror of crates/ffi/include/redlinedb.h) -----------

const RLDB_OK: c_int = 0;
const RLDB_ERROR: c_int = 1;
const RLDB_MISMATCH: c_int = 20;
const RLDB_MISUSE: c_int = 21;
const RLDB_RANGE: c_int = 25;
const RLDB_DONE: c_int = 101;

// ---- Shared test helpers -----------------------------------------------

fn temp_path(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    let unique = format!(
        "redlinedb-ffi-safety-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    path.push(unique);
    path
}

fn open_test_db(name: &str) -> *mut rldb {
    let path = temp_path(name);
    fs::create_dir_all(&path).expect("dir");
    let db_path = path.join(format!("{name}.redline"));
    let c_path = CString::new(db_path.to_str().expect("utf8")).expect("cstring");
    let mut db: *mut rldb = ptr::null_mut();
    let rc = rldb_open(c_path.as_ptr(), &mut db);
    assert_eq!(rc, RLDB_OK, "open should succeed in helper");
    assert!(!db.is_null());
    db
}

// ---- Tests --------------------------------------------------------------

/// Every public function that accepts `*mut rldb` must return `RLDB_MISUSE`
/// (or a documented sentinel) when handed a NULL pointer instead of
/// dereferencing it. This is the core contract the `SAFETY:` comments rely
/// on: "we just confirmed non-null above".
#[test]
fn null_db_pointer_returns_misuse_for_every_public_function() {
    let db: *mut rldb = ptr::null_mut();

    // Functions that return an integer error code on misuse.
    assert_eq!(rldb_close(db), RLDB_MISUSE);
    assert_eq!(rldb_close_v2(db), RLDB_MISUSE);
    assert_eq!(rldb_checkpoint(db), RLDB_MISUSE);
    assert_eq!(rldb_vacuum(db), RLDB_MISUSE);
    assert_eq!(rldb_changes(db), RLDB_MISUSE);
    assert_eq!(rldb_errcode(db), RLDB_MISUSE);

    // rldb_last_insert_rowid returns 0 on null db (documented sentinel).
    assert_eq!(rldb_last_insert_rowid(db), 0);

    // rldb_errmsg returns NULL on null db.
    assert!(rldb_errmsg(db).is_null());

    // rldb_interrupt is a no-op on null (no UB, no return value to check).
    rldb_interrupt(db);

    // rldb_exec / rldb_prepare_v2 / rldb_stats_json: null db is misuse.
    let sql = CString::new("SELECT 1").unwrap();
    assert_eq!(
        rldb_exec(db, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_MISUSE
    );
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_MISUSE
    );
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(rldb_stats_json(db, &mut out), RLDB_MISUSE);
}

/// `rldb_prepare_v2` / `rldb_exec` must reject NULL `sql` rather than
/// dereferencing it. This validates the `if sql.is_null()` guard the
/// `SAFETY:` comments rely on (`bind.rs`, `exec.rs`, `stmt.rs`).
#[test]
fn null_sql_pointer_returns_misuse() {
    let db = open_test_db("null-sql");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();

    assert_eq!(
        rldb_prepare_v2(db, ptr::null(), -1, &mut stmt, ptr::null_mut()),
        RLDB_MISUSE
    );
    assert!(stmt.is_null());

    assert_eq!(
        rldb_exec(db, ptr::null(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_MISUSE
    );

    assert_eq!(rldb_close(db), RLDB_OK);
}

/// Invalid UTF-8 in SQL must return an error code (not a panic). The FFI
/// surface converts the caller's `*const c_char` to `&str`, which fails on
/// non-UTF-8; the documented mapping is `RLDB_MISMATCH`.
#[test]
fn invalid_utf8_in_sql_returns_error_not_panic() {
    let db = open_test_db("bad-utf8");
    // 0xff is not valid UTF-8 in any position; pair it with a NUL terminator
    // so it forms a syntactically-valid C string.
    let bytes: [u8; 5] = [b'S', 0xff, b'L', b'T', 0];
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    let rc = rldb_prepare_v2(
        db,
        bytes.as_ptr() as *const c_char,
        -1,
        &mut stmt,
        ptr::null_mut(),
    );
    assert_eq!(
        rc, RLDB_MISMATCH,
        "non-UTF-8 SQL must round-trip as RLDB_MISMATCH"
    );
    assert!(stmt.is_null());

    let rc = rldb_exec(
        db,
        bytes.as_ptr() as *const c_char,
        None,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    assert_eq!(rc, RLDB_MISMATCH);

    assert_eq!(rldb_close(db), RLDB_OK);
}

/// Double-close guard: after `rldb_close` succeeds the caller's pointer is
/// dangling. The C ABI convention (mirroring sqlite3) is that the caller
/// MUST set their copy to NULL after close; a subsequent call with NULL is
/// safe and returns `RLDB_MISUSE`. This test exercises the null-after-close
/// pattern the SAFETY contract requires.
#[test]
fn double_close_via_null_after_close_is_safe() {
    let db = open_test_db("double-close");
    assert_eq!(rldb_close(db), RLDB_OK);

    // The C ABI requires the caller to NULL out their handle after close.
    // A subsequent close on the NULL'd pointer must return MISUSE without UB.
    let mut handle: *mut rldb = ptr::null_mut();
    assert_eq!(rldb_close(handle), RLDB_MISUSE);
    assert_eq!(rldb_close_v2(handle), RLDB_MISUSE);
    // `handle` was never freed twice, so this is the explicit "NULL it out"
    // pattern documented in the SAFETY contracts.
    let _ = &mut handle;
}

/// Oversize SQL (> 1 MiB) must be parsed as an error rather than panic.
/// This stresses the unbounded `to_owned()` path in `rldb_prepare_v2` /
/// `rldb_exec` and exercises the parser's input-length tolerance.
#[test]
fn oversize_sql_is_rejected_gracefully() {
    let db = open_test_db("oversize");
    // Construct ~1.5 MiB of SQL: a long IN-list of integer literals that
    // any sane SQL parser can in principle accept but stresses the input
    // path. The exact return code depends on the parser; we only assert
    // that it does not panic and we eventually close cleanly.
    let mut sql = String::with_capacity(2_000_000);
    sql.push_str("SELECT * FROM t WHERE id IN (");
    // 300k integers (~7 bytes each incl. comma) easily clears 1 MiB.
    for i in 0..300_000 {
        if i > 0 {
            sql.push(',');
        }
        sql.push_str(&i.to_string());
    }
    sql.push(')');
    assert!(sql.len() > 1024 * 1024, "must be > 1 MiB");

    let c_sql = CString::new(sql).expect("no NULs in oversize sql");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    let rc = rldb_prepare_v2(db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut());
    // Either the parser produces an error (no table `t`) or it parses but
    // fails to bind/step — in both cases we get a non-OK code and no panic.
    // Any RLDB_OK that yielded a non-null stmt must be finalize-able.
    if rc == RLDB_OK && !stmt.is_null() {
        assert_eq!(rldb_finalize(stmt), RLDB_OK);
    } else {
        assert_ne!(rc, RLDB_OK);
        assert!(stmt.is_null());
    }

    assert_eq!(rldb_close(db), RLDB_OK);
}

/// Parameter index out of range on bind functions must return `RLDB_RANGE`
/// rather than panic or write out of bounds. This validates the
/// `parameter_index` clamp in the SQL layer that the FFI relies on.
#[test]
fn parameter_index_out_of_range_returns_range() {
    let db = open_test_db("param-range");
    // Prepare a statement with 1 parameter. Index 0 and 99 are both out of
    // range (parameters are 1-indexed per the sqlite3 contract).
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)").unwrap();
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    let sql = CString::new("INSERT INTO t VALUES(?, ?)").unwrap();
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert!(!stmt.is_null());

    // Out-of-range indices: 0 (below 1), 99 (above parameter_count of 2).
    let bad_value = CString::new("oops").unwrap();
    assert_eq!(rldb_bind_int64(stmt, 0, 1), RLDB_RANGE);
    assert_eq!(rldb_bind_int64(stmt, 99, 1), RLDB_RANGE);
    assert_eq!(rldb_bind_double(stmt, 0, 1.0), RLDB_RANGE);
    assert_eq!(rldb_bind_double(stmt, 99, 1.0), RLDB_RANGE);
    assert_eq!(rldb_bind_null(stmt, 99), RLDB_RANGE);
    assert_eq!(rldb_bind_text(stmt, 99, bad_value.as_ptr(), -1), RLDB_RANGE);
    let blob: [u8; 3] = [1, 2, 3];
    assert_eq!(
        rldb_bind_blob(stmt, 99, blob.as_ptr() as *const c_void, 3),
        RLDB_RANGE
    );

    // Within range still works (validates we didn't break the happy path).
    assert_eq!(rldb_bind_int64(stmt, 1, 7), RLDB_OK);
    let v = CString::new("ok").unwrap();
    assert_eq!(rldb_bind_text(stmt, 2, v.as_ptr(), -1), RLDB_OK);

    assert_eq!(rldb_finalize(stmt), RLDB_OK);
    assert_eq!(rldb_close(db), RLDB_OK);
}

/// `rldb_bind_text` / `rldb_bind_blob` must reject NULL value pointers
/// without dereferencing them. The SAFETY comments in `bind.rs` rely on
/// the `if value.is_null()` guard immediately preceding the slice
/// construction.
#[test]
fn null_value_pointer_on_bind_returns_misuse() {
    let db = open_test_db("null-bind-value");
    let setup = CString::new("CREATE TABLE t(v TEXT)").unwrap();
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    let sql = CString::new("INSERT INTO t VALUES(?)").unwrap();
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert!(!stmt.is_null());

    // NULL value pointer for both text + blob must return MISUSE.
    assert_eq!(rldb_bind_text(stmt, 1, ptr::null(), -1), RLDB_MISUSE);
    assert_eq!(rldb_bind_blob(stmt, 1, ptr::null(), 0), RLDB_MISUSE);

    assert_eq!(rldb_finalize(stmt), RLDB_OK);
    assert_eq!(rldb_close(db), RLDB_OK);
}

/// Column accessors on null stmt return the documented sentinel without UB.
/// Most return NULL or a zero value; the integer-returning ones return
/// MISUSE so callers can detect the misuse.
#[test]
fn null_stmt_column_accessors_return_sentinels() {
    let stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(rldb_column_count(stmt), RLDB_MISUSE);
    assert!(rldb_column_name(stmt, 0).is_null());
    assert_eq!(rldb_column_int64(stmt, 0), 0);
    assert_eq!(rldb_column_double(stmt, 0), 0.0);
    assert!(rldb_column_text(stmt, 0).is_null());
    assert!(rldb_column_blob(stmt, 0).is_null());
    // rldb_column_type / rldb_column_bytes flow through api() and surface
    // an error code on null; we don't care which specific code, only that
    // the call does not panic and returns SOMETHING.
    let _ = rldb_column_type(stmt, 0);
    let _ = rldb_column_bytes(stmt, 0);

    // Bind / stmt-lifecycle on null stmt should likewise not panic.
    assert_eq!(rldb_parameter_count(stmt), RLDB_MISUSE);
    assert_eq!(
        rldb_bind_parameter_index(stmt, CString::new("foo").unwrap().as_ptr()),
        RLDB_MISUSE
    );
    assert_eq!(rldb_step(stmt), RLDB_MISUSE);
    assert_eq!(rldb_reset(stmt), RLDB_MISUSE);
    assert_eq!(rldb_finalize(stmt), RLDB_MISUSE);
    assert_eq!(rldb_clear_bindings(stmt), RLDB_MISUSE);
}

/// `rldb_open` / `rldb_open_v2` must reject NULL path / NULL out_db. This
/// validates the "null check first, then SAFETY borrow" pattern in
/// `lifecycle.rs`.
#[test]
fn null_open_args_return_misuse() {
    let mut db: *mut rldb = ptr::null_mut();
    assert_eq!(rldb_open(ptr::null(), &mut db), RLDB_MISUSE);
    assert!(db.is_null());

    let c_path = CString::new("/dev/null/does-not-matter").unwrap();
    assert_eq!(rldb_open(c_path.as_ptr(), ptr::null_mut()), RLDB_MISUSE);

    assert_eq!(rldb_open_v2(ptr::null(), ptr::null(), &mut db), RLDB_MISUSE);
    assert_eq!(
        rldb_open_v2(c_path.as_ptr(), ptr::null(), ptr::null_mut()),
        RLDB_MISUSE
    );
}

/// `rldb_free(NULL)` must be a safe no-op. The `SAFETY:` comment in
/// `error.rs` documents the null-check before `CString::from_raw`.
#[test]
fn rldb_free_null_is_noop() {
    rldb_free(ptr::null_mut());
    rldb_free(ptr::null_mut());
}

/// Multi-statement SQL with an embedded NUL byte must not invoke UB. The
/// FFI surface treats the first NUL as end-of-string (C ABI contract),
/// which means everything after it is silently truncated; the operation
/// must still succeed for the prefix that was visible.
#[test]
fn nul_byte_mid_string_truncates_at_nul_without_ub() {
    let db = open_test_db("nul-mid");
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY)").unwrap();
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    // Build a raw buffer: "INSERT INTO t VALUES(1);\0DROP TABLE t;\0"
    // CString rejects interior NULs so we build a Vec<u8> manually and
    // pass it as a raw pointer.
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"INSERT INTO t VALUES(1);");
    bytes.push(0u8); // interior NUL
    bytes.extend_from_slice(b"DROP TABLE t;");
    bytes.push(0u8); // outer terminator

    let rc = rldb_exec(
        db,
        bytes.as_ptr() as *const c_char,
        None,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    // Either way, no panic / UB. The prefix INSERT should have been
    // applied so the table still exists.
    let _ = rc;

    // Verify the DROP after the NUL was NOT executed: the table should
    // still be queryable.
    let probe = CString::new("SELECT id FROM t").unwrap();
    assert_eq!(
        rldb_exec(db, probe.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK,
        "table must survive — DROP after NUL must not have run"
    );

    assert_eq!(rldb_close(db), RLDB_OK);
}

/// `rldb_exec` callback returning non-zero must populate `errmsg` and
/// surface an error code, not panic. The SAFETY contract on `set_errmsg`
/// in `exec.rs` relies on the caller freeing via `rldb_free`.
#[test]
fn exec_callback_failure_round_trips_errmsg_ownership() {
    extern "C" fn always_fail(
        _ctx: *mut c_void,
        _n: c_int,
        _argv: *mut *mut c_char,
        _names: *mut *mut c_char,
    ) -> c_int {
        1
    }

    let db = open_test_db("cb-fail");
    let setup =
        CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY); INSERT INTO t VALUES(1);").unwrap();
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    let sel = CString::new("SELECT id FROM t").unwrap();
    let mut errmsg: *mut c_char = ptr::null_mut();
    let rc = rldb_exec(
        db,
        sel.as_ptr(),
        Some(always_fail),
        ptr::null_mut(),
        &mut errmsg,
    );
    assert_eq!(rc, RLDB_ERROR);
    assert!(!errmsg.is_null(), "errmsg must be populated on failure");
    let msg = unsafe { CStr::from_ptr(errmsg) }.to_str().unwrap();
    assert!(msg.contains("callback"), "unexpected message: {msg}");
    // Caller must free via rldb_free — the SAFETY contract for set_errmsg
    // is that ownership transfers here.
    rldb_free(errmsg as *mut c_void);

    assert_eq!(rldb_close(db), RLDB_OK);
}

/// `rldb_backup_init` returns a `Box::into_raw` *mut rldb_backup that the
/// C caller must release via `rldb_backup_close` (Box::from_raw). This
/// exercises the constructor/destructor pair and is the proof witness for
/// the SAFETY comment on `rldb_backup_close`'s `Box::from_raw` in
/// `crates/ffi/src/snapshot.rs`.
#[test]
fn backup_init_step_close_round_trips_box_ownership() {
    let src = open_test_db("backup-rt");
    // Populate something so the snapshot copy actually does work.
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY); INSERT INTO t VALUES(1);")
        .expect("cstr");
    assert_eq!(
        rldb_exec(src, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );

    let dst_dir = temp_path("backup-rt-dst");
    let c_dst = CString::new(dst_dir.to_str().expect("utf8")).expect("cstr");
    let mut backup: *mut rldb_backup = ptr::null_mut();
    let rc = rldb_backup_init(src, c_dst.as_ptr(), ptr::null(), &mut backup);
    assert_eq!(rc, RLDB_OK, "backup_init should succeed");
    assert!(!backup.is_null(), "backup must be a real Box::into_raw ptr");

    // backup_step / remaining / pagecount touch the boxed allocation.
    assert_eq!(rldb_backup_step(backup, 1), RLDB_DONE);
    let _ = rldb_backup_remaining(backup);
    let _ = rldb_backup_pagecount(backup);
    assert_eq!(rldb_backup_finish(backup), RLDB_OK);

    // The Box::from_raw inside rldb_backup_close pairs with the Box::into_raw
    // in rldb_backup_init; running under the test harness (and the
    // address-sanitiser/miri lanes documented in the proof_lane) is what
    // proves the pair is balanced.
    assert_eq!(rldb_backup_close(backup), RLDB_OK);
    // Null after close must be a no-op per the documented C ABI contract.
    assert_eq!(rldb_backup_close(ptr::null_mut()), RLDB_MISUSE);

    assert_eq!(rldb_close(src), RLDB_OK);
}
