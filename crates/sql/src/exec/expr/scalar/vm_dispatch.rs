//! Phase 6 R2-A — ScalarProgram VM dispatch wiring.
//!
//! The Tier-0 VM lives at `super::super::program`. This module wraps it
//! with the per-row hot-path entrypoint used by
//! `crate::exec::expr::eval_scalar`:
//!
//!   * [`try_eval_scalar_via_vm`] — checks the global opt-in toggle, builds
//!     a `CompileCtx` from the active `RowContext`, compiles the AST,
//!     and runs the bytecode. Returns `None` for shapes the Tier-0
//!     compiler doesn't support so the caller can fall back to the AST
//!     walker without any user-visible behaviour change.
//!
//! The dispatch never inspects `Connection` directly — it queries the
//! process-wide flag set by `program::set_vm_dispatch_enabled`. Test
//! harnesses (and a future PRAGMA `redline_scalar_vm = ON` handler)
//! flip the toggle around the workload they want to run through the VM.

use super::super::program::{
    CompileCtx, evaluate, is_vm_dispatch_enabled, record_compile_failure, with_program_cache,
};
use super::row::RowContext;
use crate::error::Result;
use crate::value::SqlValue;
use redlinedb_kernel::catalog::TableDef;
use sqlparser::ast::Expr;
use std::sync::Arc;

/// Hot-path dispatch entry point. Returns:
///
/// * `Some(Ok(value))` — the VM compiled the expression and produced a
///   value. Caller uses this and skips the AST walker.
/// * `Some(Err(_))` — the VM compiled the expression but evaluation
///   failed (e.g. `Error::DatatypeMismatch`). The caller surfaces the
///   error.
/// * `None` — the VM rejected the shape (returned `Ok(None)` or a
///   compile error). The caller falls back to the AST evaluator.
///
/// When the global opt-in toggle is OFF (the production default), this
/// short-circuits to `None` so the executor's behaviour is bit-identical
/// to the pre-VM build.
pub(crate) fn try_eval_scalar_via_vm(
    expr: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Option<Result<SqlValue>> {
    if !is_vm_dispatch_enabled() {
        return None;
    }
    let cols = columns_from_row(row);
    let ctx = CompileCtx {
        columns: cols.as_slice(),
    };
    // Phase 6 R3-B: route through the per-execution program cache so
    // the SAME expression evaluated against successive rows compiles
    // exactly once per query. The cache is a thread-local scoped to
    // `execute_prepared` via `with_program_cache_scope`, so each
    // statement starts with a clean slate and cross-query pollution is
    // impossible.
    //
    // `record_compile_failure` is bumped only when the cache *misses*
    // and produces a `None` sentinel — subsequent rows return the
    // cached `None` via a cache-hit path and short-circuit silently.
    // Doing it the naive way (bumping on every `Ok(None)` arm below)
    // would explode the failure counter on a 1M-row scan over an
    // unsupported shape, defeating the purpose of the diagnostic.
    let compiled = match with_program_cache(|cache| {
        // Snapshot whether the entry already exists so we can
        // disambiguate first-time misses (record_compile_failure
        // should fire) from cached negative hits (it should NOT —
        // otherwise the counter explodes on a 1M-row scan).
        let was_first_seen = !cache.entries_contains_key(expr, &ctx);
        cache
            .get_or_compile(expr, &ctx)
            .map(|res| (res, was_first_seen))
    }) {
        Ok((Some(p), _)) => p,
        Ok((None, was_first_seen)) => {
            if was_first_seen {
                record_compile_failure();
            }
            return None;
        }
        Err(_) => {
            // Hard compile error path: NOT cached (the cache stores
            // only `Ok(...)` results). Always bumps the failure
            // counter because the surface is rare and a steady
            // trickle is exactly what we want to surface.
            record_compile_failure();
            return None;
        }
    };
    let row_values = flatten_row(row)?;
    Some(evaluate(&compiled, &row_values, bindings))
}

/// Build a `CompileCtx` column map from a row context. Returns an
/// empty `Vec` for contexts the Tier-0 compiler cannot map ordinals
/// against — those callers will fall back via `compile() == Ok(None)`
/// for any `Expr::Identifier` shape.
fn columns_from_row(row: &RowContext<'_>) -> Vec<(String, usize)> {
    match row {
        RowContext::Table(table) | RowContext::Upsert { current: table, .. } => {
            columns_from_table_def(&table.table, 0)
        }
        RowContext::Joined(rows) => {
            let mut out = Vec::new();
            let mut offset = 0usize;
            for joined in rows.iter() {
                let table = &joined.table;
                out.extend(columns_from_table_def(table, offset));
                offset += table.columns.len();
            }
            out
        }
        _ => Vec::new(),
    }
}

fn columns_from_table_def(table: &Arc<TableDef>, offset: usize) -> Vec<(String, usize)> {
    table
        .columns
        .iter()
        .enumerate()
        .map(|(idx, col)| (col.folded.as_ref().to_owned(), offset + idx))
        .collect()
}

/// Materialise a row context into a flat `Vec<SqlValue>` whose ordinals
/// match `columns_from_row(row)`. `None` means "no compatible row" — the
/// caller falls back to the AST evaluator.
fn flatten_row(row: &RowContext<'_>) -> Option<Vec<SqlValue>> {
    match row {
        RowContext::Table(table) | RowContext::Upsert { current: table, .. } => {
            Some(table.values.clone())
        }
        RowContext::Joined(rows) => {
            let mut out = Vec::new();
            for joined in rows.iter() {
                match &joined.row {
                    Some(present) => out.extend(present.values.iter().cloned()),
                    None => {
                        // Outer-join NULL fill, one slot per declared column.
                        for _ in 0..joined.table.columns.len() {
                            out.push(SqlValue::Null);
                        }
                    }
                }
            }
            Some(out)
        }
        RowContext::Empty => Some(Vec::new()),
        _ => None,
    }
}
