//! B3 blob I/O API tests.
//!
//! Round-trips data through `sqlite3_blob_open` -> `_read` -> `_write` ->
//! `_close` against a heap-stored BLOB cell. SQLite's blob API does not
//! resize the cell, so out-of-bounds writes must be rejected.

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

use redlinedb::types::rldb;
use redlinedb::{
    RldbBlob, rldb_close, rldb_exec, rldb_open, sqlite3_blob_bytes, sqlite3_blob_close,
    sqlite3_blob_open, sqlite3_blob_read, sqlite3_blob_reopen, sqlite3_blob_write,
};
use tempfile::TempDir;

const RLDB_OK: i32 = 0;
const RLDB_MISUSE: i32 = 21;
const RLDB_ERROR: i32 = 1;

fn open_db() -> (TempDir, *mut rldb) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blob.redline");
    let c_path = CString::new(path.to_str().unwrap()).unwrap();
    let mut db: *mut rldb = ptr::null_mut();
    let rc = rldb_open(c_path.as_ptr(), &mut db);
    assert_eq!(rc, RLDB_OK);
    (dir, db)
}

fn exec(db: *mut rldb, sql: &str) {
    let c_sql = CString::new(sql).unwrap();
    let rc = rldb_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    assert_eq!(rc, RLDB_OK, "exec({sql})");
}

#[test]
fn open_read_write_close_round_trip() {
    let (_dir, db) = open_db();
    exec(db, "CREATE TABLE docs(id INTEGER PRIMARY KEY, body BLOB)");
    exec(
        db,
        "INSERT INTO docs(id, body) VALUES (1, x'00010203040506070809')",
    );
    let main = CString::new("main").unwrap();
    let table = CString::new("docs").unwrap();
    let column = CString::new("body").unwrap();
    let mut blob: *mut RldbBlob = ptr::null_mut();
    let rc = unsafe {
        sqlite3_blob_open(
            db,
            main.as_ptr(),
            table.as_ptr(),
            column.as_ptr(),
            1,
            1,
            &mut blob,
        )
    };
    assert_eq!(rc, RLDB_OK);
    assert!(!blob.is_null());
    unsafe {
        assert_eq!(sqlite3_blob_bytes(blob), 10);
        let mut buf = [0u8; 4];
        let rc = sqlite3_blob_read(blob, buf.as_mut_ptr() as *mut c_void, 4, 2);
        assert_eq!(rc, RLDB_OK);
        assert_eq!(buf, [2u8, 3, 4, 5]);

        let payload = [0xAAu8, 0xBB, 0xCC];
        let rc = sqlite3_blob_write(blob, payload.as_ptr() as *const c_void, 3, 0);
        assert_eq!(rc, RLDB_OK);

        // Re-read entire cell via a fresh read after close+reopen.
        let rc = sqlite3_blob_close(blob);
        assert_eq!(rc, RLDB_OK);
    }
    let mut blob2: *mut RldbBlob = ptr::null_mut();
    let rc = unsafe {
        sqlite3_blob_open(
            db,
            main.as_ptr(),
            table.as_ptr(),
            column.as_ptr(),
            1,
            1,
            &mut blob2,
        )
    };
    assert_eq!(rc, RLDB_OK);
    unsafe {
        let mut all = vec![0u8; 10];
        let rc = sqlite3_blob_read(blob2, all.as_mut_ptr() as *mut c_void, 10, 0);
        assert_eq!(rc, RLDB_OK);
        assert_eq!(&all[..3], &[0xAA, 0xBB, 0xCC]);
        assert_eq!(&all[3..], &[3u8, 4, 5, 6, 7, 8, 9]);
        sqlite3_blob_close(blob2);
    }
    rldb_close(db);
}

#[test]
fn write_past_cell_length_is_rejected() {
    let (_dir, db) = open_db();
    exec(db, "CREATE TABLE t(id INTEGER PRIMARY KEY, b BLOB)");
    exec(db, "INSERT INTO t(id, b) VALUES (1, x'AABB')");
    let main = CString::new("main").unwrap();
    let table = CString::new("t").unwrap();
    let column = CString::new("b").unwrap();
    let mut blob: *mut RldbBlob = ptr::null_mut();
    let rc = unsafe {
        sqlite3_blob_open(
            db,
            main.as_ptr(),
            table.as_ptr(),
            column.as_ptr(),
            1,
            1,
            &mut blob,
        )
    };
    assert_eq!(rc, RLDB_OK);
    unsafe {
        assert_eq!(sqlite3_blob_bytes(blob), 2);
        let bytes = [0xFFu8; 5];
        let rc = sqlite3_blob_write(blob, bytes.as_ptr() as *const c_void, 5, 0);
        assert_eq!(rc, RLDB_ERROR);
        sqlite3_blob_close(blob);
    }
    rldb_close(db);
}

#[test]
fn reopen_changes_rowid() {
    let (_dir, db) = open_db();
    exec(db, "CREATE TABLE t(id INTEGER PRIMARY KEY, b BLOB)");
    exec(db, "INSERT INTO t(id, b) VALUES (1, x'AA')");
    exec(db, "INSERT INTO t(id, b) VALUES (2, x'BBCC')");
    let main = CString::new("main").unwrap();
    let table = CString::new("t").unwrap();
    let column = CString::new("b").unwrap();
    let mut blob: *mut RldbBlob = ptr::null_mut();
    let rc = unsafe {
        sqlite3_blob_open(
            db,
            main.as_ptr(),
            table.as_ptr(),
            column.as_ptr(),
            1,
            1,
            &mut blob,
        )
    };
    assert_eq!(rc, RLDB_OK);
    unsafe {
        assert_eq!(sqlite3_blob_bytes(blob), 1);
        let rc = sqlite3_blob_reopen(blob, 2);
        assert_eq!(rc, RLDB_OK);
        assert_eq!(sqlite3_blob_bytes(blob), 2);
        sqlite3_blob_close(blob);
    }
    rldb_close(db);
}

#[test]
fn null_pointers_to_blob_api_return_misuse_or_noop() {
    unsafe {
        assert_eq!(sqlite3_blob_close(ptr::null_mut()), RLDB_OK);
        assert_eq!(sqlite3_blob_bytes(ptr::null_mut()), 0);
        assert_eq!(sqlite3_blob_reopen(ptr::null_mut(), 0), RLDB_MISUSE);
        let mut buf = [0u8; 4];
        assert_eq!(
            sqlite3_blob_read(ptr::null_mut(), buf.as_mut_ptr() as *mut c_void, 4, 0),
            RLDB_MISUSE
        );
        let bytes = [0u8; 4];
        assert_eq!(
            sqlite3_blob_write(ptr::null_mut(), bytes.as_ptr() as *const c_void, 4, 0),
            RLDB_MISUSE
        );
    }
}
