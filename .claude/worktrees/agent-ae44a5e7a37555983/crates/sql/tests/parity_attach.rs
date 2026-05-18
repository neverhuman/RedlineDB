//! ATTACH / DETACH DATABASE parity (Workstream A2).
//!
//! The minimum implementation maintains a per-connection alias map (the
//! `crate::exec::attach::AttachMap`). These tests exercise the
//! parser/executor path through the public `Connection` API. Full
//! cross-database SELECT resolution is a follow-up; here we cover:
//!
//!   * ATTACH succeeds and the alias becomes known
//!   * DETACH succeeds for a previously-attached alias
//!   * DETACH errors on an unknown alias
//!   * ATTACH errors on the reserved "main" / "temp" aliases
//!
//! The rusqlite oracle is exercised in the first test so we know the
//! parser shapes match SQLite (both engines accept the literal SQL).

use redlinedb_sql::{Database, DbOptions};

fn open_redline() -> (tempfile::TempDir, std::sync::Arc<redlinedb_sql::Connection>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::create(dir.path().join("main.db"), DbOptions::default())
        .expect("create main");
    (dir, db.connect())
}

#[test]
fn attach_then_detach_alias_round_trip() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    let attach_sql = format!("ATTACH DATABASE '{aux_str}' AS aux");
    conn.execute(&attach_sql).expect("attach");

    // The DETACH form does not round-trip through sqlparser; our text-level
    // detector accepts it.
    conn.execute("DETACH DATABASE aux").expect("detach");

    // Re-attaching the same alias should now succeed.
    conn.execute(&attach_sql).expect("re-attach");

    // And the short form (`DETACH alias`) without DATABASE should also work.
    conn.execute("DETACH aux").expect("detach short form");
}

#[test]
fn detach_unknown_alias_errors() {
    let (_dir, conn) = open_redline();
    let err = conn.execute("DETACH DATABASE nope").unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("no such database") || msg.contains("UnknownTable"),
        "expected unknown-database error, got: {msg}"
    );
}

#[test]
fn attach_reserved_alias_errors() {
    let (dir, conn) = open_redline();
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();
    for reserved in &["main", "temp"] {
        let sql = format!("ATTACH DATABASE '{aux_str}' AS {reserved}");
        let err = conn.execute(&sql).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("reserved") || msg.contains("Unsupported"),
            "expected reserved-alias error for {reserved}, got: {msg}"
        );
    }
}

#[test]
fn attach_via_rusqlite_oracle_smoke() {
    // Confirms the literal SQL syntax we emit is the same shape SQLite
    // accepts — guards against future divergence of the surface text.
    let dir = tempfile::tempdir().expect("tempdir");
    let aux = dir.path().join("aux.db");
    let aux_str = aux.display().to_string();

    let oracle = rusqlite::Connection::open_in_memory().expect("ru open");
    let attach_sql = format!("ATTACH DATABASE '{aux_str}' AS aux");
    oracle.execute_batch(&attach_sql).expect("rusqlite attach");
    oracle.execute_batch("DETACH DATABASE aux").expect("rusqlite detach");
}
