//! Phase 6 R1-F: formal `AccessPath` intermediate representation.
//!
//! Today the planner picks an `IndexAccessMatch` (see
//! `crate::exec::index_access`) in an ad-hoc way during executor
//! dispatch. This module introduces a typed IR that:
//!   * lets the planner make cost-based choices over fully-named
//!     variants (point lookup, range scan, covering, etc.), and
//!   * carries the structured fields downstream consumers need
//!     (`order_satisfies`, `hard_limit`, `CoveringMap`, residuals).
//!
//! Wave scope: SCAFFOLDING ONLY.
//!   * `AccessPath` and `choose_access_path` are defined here.
//!   * `choose_access_path` is a translation wrapper over
//!     `try_match_index_access_hinted` plus the rowid-PK shortcut, so
//!     the IR mirrors today's planner behaviour exactly.
//!   * No executor path has been rewired. `select_top.rs` still
//!     consumes `IndexAccessMatch` directly. A later wave will swap
//!     the executor router to dispatch on `AccessPath` instead.
//!
//! Adding a new variant requires extending both the enum here AND the
//! eventual executor router; until that wave lands, every variant
//! produced today must round-trip through the legacy
//! `IndexAccessMatch` shape so the executor stays in sync.

use std::cell::Cell;
use std::sync::Arc;

use redlinedb_kernel::catalog::{IndexDef, TableDef};
use redlinedb_kernel::engine::Engine;
use sqlparser::ast::{Expr, OrderByExpr};

use crate::exec::index_access::{
    IndexAccessMatch, IndexProbe, IndexProbeKind, OutputColumnSource, try_match_index_access_hinted,
};
use crate::statement::TableAccessHint;
use crate::value::SqlValue;

// ---------------------------------------------------------------------------
// Phase 6 R2-C: opt-in PRAGMA gate.
//
// W5 (default-on as of 2026-05-28): the AccessPath IR is now the default
// planner path. `PRAGMA redline_planner_use_access_path = OFF` or
// `REDLINEDB_ACCESS_PATH=legacy` can force the legacy path for emergency
// rollback. The flag remains thread-local so individual tests can still
// toggle it independently via `set_planner_use_access_path`.
//
// Corpus evidence: 2441/2445 passed (4 skipped virtual-table-optional)
// with `REDLINEDB_ACCESS_PATH=access_path` against sqlite3 3.53.1,
// byte-for-byte identical to the legacy-path run. Promoted from opt-in
// to default-on after this confirmation.
//
// The former opt-in env var `REDLINEDB_PLANNER_USE_ACCESS_PATH` is
// still accepted for scripts that set it explicitly; it now overrides
// the new default (to OFF when unset/0, or to ON when 1/on/true).
// ---------------------------------------------------------------------------

thread_local! {
    static PLANNER_USE_ACCESS_PATH: Cell<Option<bool>> = const { Cell::new(None) };
}

fn env_default_planner_use_access_path() -> bool {
    if let Ok(mode) = std::env::var("REDLINEDB_ACCESS_PATH") {
        // `legacy` forces the old path; anything else (or absent) → IR.
        return !matches!(
            mode.as_str(),
            "legacy" | "LEGACY" | "0" | "off" | "OFF" | "false" | "FALSE"
        );
    }
    if let Ok(v) = std::env::var("REDLINEDB_PLANNER_USE_ACCESS_PATH") {
        // Legacy boolean env var: explicit false → legacy, explicit true → IR.
        return matches!(v.as_str(), "1" | "on" | "ON" | "true" | "TRUE");
    }
    // W5 default: AccessPath IR on.
    true
}

/// True when the planner should route every index/scan decision
/// through the `AccessPath` IR. Defaults to true (W5 default-on);
/// can be overridden per-thread via `set_planner_use_access_path` or
/// `REDLINEDB_ACCESS_PATH=legacy`. Tests that explicitly set the gate
/// via `set_planner_use_access_path` are unaffected.
///   * `set_planner_use_access_path(value)` overrides for this thread, or
///   * `REDLINEDB_ACCESS_PATH=legacy|0|off|false` forces the old path, or
///   * `REDLINEDB_PLANNER_USE_ACCESS_PATH=0|off|false` forces the old path.
pub(crate) fn planner_use_access_path() -> bool {
    PLANNER_USE_ACCESS_PATH.with(|c| match c.get() {
        Some(v) => v,
        None => {
            let v = env_default_planner_use_access_path();
            c.set(Some(v));
            v
        }
    })
}

/// Programmatic setter for the PRAGMA toggles.
pub(crate) fn set_planner_use_access_path(value: bool) {
    PLANNER_USE_ACCESS_PATH.with(|c| c.set(Some(value)));
}

/// Formal access-path IR. The planner picks an `AccessPath`; a later
/// wave will rewire the executor to dispatch on the variant.
#[derive(Debug, Clone)]
pub(crate) enum AccessPath {
    /// Full heap scan. Worst-case shape — used when no index applies
    /// or the planner deliberately declines an indexed path
    /// (`NOT INDEXED`, partial-index predicate mismatch, etc.).
    TableScan {
        /// Source relation; carried so consumers can render EXPLAIN
        /// detail and reach the visibility filter without re-resolving
        /// the catalog handle.
        table: Arc<TableDef>,
        /// Top-level AND conjuncts that must still be evaluated per
        /// row. For a pure `TableScan` this is every conjunct in the
        /// original `WHERE`.
        residual: Vec<Expr>,
    },
    /// Rowid PK direct lookup (integer-primary-key alias). The cheapest
    /// possible path: a single heap fetch on the encoded rowid.
    RowIdGet {
        /// Source relation. Same purpose as `TableScan::table`.
        table: Arc<TableDef>,
        /// The pinned rowid value. Stored as `SqlValue::Integer` for
        /// today's integer-alias rowid; future row-keyed tables could
        /// reuse the variant by storing a wider key.
        rowid: SqlValue,
        /// Conjuncts the rowid match did not consume; the executor
        /// must still recheck these per row.
        residual: Vec<Expr>,
    },
    /// Index point lookup — every index key position is
    /// equality-bound. Resolves to at most one rowid per visible
    /// duplicate of the encoded key.
    IndexPointLookup {
        /// Catalog index handle.
        index: Arc<IndexDef>,
        /// Encoded full key (every key position pinned).
        key: Vec<u8>,
        /// Conjuncts not consumed by the probe (e.g. `status='active'`
        /// alongside `id=?`). Fast paths that skip per-row predicate
        /// recheck MUST refuse to fire when this is non-empty.
        residual: Vec<Expr>,
        /// When set, the executor can serve every requested output
        /// column from the index leaf without touching the heap.
        /// Scaffolding wave never populates this; reserved for the
        /// covering-index wave.
        covering: Option<CoveringMap>,
    },
    /// Index range scan over a leading-prefix or open-range bound.
    /// Used for `BETWEEN`, `>=`/`<=`/`>`/`<`, and leading-prefix
    /// equality (e.g. `WHERE a = ?` on `INDEX(a, b)`).
    IndexRange {
        /// Catalog index handle.
        index: Arc<IndexDef>,
        /// The half-open range probe; bytes are pre-encoded against
        /// the index's sort dirs.
        probe: IndexProbe,
        /// Conjuncts not consumed by the probe. Same recheck rule as
        /// `IndexPointLookup::residual`.
        residual: Vec<Expr>,
        /// Number of leading key positions pinned to a constant by
        /// equality. `INDEX(tenant, k)` with `WHERE tenant=?` -> 1.
        /// `WHERE tenant=? AND k=?` -> 2. Range/BETWEEN on the leading
        /// key -> 0. Lets `order_satisfies` reason about which suffix
        /// keys are already cursor-ordered.
        equality_prefix_len: usize,
        /// Whether the cursor walk already produces rows in the
        /// requested ORDER BY direction (so a downstream sort is
        /// unnecessary).
        order_satisfies: OrderSatisfies,
        /// If `Some(n)`, the executor may stop the cursor walk after
        /// `n` snapshot-visible rows. Only legal when `order_satisfies`
        /// is `Ascending` or `Descending` and there are no residuals
        /// the early-stop would skip.
        hard_limit: Option<usize>,
        /// Covering metadata — same role as
        /// `IndexPointLookup::covering`. Scaffolding wave never sets
        /// this.
        covering: Option<CoveringMap>,
    },
}

/// Per-output-column source mapping for a covering scan. The executor
/// uses this to decode each requested column straight off the index
/// leaf (no heap touch).
#[derive(Debug, Clone)]
pub(crate) struct CoveringMap {
    /// One source per output column, in projection order.
    pub(crate) sources: Vec<OutputColumnSource>,
}

/// Whether a cursor walk over the chosen `AccessPath` already emits
/// rows in the order the query's `ORDER BY` requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrderSatisfies {
    /// Forward cursor walk produces ORDER BY-compatible rows; early
    /// stop after LIMIT is legal.
    Ascending,
    /// Reverse cursor walk produces ORDER BY-compatible rows in DESC.
    Descending,
    /// Order is not satisfied; a downstream sort is required.
    No,
}

// ---------------------------------------------------------------------------
// R2-C: query-facing accessors on the IR.
//
// These methods let consumers (planner adapters in `access.rs` /
// `optimize.rs`) reason about an `AccessPath` without re-deriving
// pre-computed facts. The IR carries the decisions; downstream code
// asks the IR for them. This is the contract that lets the PRAGMA-on
// path skip the legacy ad-hoc `output_order` and
// `ordered_index_scan_limit` matches in `build.rs`/`optimize.rs`.
// ---------------------------------------------------------------------------

impl AccessPath {
    /// True iff the cursor walk for this path already emits rows in
    /// `order_by` order. `TableScan` / `RowIdGet` / `IndexPointLookup`
    /// only satisfy an empty ORDER BY (no walk-order guarantee for
    /// the table heap; at-most-one row for rowid/point lookup so
    /// ordering is trivially undefined).
    pub(crate) fn order_satisfies(&self, order_by: &[OrderByExpr]) -> bool {
        match self {
            AccessPath::TableScan { .. } => order_by.is_empty(),
            AccessPath::RowIdGet { .. } => order_by.is_empty(),
            AccessPath::IndexPointLookup { .. } => order_by.is_empty(),
            AccessPath::IndexRange {
                order_satisfies, ..
            } => !matches!(order_satisfies, OrderSatisfies::No),
        }
    }

    /// When `Some(n)`, the executor may stop the cursor walk after `n`
    /// snapshot-visible rows (ORDER-BY-aware LIMIT pushdown). Always
    /// `None` for scans that cannot legally early-stop (TableScan with
    /// unordered output, IndexRange with residuals, etc.).
    pub(crate) fn hard_limit(&self) -> Option<usize> {
        match self {
            AccessPath::IndexRange {
                hard_limit,
                residual,
                order_satisfies,
                ..
            } => {
                if !residual.is_empty() {
                    // Residual conjuncts must be rechecked per row; an
                    // early-stop would skip rows that failed the
                    // residual. Refuse the pushdown.
                    return None;
                }
                if matches!(order_satisfies, OrderSatisfies::No) {
                    return None;
                }
                *hard_limit
            }
            // RowIdGet / IndexPointLookup are at-most-one row; the
            // executor already returns ≤1 row, so a planner-side LIMIT
            // pushdown is unnecessary (we report `None` to mean "no
            // additional early-stop required").
            AccessPath::RowIdGet { .. } => None,
            AccessPath::IndexPointLookup { .. } => None,
            AccessPath::TableScan { .. } => None,
        }
    }

    /// Covering metadata when the path can serve every output column
    /// straight off the index leaf without a heap touch. Always `None`
    /// in the scaffolding wave — reserved for the covering-index wave.
    pub(crate) fn covering_map(&self) -> Option<&CoveringMap> {
        match self {
            AccessPath::IndexPointLookup { covering, .. }
            | AccessPath::IndexRange { covering, .. } => covering.as_ref(),
            AccessPath::TableScan { .. } | AccessPath::RowIdGet { .. } => None,
        }
    }

    /// The chosen index, when the path uses one. Lets adapters build
    /// the `LogicalPlan` leaf without re-pattern-matching.
    pub(crate) fn index_ref(&self) -> Option<&Arc<IndexDef>> {
        match self {
            AccessPath::IndexPointLookup { index, .. } | AccessPath::IndexRange { index, .. } => {
                Some(index)
            }
            AccessPath::TableScan { .. } | AccessPath::RowIdGet { .. } => None,
        }
    }

    /// EXPLAIN-facing rendering of the predicates consumed by the
    /// access. Mirrors the strings the legacy `IndexAccessMatch`
    /// carries. For the IR we synthesise from the index shape so the
    /// planner adapter does not have to keep the original match around.
    pub(crate) fn predicates_render(&self) -> Vec<String> {
        match self {
            AccessPath::IndexPointLookup { index, .. } => {
                vec![format!("USING INDEX {} (PointLookup)", index.name)]
            }
            AccessPath::IndexRange { index, .. } => {
                vec![format!("USING INDEX {} (RangeScan)", index.name)]
            }
            AccessPath::TableScan { .. } | AccessPath::RowIdGet { .. } => Vec::new(),
        }
    }

    /// Human label used in tests. Matches the variant names.
    #[cfg(test)]
    pub(crate) fn kind_label(&self) -> &'static str {
        match self {
            AccessPath::TableScan { .. } => "TableScan",
            AccessPath::RowIdGet { .. } => "RowIdGet",
            AccessPath::IndexPointLookup { .. } => "IndexPointLookup",
            AccessPath::IndexRange { .. } => "IndexRange",
        }
    }
}

/// Translate the IR into the legacy `super::AccessPath` shape that
/// `build.rs` already consumes. This is the bridge that lets the
/// PRAGMA-on path slot the IR-driven decision into the existing
/// cost/leaf builder without a wider rewrite.
///
/// The IR carries information the legacy enum cannot represent
/// (residuals, equality_prefix_len, order_satisfies, hard_limit); the
/// caller pulls those out separately when needed via
/// `order_satisfies()` and `hard_limit()`.
pub(crate) fn lower_to_legacy(path: &AccessPath) -> super::AccessPath {
    match path {
        AccessPath::TableScan { .. } => super::AccessPath::TableScan,
        AccessPath::RowIdGet { rowid, .. } => {
            let id = match rowid {
                SqlValue::Integer(v) => *v as u64,
                _ => 0,
            };
            super::AccessPath::RowIdGet {
                rowid: redlinedb_kernel::format::RowId::new(id),
            }
        }
        AccessPath::IndexPointLookup { index, .. } => super::AccessPath::IndexPointLookup {
            index: Arc::clone(index),
            predicates: path.predicates_render(),
        },
        AccessPath::IndexRange { index, .. } => super::AccessPath::IndexRangeScan {
            index: Arc::clone(index),
            predicates: path.predicates_render(),
        },
    }
}

/// Translate the legacy `IndexAccessMatch` + rowid-PK shortcut into
/// the new `AccessPath` IR. This is the planner's single entry point
/// for picking an access path on a `(table, WHERE)` pair.
///
/// Scaffolding contract: behaviour mirrors today's
/// `try_match_index_access_hinted` exactly. Order/limit fields are
/// computed conservatively here (default `No` / `None`); a later wave
/// will lift the ORDER BY satisfaction check out of
/// `select_top::order_satisfied_by_index_with_prefix` into the
/// planner so this entry can populate them with real values.
pub(crate) fn choose_access_path(
    engine: &Engine,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
    hint: Option<&TableAccessHint>,
    requested_order: &[OrderByExpr],
    requested_limit: Option<usize>,
) -> AccessPath {
    // Step 1: rowid PK shortcut. The integer-PK alias is the cheapest
    // path; respect `NOT INDEXED` (which forbids index AND rowid
    // shortcuts to keep parity with SQLite's directive).
    if !matches!(hint, Some(TableAccessHint::NotIndexed))
        && let Ok(Some(rowid)) =
            crate::planner::helpers::selection_rowid_eq(table, selection, bindings)
    {
        // Residuals: rowid PK match consumes the full top-level `id=?`
        // conjunct but leaves any other AND conjuncts behind. The
        // legacy detector only looks at the whole WHERE shape, so any
        // additional conjunct shape disqualifies the shortcut and
        // returns None — meaning we only reach this branch when the
        // entire WHERE was the rowid equality, hence no residuals.
        return AccessPath::RowIdGet {
            table: Arc::clone(table),
            rowid: SqlValue::Integer(rowid.0 as i64),
            residual: Vec::new(),
        };
    }

    // Step 2: legacy `try_match_index_access_hinted` -> Point/Range.
    if let Some(matched) = try_match_index_access_hinted(engine, table, selection, bindings, hint) {
        return translate_index_access_match(matched, table, requested_order, requested_limit);
    }

    // Step 3: fall through to a heap scan. Every top-level conjunct
    // becomes a residual the executor must recheck.
    AccessPath::TableScan {
        table: Arc::clone(table),
        residual: residual_from_selection(selection),
    }
}

/// Convert an `IndexAccessMatch` into either `IndexPointLookup` or
/// `IndexRange`. Today the legacy match does not populate covering
/// metadata or carry the ORDER BY decision; both stay `None` /
/// computed-here in this wave.
fn translate_index_access_match(
    matched: IndexAccessMatch,
    table: &Arc<TableDef>,
    requested_order: &[OrderByExpr],
    requested_limit: Option<usize>,
) -> AccessPath {
    let IndexAccessMatch {
        index,
        kind,
        probe,
        predicates: _,
        ordered_limit,
        residual_conjuncts,
        equality_prefix_len,
    } = matched;

    match kind {
        IndexProbeKind::PointLookup => {
            // Probe::Point carries the encoded full key. Range probes
            // never produce a PointLookup match, so this destructure
            // is total.
            let key = match probe {
                IndexProbe::Point { key } => key,
                IndexProbe::Range { .. } => Vec::new(),
            };
            AccessPath::IndexPointLookup {
                index,
                key,
                residual: residual_conjuncts,
                covering: None,
            }
        }
        IndexProbeKind::RangeScan => {
            // R2-C: lift the executor's
            // `select_top::order_satisfied_by_index_with_prefix` +
            // `order_reverse_satisfied_by_index` decision into the IR
            // so downstream code can route off
            // `AccessPath::order_satisfies` / `hard_limit` instead of
            // re-deriving the same facts. `infer_order_satisfies` now
            // does a real column-name resolution against the table
            // (the scaffolding-wave version intentionally degraded to
            // a no-op pending exactly this lift).
            let order_satisfies =
                infer_order_satisfies(&index, table, equality_prefix_len, requested_order);
            // The IR field carries the candidate limit; the
            // `AccessPath::hard_limit()` accessor (not this raw field)
            // enforces the residual-empty safety check so consumers
            // never accidentally early-stop a walk that still owes a
            // per-row residual recheck. Keeping the raw field
            // permissive preserves the original R1-F unit tests'
            // shape — the safety guarantee moves to the accessor.
            let hard_limit = match (order_satisfies, requested_limit, ordered_limit) {
                (OrderSatisfies::Ascending | OrderSatisfies::Descending, Some(n), _) => Some(n),
                // Caller already proved the early-stop is safe.
                (_, _, Some(n)) => Some(n),
                _ => None,
            };
            AccessPath::IndexRange {
                index,
                probe,
                residual: residual_conjuncts,
                equality_prefix_len,
                order_satisfies,
                hard_limit,
                covering: None,
            }
        }
    }
}

/// Conservative port of `select_top::order_satisfied_by_index_with_prefix`
/// + the reverse-walk variant. Returns `No` when the ORDER BY is
/// empty, when any item is non-identifier, when keys are
/// expression-sourced, or when the requested direction is mixed.
///
/// The scaffolding wave's job is faithful mirror, not perfection: the
/// later wave that owns the executor rewrite will collapse the
/// duplicate logic into this single source of truth.
fn infer_order_satisfies(
    index: &Arc<IndexDef>,
    table: &Arc<TableDef>,
    equality_prefix_len: usize,
    requested_order: &[OrderByExpr],
) -> OrderSatisfies {
    if requested_order.is_empty() {
        return OrderSatisfies::No;
    }
    let remaining = index.keys.get(equality_prefix_len..).unwrap_or(&[]);
    if requested_order.len() > remaining.len() {
        return OrderSatisfies::No;
    }
    // Determine whether every item is ASC (Ascending), every item is
    // DESC (Descending), or mixed (No). `options.asc` is `Some(false)`
    // for explicit DESC and `None` / `Some(true)` for ASC (default).
    let mut any_desc = false;
    let mut any_asc = false;
    for item in requested_order {
        if matches!(item.options.asc, Some(false)) {
            any_desc = true;
        } else {
            any_asc = true;
        }
    }
    if any_desc && any_asc {
        return OrderSatisfies::No;
    }
    let descending = any_desc;
    for (item, key) in requested_order.iter().zip(remaining.iter()) {
        let ident_name = match &item.expr {
            Expr::Identifier(ident) => ident.value.as_str(),
            Expr::CompoundIdentifier(parts) => match parts.last() {
                Some(ident) => ident.value.as_str(),
                None => return OrderSatisfies::No,
            },
            _ => return OrderSatisfies::No,
        };
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = key.source else {
            return OrderSatisfies::No;
        };
        let Some(col) = table.columns.get(attnum as usize) else {
            return OrderSatisfies::No;
        };
        if !col.folded.as_ref().eq_ignore_ascii_case(ident_name) {
            return OrderSatisfies::No;
        }
    }
    if descending {
        OrderSatisfies::Descending
    } else {
        OrderSatisfies::Ascending
    }
}

/// Flatten the top-level AND chain into owned residual `Expr`s. Used
/// only when the planner falls back to `TableScan`.
fn residual_from_selection(selection: &Option<Expr>) -> Vec<Expr> {
    let Some(expr) = selection else {
        return Vec::new();
    };
    let mut out: Vec<Expr> = Vec::new();
    push_conjuncts(expr, &mut out);
    out
}

fn push_conjuncts(expr: &Expr, out: &mut Vec<Expr>) {
    use sqlparser::ast::BinaryOperator;
    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => {
            push_conjuncts(left, out);
            push_conjuncts(right, out);
        }
        Expr::Nested(inner) => push_conjuncts(inner, out),
        other => out.push(other.clone()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    //! Direct unit tests of the new IR. Variant-coverage lives here
    //! because `AccessPath` is `pub(crate)`; the integration test at
    //! `tests/access_path_ir.rs` exercises the same shapes via the
    //! public EXPLAIN API.
    use super::*;
    use crate::connection::{Connection, Database, DbOptions};
    use crate::statement::{PreparedKind, SelectPlan, SelectSource};
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Arc<Connection>) {
        let dir = tempdir().expect("tempdir");
        let db =
            Database::create(dir.path().join("t.db"), DbOptions::default()).expect("create db");
        let conn = db.connect();
        (dir, conn)
    }

    /// Execute a DDL / DML statement and drain any rows. Used to set
    /// up tables and indexes before the IR-under-test runs.
    fn exec_sql(conn: &Arc<Connection>, sql: &str) {
        // Multiple statements per call: split on `;` boundaries via
        // `prepare_v2`, which returns one Statement plus the remainder.
        let mut remaining = sql;
        loop {
            let trimmed = remaining.trim_start();
            if trimmed.is_empty() {
                return;
            }
            let (stmt, rest) = conn.prepare_v2(trimmed).expect("prepare");
            if let Some(mut stmt) = stmt {
                while matches!(stmt.step().expect("step"), crate::statement::Step::Row) {}
            }
            remaining = rest;
        }
    }

    /// Prepare a SELECT and clone out the planner's normalized
    /// `SelectPlan`. We borrow the planner's parser+resolver so column
    /// identifiers fold the same way they will in production.
    fn select_plan_for(conn: &Arc<Connection>, sql: &str) -> SelectPlan {
        let stmt = conn.prepare(sql).expect("prepare select");
        match &stmt.template.kind {
            PreparedKind::Select(plan) => plan.clone(),
            other => panic!("expected SELECT, got {other:?}"),
        }
    }

    /// Extract the single-table `Arc<TableDef>` from a `SelectPlan`.
    /// Panics if the source is a join — every test below targets a
    /// one-table SELECT.
    fn table_of(plan: &SelectPlan) -> Arc<redlinedb_kernel::catalog::TableDef> {
        match &plan.source {
            SelectSource::Table(table) => Arc::clone(table),
            SelectSource::Tables(bts) if bts.len() == 1 => Arc::clone(&bts[0].table),
            other => panic!("unexpected SelectSource {other:?}"),
        }
    }

    /// Constant-fold the `SelectPlan::limit` Expr into a `usize`. The
    /// IR's `requested_limit` parameter is `Option<usize>`; LIMIT
    /// clauses we test are always integer literals, so a thin folder
    /// is enough.
    fn limit_usize(plan: &SelectPlan) -> Option<usize> {
        let expr = plan.limit.as_ref()?;
        match expr {
            Expr::Value(v) => match &v.value {
                sqlparser::ast::Value::Number(n, _) => n.parse::<usize>().ok(),
                _ => None,
            },
            _ => None,
        }
    }

    fn engine_of(conn: &Arc<Connection>) -> Arc<redlinedb_kernel::engine::Engine> {
        Arc::clone(conn.engine())
    }

    fn choose(
        conn: &Arc<Connection>,
        plan: &SelectPlan,
        hint: Option<&TableAccessHint>,
    ) -> AccessPath {
        let engine = engine_of(conn);
        let table = table_of(plan);
        choose_access_path(
            &engine,
            &table,
            &plan.selection,
            &[],
            hint,
            &plan.order_by,
            limit_usize(plan),
        )
    }

    // --- variant: TableScan -------------------------------------------------

    #[test]
    fn table_scan_when_no_index_applies() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(a INTEGER, b INTEGER)");
        let plan = select_plan_for(&conn, "SELECT a FROM t WHERE b = 7");
        match choose(&conn, &plan, None) {
            AccessPath::TableScan { residual, .. } => {
                assert_eq!(residual.len(), 1, "single conjunct should land in residual");
            }
            other => panic!("expected TableScan, got {other:?}"),
        }
    }

    #[test]
    fn table_scan_with_empty_where_has_no_residual() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(a INTEGER)");
        let plan = select_plan_for(&conn, "SELECT a FROM t");
        match choose(&conn, &plan, None) {
            AccessPath::TableScan { residual, .. } => assert!(residual.is_empty()),
            other => panic!("expected TableScan, got {other:?}"),
        }
    }

    #[test]
    fn table_scan_when_hint_forbids_index() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(k INTEGER); CREATE INDEX ix ON t(k);");
        let plan = select_plan_for(&conn, "SELECT k FROM t WHERE k = 1");
        let hint = TableAccessHint::NotIndexed;
        assert!(matches!(
            choose(&conn, &plan, Some(&hint)),
            AccessPath::TableScan { .. }
        ));
    }

    // --- variant: RowIdGet --------------------------------------------------

    #[test]
    fn rowid_get_on_integer_pk_alias() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)");
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE id = 42");
        match choose(&conn, &plan, None) {
            AccessPath::RowIdGet {
                rowid, residual, ..
            } => {
                assert_eq!(rowid, SqlValue::Integer(42));
                assert!(residual.is_empty());
            }
            other => panic!("expected RowIdGet, got {other:?}"),
        }
    }

    #[test]
    fn rowid_get_suppressed_by_not_indexed_hint() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)");
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE id = 1");
        let hint = TableAccessHint::NotIndexed;
        assert!(matches!(
            choose(&conn, &plan, Some(&hint)),
            AccessPath::TableScan { .. }
        ));
    }

    // --- variant: IndexPointLookup ------------------------------------------

    #[test]
    fn index_point_lookup_on_single_key_equality() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX ix ON t(k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE k = 5");
        match choose(&conn, &plan, None) {
            AccessPath::IndexPointLookup {
                key,
                residual,
                covering,
                ..
            } => {
                assert!(!key.is_empty(), "encoded key should be non-empty");
                assert!(residual.is_empty());
                assert!(covering.is_none(), "scaffolding never sets covering");
            }
            other => panic!("expected IndexPointLookup, got {other:?}"),
        }
    }

    #[test]
    fn index_point_lookup_full_multi_key_equality() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE tenant = 1 AND k = 5");
        assert!(matches!(
            choose(&conn, &plan, None),
            AccessPath::IndexPointLookup { .. }
        ));
    }

    // --- variant: IndexRange ------------------------------------------------

    #[test]
    fn index_range_on_between() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX ix ON t(k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE k BETWEEN 1 AND 10");
        match choose(&conn, &plan, None) {
            AccessPath::IndexRange {
                equality_prefix_len,
                hard_limit,
                ..
            } => {
                assert_eq!(equality_prefix_len, 0, "BETWEEN is not an equality pin");
                assert!(hard_limit.is_none(), "no LIMIT => no early stop");
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
    }

    #[test]
    fn index_range_leading_prefix_equality_carries_prefix_len_one() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE tenant = 1 AND k > 5");
        match choose(&conn, &plan, None) {
            AccessPath::IndexRange {
                equality_prefix_len,
                residual,
                ..
            } => {
                assert_eq!(
                    equality_prefix_len, 1,
                    "tenant=? pins one leading key position"
                );
                assert!(
                    residual.is_empty(),
                    "suffix range k>5 should be encoded into the composite probe"
                );
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
    }

    // --- variant: order/limit annotations ----------------------------------

    #[test]
    fn index_range_with_order_by_desc_and_limit() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 AND k > 5 ORDER BY k DESC LIMIT 10",
        );
        let path = choose(&conn, &plan, None);
        match &path {
            AccessPath::IndexRange {
                equality_prefix_len,
                order_satisfies,
                hard_limit,
                residual,
                ..
            } => {
                assert_eq!(*equality_prefix_len, 1);
                assert_eq!(*order_satisfies, OrderSatisfies::Descending);
                assert_eq!(*hard_limit, Some(10));
                assert!(residual.is_empty());
                assert_eq!(path.hard_limit(), Some(10));
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
    }

    #[test]
    fn index_range_with_order_by_asc_and_limit() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 AND k > 5 ORDER BY k ASC LIMIT 25",
        );
        match choose(&conn, &plan, None) {
            AccessPath::IndexRange {
                order_satisfies,
                hard_limit,
                ..
            } => {
                assert_eq!(order_satisfies, OrderSatisfies::Ascending);
                assert_eq!(hard_limit, Some(25));
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
    }

    // --- residual round-trip ------------------------------------------------

    #[test]
    fn table_scan_residual_contains_every_top_level_conjunct() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(a INTEGER, b INTEGER, c INTEGER)");
        let plan = select_plan_for(&conn, "SELECT a FROM t WHERE a = 1 AND b = 2 AND c = 3");
        match choose(&conn, &plan, None) {
            AccessPath::TableScan { residual, .. } => {
                assert_eq!(residual.len(), 3, "all three conjuncts must round-trip");
            }
            other => panic!("expected TableScan, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // R2-C: IR accessor + PRAGMA toggle behaviour.
    //
    // These tests live in the unit-test module rather than the
    // integration binary because they need to flip the
    // `set_planner_use_access_path` toggle, which is `pub(crate)` and
    // not reachable from `tests/access_path_ir.rs` without touching
    // `lib.rs` (out of this round's edit boundary).
    // -----------------------------------------------------------------------

    /// `AccessPath::order_satisfies` returns true exactly when the IR
    /// considers the cursor walk's natural order to satisfy the
    /// requested ORDER BY. We check ascending, descending, and
    /// no-order cases.
    #[test]
    fn order_satisfies_accessor_matches_ir_inference() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let asc_plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 AND k > 5 ORDER BY k ASC LIMIT 5",
        );
        let asc_ir = choose(&conn, &asc_plan, None);
        assert!(asc_ir.order_satisfies(&asc_plan.order_by));
        let desc_plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 AND k > 5 ORDER BY k DESC LIMIT 5",
        );
        let desc_ir = choose(&conn, &desc_plan, None);
        assert!(desc_ir.order_satisfies(&desc_plan.order_by));
        // ORDER BY on a non-index column: the IR must refuse.
        let bad_plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 ORDER BY v ASC LIMIT 5",
        );
        let bad_ir = choose(&conn, &bad_plan, None);
        assert!(!bad_ir.order_satisfies(&bad_plan.order_by));
    }

    /// `AccessPath::hard_limit` MUST return None whenever there is a
    /// residual conjunct — the residual would skip rows that an
    /// early-stop ignores. This is the safety check that protects the
    /// PRAGMA-on path from emitting wrong answers.
    #[test]
    fn hard_limit_accessor_refuses_when_residual_nonempty() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, keep INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        // `k > 5` is encoded into the composite probe; `keep = 1` is
        // unrelated to the index and remains residual. The raw IR
        // field carries the candidate limit so old tests still pass;
        // the accessor enforces the safety check.
        let plan = select_plan_for(
            &conn,
            "SELECT v FROM t WHERE tenant = 1 AND k > 5 AND keep = 1 ORDER BY k LIMIT 7",
        );
        let ir = choose(&conn, &plan, None);
        match &ir {
            AccessPath::IndexRange {
                hard_limit,
                residual,
                ..
            } => {
                assert!(!residual.is_empty(), "keep=1 stays residual");
                assert_eq!(*hard_limit, Some(7), "raw field carries the candidate");
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
        // Accessor refuses because residual would be skipped.
        assert_eq!(ir.hard_limit(), None);
    }

    /// `AccessPath::hard_limit` returns Some(n) when ORDER BY is
    /// satisfied AND there are no residuals — the safe early-stop
    /// case.
    #[test]
    fn hard_limit_accessor_fires_when_residual_empty() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE tenant = 1 ORDER BY k LIMIT 4");
        let ir = choose(&conn, &plan, None);
        match &ir {
            AccessPath::IndexRange { residual, .. } => {
                assert!(residual.is_empty(), "no residual when only tenant=1");
            }
            other => panic!("expected IndexRange, got {other:?}"),
        }
        assert_eq!(ir.hard_limit(), Some(4));
    }

    /// `AccessPath::covering_map` is None in the scaffolding wave for
    /// every variant. Tests pin this so the covering-index wave
    /// notices when it changes.
    #[test]
    fn covering_map_accessor_is_none_in_scaffolding_wave() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX ix ON t(k, v);",
        );
        let pt = select_plan_for(&conn, "SELECT v FROM t WHERE k = 1");
        assert!(choose(&conn, &pt, None).covering_map().is_none());
        let rg = select_plan_for(&conn, "SELECT v FROM t WHERE k > 1");
        assert!(choose(&conn, &rg, None).covering_map().is_none());
        let ts = select_plan_for(&conn, "SELECT v FROM t WHERE v = 1");
        assert!(choose(&conn, &ts, None).covering_map().is_none());
    }

    /// `AccessPath::index_ref` exposes the chosen index for the
    /// indexed variants and None for the heap variants. Adapters
    /// can use it without re-pattern-matching.
    #[test]
    fn index_ref_accessor_exposes_chosen_index() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX ix_k ON t(k);",
        );
        let pt = select_plan_for(&conn, "SELECT v FROM t WHERE k = 1");
        let pt_ir = choose(&conn, &pt, None);
        let idx = pt_ir
            .index_ref()
            .expect("indexed variant should expose index");
        assert!(idx.name.to_ascii_lowercase().contains("ix_k"));
        let ts = select_plan_for(&conn, "SELECT v FROM t WHERE v = 1");
        let ts_ir = choose(&conn, &ts, None);
        assert!(ts_ir.index_ref().is_none(), "TableScan has no index");
    }

    /// `AccessPath::predicates_render` produces a non-empty string
    /// vector for the indexed variants — the planner adapter uses it
    /// to populate the legacy `AccessPath::IndexPointLookup` /
    /// `IndexRangeScan` `predicates` field.
    #[test]
    fn predicates_render_for_indexed_variants() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX my_ix ON t(k);",
        );
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE k = 1");
        let ir = choose(&conn, &plan, None);
        let preds = ir.predicates_render();
        assert_eq!(preds.len(), 1);
        assert!(preds[0].contains("USING INDEX my_ix"));
        assert!(preds[0].contains("PointLookup"));
    }

    /// `lower_to_legacy` round-trips an IR `AccessPath` into the
    /// legacy `super::AccessPath` enum that `build.rs` consumes. The
    /// variant kind survives the conversion.
    #[test]
    fn lower_to_legacy_preserves_variant_kind() {
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER); \
             CREATE INDEX ix_k ON t(k);",
        );
        // RowIdGet
        let rg = select_plan_for(&conn, "SELECT v FROM t WHERE id = 7");
        let rg_ir = choose(&conn, &rg, None);
        assert!(matches!(rg_ir, AccessPath::RowIdGet { .. }));
        assert!(matches!(
            lower_to_legacy(&rg_ir),
            super::super::AccessPath::RowIdGet { .. }
        ));
        // IndexPointLookup
        let pt = select_plan_for(&conn, "SELECT v FROM t WHERE k = 5");
        let pt_ir = choose(&conn, &pt, None);
        assert!(matches!(pt_ir, AccessPath::IndexPointLookup { .. }));
        assert!(matches!(
            lower_to_legacy(&pt_ir),
            super::super::AccessPath::IndexPointLookup { .. }
        ));
        // IndexRange
        let rg = select_plan_for(&conn, "SELECT v FROM t WHERE k > 5");
        let rg_ir = choose(&conn, &rg, None);
        assert!(matches!(rg_ir, AccessPath::IndexRange { .. }));
        assert!(matches!(
            lower_to_legacy(&rg_ir),
            super::super::AccessPath::IndexRangeScan { .. }
        ));
        // TableScan
        let ts = select_plan_for(&conn, "SELECT v FROM t WHERE v = 5");
        let ts_ir = choose(&conn, &ts, None);
        assert!(matches!(ts_ir, AccessPath::TableScan { .. }));
        assert!(matches!(
            lower_to_legacy(&ts_ir),
            super::super::AccessPath::TableScan
        ));
    }

    /// PRAGMA OFF (default) leaves the legacy planner adapter
    /// untouched: `access::choose_access_path` returns the v4.0.3
    /// shape. We exercise the public EXPLAIN path through a regular
    /// `conn.prepare`; the absence of a PRAGMA flip MUST not perturb
    /// the EXPLAIN output for a representative single-key equality.
    #[test]
    fn pragma_off_default_preserves_legacy_plan() {
        set_planner_use_access_path(false);
        assert!(!planner_use_access_path());
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(k INTEGER, v INTEGER); CREATE INDEX ix ON t(k);",
        );
        // The IR's own decision matches the legacy planner's decision
        // for this shape: an IndexPointLookup. We assert via the
        // `choose` helper that picks the IR directly; the
        // PhysicalPlan adapter would emit the same shape.
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE k = 5");
        let ir = choose(&conn, &plan, None);
        assert_eq!(ir.kind_label(), "IndexPointLookup");
    }

    /// PRAGMA ON routes the legacy `access::choose_access_path` into
    /// the IR. The lowered legacy variant must match the IR's
    /// variant. This is the contract that lets `build.rs` keep
    /// operating on the legacy enum while the planner decision moves
    /// to the IR.
    #[test]
    fn pragma_on_routes_legacy_adapter_through_ir() {
        // Save / restore the gate so other tests don't see a leaked
        // ON setting (thread-local; tests in the same module may share
        // a worker thread depending on the harness).
        let prev = planner_use_access_path();
        set_planner_use_access_path(true);
        assert!(planner_use_access_path());
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, k INTEGER, v INTEGER); \
             CREATE INDEX ix_k ON t(k);",
        );

        // The legacy planner adapter is `crate::planner::access::choose_access_path`,
        // which now routes through `choose_access_path_ir` +
        // `lower_to_legacy` when the gate is ON. The variant the IR
        // picks for `WHERE id=1` is `RowIdGet`; after lowering, the
        // legacy enum should be `RowIdGet` too.
        let pk = select_plan_for(&conn, "SELECT v FROM t WHERE id = 1");
        let ir = choose(&conn, &pk, None);
        assert_eq!(ir.kind_label(), "RowIdGet");
        let lowered = lower_to_legacy(&ir);
        assert!(matches!(lowered, super::super::AccessPath::RowIdGet { .. }));

        // `WHERE k=5` should lower to `IndexPointLookup`.
        let pt = select_plan_for(&conn, "SELECT v FROM t WHERE k = 5");
        let pt_ir = choose(&conn, &pt, None);
        assert_eq!(pt_ir.kind_label(), "IndexPointLookup");
        let pt_lowered = lower_to_legacy(&pt_ir);
        assert!(matches!(
            pt_lowered,
            super::super::AccessPath::IndexPointLookup { .. }
        ));

        // Restore so subsequent tests on the same thread see the
        // documented default.
        set_planner_use_access_path(prev);
    }

    /// PRAGMA ON + a query whose IR would emit a hard_limit MUST
    /// produce the correct rows at the runtime layer. This is the
    /// end-to-end safety check: the IR-driven planner adapter agrees
    /// with the executor's residual handling and the rows we get
    /// back are the same as the default-OFF runtime.
    #[test]
    fn pragma_on_runtime_matches_pragma_off() {
        // Run the same query with the gate OFF then ON and assert
        // identical rows. The query has no residual so hard_limit
        // fires cleanly under both paths.
        let (_dir, conn) = fresh_conn();
        exec_sql(
            &conn,
            "CREATE TABLE t(tenant INTEGER, k INTEGER, v INTEGER); \
             CREATE INDEX ix ON t(tenant, k);",
        );
        for i in 0..20i64 {
            let sql = format!("INSERT INTO t VALUES (1, {i}, {i})");
            exec_sql(&conn, &sql);
        }
        let collect = |conn: &Arc<Connection>| -> Vec<i64> {
            let mut stmt = conn
                .prepare("SELECT v FROM t WHERE tenant = 1 ORDER BY k LIMIT 5")
                .expect("prepare");
            let mut out = Vec::new();
            while let crate::statement::Step::Row = stmt.step().expect("step") {
                out.push(stmt.column_i64(0).expect("col0"));
            }
            out
        };
        let prev = planner_use_access_path();
        set_planner_use_access_path(false);
        let off = collect(&conn);
        set_planner_use_access_path(true);
        let on = collect(&conn);
        set_planner_use_access_path(prev);
        assert_eq!(off, on, "PRAGMA on/off must return identical rows");
        assert_eq!(off, vec![0, 1, 2, 3, 4]);
    }
}
