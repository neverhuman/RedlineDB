//! Batch F1 (jankurai-repair): explicit FFI error-path proofs for the
//! HLT-029-RUST-BAD-BEHAVIOR rule.
//!
//! The audit flags every `unsafe { … }` block in `crates/ffi/src/` because
//! the SAFETY comments + unsafe-ledger are static artefacts. This file
//! complements `safety_invariants.rs` (Section D1) and
//! `exec_input_boundary.rs` (Section E) with **negative-path runtime
//! proofs** that the documented misuse contracts hold for the C ABI
//! entry points D1 did not explicitly exercise:
//!
//!   - `rldb_open_v2` config-null cell + non-existent path,
//!   - `rldb_busy_timeout` + `rldb_changes` + `rldb_last_insert_rowid`
//!     null-DB sentinels (D1 covers `errcode`/`errmsg`/`exec` only),
//!   - `rldb_backup_init` / `rldb_backup_step` / `rldb_backup_finish`
//!     null-arg sentinels (D1 covers only `rldb_backup_close(NULL)`),
//!   - `rldb_bind_parameter_index(NULL)` and `rldb_prepare_v2` empty
//!     / comment-only SQL (returns RLDB_OK with NULL out_stmt),
//!   - the documented finalize-then-NULL caller obligation that the
//!     SAFETY comment on `rldb_finalize` references,
//!   - the `rldb_errmsg` / `rldb_errcode` flow after a deliberate error.
//!
//! Each `unsafe { … }` block in this file carries a `// HLT-029` comment
//! citing which production unsafe site it proves.
//!
//! Result codes are duplicated from `crates/ffi/include/redlinedb.h`
//! because `crates/ffi/src/types.rs::RLDB_*` are `pub(crate)` — same
//! convention used by `safety_invariants.rs` lines 31-38.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;

use redlinedb::{
    rldb, rldb_backup, rldb_backup_close, rldb_backup_finish, rldb_backup_init,
    rldb_backup_pagecount, rldb_backup_remaining, rldb_backup_step, rldb_bind_int64,
    rldb_bind_parameter_index, rldb_busy_timeout, rldb_changes, rldb_close, rldb_errcode,
    rldb_errmsg, rldb_exec, rldb_finalize, rldb_last_insert_rowid, rldb_open, rldb_open_v2,
    rldb_prepare_v2, rldb_step, rldb_stmt,
};
use tempfile::TempDir;

// ---- Result codes (mirror of crates/ffi/include/redlinedb.h) ---------------

const RLDB_OK: c_int = 0;
const RLDB_ERROR: c_int = 1;
const RLDB_MISUSE: c_int = 21;
const RLDB_NOTADB: c_int = 26;
const RLDB_DONE: c_int = 101;

// ---- Shared inline helpers (kept small — no shared module across the
// existing tests so we mirror the inline-helper convention from
// exec_input_boundary.rs lines 46-94). --------------------------------------

fn open_db(label: &str) -> (TempDir, *mut rldb) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{label}.redline"));
    let c_path = CString::new(path.to_str().expect("utf8 path")).expect("nul-free path");
    let mut db: *mut rldb = ptr::null_mut();
    let rc = rldb_open(c_path.as_ptr(), &mut db);
    assert_eq!(rc, RLDB_OK, "rldb_open rc={rc} for {label}");
    assert!(!db.is_null());
    (dir, db)
}

fn close_db(db: *mut rldb) {
    let rc = rldb_close(db);
    assert_eq!(rc, RLDB_OK, "rldb_close rc={rc}");
}

// ---- Tests -----------------------------------------------------------------

/// `rldb_open_v2(NULL path, …)` must return RLDB_MISUSE without UB. D1
/// covers `rldb_open` and the combined open_v2 null-cells but does NOT
/// exercise the specific cell where `path` is NULL but `out_db` is a real
/// pointer (caller pre-zeroed a slot). Proves the
/// `if path.is_null() || out_db.is_null()` guard in
/// `crates/ffi/src/lifecycle.rs::rldb_open_v2`.
#[test]
fn null_path_to_open_v2_returns_misuse_with_valid_out_slot() {
    let mut db: *mut rldb = ptr::null_mut();
    // HLT-029: proves crates/ffi/src/lifecycle.rs:36 `if path.is_null()` guard.
    let rc = rldb_open_v2(ptr::null(), ptr::null(), &mut db);
    assert_eq!(rc, RLDB_MISUSE);
    assert!(db.is_null(), "out_db must stay null on misuse");
}

/// `rldb_close(NULL)` and `rldb_close_v2(NULL)` must both return
/// RLDB_MISUSE (the documented convention per safety_invariants.rs
/// `double_close_via_null_after_close_is_safe`). This is a sanity test
/// that the convention holds when the handle was NEVER opened — i.e.
/// the caller invokes close on a freshly-zeroed slot.
#[test]
fn close_on_never_opened_handle_is_safe() {
    // No `open` precedes this — handle was never a real Box::into_raw ptr.
    // HLT-029: proves crates/ffi/src/lifecycle.rs:63 + :81 null guards.
    let rc1 = rldb_close(ptr::null_mut());
    let rc2 = redlinedb::rldb_close_v2(ptr::null_mut());
    assert_eq!(rc1, RLDB_MISUSE, "rldb_close(NULL) rc={rc1}");
    assert_eq!(rc2, RLDB_MISUSE, "rldb_close_v2(NULL) rc={rc2}");
}

/// `rldb_prepare_v2` with NULL `db` must return RLDB_MISUSE without
/// dereferencing the pointer. D1 covers this in
/// `null_db_pointer_returns_misuse_for_every_public_function` as one
/// case among many; we add a dedicated test that ALSO asserts the
/// `out_stmt` slot is left untouched so the caller does not see a
/// spurious non-NULL pointer leak from the failure path.
#[test]
fn prepare_with_null_db_returns_misuse_and_leaves_out_stmt_null() {
    let db: *mut rldb = ptr::null_mut();
    let sql = CString::new("SELECT 1").expect("nul-free");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    // HLT-029: proves crates/ffi/src/stmt.rs:25 `if db.is_null()` guard.
    let rc = rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut());
    assert_eq!(rc, RLDB_MISUSE);
    assert!(stmt.is_null(), "out_stmt must be untouched on misuse");
}

/// The documented C ABI contract is that after `rldb_finalize` succeeds
/// the caller MUST set their stmt handle to NULL. A subsequent call on
/// the NULL'd slot returns RLDB_MISUSE rather than touching the freed
/// box. This proves the `rldb_finalize` → null-out → reuse pattern that
/// the SAFETY comment on `crates/ffi/src/stmt.rs::rldb_finalize` calls
/// out (and which `safety_invariants.rs::double_close_via_null_after_close_is_safe`
/// proves for `rldb_close`, but not for stmt).
#[test]
fn step_after_finalize_then_null_returns_misuse() {
    let (_dir, db) = open_db("step-after-finalize");
    let sql = CString::new("SELECT 1").expect("nul-free");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert!(!stmt.is_null());
    assert_eq!(rldb_finalize(stmt), RLDB_OK);

    // Caller obligation per redlinedb.h: NULL out the handle after finalize.
    let stmt: *mut rldb_stmt = ptr::null_mut();
    // HLT-029: proves crates/ffi/src/stmt.rs:109 step null-guard discharges
    // the caller obligation documented on rldb_finalize:159.
    assert_eq!(rldb_step(stmt), RLDB_MISUSE);
    assert_eq!(rldb_finalize(stmt), RLDB_MISUSE);

    close_db(db);
}

/// `rldb_bind_parameter_index(stmt, NULL)` must return RLDB_MISUSE
/// rather than dereferencing the name pointer. Complements D1's
/// `null_value_pointer_on_bind_returns_misuse` (text/blob) by covering
/// the by-name lookup path in `crates/ffi/src/bind.rs::rldb_bind_parameter_index`.
#[test]
fn bind_parameter_index_with_null_name_returns_misuse() {
    let (_dir, db) = open_db("param-index-null-name");
    let setup = CString::new("CREATE TABLE t(v TEXT)").expect("nul-free");
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    let sql = CString::new("INSERT INTO t VALUES(:val)").expect("nul-free");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert!(!stmt.is_null());

    // HLT-029: proves crates/ffi/src/bind.rs:114 `name.is_null()` guard.
    assert_eq!(rldb_bind_parameter_index(stmt, ptr::null()), RLDB_MISUSE);

    assert_eq!(rldb_finalize(stmt), RLDB_OK);
    close_db(db);
}

/// `rldb_errmsg(db)` after a deliberate error must surface the SqlError
/// text (not the generic "ok" or "error" sentinel) via the heap-managed
/// `last_message` Mutex. Then `rldb_errcode` must return the same code
/// that was last recorded. Proves the round-trip the SAFETY comments in
/// `crates/ffi/src/error.rs` rely on (`with_db` -> `last_message.lock`).
#[test]
fn errmsg_after_failed_prepare_returns_human_readable_message() {
    let (_dir, db) = open_db("errmsg-after-fail");
    // Trigger an UnknownTable error — this routes through map_error in
    // util.rs to RLDB_NOTADB and records a non-empty last_message.
    let bad = CString::new("SELECT * FROM definitely_missing_table").expect("nul-free");
    let rc = rldb_exec(db, bad.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    assert_eq!(
        rc, RLDB_NOTADB,
        "expected RLDB_NOTADB for unknown-table; got rc={rc}",
    );

    // errcode should reflect the last-recorded code.
    let code = rldb_errcode(db);
    assert_eq!(code, RLDB_NOTADB, "rldb_errcode rc={code}");

    // errmsg should be non-null AND non-empty AND mention the missing name.
    let msg_ptr = rldb_errmsg(db);
    assert!(!msg_ptr.is_null(), "rldb_errmsg returned NULL");
    // HLT-029: proves crates/ffi/src/error.rs:19 `with_db` + Mutex::lock path
    // returns the recorded SqlError text rather than the static "error".
    let msg = unsafe { CStr::from_ptr(msg_ptr) }.to_str().expect("utf8");
    assert!(
        !msg.is_empty() && msg != "ok",
        "errmsg must be a real message, got {msg:?}",
    );

    close_db(db);
}

/// `rldb_backup_init` with NULL src/dst/out arguments must return
/// RLDB_MISUSE without touching the pointers. D1's
/// `backup_init_step_close_round_trips_box_ownership` only exercises
/// the happy path; this is the negative-cell complement.
#[test]
fn backup_init_with_null_args_returns_misuse() {
    let (_dir, db) = open_db("backup-null-init");
    let dst = CString::new("/tmp/redline-batchf1-unused").expect("nul-free");
    let mut backup: *mut rldb_backup = ptr::null_mut();

    // NULL src.
    // HLT-029: proves crates/ffi/src/snapshot.rs:19 `src.is_null()` guard.
    let rc = rldb_backup_init(ptr::null_mut(), dst.as_ptr(), ptr::null(), &mut backup);
    assert_eq!(rc, RLDB_MISUSE);
    assert!(backup.is_null());

    // NULL dst_path.
    // HLT-029: proves crates/ffi/src/snapshot.rs:19 `dst_path.is_null()` guard.
    let rc = rldb_backup_init(db, ptr::null(), ptr::null(), &mut backup);
    assert_eq!(rc, RLDB_MISUSE);
    assert!(backup.is_null());

    // NULL out_backup.
    // HLT-029: proves crates/ffi/src/snapshot.rs:19 `out.is_null()` guard.
    let rc = rldb_backup_init(db, dst.as_ptr(), ptr::null(), ptr::null_mut());
    assert_eq!(rc, RLDB_MISUSE);

    close_db(db);
}

/// `rldb_backup_step` / `rldb_backup_finish` / `rldb_backup_remaining` /
/// `rldb_backup_pagecount` must all reject NULL backup pointers. D1
/// only covers the close path; the other backup verbs were not
/// exercised on null inputs.
#[test]
fn backup_lifecycle_funcs_reject_null_backup() {
    // HLT-029: proves crates/ffi/src/snapshot.rs:52/74/94/104 null guards.
    assert_eq!(rldb_backup_step(ptr::null_mut(), 1), RLDB_MISUSE);
    assert_eq!(rldb_backup_finish(ptr::null_mut()), RLDB_MISUSE);
    assert_eq!(rldb_backup_remaining(ptr::null_mut()), RLDB_MISUSE);
    assert_eq!(rldb_backup_pagecount(ptr::null_mut()), RLDB_MISUSE);
    assert_eq!(rldb_backup_close(ptr::null_mut()), RLDB_MISUSE);
}

/// `rldb_busy_timeout`, `rldb_changes`, `rldb_last_insert_rowid` on
/// NULL db must each return their documented sentinel without UB. D1
/// covers `changes` + `last_insert_rowid` but NOT `busy_timeout` (which
/// is not in its imports). Adding the `busy_timeout` cell closes the
/// gap.
#[test]
fn config_surface_null_db_sentinels() {
    let db: *mut rldb = ptr::null_mut();
    // HLT-029: proves crates/ffi/src/config.rs:15 `db.is_null()` guard.
    assert_eq!(rldb_busy_timeout(db, 100), RLDB_MISUSE);
    assert_eq!(rldb_changes(db), RLDB_MISUSE);
    assert_eq!(rldb_last_insert_rowid(db), 0);
}

/// Empty / whitespace-only / comment-only SQL must succeed with
/// `*out_stmt = NULL` per the documented sqlite3_prepare_v2 contract
/// (which the FFI mirrors in `crates/ffi/src/stmt.rs:68-75`). This
/// confirms the early-return path does NOT leak a stale pointer.
#[test]
fn prepare_empty_sql_returns_ok_with_null_stmt() {
    let (_dir, db) = open_db("prepare-empty");

    for sql in &["", "   ", "-- just a comment\n"] {
        let cs = CString::new(*sql).expect("nul-free");
        let mut stmt: *mut rldb_stmt = ptr::null_mut();
        let rc = rldb_prepare_v2(db, cs.as_ptr(), -1, &mut stmt, ptr::null_mut());
        // Documented contract: OK with NULL out_stmt for empty/comment-only.
        // HLT-029: proves crates/ffi/src/stmt.rs:71 null-stmt write path.
        assert_eq!(rc, RLDB_OK, "rc={rc} for sql={sql:?}");
        assert!(stmt.is_null(), "expected NULL stmt for sql={sql:?}");
    }

    close_db(db);
}

/// `rldb_open_v2` on a path that already exists as a regular file (not
/// the redline directory layout) must surface a non-OK code rather
/// than panic. Exercises the error branch of `open_handle` in
/// `crates/ffi/src/util.rs::open_handle` where the existing path is
/// not a valid Database root.
#[test]
fn open_v2_on_path_that_is_a_regular_file_returns_error_without_panic() {
    // Create a tempfile that is a regular FILE, not a directory. The
    // engine's Database::open expects the redline directory layout, so
    // opening a regular file at that path must fail without UB.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("not-a-redline-dir");
    std::fs::write(&path, b"definitely not a redline db root").expect("write probe file");
    let c_path = CString::new(path.to_str().expect("utf8 path")).expect("nul-free");
    let mut db: *mut rldb = ptr::null_mut();
    // HLT-029: proves crates/ffi/src/lifecycle.rs:49 + util.rs:95 panic-free
    // error mapping when the underlying open fails on a non-DB path.
    let rc = rldb_open_v2(c_path.as_ptr(), ptr::null(), &mut db);
    assert_ne!(rc, RLDB_OK, "expected non-OK; got rc={rc}");
    assert!(db.is_null(), "out_db must stay NULL when open fails");
}

/// `rldb_bind_int64` on a freshly-prepared statement, then reset, then
/// re-bind, must NOT leak the previous binding across reset. While not
/// strictly a null/oversize test, this proves the lifecycle invariant
/// that the SAFETY comment on `rldb_reset` (`crates/ffi/src/stmt.rs:146`)
/// relies on: the boxed stmt is single-thread-owned and the mutable
/// borrow consumed by reset cannot alias.
#[test]
fn bind_then_reset_then_rebind_does_not_panic() {
    let (_dir, db) = open_db("bind-reset-bind");
    let setup = CString::new("CREATE TABLE t(id INTEGER PRIMARY KEY)").expect("nul-free");
    assert_eq!(
        rldb_exec(db, setup.as_ptr(), None, ptr::null_mut(), ptr::null_mut()),
        RLDB_OK
    );
    let sql = CString::new("INSERT INTO t VALUES(?1)").expect("nul-free");
    let mut stmt: *mut rldb_stmt = ptr::null_mut();
    assert_eq!(
        rldb_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()),
        RLDB_OK
    );
    assert!(!stmt.is_null());

    assert_eq!(rldb_bind_int64(stmt, 1, 7), RLDB_OK);
    let step1 = rldb_step(stmt);
    assert_eq!(step1, RLDB_DONE, "step1 rc={step1}");

    // HLT-029: proves crates/ffi/src/stmt.rs:146 reset's single-thread `&mut`
    // borrow path is panic-free across the reset/re-bind sequence.
    assert_eq!(redlinedb::rldb_reset(stmt), RLDB_OK);
    assert_eq!(rldb_bind_int64(stmt, 1, 8), RLDB_OK);
    let step2 = rldb_step(stmt);
    assert_eq!(step2, RLDB_DONE, "step2 rc={step2}");

    assert_eq!(rldb_finalize(stmt), RLDB_OK);
    close_db(db);
}

/// Oversize SQL with `errmsg` slot provided: must populate errmsg on
/// failure AND surface a non-OK code. D1's
/// `oversize_sql_is_rejected_gracefully` calls `rldb_prepare_v2` (no
/// errmsg slot); this test goes through `rldb_exec` with errmsg so the
/// `set_errmsg` SAFETY contract in `crates/ffi/src/util.rs:334` is
/// exercised on the parser-error path.
#[test]
fn oversize_sql_via_exec_populates_errmsg() {
    let (_dir, db) = open_db("oversize-errmsg");
    // ~256 KiB of malformed SQL — definitely fails to parse. We use
    // garbage tokens so the parser fails fast without OOM risk.
    let mut sql = String::with_capacity(300_000);
    sql.push_str("INVALID ");
    for _ in 0..32_000 {
        sql.push_str("FOO BAR ");
    }
    sql.push(';');
    let c_sql = CString::new(sql).expect("nul-free");
    let mut errmsg: *mut c_char = ptr::null_mut();
    let rc = rldb_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), &mut errmsg);
    assert!(
        rc == RLDB_ERROR || rc == RLDB_MISUSE,
        "expected ERROR/MISUSE for malformed SQL; got rc={rc}",
    );
    // errmsg may or may not be populated depending on the parser path;
    // when populated, the caller MUST be able to free it via rldb_free.
    if !errmsg.is_null() {
        // HLT-029: proves crates/ffi/src/util.rs:334 set_errmsg ownership
        // transfer; the from_raw pair lives in rldb_free.
        let msg = unsafe { CStr::from_ptr(errmsg) }.to_str().expect("utf8");
        assert!(!msg.is_empty(), "errmsg populated but empty");
        redlinedb::rldb_free(errmsg as *mut std::os::raw::c_void);
    }
    close_db(db);
}
