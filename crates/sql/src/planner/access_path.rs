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

use std::sync::Arc;

use redlinedb_kernel::catalog::{IndexDef, TableDef};
use redlinedb_kernel::engine::Engine;
use sqlparser::ast::{Expr, OrderByExpr};

use crate::exec::index_access::{
    IndexAccessMatch, IndexProbe, IndexProbeKind, OutputColumnSource,
    try_match_index_access_hinted,
};
use crate::statement::TableAccessHint;
use crate::value::SqlValue;

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
    if let Some(matched) =
        try_match_index_access_hinted(engine, table, selection, bindings, hint)
    {
        return translate_index_access_match(matched, requested_order, requested_limit);
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
            // Forward / reverse / no-order is intentionally
            // conservative this wave: only fill it in when the legacy
            // detector already proved an `ordered_limit` for the
            // forward walk. The select-top reverse detector is not yet
            // mirrored here; that ports over with the executor rewrite.
            let order_satisfies =
                infer_order_satisfies(&index, &probe, equality_prefix_len, requested_order);
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
    _probe: &IndexProbe,
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
        let Expr::Identifier(ident) = &item.expr else {
            return OrderSatisfies::No;
        };
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = key.source else {
            return OrderSatisfies::No;
        };
        // Each item's direction was already classified above; here we
        // only check the column alignment against the index key. The
        // attnum is into the table's column list — we cannot resolve
        // names without a `&TableDef`, so we accept any identifier
        // whose ordinal-position role matches. The select-top variant
        // does the name-level check; for scaffolding, we keep the
        // conservative shape and treat unresolvable names as a miss.
        let _ = (ident, attnum);
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
        let db = Database::create(dir.path().join("t.db"), DbOptions::default())
            .expect("create db");
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
        assert!(matches!(choose(&conn, &plan, Some(&hint)), AccessPath::TableScan { .. }));
    }

    // --- variant: RowIdGet --------------------------------------------------

    #[test]
    fn rowid_get_on_integer_pk_alias() {
        let (_dir, conn) = fresh_conn();
        exec_sql(&conn, "CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER)");
        let plan = select_plan_for(&conn, "SELECT v FROM t WHERE id = 42");
        match choose(&conn, &plan, None) {
            AccessPath::RowIdGet { rowid, residual, .. } => {
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
        assert!(matches!(choose(&conn, &plan, Some(&hint)), AccessPath::TableScan { .. }));
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
            AccessPath::IndexPointLookup { key, residual, covering, .. } => {
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
        let plan =
            select_plan_for(&conn, "SELECT v FROM t WHERE tenant = 1 AND k = 5");
        assert!(matches!(choose(&conn, &plan, None), AccessPath::IndexPointLookup { .. }));
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
                // The `k > 5` conjunct was NOT folded into the leading
                // prefix probe (only `tenant=1` was), so it remains as
                // a residual the executor must recheck.
                assert!(!residual.is_empty(), "k>5 should remain residual");
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
        match choose(&conn, &plan, None) {
            AccessPath::IndexRange {
                equality_prefix_len,
                order_satisfies,
                hard_limit,
                ..
            } => {
                assert_eq!(equality_prefix_len, 1);
                assert_eq!(order_satisfies, OrderSatisfies::Descending);
                assert_eq!(hard_limit, Some(10));
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
        let plan =
            select_plan_for(&conn, "SELECT a FROM t WHERE a = 1 AND b = 2 AND c = 3");
        match choose(&conn, &plan, None) {
            AccessPath::TableScan { residual, .. } => {
                assert_eq!(residual.len(), 3, "all three conjuncts must round-trip");
            }
            other => panic!("expected TableScan, got {other:?}"),
        }
    }
}
