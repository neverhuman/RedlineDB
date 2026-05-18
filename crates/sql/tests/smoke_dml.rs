//! DML-surface smoke tests: INSERT (including INSERT ... SELECT and
//! defaults), UPDATE, DELETE, UPSERT (`INSERT OR IGNORE` /
//! `INSERT OR REPLACE` / `ON CONFLICT`), `RETURNING`, implicit rowid
//! allocation, and the Lane B physical-index DML invariants (index
//! maintenance under INSERT/UPDATE/DELETE, rollback, and crash recovery).
//!
//! Split off from the original `tests/sql_smoke.rs` (Phase 11 Wave 0).
//! Each `#[test] fn` here is verbatim from the source file.

mod common;

use common::open_database;
use redlinedb_sql::{Database, DbOptions, Step};

#[test]
fn column_defaults_survive_catalog_reopen() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("defaults.redline");
    {
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let conn = db.connect();
        conn.execute(
            "CREATE TABLE t(\
                id INTEGER PRIMARY KEY,\
                boolish BOOLEAN DEFAULT FALSE,\
                ts_text TEXT DEFAULT '1970-01-01T00:00:00Z',\
                metadata JSONB DEFAULT '{}'\
            )",
        )
        .expect("create table with defaults");
        conn.execute("INSERT INTO t(id) VALUES (1)")
            .expect("insert");
    }

    let db = Database::open(&path, DbOptions::default()).expect("reopen");
    let conn = db.connect();
    let mut stmt = conn
        .prepare("SELECT boolish, ts_text, metadata FROM t WHERE id = 1")
        .expect("select");
    assert_eq!(stmt.step().expect("row"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("bool default"), 0);
    assert_eq!(
        stmt.column_text(1).expect("timestamp default"),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(stmt.column_text(2).expect("json default"), "{}");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn explicit_null_does_not_receive_column_default() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT DEFAULT 'fallback')")
        .expect("create table");

    conn.execute("INSERT INTO t(id, v) VALUES (1, NULL)")
        .expect("insert explicit null");
    conn.execute("INSERT INTO t(id) VALUES (2)")
        .expect("insert omitted default");

    let mut stmt = conn
        .prepare("SELECT id, v FROM t ORDER BY id")
        .expect("select");
    assert_eq!(stmt.step().expect("row 1"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id 1"), 1);
    assert!(matches!(
        stmt.column_value(1).expect("explicit null"),
        redlinedb_sql::SqlValue::Null
    ));
    assert_eq!(stmt.step().expect("row 2"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id 2"), 2);
    assert_eq!(stmt.column_text(1).expect("default"), "fallback");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn insert_select_populates_target_rows() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE src(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create src");
    conn.execute("CREATE TABLE dst(id INTEGER PRIMARY KEY, v TEXT DEFAULT 'd')")
        .expect("create dst");
    conn.execute("INSERT INTO src VALUES (1, 'one'), (2, 'two')")
        .expect("insert src");
    assert_eq!(
        conn.execute("INSERT INTO dst(id, v) SELECT id + 10, upper(v) FROM src ORDER BY id")
            .expect("insert select"),
        2
    );
    let mut stmt = conn
        .prepare("SELECT id, v FROM dst ORDER BY id")
        .expect("select dst");
    assert_eq!(stmt.step().expect("row 1"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 11);
    assert_eq!(stmt.column_text(1).expect("v"), "ONE");
    assert_eq!(stmt.step().expect("row 2"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 12);
    assert_eq!(stmt.column_text(1).expect("v"), "TWO");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn implicit_rowids_come_from_the_kernel_allocator() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t(v) VALUES ('one')")
        .expect("insert 1");
    conn.execute("INSERT INTO t(v) VALUES ('two')")
        .expect("insert 2");

    let mut stmt = conn
        .prepare("SELECT id FROM t ORDER BY id")
        .expect("prepare select");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    let first = stmt.column_i64(0).expect("first rowid");
    assert!(first > 0);
    assert_eq!(stmt.step().expect("step"), Step::Row);
    let second = stmt.column_i64(0).expect("second rowid");
    assert!(second > first);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn returning_clauses_surface_write_rows() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");

    let mut insert = conn
        .prepare("INSERT INTO t(a, b) VALUES (1, 'one') RETURNING a, b")
        .expect("prepare insert returning");
    assert_eq!(insert.step().expect("step"), Step::Row);
    assert_eq!(insert.column_i64(0).expect("a"), 1);
    assert_eq!(insert.column_text(1).expect("b"), "one");
    assert_eq!(insert.step().expect("done"), Step::Done);

    let mut update = conn
        .prepare("UPDATE t SET b = 'two' WHERE a = 1 RETURNING a, b")
        .expect("prepare update returning");
    assert_eq!(update.step().expect("step"), Step::Row);
    assert_eq!(update.column_i64(0).expect("a"), 1);
    assert_eq!(update.column_text(1).expect("b"), "two");
    assert_eq!(update.step().expect("done"), Step::Done);

    let mut delete = conn
        .prepare("DELETE FROM t WHERE a = 1 RETURNING a")
        .expect("prepare delete returning");
    assert_eq!(delete.step().expect("step"), Step::Row);
    assert_eq!(delete.column_i64(0).expect("a"), 1);
    assert_eq!(delete.step().expect("done"), Step::Done);
}

#[test]
fn upsert_and_conflict_algorithms_work() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT UNIQUE, note TEXT)")
        .expect("create table");
    conn.execute("INSERT INTO t VALUES (1, 'one', 'original')")
        .expect("insert original");

    conn.execute("INSERT OR IGNORE INTO t VALUES (2, 'one', 'ignored')")
        .expect("insert or ignore");

    let mut stmt = conn
        .prepare("SELECT id, v, note FROM t ORDER BY id")
        .expect("select after ignore");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 1);
    assert_eq!(stmt.column_text(1).expect("v"), "one");
    assert_eq!(stmt.column_text(2).expect("note"), "original");
    assert_eq!(stmt.step().expect("done"), Step::Done);

    conn.execute("INSERT OR REPLACE INTO t VALUES (2, 'one', 'replaced')")
        .expect("insert or replace");

    let mut stmt = conn
        .prepare("SELECT id, v, note FROM t ORDER BY id")
        .expect("select after replace");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 2);
    assert_eq!(stmt.column_text(1).expect("v"), "one");
    assert_eq!(stmt.column_text(2).expect("note"), "replaced");
    assert_eq!(stmt.step().expect("done"), Step::Done);

    conn.execute("INSERT INTO t VALUES (3, 'two', 'second')")
        .expect("insert second");
    conn.execute(
        "INSERT INTO t(id, v, note) VALUES (4, 'two', 'updated') ON CONFLICT(v) DO NOTHING",
    )
    .expect("insert on conflict do nothing");

    let mut stmt = conn
        .prepare("SELECT id, v, note FROM t WHERE v = 'two'")
        .expect("select after do nothing");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 3);
    assert_eq!(stmt.column_text(2).expect("note"), "second");
    assert_eq!(stmt.step().expect("done"), Step::Done);

    conn.execute("INSERT INTO t(id, v, note) VALUES (4, 'two', 'conflict update') ON CONFLICT(v) DO UPDATE SET note = excluded.note")
        .expect("insert on conflict do update");

    let mut stmt = conn
        .prepare("SELECT id, v, note FROM t WHERE v = 'two'")
        .expect("select after do update");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 3);
    assert_eq!(stmt.column_text(2).expect("note"), "conflict update");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

// ---------------------------------------------------------------------------
// Lane B: physical-index DML maintenance
//
// These tests assert that SQL INSERT/UPDATE/DELETE keep the kernel B-tree in
// sync with the heap. We probe the B-tree directly via
// `Engine::index_handle` to guarantee the entries actually moved (and were
// not just enforced by the legacy O(N) scan).
// ---------------------------------------------------------------------------

mod lane_b {
    use super::open_database;
    use redlinedb_kernel::catalog::{
        EncodedIndexKey, IndexDef, IndexKeySource, OwnedValue, SortDir, encode_index_key,
    };
    use redlinedb_kernel::txn::Isolation;
    use redlinedb_sql::{Database, DbOptions, Step};
    use tempfile::tempdir;

    /// Build the same encoded index key bytes that the SQL exec layer
    /// produces, so the test can probe the physical B-tree directly.
    fn build_index_key_for_test(index: &IndexDef, values: &[OwnedValue]) -> Vec<u8> {
        let mut dirs: Vec<SortDir> = Vec::with_capacity(index.keys.len());
        let mut owned_refs: Vec<&OwnedValue> = Vec::with_capacity(index.keys.len());
        for key in &index.keys {
            let IndexKeySource::Column { attnum } = key.source;
            owned_refs.push(values.get(attnum as usize).unwrap_or(&OwnedValue::Null));
            dirs.push(key.sort_dir);
        }
        let value_refs: Vec<_> = owned_refs.iter().map(|v| v.as_ref()).collect();
        let mut buf = Vec::new();
        let EncodedIndexKey { bytes, .. } = encode_index_key(&value_refs, &dirs, &mut buf);
        bytes
    }

    fn lookup_index_def(
        conn: &redlinedb_sql::Connection,
        schema: &str,
        name: &str,
    ) -> std::sync::Arc<IndexDef> {
        let snapshot = conn.engine_for_tests().schema_snapshot();
        let _ = schema;
        snapshot
            .indexes
            .iter()
            .find(|idx| idx.name.eq_ignore_ascii_case(name))
            .cloned()
            .unwrap_or_else(|| panic!("index `{name}` missing from snapshot"))
    }

    fn assert_index_has_key(
        conn: &redlinedb_sql::Connection,
        index_name: &str,
        values: &[OwnedValue],
    ) {
        let index = lookup_index_def(conn, "main", index_name);
        let bytes = build_index_key_for_test(&index, values);
        let handle = conn
            .engine_for_tests()
            .index_handle(index.index_id)
            .unwrap_or_else(|| panic!("no physical handle for index `{index_name}`"));
        let engine = conn.engine_for_tests();
        let tx = engine
            .begin(Isolation::Snapshot)
            .expect("begin visible probe");
        let rows = handle
            .point_lookup_visible(engine.tx_status(), tx.snapshot(), Some(tx.id()), &bytes)
            .expect("point_lookup_visible");
        engine.rollback(tx).expect("rollback visible probe");
        assert!(
            !rows.is_empty(),
            "expected index `{index_name}` to contain key for {values:?}"
        );
    }

    fn assert_index_missing_key(
        conn: &redlinedb_sql::Connection,
        index_name: &str,
        values: &[OwnedValue],
    ) {
        let index = lookup_index_def(conn, "main", index_name);
        let bytes = build_index_key_for_test(&index, values);
        let handle = conn
            .engine_for_tests()
            .index_handle(index.index_id)
            .unwrap_or_else(|| panic!("no physical handle for index `{index_name}`"));
        let engine = conn.engine_for_tests();
        let tx = engine
            .begin(Isolation::Snapshot)
            .expect("begin visible probe");
        let rows = handle
            .point_lookup_visible(engine.tx_status(), tx.snapshot(), Some(tx.id()), &bytes)
            .expect("point_lookup_visible");
        engine.rollback(tx).expect("rollback visible probe");
        assert!(
            rows.is_empty(),
            "expected index `{index_name}` to NOT contain key for {values:?}, got {rows:?}"
        );
    }

    #[test]
    fn single_column_unique_index_rejects_duplicate_insert() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE UNIQUE INDEX t_a_uq ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 'one')")
            .expect("first insert");
        let err = conn
            .execute("INSERT INTO t VALUES (1, 'duplicate')")
            .expect_err("duplicate must error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("UNIQUE") || msg.contains("Constraint"),
            "expected unique-violation error, got {msg}"
        );
        // Index should still report the original key (and only that rowid).
        assert_index_has_key(&conn, "t_a_uq", &[OwnedValue::Integer(1)]);
        // The non-conflicting insert path should also succeed and be indexed.
        conn.execute("INSERT INTO t VALUES (2, 'two')")
            .expect("second insert");
        assert_index_has_key(&conn, "t_a_uq", &[OwnedValue::Integer(2)]);
    }

    #[test]
    fn multi_column_unique_index_skips_check_when_any_part_null() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b INTEGER, c TEXT)")
            .expect("create");
        conn.execute("CREATE UNIQUE INDEX t_ab_uq ON t(a, b)")
            .expect("create index");
        // Two rows with NULL in one component — both must succeed (SQLite
        // NULL parity: NULL is never a duplicate).
        conn.execute("INSERT INTO t(a, b, c) VALUES (1, NULL, 'x')")
            .expect("insert null b 1");
        conn.execute("INSERT INTO t(a, b, c) VALUES (1, NULL, 'y')")
            .expect("insert null b 2");
        conn.execute("INSERT INTO t(a, b, c) VALUES (NULL, 5, 'z')")
            .expect("insert null a");
        // Two non-null tuples — duplicates of (1,1) must error.
        conn.execute("INSERT INTO t(a, b, c) VALUES (1, 1, 'first')")
            .expect("first non-null");
        let err = conn
            .execute("INSERT INTO t(a, b, c) VALUES (1, 1, 'second')")
            .expect_err("duplicate must error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("UNIQUE") || msg.contains("Constraint"),
            "expected unique-violation error, got {msg}"
        );
        // The non-null pair is in the index; NULL-bearing entries also get
        // indexed but are not subject to the unique check.
        assert_index_has_key(
            &conn,
            "t_ab_uq",
            &[OwnedValue::Integer(1), OwnedValue::Integer(1)],
        );
    }

    #[test]
    fn insert_or_replace_replaces_on_unique_conflict() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE UNIQUE INDEX t_a_uq ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (1, 'first')")
            .expect("first insert");
        // INSERT OR REPLACE must succeed and overwrite the existing row.
        conn.execute("INSERT OR REPLACE INTO t VALUES (1, 'second')")
            .expect("replace must succeed");
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "second");
        assert_eq!(stmt.step().expect("done"), Step::Done);
        // Index still has the unique key (1) — Lane B re-inserted it after
        // delete-marking the old entry.
        assert_index_has_key(&conn, "t_a_uq", &[OwnedValue::Integer(1)]);
    }

    #[test]
    fn update_to_indexed_column_moves_index_entry() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (10, 'ten')")
            .expect("insert");
        assert_index_has_key(&conn, "t_a_idx", &[OwnedValue::Integer(10)]);
        // Move the row's indexed-column value from 10 -> 20.
        conn.execute("UPDATE t SET a = 20 WHERE b = 'ten'")
            .expect("update");
        // Old key delete-marked, new key inserted.
        assert_index_missing_key(&conn, "t_a_idx", &[OwnedValue::Integer(10)]);
        assert_index_has_key(&conn, "t_a_idx", &[OwnedValue::Integer(20)]);
    }

    #[test]
    fn delete_removes_index_entry() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (7, 'seven')")
            .expect("insert");
        assert_index_has_key(&conn, "t_a_idx", &[OwnedValue::Integer(7)]);
        conn.execute("DELETE FROM t WHERE a = 7").expect("delete");
        assert_index_missing_key(&conn, "t_a_idx", &[OwnedValue::Integer(7)]);
    }

    #[test]
    fn recovery_after_crash_mid_insert_with_index_half_written() {
        // Simulate a crash mid-insert: open a writer, INSERT inside an
        // explicit transaction, and DROP the connection without
        // committing. The kernel WAL contains no commit record for that
        // tx, so recovery must reject both the heap row AND the index
        // entry — atomicity at "either both or neither".
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("recovery.db");
        {
            let db = Database::create(&path, DbOptions::default()).expect("create");
            let conn = db.connect();
            conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
                .expect("create");
            conn.execute("CREATE UNIQUE INDEX t_a_uq ON t(a)")
                .expect("create index");
            // Sanity: a committed insert should be visible after reopen,
            // so we plant one before the kill window.
            conn.execute("INSERT INTO t VALUES (1, 'committed')")
                .expect("commit-row");
            // Now begin a tx and INSERT but never commit. Drop the conn
            // without rolling back to mirror an abrupt crash.
            conn.begin(redlinedb_sql::BeginMode::Deferred)
                .expect("begin");
            conn.execute("INSERT INTO t VALUES (2, 'killed')")
                .expect("insert");
            // Drop without commit — uncommitted state must not survive.
        }
        // Reopen the database. The kernel replays the WAL up to the
        // last commit; the second tx is uncommitted, so neither the
        // heap row nor the index entry must persist.
        let db = Database::open(&path, DbOptions::default()).expect("reopen");
        let conn = db.connect();
        // Heap side: only the committed row is visible.
        let mut stmt = conn
            .prepare("SELECT a, b FROM t ORDER BY a")
            .expect("prepare");
        let mut rows = Vec::new();
        while let Step::Row = stmt.step().expect("step") {
            rows.push((
                stmt.column_i64(0).expect("a"),
                stmt.column_text(1).expect("b").to_owned(),
            ));
        }
        assert_eq!(rows, vec![(1, "committed".to_owned())]);
        // Index side: the committed key is present, the uncommitted key
        // is absent — atomicity of (heap, index) holds across recovery.
        assert_index_has_key(&conn, "t_a_uq", &[OwnedValue::Integer(1)]);
        assert_index_missing_key(&conn, "t_a_uq", &[OwnedValue::Integer(2)]);
    }

    /// Regression: rolling back an INSERT must remove the durable index
    /// entry the SQL DML wrote. Without per-tx index undo, the insert_tx
    /// page mutation persists past rollback and the next legitimate INSERT
    /// of the same key fails with `Constraint`.
    #[test]
    fn rolled_back_insert_does_not_leave_stale_index_entry() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE UNIQUE INDEX t_a_uq ON t(a)")
            .expect("create index");

        // Open a tx, insert (1, 'rolled-back'), then roll back.
        conn.begin(redlinedb_sql::BeginMode::Deferred)
            .expect("begin");
        conn.execute("INSERT INTO t VALUES (1, 'rolled-back')")
            .expect("insert under tx");
        conn.rollback().expect("rollback");

        // Index must NOT carry a stale entry for key 1.
        assert_index_missing_key(&conn, "t_a_uq", &[OwnedValue::Integer(1)]);

        // A fresh INSERT of the same UNIQUE key must succeed (no false
        // conflict from the rolled-back entry).
        conn.execute("INSERT INTO t VALUES (1, 'fresh')")
            .expect("re-insert after rollback must succeed");

        // And the new row should be visible via both the heap and the index.
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 1")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "fresh");
        assert_eq!(stmt.step().expect("done"), Step::Done);
        assert_index_has_key(&conn, "t_a_uq", &[OwnedValue::Integer(1)]);
    }

    /// Regression: rolling back a DELETE must clear the dead flag on the
    /// committed row's index entry. Without index-undo, the delete_mark
    /// stays durable and indexed reads silently miss the row.
    #[test]
    fn rolled_back_delete_does_not_hide_committed_row() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (5, 'five')")
            .expect("insert + commit");

        conn.begin(redlinedb_sql::BeginMode::Deferred)
            .expect("begin");
        conn.execute("DELETE FROM t WHERE a = 5")
            .expect("delete under tx");
        conn.rollback().expect("rollback");

        // Heap path: row is back, visible via TableScan.
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE b = 'five'")
            .expect("prepare heap path");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "five");
        assert_eq!(stmt.step().expect("done"), Step::Done);

        // Index path: row is also back, no longer hidden by a durable dead
        // flag (the SQL-side undo replayed the inverse undelete_mark).
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 5")
            .expect("prepare index path");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "five");
        assert_eq!(stmt.step().expect("done"), Step::Done);

        assert_index_has_key(&conn, "t_a_idx", &[OwnedValue::Integer(5)]);
    }

    /// Regression: rolling back an UPDATE that moved an indexed value must
    /// keep the OLD index key live — the rolled-back tx delete-marked the
    /// old entry and inserted a new one; rollback must reverse both.
    #[test]
    fn rolled_back_update_restores_old_indexed_value() {
        let (_dir, conn) = open_database();
        conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
            .expect("create");
        conn.execute("CREATE INDEX t_a_idx ON t(a)")
            .expect("create index");
        conn.execute("INSERT INTO t VALUES (10, 'ten')")
            .expect("seed");

        conn.begin(redlinedb_sql::BeginMode::Deferred)
            .expect("begin");
        conn.execute("UPDATE t SET a = 20 WHERE b = 'ten'")
            .expect("update under tx");
        conn.rollback().expect("rollback");

        // Index path: the old key (10) is alive again; the new key (20) is
        // gone (its insert was rolled back).
        assert_index_has_key(&conn, "t_a_idx", &[OwnedValue::Integer(10)]);
        assert_index_missing_key(&conn, "t_a_idx", &[OwnedValue::Integer(20)]);

        // SELECT via the index must still return the original row.
        let mut stmt = conn
            .prepare("SELECT b FROM t WHERE a = 10")
            .expect("prepare");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_text(0).expect("b"), "ten");
        assert_eq!(stmt.step().expect("done"), Step::Done);
    }

    /// Regression: with the kernel `UniqueKeyGuard` held across the heap
    /// insert, two concurrent writers attempting the same UNIQUE key must
    /// have exactly one succeed and the other surface a `Constraint`.
    /// Repeated 10x to flush the race window.
    #[test]
    fn concurrent_unique_inserts_only_one_succeeds() {
        use std::sync::Arc as StdArc;
        use std::thread;

        for run in 0..10 {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("concurrent-uq.db");
            let db = Database::create(&path, DbOptions::default()).expect("create");
            let conn = db.connect();
            conn.execute("CREATE TABLE t(a INTEGER, b TEXT)")
                .expect("create");
            conn.execute("CREATE UNIQUE INDEX t_a_uq ON t(a)")
                .expect("create index");
            drop(conn);

            // Two threads racing on the same DB file via two SQL connections,
            // each issuing an INSERT of (1, ...). One must win.
            let key = run as i64 + 1;
            let db_a = StdArc::clone(&db);
            let db_b = StdArc::clone(&db);
            let handle_a = thread::spawn(move || {
                let conn = db_a.connect();
                conn.execute(&format!("INSERT INTO t VALUES ({key}, 'A')"))
            });
            let handle_b = thread::spawn(move || {
                let conn = db_b.connect();
                conn.execute(&format!("INSERT INTO t VALUES ({key}, 'B')"))
            });
            let result_a = handle_a.join().expect("thread A join");
            let result_b = handle_b.join().expect("thread B join");

            // Exactly one must succeed; the other must surface the unique
            // violation. Either ordering is acceptable.
            let successes = [&result_a, &result_b].iter().filter(|r| r.is_ok()).count();
            assert_eq!(
                successes, 1,
                "run {run}: exactly one writer must win; got A={result_a:?} B={result_b:?}"
            );
            let failures: Vec<_> = [&result_a, &result_b]
                .iter()
                .filter_map(|r| r.as_ref().err())
                .collect();
            assert_eq!(failures.len(), 1);
            let msg = format!("{:?}", failures[0]);
            assert!(
                msg.contains("UNIQUE") || msg.contains("Constraint"),
                "run {run}: loser must surface unique violation, got {msg}"
            );

            // The winning row must be in the heap exactly once.
            let conn = db.connect();
            let mut stmt = conn
                .prepare(&format!("SELECT b FROM t WHERE a = {key}"))
                .expect("prepare");
            let mut rows = Vec::new();
            while let Step::Row = stmt.step().expect("step") {
                rows.push(stmt.column_text(0).expect("b").to_owned());
            }
            assert_eq!(rows.len(), 1, "run {run}: exactly one row must commit");
        }
    }

    #[test]
    fn concurrent_autocommit_updates_same_row_all_succeed() {
        use std::sync::{Arc as StdArc, Barrier};
        use std::thread;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("hot-row.db");
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let conn = db.connect();
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)")
            .expect("create");
        conn.execute("INSERT INTO t(id, v) VALUES (1, 0)")
            .expect("insert");
        drop(conn);

        let workers = 4;
        let barrier = StdArc::new(Barrier::new(workers));
        let mut handles = Vec::new();
        for _ in 0..workers {
            let db = StdArc::clone(&db);
            let barrier = StdArc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let conn = db.connect();
                barrier.wait();
                conn.execute("UPDATE t SET v = v + 1 WHERE id = 1")
            }));
        }
        for handle in handles {
            handle
                .join()
                .expect("thread join")
                .expect("autocommit update succeeds");
        }

        let conn = db.connect();
        let mut stmt = conn
            .prepare("SELECT v FROM t WHERE id = 1")
            .expect("select");
        assert_eq!(stmt.step().expect("step"), Step::Row);
        assert_eq!(stmt.column_i64(0).expect("v"), workers as i64);
    }
}
