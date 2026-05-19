//! Regression coverage for jeryu's RedlineDB-backed state-store SQL surface.

mod common;

use common::open_database;
use redlinedb_sql::Step;

#[test]
fn scalar_wrapped_aggregates_match_sqlite_shapes() {
    let (_dir, conn) = open_database();
    conn.execute(
        "CREATE TABLE cache_requests (
            url TEXT,
            method TEXT,
            hit INTEGER,
            reason_code TEXT,
            bytes_served INTEGER,
            timestamp TEXT
        )",
    )
    .expect("create cache_requests");
    conn.execute(
        "INSERT INTO cache_requests VALUES
            ('crates.io:443', 'CONNECT', 0, 'intercepted_passthrough', 1024, '2026-05-18T00:00:00Z'),
            ('registry.npmjs.org:443', 'GET', 1, 'hit', 2048, '2026-05-18T00:00:01Z')",
    )
    .expect("insert cache requests");

    let mut stmt = conn
        .prepare(
            "SELECT
                COALESCE(SUM(bytes_served), 0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN hit THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN reason_code = 'singleflight_coalesced' THEN 1 ELSE 0 END), 0)
             FROM cache_requests",
        )
        .expect("prepare aggregate query");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("bytes"), 3072);
    assert_eq!(stmt.column_i64(1).expect("count"), 2);
    assert_eq!(stmt.column_i64(2).expect("hits"), 1);
    assert_eq!(stmt.column_i64(3).expect("coalesced"), 0);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn redline_master_alias_lists_schema_objects() {
    let (_dir, conn) = open_database();
    conn.execute("CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT)")
        .expect("create table");
    conn.execute("CREATE INDEX t_b_idx ON t(b)")
        .expect("create index");

    let mut stmt = conn
        .prepare(
            "SELECT type, name, tbl_name
             FROM redline_master
             WHERE type = 'index' AND name = 't_b_idx'",
        )
        .expect("prepare redline_master query");

    assert_eq!(stmt.step().expect("step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("type"), "index");
    assert_eq!(stmt.column_text(1).expect("name"), "t_b_idx");
    assert_eq!(stmt.column_text(2).expect("tbl_name"), "t");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn correlated_subquery_respects_table_alias_shadowing() {
    let (_dir, conn) = open_database();
    conn.execute(
        "CREATE TABLE ci_job_runs (
            job_id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL,
            pipeline_id INTEGER NOT NULL,
            root_pipeline_id INTEGER NOT NULL,
            pipeline_sha TEXT NOT NULL,
            ref_name TEXT NOT NULL,
            job_name TEXT NOT NULL,
            stage TEXT NOT NULL,
            status TEXT NOT NULL,
            runner TEXT,
            runner_pool TEXT,
            queued_duration_secs REAL,
            duration_secs REAL,
            started_at TEXT,
            finished_at TEXT,
            web_url TEXT,
            observed_at TEXT NOT NULL
        )",
    )
    .expect("create ci_job_runs");
    conn.execute(
        "INSERT INTO ci_job_runs VALUES
            (42, 2, 433, 433, 'abc123', 'main', 'compile-workspace', 'compile', 'success',
             'jeryu-build', 'build', 1.0, 12.5, '2026-04-23T00:00:00Z',
             '2026-04-23T00:00:13Z', 'http://localhost/root/dougx/-/jobs/42',
             '2026-04-23T00:00:00Z'),
            (43, 2, 434, 433, 'abc123', 'main', 'test-rust-nextest-1', 'test', 'success',
             'jeryu-build', 'build', 0.5, 9.0, '2026-04-23T00:00:10Z',
             '2026-04-23T00:00:19Z', 'http://localhost/root/dougx/-/jobs/43',
             '2026-04-23T00:00:10Z')",
    )
    .expect("insert ci runs");

    let mut stmt = conn
        .prepare(
            "SELECT job_name,
                    AVG(duration_secs) AS avg_duration_secs,
                    (SELECT duration_secs
                       FROM ci_job_runs latest
                      WHERE latest.project_id = ci_job_runs.project_id
                        AND latest.ref_name = ci_job_runs.ref_name
                        AND latest.job_name = ci_job_runs.job_name
                      ORDER BY observed_at DESC
                      LIMIT 1) AS latest_duration_secs,
                    MAX(duration_secs) AS max_duration_secs,
                    COUNT(*) AS runs
             FROM ci_job_runs
             WHERE project_id = 2 AND ref_name = 'main' AND duration_secs IS NOT NULL
             GROUP BY job_name, stage, runner_pool
             ORDER BY avg_duration_secs DESC
             LIMIT 10",
        )
        .expect("prepare correlated aggregate query");

    assert_eq!(stmt.step().expect("first step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("job"), "compile-workspace");
    assert_eq!(stmt.column_f64(1).expect("avg"), 12.5);
    assert_eq!(stmt.column_f64(2).expect("latest"), 12.5);
    assert_eq!(stmt.column_f64(3).expect("max"), 12.5);
    assert_eq!(stmt.column_i64(4).expect("runs"), 1);

    assert_eq!(stmt.step().expect("second step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("job"), "test-rust-nextest-1");
    assert_eq!(stmt.column_f64(2).expect("latest"), 9.0);
    assert_eq!(stmt.step().expect("done"), Step::Done);
}

#[test]
fn recursive_cte_walks_cache_dependency_graph() {
    let (_dir, conn) = open_database();
    conn.execute(
        "CREATE TABLE cache_verdicts (
            id INTEGER PRIMARY KEY,
            object_hash TEXT NOT NULL,
            inputs_hash TEXT NOT NULL,
            verdict TEXT NOT NULL,
            tier TEXT NOT NULL
        )",
    )
    .expect("create cache_verdicts");
    conn.execute(
        "INSERT INTO cache_verdicts (object_hash, inputs_hash, verdict, tier) VALUES
            ('child-hash-1', 'root-hash', 'hit', 'untrusted'),
            ('grandchild-hash-1', 'child-hash-1', 'hit', 'untrusted')",
    )
    .expect("insert graph");

    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE taint_tree AS (
                SELECT object_hash FROM cache_verdicts WHERE inputs_hash = 'root-hash'
                UNION
                SELECT cv.object_hash FROM cache_verdicts cv
                JOIN taint_tree tt ON cv.inputs_hash = tt.object_hash
            )
            SELECT object_hash FROM taint_tree ORDER BY object_hash",
        )
        .expect("prepare recursive cte");

    assert_eq!(stmt.step().expect("first step"), Step::Row);
    assert_eq!(stmt.column_text(0).expect("child"), "child-hash-1");
    assert_eq!(stmt.step().expect("second step"), Step::Row);
    assert_eq!(
        stmt.column_text(0).expect("grandchild"),
        "grandchild-hash-1"
    );
    assert_eq!(stmt.step().expect("done"), Step::Done);
}
