use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use redlinedb_sql::{Connection, Database, DbOptions, Step};

static TRACE_ENV_LOCK: Mutex<()> = Mutex::new(());

struct TraceEnvGuard {
    old: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl TraceEnvGuard {
    fn set(dir: &Path) -> Self {
        let guard = TRACE_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old = std::env::var_os("REDLINEDB_PLANNER_TRACE_DIR");
        // SAFETY: this test file serializes mutations of this process env var.
        unsafe {
            std::env::set_var("REDLINEDB_PLANNER_TRACE_DIR", dir);
        }
        Self { old, _guard: guard }
    }
}

impl Drop for TraceEnvGuard {
    fn drop(&mut self) {
        // SAFETY: this test file serializes mutations of this process env var.
        unsafe {
            match &self.old {
                Some(value) => std::env::set_var("REDLINEDB_PLANNER_TRACE_DIR", value),
                None => std::env::remove_var("REDLINEDB_PLANNER_TRACE_DIR"),
            }
        }
    }
}

fn open_database() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("planner_trace.db");
    let db = Database::create(&path, DbOptions::default()).expect("create database");
    let conn = db.connect();
    (dir, conn)
}

fn seed_indexed_table(conn: &Arc<Connection>) {
    conn.execute("CREATE TABLE t(k INTEGER, v TEXT)")
        .expect("create table");
    conn.execute("CREATE INDEX t_k_idx ON t(k)")
        .expect("create index");
    conn.execute("INSERT INTO t VALUES (1, 'one'), (2, 'two'), (3, 'three')")
        .expect("seed rows");
}

fn drain(conn: &Arc<Connection>, sql: &str) {
    let mut stmt = conn.prepare(sql).expect("prepare");
    while let Step::Row = stmt.step().expect("step") {}
}

fn trace_path(dir: &Path) -> PathBuf {
    dir.join("planner-trace.jsonl")
}

#[test]
fn planner_trace_env_records_explicit_explain_only() {
    let trace_dir = tempfile::tempdir().expect("trace dir");
    let _env = TraceEnvGuard::set(trace_dir.path());
    let (_db_dir, conn) = open_database();
    seed_indexed_table(&conn);

    drain(&conn, "SELECT v FROM t WHERE k BETWEEN 1 AND 2");
    assert!(
        !trace_path(trace_dir.path()).exists(),
        "ordinary SELECT should not emit planner trace"
    );

    drain(
        &conn,
        "EXPLAIN QUERY PLAN SELECT v FROM t WHERE k BETWEEN 1 AND 2",
    );

    let text = std::fs::read_to_string(trace_path(trace_dir.path())).expect("trace jsonl");
    let rows = text.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    let record: serde_json::Value = serde_json::from_str(rows[0]).expect("trace row json");
    assert_eq!(record["trace_version"], 1);
    assert_eq!(record["planner"]["access_path_gate"], false);
    assert_eq!(record["plan"]["root_kind"], "Project");
    assert_eq!(record["chosen_access"]["kind"], "IndexScan");
    assert_eq!(record["chosen_access"]["relation"], "t");
    assert_eq!(record["chosen_access"]["index"], "t_k_idx");
    assert_eq!(record["chosen_access"]["index_probe_kind"], "RangeScan");
    assert_eq!(record["chosen_access"]["covering"], false);
    assert_eq!(record["rejected_paths_complete"], false);
}
