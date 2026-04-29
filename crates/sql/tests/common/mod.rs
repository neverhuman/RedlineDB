//! Shared helpers for the `smoke_*` integration test suite.
//!
//! Cargo treats files under `tests/common/` as a normal module included via
//! `mod common;` in each integration test binary, so it does not produce its
//! own test binary. The functions here mirror the bootstrap helpers that
//! used to live at the top of `tests/sql_smoke.rs` before that file was
//! split by SQL surface area.
//!
//! `#![allow(dead_code)]` is required because each integration test binary
//! includes the full module but typically only uses a subset of the
//! helpers; without it, Cargo would warn for every binary that doesn't
//! call every helper.

#![allow(dead_code)]

use std::sync::Arc;

use redlinedb_sql::{Database, DbOptions, Step};

pub fn open_database() -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-sql-smoke.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

pub fn open_database_with_options(
    opts: DbOptions,
) -> (tempfile::TempDir, Arc<redlinedb_sql::Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("redlinedb-sql-smoke.db");
    let db = Database::create(&path, opts).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

pub fn step_done(stmt: &mut redlinedb_sql::Statement) {
    assert_eq!(stmt.step().expect("step"), Step::Done);
}
