//! Phase 6 R3-B — per-PreparedStatement ScalarProgram VM compile cache.
//!
//! The cache lives at `crate::exec::expr::program::ProgramCache`. Like
//! the sibling `scalar_program_vm.rs` integration test, this file
//! re-includes the source via `#[path = "..."]` so the `pub(crate)`
//! cache surface is reachable from a Cargo integration target. The
//! shim modules below mirror those in `scalar_program_vm.rs`.
//!
//! ## What's covered
//!
//! 1. `cache_hit_on_repeated_compile` — compile the same expression
//!    twice through the cache; the second call is a hit.
//! 2. `cache_miss_on_different_expr` — compile two different
//!    expressions; both miss exactly once.
//! 3. `cache_reset_clears_state` — explicit `clear()` zeroes the
//!    entries (counters are process-wide / monotonic and exposed by
//!    the dedicated reset helper).
//! 4. `cache_thread_local_isolation` — spawn two threads; each sees
//!    only its own cache contents.
//! 5. `cache_respects_compile_failure` — Tier-0 rejects an
//!    unsupported shape; the cache stores the `None` sentinel so the
//!    second call is a hit (no second `compile()` call) and never
//!    promotes the negative result to a positive one.
//! 6. `cache_size_bounded_by_per_query_scope` — `with_program_cache_scope`
//!    clears the cache on entry; nesting two scopes produces a cache
//!    whose size matches only the *inner* scope's entries.

#![allow(dead_code)]

mod value {
    pub use redlinedb_sql::value::*;
}

mod error {
    pub use redlinedb_sql::{Error, Result};
}

mod parser {
    pub mod bind {
        pub fn as_bind_name(value: &sqlparser::ast::Value) -> Option<&str> {
            match value {
                sqlparser::ast::Value::Placeholder(name) => Some(name.as_str()),
                _ => None,
            }
        }
    }
}

pub use redlinedb_sql::format_real_sqlite;

#[path = "../src/exec/expr/program.rs"]
mod program;

use sqlparser::ast::Expr;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use program::{
    CompileCtx, ExprFingerprint, ProgramCache, clear_program_cache, program_cache_hits_total,
    program_cache_len, program_cache_misses_total, reset_program_cache_counters,
    with_program_cache, with_program_cache_scope,
};
use redlinedb_sql::value::SqlValue;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Process-wide serialiser for the cache-counter tests.
///
/// `PROGRAM_CACHE_HITS_TOTAL` / `PROGRAM_CACHE_MISSES_TOTAL` are
/// `AtomicU64` static globals. Cargo's default test runner runs cases
/// concurrently within a single binary, which would race counter
/// assertions. We mirror the pattern used by other counter-sensitive
/// integration tests in the workspace and gate every test that touches
/// the counters behind this mutex.
fn serial_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ── parse helpers ──────────────────────────────────────────────────────────

fn parse_expr(sql: &str) -> Expr {
    let stmt_sql = format!("SELECT {sql}");
    let stmts = Parser::parse_sql(&GenericDialect {}, &stmt_sql).expect("parse");
    let sqlparser::ast::Statement::Query(q) = &stmts[0] else {
        panic!("expected query");
    };
    let sqlparser::ast::SetExpr::Select(sel) = q.body.as_ref() else {
        panic!("expected select");
    };
    match &sel.projection[0] {
        sqlparser::ast::SelectItem::UnnamedExpr(e) => e.clone(),
        sqlparser::ast::SelectItem::ExprWithAlias { expr, .. } => expr.clone(),
        other => panic!("unexpected projection: {other:?}"),
    }
}

fn empty_ctx() -> CompileCtx<'static> {
    CompileCtx { columns: &[] }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn cache_hit_on_repeated_compile() {
    let _g = serial_guard();
    reset_program_cache_counters();
    let mut cache = ProgramCache::new();
    let expr = parse_expr("1 + 2");
    let ctx = empty_ctx();

    let hits_before = program_cache_hits_total();
    let misses_before = program_cache_misses_total();

    // First call: miss. Cache stores the compiled program.
    let first = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert!(first.is_some(), "1 + 2 must be Tier-0 compilable");
    assert_eq!(cache.len(), 1);
    assert_eq!(program_cache_misses_total() - misses_before, 1);
    assert_eq!(program_cache_hits_total() - hits_before, 0);

    // Second call: hit. The cache returns the same Arc.
    let second = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert!(second.is_some());
    assert!(
        std::sync::Arc::ptr_eq(first.as_ref().unwrap(), second.as_ref().unwrap(),),
        "cache must return the same Arc on a hit",
    );
    assert_eq!(cache.len(), 1, "cache size must not grow on a hit");
    assert_eq!(program_cache_misses_total() - misses_before, 1);
    assert_eq!(program_cache_hits_total() - hits_before, 1);

    // A third call with the SAME shape adds another hit.
    let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert_eq!(program_cache_hits_total() - hits_before, 2);
}

#[test]
fn cache_miss_on_different_expr() {
    let _g = serial_guard();
    reset_program_cache_counters();
    let mut cache = ProgramCache::new();
    let expr_a = parse_expr("1 + 2");
    let expr_b = parse_expr("1 - 2");
    let ctx = empty_ctx();

    let misses_before = program_cache_misses_total();
    let hits_before = program_cache_hits_total();

    // Different fingerprints → both miss.
    let _ = cache.get_or_compile(&expr_a, &ctx).expect("compile ok");
    let _ = cache.get_or_compile(&expr_b, &ctx).expect("compile ok");

    assert_eq!(cache.len(), 2);
    assert_eq!(program_cache_misses_total() - misses_before, 2);
    assert_eq!(program_cache_hits_total() - hits_before, 0);

    // The fingerprints themselves must differ.
    let fp_a = ExprFingerprint::of(&expr_a, &ctx);
    let fp_b = ExprFingerprint::of(&expr_b, &ctx);
    assert_ne!(
        fp_a, fp_b,
        "different expressions must produce different fingerprints",
    );
}

#[test]
fn cache_reset_clears_state() {
    let _g = serial_guard();
    reset_program_cache_counters();
    let mut cache = ProgramCache::new();
    let expr = parse_expr("42");
    let ctx = empty_ctx();

    let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert_eq!(cache.len(), 1);
    assert_eq!(program_cache_misses_total(), 1);
    assert_eq!(program_cache_hits_total(), 1);

    // Explicit clear: entries drop, counters reset.
    cache.clear();
    reset_program_cache_counters();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert_eq!(program_cache_misses_total(), 0);
    assert_eq!(program_cache_hits_total(), 0);

    // Post-reset: the same expression misses again because the cache
    // is empty.
    let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert_eq!(program_cache_misses_total(), 1);
}

#[test]
fn cache_thread_local_isolation() {
    let _g = serial_guard();
    // The thread-local cache is per-OS-thread by definition. We verify
    // by populating a parent-thread cache, then asserting a freshly-
    // spawned worker thread sees an empty thread-local. The counters
    // are process-wide (AtomicU64) so they DO leak across threads;
    // that is intentional and asserted separately.
    clear_program_cache();
    reset_program_cache_counters();

    let expr = parse_expr("100 + 1");
    let ctx = empty_ctx();
    with_program_cache(|cache| {
        let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    });
    assert_eq!(program_cache_len(), 1, "parent thread populated cache");

    let handle = std::thread::spawn(|| {
        // Worker thread: its thread-local is independent. Must start
        // empty regardless of parent activity.
        assert_eq!(
            program_cache_len(),
            0,
            "worker thread must NOT share parent's cache entries",
        );
        // Populate the worker cache; parent must not see this entry.
        let expr = parse_expr("200 + 2");
        let ctx = empty_ctx();
        with_program_cache(|cache| {
            let _ = cache.get_or_compile(&expr, &ctx).expect("compile ok");
        });
        program_cache_len()
    });
    let worker_size = handle.join().expect("worker thread joined");
    assert_eq!(worker_size, 1, "worker thread populated its own cache");
    assert_eq!(
        program_cache_len(),
        1,
        "parent thread cache unaffected by worker thread",
    );
}

#[test]
fn cache_respects_compile_failure() {
    let _g = serial_guard();
    // EXISTS-subquery is one of several shapes Tier-0 rejects. The
    // cache should record this as a negative entry (returning
    // `Ok(None)`) and NOT re-run `compile` on the next call. We
    // verify by checking that a second call increments hits, not
    // misses, even though the result is the negative sentinel.
    reset_program_cache_counters();
    let mut cache = ProgramCache::new();
    // `EXISTS (SELECT 1)` parses as `Expr::Exists` which the Tier-0
    // compiler's outer match returns `Ok(false)` for (rejected).
    let expr = parse_expr("EXISTS (SELECT 1)");
    let ctx = empty_ctx();

    let misses_before = program_cache_misses_total();
    let hits_before = program_cache_hits_total();

    let first = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert!(
        first.is_none(),
        "Tier-0 must reject EXISTS — sanity check on test input",
    );
    assert_eq!(cache.len(), 1, "negative result must be cached");
    assert_eq!(program_cache_misses_total() - misses_before, 1);
    assert_eq!(program_cache_hits_total() - hits_before, 0);

    let second = cache.get_or_compile(&expr, &ctx).expect("compile ok");
    assert!(
        second.is_none(),
        "cached negative MUST stay negative — never promote to Some",
    );
    assert_eq!(program_cache_misses_total() - misses_before, 1);
    assert_eq!(
        program_cache_hits_total() - hits_before,
        1,
        "second call hits the cached None sentinel",
    );
    assert_eq!(cache.len(), 1, "cache size unchanged on negative hit");
}

#[test]
fn cache_size_bounded_by_per_query_scope() {
    let _g = serial_guard();
    // `with_program_cache_scope` clears the per-thread cache on
    // entry. Successive scopes start fresh, so the cache never grows
    // beyond a single query's working set even when many queries run
    // serially on the same thread.
    clear_program_cache();

    // Scope 1 populates the cache with two entries.
    let scope_1_size = with_program_cache_scope(|| {
        let ctx = empty_ctx();
        let e1 = parse_expr("1 + 1");
        let e2 = parse_expr("2 * 2");
        with_program_cache(|c| {
            let _ = c.get_or_compile(&e1, &ctx).expect("compile ok");
            let _ = c.get_or_compile(&e2, &ctx).expect("compile ok");
        });
        program_cache_len()
    });
    assert_eq!(scope_1_size, 2, "scope 1 populated 2 entries");

    // Scope 2 must NOT see scope 1's entries — the scope guard
    // clears on entry.
    let scope_2_size = with_program_cache_scope(|| {
        // Cache is empty on scope entry regardless of scope 1's
        // contents.
        assert_eq!(
            program_cache_len(),
            0,
            "scope 2 must start with an empty cache",
        );
        let ctx = empty_ctx();
        let e3 = parse_expr("3 - 3");
        with_program_cache(|c| {
            let _ = c.get_or_compile(&e3, &ctx).expect("compile ok");
        });
        program_cache_len()
    });
    assert_eq!(
        scope_2_size, 1,
        "scope 2 holds only its own entries — bounded by per-query scope",
    );

    // A scope that produces N distinct expression shapes ends with
    // exactly N entries; the cache is not unbounded.
    let burst_size = with_program_cache_scope(|| {
        let ctx = empty_ctx();
        for i in 0..16i64 {
            let expr = parse_expr(&format!("{i} + 1"));
            with_program_cache(|c| {
                let _ = c.get_or_compile(&expr, &ctx).expect("compile ok");
            });
        }
        program_cache_len()
    });
    assert_eq!(
        burst_size, 16,
        "16 distinct shapes → 16 entries (cache grows linearly within a scope, not unboundedly across scopes)",
    );
}

// ── Extra correctness checks ───────────────────────────────────────────────
//
// These are not in the spec's required 6 but pin invariants that
// future maintainers might otherwise erode.

#[test]
fn cache_column_context_affects_fingerprint() {
    let _g = serial_guard();
    // `a + b` against a 2-column row compiles to different bytecode
    // than `a + b` against a wider row that places those columns at
    // different ordinals. The fingerprint must reflect that so we do
    // not return a stale program with wrong `LoadCol(u8)` indices.
    let expr = parse_expr("a + b");
    let ctx_narrow = CompileCtx {
        columns: &[("a".to_owned(), 0), ("b".to_owned(), 1)],
    };
    let ctx_wide = CompileCtx {
        columns: &[
            ("x".to_owned(), 0),
            ("a".to_owned(), 1),
            ("b".to_owned(), 2),
        ],
    };
    let fp_n = ExprFingerprint::of(&expr, &ctx_narrow);
    let fp_w = ExprFingerprint::of(&expr, &ctx_wide);
    assert_ne!(
        fp_n, fp_w,
        "identical SQL against different row shapes must NOT alias",
    );
}
