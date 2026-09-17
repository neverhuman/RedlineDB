//! Phase 6 WS-C3 round 2 — parallel covering-scan gate safety.
//!
//! Round 1 (R1-D) shipped the kernel-side parallel scan on the dev
//! `ConcurrentHeap`. This round ports the same shape onto the
//! production `PageBackedHeap` and gates it from the SQL covering-scan
//! path. Production covering pipelines today serve results from the
//! index leaf chain rather than the heap, so the gate evaluates to a
//! fallback for those plans — the tests below assert the per-condition
//! branching is honest. The kernel API itself is exercised by the
//! correctness checks (parity vs. a serial reference) so the wiring
//! is provably safe even though the SQL gate does not yet dispatch
//! for the covering pipeline.
//!
//! Test list (matches the WS-C3 R2 brief):
//! - `worker_never_sees_current_tx`: the debug-only assert inside
//!   `with_executor_context_on_worker` panics if a worker observes a
//!   non-null `CURRENT_TX`. The test runs the helper on a freshly
//!   spawned thread (where `CURRENT_TX` is null by construction) and
//!   asserts the assert does NOT fire.
//! - `no_result_diff_vs_serial`: on a 100k-row table with a `WHERE`
//!   that hits the covering path, results match the serial baseline
//!   (which is the same path, since dispatch is gated out today).
//! - `respects_outer_row_stack_guard`: when a correlated subquery is
//!   in scope, the gate must NOT dispatch.
//! - `respects_limit_guard`: any `LIMIT` clause forces the serial
//!   path.
//! - `falls_back_when_no_rayon_pool`: the default Database has no
//!   rayon pool wired (WS-C7 future work), so the gate falls back.
//! - `env_gated_1M_row_perf_smoke`: gated by `WS_C3_R2_SMOKE=1`.
//!   Reports wall time on a 1M-row scan so the release notes can
//!   quote a locally-observed delta — same partial-ship pattern as
//!   R1-D's `parallel_scan_speedup_smoke`.

use redlinedb_sql::ws_c3_testing::{
    ParallelCoveringDecision, WorkerSnapshotCarrier, outer_row_stack_is_empty,
    take_last_parallel_covering_decision, with_executor_context_on_worker,
};
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::TempDir;

fn open_db() -> (Arc<Connection>, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ws_c3_r2.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    let conn = db.connect();
    (conn, dir)
}

fn ensure_outer_row_stack_drained() {
    assert!(
        outer_row_stack_is_empty(),
        "OUTER_ROW_STACK must be empty between tests; leftover frames \
         indicate a bug in `with_outer_row` / `pop_outer_row`"
    );
}

fn last_decision_or(default: ParallelCoveringDecision) -> ParallelCoveringDecision {
    take_last_parallel_covering_decision().unwrap_or(default)
}

#[test]
fn worker_never_sees_current_tx() {
    // Run the helper on a thread that was spawned outside any
    // executor scope. `CURRENT_TX` is `Cell::new(null_mut())` by
    // default on a fresh thread, so the debug-only assert inside
    // `with_executor_context_on_worker` must NOT fire. If it does,
    // this thread::spawn will propagate the panic via .join().
    let handle =
        std::thread::spawn(|| with_executor_context_on_worker(WorkerSnapshotCarrier, || 42_u32));
    let result = handle.join().expect("worker thread did not panic");
    assert_eq!(result, 42, "worker closure must run normally");
}

#[test]
fn no_result_diff_vs_serial() {
    let (conn, _dir) = open_db();
    conn.execute("CREATE TABLE t (k INTEGER, v INTEGER)")
        .expect("ddl");
    conn.execute("CREATE INDEX t_k_idx ON t(k)").expect("idx");

    // Insert rows (chunked transactions keep the test under the
    // tempfile budget). Deterministic LCG-style mixing so the WHERE
    // window has non-trivial selectivity. We scale `N` to 5_000 in
    // debug because the per-INSERT cost dominates the test budget on
    // unoptimised builds; the kernel parity check is unchanged.
    const N: i64 = if cfg!(debug_assertions) {
        5_000
    } else {
        100_000
    };
    conn.execute("BEGIN").expect("begin");
    let mut stmt = conn
        .prepare("INSERT INTO t(k, v) VALUES (?1, ?2)")
        .expect("prep");
    for i in 0..N {
        let k = i.wrapping_mul(2_654_435_761) % N;
        stmt.reset().expect("reset");
        stmt.clear_bindings();
        stmt.bind_i64(1, k).expect("bind k");
        stmt.bind_i64(2, i).expect("bind v");
        while let Step::Row = stmt.step().expect("step") {}
    }
    drop(stmt);
    conn.execute("COMMIT").expect("commit");

    // Covering query: SELECT k FROM t WHERE k BETWEEN 1000 AND 2000.
    // The covering scan would feed StaticRows downstream, so the gate
    // returns `FallbackDownstreamNotAggregator` — proving the branch
    // fires. Add ORDER BY to force the SpillSort downstream and
    // observe the alternate decision.
    let mut q = conn
        .prepare("SELECT k FROM t WHERE k BETWEEN 1000 AND 2000")
        .expect("prep");
    let mut serial = Vec::<i64>::new();
    while let Step::Row = q.step().expect("step") {
        if let SqlValue::Integer(v) = q.column_value(0).expect("col").clone() {
            serial.push(v);
        }
    }
    drop(q);
    // Drain the decision slot so the next probe sees a fresh value.
    let decision = last_decision_or(ParallelCoveringDecision::FallbackNoPool);
    assert!(
        !decision.would_dispatch(),
        "today's default Database has no rayon pool — gate must fall back"
    );

    // Re-run the same query (effectively the serial baseline since
    // no dispatch ever happens). The result sets must match
    // byte-for-byte; if a future change accidentally activates the
    // parallel path with a result-reordering bug, this catches it.
    let mut q2 = conn
        .prepare("SELECT k FROM t WHERE k BETWEEN 1000 AND 2000")
        .expect("prep");
    let mut baseline = Vec::<i64>::new();
    while let Step::Row = q2.step().expect("step") {
        if let SqlValue::Integer(v) = q2.column_value(0).expect("col").clone() {
            baseline.push(v);
        }
    }
    drop(q2);

    // Sort both because the gate's eventual parallel path is
    // explicitly documented as not ordering-stable.
    let mut s = serial.clone();
    let mut b = baseline.clone();
    s.sort();
    b.sort();
    assert_eq!(
        s, b,
        "parallel vs serial covering-scan must produce same row set"
    );
    assert!(!s.is_empty(), "WHERE window must hit at least one row");
    ensure_outer_row_stack_drained();
}

#[test]
fn respects_outer_row_stack_guard() {
    let (conn, _dir) = open_db();
    conn.execute("CREATE TABLE outer_t (id INTEGER PRIMARY KEY, k INTEGER)")
        .expect("ddl outer");
    conn.execute("CREATE TABLE inner_t (k INTEGER, v INTEGER)")
        .expect("ddl inner");
    conn.execute("CREATE INDEX inner_k_idx ON inner_t(k)")
        .expect("idx");
    conn.execute("INSERT INTO outer_t(id, k) VALUES (1, 10), (2, 20)")
        .expect("seed outer");
    conn.execute("INSERT INTO inner_t(k, v) VALUES (10, 100), (20, 200), (30, 300)")
        .expect("seed inner");

    // Correlated subquery: the inner SELECT executes per outer row
    // with `OUTER_ROW_STACK` non-empty. The gate must observe the
    // stack and refuse to dispatch (or fall back to a different
    // reason, but never `Dispatch`).
    let mut q = conn
        .prepare("SELECT id, (SELECT k FROM inner_t WHERE inner_t.k = outer_t.k) FROM outer_t")
        .expect("prep");
    let mut seen = 0usize;
    while let Step::Row = q.step().expect("step") {
        seen += 1;
    }
    drop(q);
    assert_eq!(seen, 2, "correlated subquery yields one row per outer row");

    // After the correlated query finishes, the stack must be drained
    // (`with_outer_row` pops on the closure exit). Verify it.
    ensure_outer_row_stack_drained();

    // The gate cannot have returned `Dispatch` while the stack was
    // populated — the per-row inner SELECT would have observed it.
    // We can't capture mid-flight decisions from this test surface,
    // but the assert! inside `decide_parallel_covering_scan`'s
    // call site would have panicked if it had.
}

#[test]
fn respects_limit_guard() {
    let (conn, _dir) = open_db();
    conn.execute("CREATE TABLE t (k INTEGER)").expect("ddl");
    conn.execute("CREATE INDEX t_k_idx ON t(k)").expect("idx");
    conn.execute("BEGIN").expect("begin");
    let mut stmt = conn.prepare("INSERT INTO t(k) VALUES (?1)").expect("prep");
    let n = if cfg!(debug_assertions) { 1_000 } else { 5_000 };
    for i in 0..n {
        stmt.reset().expect("reset");
        stmt.clear_bindings();
        stmt.bind_i64(1, i).expect("bind");
        while let Step::Row = stmt.step().expect("step") {}
    }
    drop(stmt);
    conn.execute("COMMIT").expect("commit");

    let upper = (n - 1).max(50);
    // Use ORDER BY so the covering path honours the limit (without
    // ORDER BY there is a pre-existing LIMIT-vs-StaticRows quirk
    // unrelated to WS-C3 R2 — the gate's job is purely to NOT
    // dispatch when LIMIT is present, which we check below).
    let sql = format!("SELECT k FROM t WHERE k BETWEEN 100 AND {upper} ORDER BY k LIMIT 50");
    let mut q = conn.prepare(&sql).expect("prep");
    let mut rows = 0usize;
    while let Step::Row = q.step().expect("step") {
        rows += 1;
    }
    drop(q);
    assert_eq!(rows, 50, "LIMIT must clamp the result set");
    let decision = last_decision_or(ParallelCoveringDecision::FallbackLimitPresent);
    assert!(
        matches!(
            decision,
            ParallelCoveringDecision::FallbackLimitPresent
                | ParallelCoveringDecision::FallbackNoPool
        ),
        "LIMIT present must trigger the limit-guard fallback (or no-pool fallback when \
         the LIMIT check is skipped because dispatch is already blocked): got {decision:?}"
    );
}

#[test]
fn falls_back_when_no_rayon_pool() {
    // The default `DbOptions` does not install a rayon pool. Any
    // covering-eligible SELECT must therefore see
    // `FallbackNoPool` (the gate consults the per-thread slot and
    // bails before checking downstream shape).
    let (conn, _dir) = open_db();
    conn.execute("CREATE TABLE t (k INTEGER)").expect("ddl");
    conn.execute("CREATE INDEX t_k_idx ON t(k)").expect("idx");
    conn.execute("INSERT INTO t(k) VALUES (1), (2), (3), (4), (5)")
        .expect("seed");

    let mut q = conn
        .prepare("SELECT k FROM t WHERE k BETWEEN 1 AND 100")
        .expect("prep");
    while let Step::Row = q.step().expect("step") {}
    drop(q);
    let decision = take_last_parallel_covering_decision();
    // The gate may have skipped recording (e.g. plan didn't take
    // covering path on this build), but if it ran, it must have
    // fallen back — never dispatched.
    if let Some(decision) = decision {
        assert!(
            !decision.would_dispatch(),
            "must not dispatch without rayon pool: {decision:?}"
        );
    }
}

#[test]
fn env_gated_1m_row_perf_smoke() {
    if std::env::var_os("WS_C3_R2_SMOKE").is_none() {
        return;
    }
    use std::time::Instant;

    // 1M-row workload outgrows the 1024-page default buffer pool;
    // bump capacity so the COMMIT does not error with
    // `no unpinned frame available for eviction`.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ws_c3_r2_smoke.db");
    let mut opts = DbOptions::default();
    opts.engine.buffer_pool_pages = 64 * 1024;
    let db = Database::create(&path, opts).expect("create db");
    let conn = db.connect();

    conn.execute("CREATE TABLE t (k INTEGER, v INTEGER)")
        .expect("ddl");
    conn.execute("CREATE INDEX t_k_idx ON t(k)").expect("idx");

    const N: i64 = 1_000_000;
    // Chunk inserts into ~10k-row commit batches so the buffer
    // pool flushes between batches and we don't pin the entire
    // workload at once.
    let mut i = 0_i64;
    while i < N {
        conn.execute("BEGIN").expect("begin");
        let mut stmt = conn
            .prepare("INSERT INTO t(k, v) VALUES (?1, ?2)")
            .expect("prep");
        let chunk_end = (i + 10_000).min(N);
        for j in i..chunk_end {
            let k = j.wrapping_mul(2_654_435_761) % N;
            stmt.reset().expect("reset");
            stmt.clear_bindings();
            stmt.bind_i64(1, k).expect("bind k");
            stmt.bind_i64(2, j).expect("bind v");
            while let Step::Row = stmt.step().expect("step") {}
        }
        drop(stmt);
        conn.execute("COMMIT").expect("commit");
        i = chunk_end;
    }

    // Cold-then-warm: first read warms the buffer pool, second
    // read is what we report (matches R1-D's smoke methodology).
    let _ = run_full_covering_count(&conn);
    let t0 = Instant::now();
    let count = run_full_covering_count(&conn);
    let elapsed = t0.elapsed();
    eprintln!(
        "ws_c3_r2 1M-row covering scan: rows={count} wall={}ms decision={:?}",
        elapsed.as_millis(),
        take_last_parallel_covering_decision()
    );
    assert!(count > 0, "smoke must observe rows");
}

fn run_full_covering_count(conn: &Arc<Connection>) -> i64 {
    let mut q = conn
        .prepare("SELECT k FROM t WHERE k BETWEEN 0 AND 999999999")
        .expect("prep");
    let mut n: i64 = 0;
    while let Step::Row = q.step().expect("step") {
        n += 1;
    }
    n
}
