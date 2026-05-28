//! W4-A: morsel-routed PrimitiveScan entry point.
//!
//! This module is the bridge between `select_top.rs::build_select_runtime`
//! and the morsel scaffolding in `super`. The W4-A series ships in three
//! commits:
//!
//! - **W4-A1** (commit 1): plumbing — `route_primitive_scan` reads the
//!   env-var route mode + the W4-T eligibility classifier and records
//!   telemetry. Always declines so the caller stays on the tuple path.
//! - **W4-A2a** (this commit): adds [`classify_for_routing`] +
//!   [`RoutingPlan`] / [`ColumnRouting`] data structures. The routing
//!   entry now derives a concrete column-and-kind map for every
//!   eligible plan, and decline branches split into projection /
//!   predicate / shape buckets so telemetry shows what's blocking us.
//!   Still no execution change — the function declines after producing
//!   the RoutingPlan.
//! - **W4-A2b** (next commit): wires `HeapRowidScanSource` + the
//!   morsel-batch loop using the `RoutingPlan` from this commit.
//! - **W4-A3** (subsequent commit): WHERE predicate translation and
//!   filter-kernel dispatch.
//!
//! Off by default. Opt in via `REDLINE_MORSEL_ROUTE=primitive_scan`
//! (also `all`, `1`, `on`) — see [`super::morsel_route_mode`].

use std::sync::Arc;
use std::sync::atomic::Ordering;

use redlinedb_kernel::catalog::{Affinity, TableDef};
use redlinedb_kernel::engine::{Engine, Txn};
use sqlparser::ast::{Expr, SelectItem};

use super::{
    MORSEL_ROUTE_FALLBACK_DISABLED, MORSEL_ROUTE_FALLBACK_DYNAMIC_KIND,
    MORSEL_ROUTE_FALLBACK_INELIGIBLE, MORSEL_ROUTE_FALLBACK_PREDICATE,
    MORSEL_ROUTE_FALLBACK_PROJECTION, MORSEL_ROUTE_FALLBACK_SHAPE, MORSEL_ROUTE_USED,
    MorselEligibility, classify_select_plan_eligibility, morsel_route_mode,
    morsel_telemetry_enabled,
};
use crate::Result;
use crate::value::SqlValue;

/// Telemetry tap for routing decisions. No-op when telemetry is disabled.
#[inline]
fn record_decline(counter: &std::sync::atomic::AtomicU64) {
    if morsel_telemetry_enabled() {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// W4-A entry point. Returns `RouteDecision::Decline(_)` whenever the
/// morsel path can't (or won't) handle this plan; the caller continues on
/// the tuple path.
///
/// W4-A2a contract: this function still ALWAYS returns `Decline`. The
/// classifier produces a [`RoutingPlan`] when the shape supports it, and
/// the decline reason is recorded with the appropriate counter — but the
/// scan + emit loop is wired in W4-A2b, not here.
pub fn route_primitive_scan(plan: &crate::statement::SelectPlan) -> Result<RouteDecision> {
    use crate::statement::SelectSource;

    // 1. Route mode gate. When the env var isn't set we bail immediately so
    //    the only cost in default builds is one OnceLock read + branch.
    let mode = match morsel_route_mode() {
        Some(mode) => mode,
        None => {
            record_decline(&MORSEL_ROUTE_FALLBACK_DISABLED);
            return Ok(RouteDecision::Decline(DeclineReason::Disabled));
        }
    };

    // 2. Eligibility gate. The W4-T classifier already knows how to spot
    //    PrimitiveScan-shaped plans. Anything else stays on tuple path.
    let eligibility = classify_select_plan_eligibility(plan);
    if !matches!(eligibility, MorselEligibility::PrimitiveScan) {
        record_decline(&MORSEL_ROUTE_FALLBACK_INELIGIBLE);
        return Ok(RouteDecision::Decline(DeclineReason::Ineligible));
    }

    // 3. Shape-level rejections that the classifier doesn't catch. ORDER BY
    //    and OFFSET are deferred until W4-A3 so morsel routing can't
    //    interfere with ordering semantics. (LIMIT + no ORDER BY is OK
    //    because StaticRows respects the limit at the runtime layer.)
    if !plan.order_by.is_empty() || plan.offset.is_some() {
        record_decline(&MORSEL_ROUTE_FALLBACK_SHAPE);
        return Ok(RouteDecision::Decline(DeclineReason::Shape));
    }

    // 4. Pull the table out of the source. The W4-T classifier already
    //    guaranteed `SelectSource::Table(_)`, so this match is exhaustive
    //    in practice — fall through to Ineligible on the off chance it
    //    isn't (defensive; matches the rest of the executor's style).
    let table = match &plan.source {
        SelectSource::Table(table) => table.as_ref(),
        _ => {
            record_decline(&MORSEL_ROUTE_FALLBACK_INELIGIBLE);
            return Ok(RouteDecision::Decline(DeclineReason::Ineligible));
        }
    };

    // 5. W4-A2a: classify projection + WHERE into a `RoutingPlan`. Any
    //    shape we don't recognise produces a precise `DeclineReason` so
    //    the routing counters surface exactly which gate fires most often
    //    on the corpus.
    let _routing_plan = match classify_for_routing(plan, table) {
        Ok(routing) => routing,
        Err(reason) => {
            let counter = match reason {
                DeclineReason::Projection => &MORSEL_ROUTE_FALLBACK_PROJECTION,
                DeclineReason::Predicate => &MORSEL_ROUTE_FALLBACK_PREDICATE,
                _ => &MORSEL_ROUTE_FALLBACK_SHAPE,
            };
            record_decline(counter);
            return Ok(RouteDecision::Decline(reason));
        }
    };

    // 6. W4-A2a stops here. The `RoutingPlan` is ready for W4-A2b's scan
    //    adapter to consume, but we still decline so behaviour is
    //    unchanged. The Shape bucket is reused for "would have routed but
    //    no scan yet" because that's the closest match — the eventual
    //    win is wired by W4-A2b, not this commit.
    let _ = mode;
    record_decline(&MORSEL_ROUTE_FALLBACK_SHAPE);
    Ok(RouteDecision::Decline(DeclineReason::NotYetImplemented))
}

/// W4-A2a: classify a plan's projection and WHERE into a concrete routing
/// plan. Returns `Err(DeclineReason)` when any shape is outside the
/// primitive-scan subset.
///
/// Initial-wave subset (will grow in W4-A3):
/// - Projection: each item must be `SelectItem::UnnamedExpr(Identifier)`
///   or `SelectItem::ExprWithAlias { expr: Identifier, .. }`. The
///   identifier must resolve to a column on `table` with Integer or Real
///   affinity. (Wildcards, expressions, qualified idents — all deferred.)
/// - WHERE: must be `None`. Predicate translation lands in W4-A3 along
///   with the filter-kernel dispatch.
pub(crate) fn classify_for_routing(
    plan: &crate::statement::SelectPlan,
    table: &TableDef,
) -> std::result::Result<RoutingPlan, DeclineReason> {
    // W4-A2a defers WHERE entirely — any selection means tuple path.
    if plan.selection.is_some() {
        return Err(DeclineReason::Predicate);
    }

    let mut projection: smallvec::SmallVec<[ColumnRouting; 8]> =
        smallvec::SmallVec::with_capacity(plan.projection.len());
    for item in &plan.projection {
        let expr = match item {
            SelectItem::UnnamedExpr(expr) => expr,
            SelectItem::ExprWithAlias { expr, .. } => expr,
            // Wildcards are technically supported but require column-list
            // synthesis we haven't wired yet.
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                return Err(DeclineReason::Projection);
            }
        };
        let ident_name = match expr {
            Expr::Identifier(ident) => ident.value.as_str(),
            // Qualified identifiers (`t.col`) and compound exprs are
            // deferred — they require name-resolution against joined
            // tables which the W4-T classifier already ruled out at the
            // source level, but reject defensively here too.
            _ => return Err(DeclineReason::Projection),
        };
        let column = table
            .columns
            .iter()
            .find(|c| c.folded.eq_ignore_ascii_case(ident_name))
            .ok_or(DeclineReason::Projection)?;
        let kind = match column.affinity {
            Affinity::Integer => MorselColumnKind::I64,
            Affinity::Real => MorselColumnKind::F64,
            // Numeric is integer-or-real at runtime; treating it as I64
            // would mishandle Real-shaped values, so defer until W4-A3
            // adds runtime kind detection.
            Affinity::Numeric | Affinity::Text | Affinity::Blob => {
                return Err(DeclineReason::Projection);
            }
        };
        projection.push(ColumnRouting {
            column_ordinal: column.ordinal as usize,
            kind,
        });
    }

    if projection.is_empty() {
        // `SELECT FROM t` is invalid SQL; the projection must have at
        // least one item. Belt-and-braces against malformed plans.
        return Err(DeclineReason::Projection);
    }

    Ok(RoutingPlan { projection })
}

/// W4-A2a: concrete plan of which columns the morsel scan should read
/// out of `table`, and what kind each column is expected to be. Consumed
/// by W4-A2b's `execute_routed_scan` (TBD).
#[derive(Debug, Clone)]
pub(crate) struct RoutingPlan {
    /// One entry per projection item, in order. The morsel scan reads
    /// these columns into morsel batches, then the emit pass projects
    /// them out in the same order to produce final tuples.
    pub(crate) projection: smallvec::SmallVec<[ColumnRouting; 8]>,
}

/// Per-column routing entry: which ordinal to read from the source row,
/// and what numeric kind to materialise it as in the morsel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColumnRouting {
    pub(crate) column_ordinal: usize,
    pub(crate) kind: MorselColumnKind,
}

/// Narrow column-kind enum for the W4-A initial wave. Mirrors the subset
/// of `super::ColumnKind` we actually route through filter kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MorselColumnKind {
    I64,
    F64,
}

/// W4-A2b: the actual scan-and-emit. Returns `Some(rows)` on a successful
/// route, `None` if the routing decides at runtime that the tuple path
/// must handle this query (e.g. a row has a value of the wrong kind for
/// the column's affinity-classified kind). Records the appropriate
/// telemetry counter on either outcome.
///
/// Initial wave (W4-A2b):
/// - WHERE must be empty (gated by `classify_for_routing`).
/// - Projection columns must be Integer/Real affinity bare identifiers.
/// - Runtime values must match the affinity-derived kind, OR be NULL.
///   Any other kind (e.g. INTEGER-affinity column holds a TEXT value via
///   SQLite's loose typing) bails to tuple path so semantics stay
///   identical.
///
/// The scan walks `Engine::scan_rowids` and `load_table_row_by_rowid`
/// directly, then projects by indexed access into `row.values[ordinal]`.
/// This skips `eval_projection_item` for the supported shape — which
/// for a 1000-row table calling `project_row` 1000× saves N expression
/// evaluations per query. Without morsel batching the work isn't
/// vectorised yet (W4-A3 adds that), but the work IS routed.
pub(crate) fn execute_routed_scan(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    plan: &crate::statement::SelectPlan,
) -> Result<Option<Vec<Vec<SqlValue>>>> {
    // Re-classify here so the caller doesn't have to thread RoutingPlan
    // through. The classification is cheap (single projection walk) and
    // keeps the public surface small.
    let routing = match classify_for_routing(plan, table.as_ref()) {
        Ok(r) => r,
        Err(reason) => {
            let counter = match reason {
                DeclineReason::Projection => &MORSEL_ROUTE_FALLBACK_PROJECTION,
                DeclineReason::Predicate => &MORSEL_ROUTE_FALLBACK_PREDICATE,
                _ => &MORSEL_ROUTE_FALLBACK_SHAPE,
            };
            record_decline(counter);
            return Ok(None);
        }
    };

    let rowids = super::super::collect_table_rowids(engine, tx, table)?;
    let mut out: Vec<Vec<SqlValue>> = Vec::with_capacity(rowids.len());

    for rowid in rowids {
        let Some(fresh) = super::super::load_table_row_by_rowid(engine, tx, table, rowid)?
        else {
            continue;
        };
        let mut row_out: Vec<SqlValue> = Vec::with_capacity(routing.projection.len());
        for col in &routing.projection {
            let value = fresh
                .values
                .get(col.column_ordinal)
                .cloned()
                .unwrap_or(SqlValue::Null);
            // Bail to tuple path if runtime kind doesn't match the
            // affinity-classified kind. SQLite's loose typing allows
            // (e.g.) a TEXT value in an INTEGER-affinity column; the
            // tuple path coerces via `apply_affinity`, but our routed
            // emit doesn't. Null is universally compatible.
            match (&value, col.kind) {
                (SqlValue::Null, _) => {}
                (SqlValue::Integer(_), MorselColumnKind::I64) => {}
                (SqlValue::Real(_), MorselColumnKind::F64) => {}
                _ => {
                    record_decline(&MORSEL_ROUTE_FALLBACK_DYNAMIC_KIND);
                    return Ok(None);
                }
            }
            row_out.push(value);
        }
        out.push(row_out);
    }

    // Routed successfully. Telemetry counter fires only when enabled.
    if morsel_telemetry_enabled() {
        MORSEL_ROUTE_USED.fetch_add(1, Ordering::Relaxed);
    }
    Ok(Some(out))
}

/// Outcome of [`route_primitive_scan`]. `Routed` is reserved for the future
/// commits that ship actual morsel-routed execution.
#[derive(Debug)]
pub enum RouteDecision {
    /// Caller continues on the tuple path. Reason is recorded for telemetry.
    Decline(DeclineReason),
    /// Routed via morsel; the caller should use the returned rows as if
    /// they came from the existing `StaticRows` fast path. Not produced
    /// by W4-A2a; reserved for W4-A2b+.
    #[allow(dead_code)]
    Routed(Vec<Vec<crate::value::SqlValue>>),
}

/// Why a routing attempt declined to morsel-route. Mapped 1:1 with the
/// `MORSEL_ROUTE_FALLBACK_*` counters in `super` (counter names kept for
/// continuity with the documented telemetry surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    /// `REDLINE_MORSEL_ROUTE` env var not set (the default).
    Disabled,
    /// W4-T classifier says the plan isn't PrimitiveScan-shaped.
    Ineligible,
    /// ORDER BY / OFFSET / LIMIT shape deferred from W4-A2a.
    Shape,
    /// Projection contains non-bare or non-primitive columns.
    Projection,
    /// WHERE predicate uses an op/value combination the filter kernels
    /// don't support yet. (W4-A2a: any WHERE; W4-A3: refines.)
    Predicate,
    /// Mid-scan: the actual row had a value of a kind incompatible with
    /// the column's affinity-derived kind (e.g. INTEGER affinity column
    /// holds a TEXT value).
    #[allow(dead_code)] // wired in W4-A3.
    DynamicKind,
    /// Skeleton-stage stub: future commits will implement the routing.
    NotYetImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decline_reason_is_pure_data() {
        let r = DeclineReason::Disabled;
        assert_eq!(r, DeclineReason::Disabled);
        assert_ne!(r, DeclineReason::Ineligible);
    }

    #[test]
    fn column_routing_is_pure_data() {
        let cr = ColumnRouting {
            column_ordinal: 0,
            kind: MorselColumnKind::I64,
        };
        assert_eq!(cr.column_ordinal, 0);
        assert_eq!(cr.kind, MorselColumnKind::I64);
    }

    #[test]
    fn routing_plan_round_trip() {
        let plan = RoutingPlan {
            projection: smallvec::smallvec![
                ColumnRouting {
                    column_ordinal: 0,
                    kind: MorselColumnKind::I64,
                },
                ColumnRouting {
                    column_ordinal: 1,
                    kind: MorselColumnKind::F64,
                },
            ],
        };
        assert_eq!(plan.projection.len(), 2);
        assert_eq!(plan.projection[0].kind, MorselColumnKind::I64);
        assert_eq!(plan.projection[1].kind, MorselColumnKind::F64);
    }
}
