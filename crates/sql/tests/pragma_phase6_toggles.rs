//! Phase 6 R3-A: PRAGMA SQL-surface hooks for the R2-A scalar-VM
//! dispatch toggle and the R2-C planner AccessPath gate.
//!
//! Both toggles already exist as thread-local / atomic flags
//! (`crate::exec::expr::program::{set,is}_vm_dispatch_enabled` and
//! `crate::planner::access_path::{set_,}_planner_use_access_path`).
//! Before this round they could only be flipped from Rust callers
//! (tests and the env-var escape hatch). The parser intercepts added in
//! `crates/sql/src/parser/pragma.rs` make them addressable from SQL:
//!
//!   PRAGMA redline_scalar_vm = ON | OFF
//!   PRAGMA redline_planner_use_access_path = ON | OFF
//!
//! With no argument, each PRAGMA returns a single-row recall of the
//! current state (`0` / `1`), mirroring SQLite's PRAGMA-recall surface.
//!
//! The scalar-VM toggle is a process-wide `AtomicBool`, so the tests
//! that touch it share a `Mutex` to keep them race-free under
//! `cargo test`'s default-parallel runner. The planner gate is
//! thread-local — concurrent tests cannot stomp on each other.

#![allow(clippy::needless_borrow)]

use std::sync::{Arc, Mutex};

use redlinedb_sql::{Connection, Database, DbOptions, Step};

fn open_database() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pragma_phase6_toggles.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

/// Run `PRAGMA <name>` (no argument) and return the single-row integer
/// recall value. The recall row reflects the live state read via the
/// same accessor the rest of the engine reads — so a recall value of
/// `1` is observably equivalent to "the underlying toggle is set" from
/// every other consumer's perspective.
fn pragma_recall(conn: &Arc<Connection>, name: &str) -> i64 {
    let sql = format!("PRAGMA {name}");
    let mut stmt = conn.prepare(&sql).expect("prepare PRAGMA recall");
    let mut value: Option<i64> = None;
    while let Step::Row = stmt.step().expect("step PRAGMA recall") {
        value = Some(stmt.column_i64(0).expect("recall column_i64"));
    }
    value.expect("PRAGMA recall produced no row")
}

/// Drive the `PRAGMA <name> = <value>` setter and read its post-set
/// recall row in the same prepared statement.
fn pragma_set_and_recall(conn: &Arc<Connection>, name: &str, value: &str) -> i64 {
    let set_sql = format!("PRAGMA {name} = {value}");
    let mut stmt = conn.prepare(&set_sql).expect("prepare PRAGMA set");
    let mut observed: Option<i64> = None;
    while let Step::Row = stmt.step().expect("step PRAGMA set") {
        observed = Some(stmt.column_i64(0).expect("set recall column_i64"));
    }
    observed.expect("PRAGMA set produced no recall row")
}

/// Serialise the process-wide `redline_scalar_vm` tests against each
/// other so they do not race through the shared `AtomicBool`.
fn scalar_vm_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    // Poisoning is acceptable here — a panicking test still leaves
    // observable state we want to inspect.
    GUARD.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------------------
// PRAGMA redline_scalar_vm  (Phase 6 R2-A)
// ---------------------------------------------------------------------------

#[test]
fn pragma_redline_scalar_vm_on_enables_dispatch() {
    let _guard = scalar_vm_guard();
    let (_dir, conn) = open_database();
    // SET ON: the inline recall must be 1, AND a follow-up no-arg
    // recall (read via the same `is_vm_dispatch_enabled()` accessor
    // the rest of the engine uses) must also be 1 — that second
    // observation is the one that proves the parser arm actually
    // called `set_vm_dispatch_enabled(true)`.
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_scalar_vm", "ON"),
        1,
        "PRAGMA redline_scalar_vm = ON recall row should be 1"
    );
    assert_eq!(
        pragma_recall(&conn, "redline_scalar_vm"),
        1,
        "is_vm_dispatch_enabled() should observe `true` after PRAGMA ON"
    );
    // Restore to OFF — `set_vm_dispatch_enabled` is process-wide.
    conn.execute("PRAGMA redline_scalar_vm = OFF")
        .expect("restore OFF");
}

#[test]
fn pragma_redline_scalar_vm_off_disables_dispatch() {
    let _guard = scalar_vm_guard();
    let (_dir, conn) = open_database();
    // Explicitly OFF — the recall row, and a follow-up no-arg recall,
    // must both be 0.
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_scalar_vm", "OFF"),
        0,
        "PRAGMA redline_scalar_vm = OFF recall row should be 0"
    );
    assert_eq!(
        pragma_recall(&conn, "redline_scalar_vm"),
        0,
        "is_vm_dispatch_enabled() should observe `false` after PRAGMA OFF"
    );
}

#[test]
fn pragma_redline_scalar_vm_round_trip_on_then_off() {
    // ON then OFF observably toggles the flag both ways within a
    // single test thread, using both the inline recall (parsed-time
    // mirror of the write) and a follow-up no-arg recall (live read
    // via `is_vm_dispatch_enabled()`).
    let _guard = scalar_vm_guard();
    let (_dir, conn) = open_database();
    assert_eq!(pragma_set_and_recall(&conn, "redline_scalar_vm", "ON"), 1);
    assert_eq!(pragma_recall(&conn, "redline_scalar_vm"), 1);
    assert_eq!(pragma_set_and_recall(&conn, "redline_scalar_vm", "OFF"), 0);
    assert_eq!(pragma_recall(&conn, "redline_scalar_vm"), 0);
}

// ---------------------------------------------------------------------------
// PRAGMA redline_planner_use_access_path  (Phase 6 R2-C)
// ---------------------------------------------------------------------------

#[test]
fn pragma_redline_planner_use_access_path_on() {
    let (_dir, conn) = open_database();
    // Same belt-and-braces shape as the scalar_vm tests: inline recall
    // AND follow-up no-arg recall. The thread-local gate cannot be
    // raced by other test threads, so no mutex is required.
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "ON"),
        1,
        "PRAGMA redline_planner_use_access_path = ON recall row should be 1"
    );
    assert_eq!(
        pragma_recall(&conn, "redline_planner_use_access_path"),
        1,
        "planner_use_access_path() should observe `true` after PRAGMA ON"
    );
    // The gate is thread-local; reset it before yielding the thread
    // back to the test harness so other tests on the same worker
    // default OFF.
    conn.execute("PRAGMA redline_planner_use_access_path = OFF")
        .expect("restore OFF");
}

#[test]
fn pragma_redline_planner_use_access_path_off() {
    let (_dir, conn) = open_database();
    // Explicit OFF — both recalls must be 0.
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "OFF"),
        0,
        "PRAGMA redline_planner_use_access_path = OFF recall row should be 0"
    );
    assert_eq!(
        pragma_recall(&conn, "redline_planner_use_access_path"),
        0,
        "planner_use_access_path() should observe `false` after PRAGMA OFF"
    );
}

#[test]
fn pragma_redline_planner_use_access_path_bool_variants() {
    // The shared `parse_pragma_bool` helper accepts `1` / `0` / `true`
    // / `false` / `on` / `off`. Spot-check at least one non-keyword
    // form per direction so the parser hook is exercised through the
    // full parsing surface, not just the ON|OFF keywords.
    let (_dir, conn) = open_database();
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "1"),
        1
    );
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "0"),
        0
    );
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "true"),
        1
    );
    assert_eq!(
        pragma_set_and_recall(&conn, "redline_planner_use_access_path", "false"),
        0
    );
    // Final cleanup — leave thread-local cleared.
    conn.execute("PRAGMA redline_planner_use_access_path = OFF")
        .expect("restore OFF");
}
