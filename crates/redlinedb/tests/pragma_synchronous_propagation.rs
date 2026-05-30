//! A1: PRAGMA synchronous propagation to engine commit_durability.
//!
//! Before A1, `PRAGMA synchronous = NORMAL;` was parsed and stored on the
//! session but never reached `Engine.commit_durability`, so the commit hot
//! path kept fsync-per-statement regardless of the request. These tests
//! exercise the full SQL → session → engine path.
//!
//! Mapping under test (mirrors `crates/sql/src/exec/mod.rs::PragmaPlan::SetSynchronous`):
//!     PRAGMA synchronous = OFF    → CommitDurability::Normal
//!     PRAGMA synchronous = NORMAL → CommitDurability::Normal
//!     PRAGMA synchronous = FULL   → CommitDurability::Strict
//!     PRAGMA synchronous = EXTRA  → CommitDurability::Strict

use redlinedb::{CommitDurability, Database, Durability, OpenOptions};

#[test]
fn default_on_disk_open_sees_strict_until_pragma_fires() {
    // Without any PRAGMA, the live durability matches `OpenOptions::durability`.
    // Note: in-memory / ephemeral opens force `Durability::UnsafeDev` via
    // `volatile_open_options` (handle.rs), so this test uses an on-disk path
    // where the default actually is `Strict`.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a1.redline");
    let db = Database::open_with_options(&path, OpenOptions::default()).expect("create db");
    assert_eq!(db.commit_durability(), CommitDurability::Strict);
}

#[test]
fn pragma_normal_flips_engine_to_normal() {
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = NORMAL;", ())
        .expect("PRAGMA NORMAL");
    assert_eq!(
        db.commit_durability(),
        CommitDurability::Normal,
        "PRAGMA synchronous = NORMAL must propagate to engine.commit_durability"
    );
}

#[test]
fn pragma_off_flips_engine_to_normal() {
    // OFF maps to Normal (buffered writes), not UnsafeDev — UnsafeDev is
    // intentionally not reachable from PRAGMA (use REDLINEDB_DEFAULT_DURABILITY).
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = OFF;", ())
        .expect("PRAGMA OFF");
    assert_eq!(db.commit_durability(), CommitDurability::Normal);
}

#[test]
fn pragma_full_returns_to_strict() {
    // Demote then re-promote — verifies the atomic store path round-trips.
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = NORMAL;", ())
        .expect("PRAGMA NORMAL");
    assert_eq!(db.commit_durability(), CommitDurability::Normal);
    conn.execute("PRAGMA synchronous = FULL;", ())
        .expect("PRAGMA FULL");
    assert_eq!(db.commit_durability(), CommitDurability::Strict);
}

#[test]
fn pragma_extra_maps_to_strict() {
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = EXTRA;", ())
        .expect("PRAGMA EXTRA");
    assert_eq!(db.commit_durability(), CommitDurability::Strict);
}

#[test]
fn unsafe_dev_open_unaffected_by_pragma_full() {
    // UnsafeDev is the explicit "no fsync, no buffered write" mode for tests/
    // benches. PRAGMA FULL on a UnsafeDev open promotes to Strict (matches
    // SQLite intent: explicit PRAGMA always wins over the implicit default).
    let opts = OpenOptions::default().with_durability(Durability::UnsafeDev);
    let db = Database::create_in_memory(opts).expect("create db");
    assert_eq!(db.commit_durability(), CommitDurability::UnsafeDev);
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = FULL;", ())
        .expect("PRAGMA FULL");
    assert_eq!(db.commit_durability(), CommitDurability::Strict);
}

#[test]
fn writes_still_succeed_after_normal_demote() {
    // Smoke test the demoted path: after PRAGMA NORMAL the engine takes the
    // buffered-write barrier instead of the fsync barrier. Writes must still
    // commit and be readable in the same connection.
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let mut conn = db.connect().expect("connect");
    conn.execute("PRAGMA synchronous = NORMAL;", ())
        .expect("PRAGMA NORMAL");
    conn.execute("CREATE TABLE t (k INTEGER PRIMARY KEY, v TEXT)", ())
        .expect("CREATE TABLE");
    for i in 0..16 {
        conn.execute("INSERT INTO t (k, v) VALUES (?, ?)", (i, format!("row{i}")))
            .expect("INSERT");
    }
    use redlinedb::Step;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM t")
        .expect("prepare COUNT");
    let count = match stmt.step().expect("step") {
        Step::Row(row) => row.get::<i64>(0).expect("col 0"),
        Step::Done => panic!("expected row"),
    };
    assert_eq!(count, 16);
    assert_eq!(db.commit_durability(), CommitDurability::Normal);
}
