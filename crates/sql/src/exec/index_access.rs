// Lane C physical-index probes (SELECT-time).
//
// This module owns the SQL-side bridge from a SELECT's `WHERE` predicate
// onto the kernel's physical B-tree indexes. Lane A built the index
// lifecycle and Lane B made the SQL DML path keep the indexes in sync;
// here we use those handles to satisfy reads without scanning the heap.
//
// Scope (Wave 4):
// - Single-key equality on a 1-key index   -> point lookup
// - Leading-key equality on an N-key index -> prefix range scan
// - Leading-key open/closed range          -> bounded range scan
// - Anything else                           -> caller falls back to a
//                                              heap scan (planner stays
//                                              conservative).
//
// We intentionally keep covering-index optimizations and multi-index
// AND/OR off this round; the planner's `AccessPath` enum already names
// those variants for future waves but the planner does not advertise
// them yet.

use std::sync::Arc;

use redlinedb_kernel::catalog::{
    EncodedIndexKey, IndexDef, IndexKeySource, SortDir, TableDef, encode_index_key,
};
use redlinedb_kernel::engine::{Engine, Txn};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::index::{CursorYield, IndexRowRef, RawPointCursor, SnapshotView};
use sqlparser::ast::{BinaryOperator, Expr, Value};

use crate::error::Result;
use crate::statement::TableAccessHint;
use crate::value::SqlValue;

use super::index_batch::{
    execute_index_count_range as batch_count_range,
    execute_index_covering_range as batch_covering_range,
    execute_index_range_scan_ordered as batch_range_ordered,
    execute_index_range_scan_ordered_desc as batch_range_ordered_desc,
    execute_index_range_scan_streaming as batch_range_streaming,
};
use super::policy::{ActiveExecBatchPolicy, ExecBatchPolicy};
use super::tail::load_table_row_by_rowid;

pub(crate) use super::index_batch::OutputColumnSource;

/// Maximum batch size used by the streaming cursor consumer. Matches
/// the prior `range_scan_visible` wrapper's chunk size so per-batch
/// telemetry stays comparable.
pub(crate) const MAX_BATCH: usize = ActiveExecBatchPolicy::INDEX_ROWID_BATCH;

/// What kind of index probe the predicate maps to. The planner names
/// `IndexPointLookup` and `IndexRangeScan`; a "point lookup" here means
/// EVERY index key is equality-constrained (and so a single
/// `point_lookup` call suffices). A "range scan" covers a leading-prefix
/// equality, an open range, or a half-open range.
#[derive(Debug, Clone)]
pub(crate) enum IndexProbe {
    /// Exact point match on the full index key.
    Point { key: Vec<u8> },
    /// `start <= key < end` over leaf cursors (kernel `range_scan` is
    /// half-open). Inclusive/exclusive endpoints are baked into the
    /// bytes via the `next_key`/`encoded_key` helpers below.
    Range { start: Vec<u8>, end: Vec<u8> },
}

/// Whether the planner should emit `IndexPointLookup` or
/// `IndexRangeScan` for this match. Mirrors `IndexProbe` so the planner
/// (which has no engine access) and the executor stay in agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexProbeKind {
    PointLookup,
    RangeScan,
}

/// A planner+executor view of "yes, this WHERE binds an index". The
/// planner uses `index` and `kind` for EXPLAIN output and cost; the
/// executor uses `probe` to drive the kernel B-tree.
#[derive(Debug, Clone)]
pub(crate) struct IndexAccessMatch {
    pub(crate) index: Arc<IndexDef>,
    pub(crate) kind: IndexProbeKind,
    pub(crate) probe: IndexProbe,
    pub(crate) predicates: Vec<String>,
    /// Phase 11 W1-D: when this match feeds an ORDER-BY-LIMIT plan
    /// where the index leading column matches the ORDER BY column, the
    /// executor stops the cursor walk after `n` snapshot-visible rows.
    /// `None` means "drain the full range". Currently set only by
    /// callers that already know the ORDER-BY/LIMIT shape (see
    /// `select_top::try_ordered_index_limit_path`); the field is
    /// reserved for the matched path and its tests.
    #[allow(dead_code)]
    pub(crate) ordered_limit: Option<usize>,
    /// Phase 5 WS-A1: conjuncts from the original WHERE that the index
    /// probe did NOT consume. The executor's normal path re-checks the
    /// full predicate on each row, so residuals are harmless there.
    /// Fast paths that skip the row-by-row recheck (COUNT-only,
    /// covering scan, ordered-limit early stop) must NOT fire when
    /// residuals exist — otherwise they ignore the residual conjunct
    /// and return wrong answers (e.g. count includes rows that fail
    /// `status='active'`).
    pub(crate) residual_conjuncts: Vec<Expr>,
    /// Phase 5 WS-A2: number of leading index key positions pinned to
    /// a constant by equality on this match. `INDEX(tenant, k)` with
    /// `WHERE tenant=?` → 1 (tenant pinned). `WHERE tenant=? AND k=?`
    /// → 2. Range/BETWEEN on the leading key → 0 (not equality).
    /// Lets ORDER-BY checks recognize that the cursor already emits in
    /// `k`-order over the slice where `tenant` is constant.
    pub(crate) equality_prefix_len: usize,
}

impl IndexAccessMatch {
    /// Phase 5 WS-A1: true when every top-level AND conjunct in the
    /// WHERE clause is consumed by the index probe. Required gate for
    /// any fast path that skips row-by-row predicate recheck.
    pub(crate) fn consumed_full_predicate(&self) -> bool {
        self.residual_conjuncts.is_empty()
    }
}

/// Try to plan an index-driven access path for `(table, selection)`.
///
/// Returns `None` when no index applies. The check is conservative: the
/// predicate must constrain a leading prefix of the index key. A
/// predicate that only constrains a non-leading column (e.g. `b = 5`
/// against an `(a, b)` index) returns `None`, forcing the caller to
/// fall back to `TableScan`.
pub(crate) fn try_match_index_access(
    engine: &Engine,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Option<IndexAccessMatch> {
    try_match_index_access_hinted(engine, table, selection, bindings, None)
}

/// Allocation-free case-insensitive substring scan for the literal
/// `"collate nocase"`. Used by `try_match_index_access_hinted` to bail out
/// of index-probe matching when the table's `normalized_sql` declares a
/// NOCASE collation (which the current index machinery doesn't honour).
#[inline]
fn contains_collate_nocase_ci(haystack: &str) -> bool {
    const NEEDLE: &[u8] = b"collate nocase";
    let bytes = haystack.as_bytes();
    if bytes.len() < NEEDLE.len() {
        return false;
    }
    bytes.windows(NEEDLE.len()).any(|window| {
        window
            .iter()
            .zip(NEEDLE.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

/// Phase 5 WS-A2e / A2g entry point. The optional `hint` rides through
/// SQLite-parity `INDEXED BY` / `NOT INDEXED` table-access hints; pass
/// `None` for callers that have no hint context (joins, CTE row sources,
/// etc.).
pub(crate) fn try_match_index_access_hinted(
    engine: &Engine,
    table: &Arc<TableDef>,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
    hint: Option<&TableAccessHint>,
) -> Option<IndexAccessMatch> {
    // Phase 5 WS-A2e: `NOT INDEXED` is permissive — it simply removes
    // every index from candidate consideration and forces a TableScan.
    if matches!(hint, Some(TableAccessHint::NotIndexed)) {
        return None;
    }
    let expr = selection.as_ref()?;
    if table.indexes.is_empty() {
        return None;
    }
    // A7: avoid the per-SELECT `to_ascii_lowercase()` allocation. Each call
    // here previously copied the entire normalized_sql into a fresh String
    // just to do a case-insensitive substring check. Common case: no COLLATE
    // NOCASE present, so the allocation was pure waste.
    if table
        .normalized_sql
        .as_deref()
        .is_some_and(contains_collate_nocase_ci)
    {
        return None;
    }
    // Collect candidate {column_ordinal, equalities, range bounds} from
    // the predicate. We only walk top-level AND chains; an OR or any
    // other shape disables the optimization for this round.
    let conjuncts = flatten_top_level_and(expr);
    if conjuncts.is_empty() {
        return None;
    }

    // For every candidate index, check whether the leading key column is
    // equality-bound. Take the first index that fits — when the integer
    // PK rowid alias is the leading column, the planner already prefers
    // `RowIdGet` ahead of this code path, so we don't have to break ties.
    for index in &table.indexes {
        // Phase 5 WS-A2e: `INDEXED BY <name>` restricts the candidate
        // set to a single named index (case-insensitive match against
        // the catalog `name`). Skipping non-matching indexes here is
        // SQLite-parity: SQLite errors when the named index doesn't
        // exist on the table, but otherwise narrows to that index.
        if let Some(TableAccessHint::IndexedBy(name)) = hint {
            if !index.name.eq_ignore_ascii_case(name) {
                continue;
            }
        }
        // Wave 7 P1 #5: do not advertise an index unless both the catalog
        // entry has a meta_page_id AND the engine has a live handle for
        // the index. Without those, the executor falls back to TableScan
        // and EXPLAIN would lie ("IndexPointLookup" while the runtime
        // scans). Skipping the candidate here keeps planner output
        // honest; the loop continues so a later index without this
        // problem can still match.
        if index.meta_page_id.is_none() || engine.index_handle(index.index_id).is_none() {
            continue;
        }
        // A6 SQL-D: partial indexes only contain rows matching their
        // WHERE predicate; the planner may use one only when the query
        // WHERE provably implies the index WHERE (today: exact match).
        // Otherwise we risk missing rows that exist in the heap but
        // were never inserted into the partial index.
        if !crate::exec::index_partial::query_implies_index_predicate(selection, index) {
            continue;
        }
        let Some(first_key) = index.keys.first() else {
            continue;
        };
        // Phase 5 WS-A2g: expression-index equality. When the leading
        // key is an expression and a top-level conjunct compares that
        // exact expression to a constant, treat it as a point lookup on
        // the encoded constant. Expression-index DML/backfill is wired,
        // so unhinted single-key expression indexes are safe to consider.
        // Multi-key expression indexes still stay out of this path until
        // every key has explicit residual and lookup proof.
        if let IndexKeySource::Expression { sql: expr_sql, .. } = &first_key.source {
            if index.keys.len() != 1 {
                continue;
            }
            if let Some((value, consumed_idx)) =
                expression_index_equality_match(&conjuncts, expr_sql, bindings)
            {
                let key = encode_single_value_key(first_key.sort_dir, &value);
                let predicates = vec![format!("{} = {}", expr_sql, sql_value_to_explain(&value))];
                let residual_conjuncts = residuals_from_consumed(&conjuncts, &[consumed_idx]);
                return Some(IndexAccessMatch {
                    index: Arc::new(index.clone()),
                    kind: IndexProbeKind::PointLookup,
                    probe: IndexProbe::Point { key },
                    predicates,
                    ordered_limit: None,
                    residual_conjuncts,
                    equality_prefix_len: 1,
                });
            }
            continue;
        }
        let IndexKeySource::Column { attnum: leading } = first_key.source else {
            // Future-proof: any new variant requires explicit handling.
            continue;
        };
        let leading = leading as usize;

        // Leading-column equality is the gateway. If we cannot bind the
        // leading column to a constant, skip this index entirely — we
        // never honor a non-leading-only predicate.
        let leading_eq = first_constant_eq_for_column(&conjuncts, table, leading, bindings);

        if let Some((leading_value, leading_idx)) = leading_eq {
            // Phase 5 WS-A1: track which conjunct indices the probe
            // consumed so residuals can be reported to the caller.
            let mut consumed_idx: Vec<usize> = vec![leading_idx];

            // Check for full-key equality (every index key has a
            // matching `col = ?` conjunct). If so, that's a point
            // lookup; otherwise it's a leading-prefix range scan.
            let mut full_key = Vec::with_capacity(index.keys.len());
            full_key.push(leading_value.clone());
            let mut full_match = true;
            for key in index.keys.iter().skip(1) {
                let IndexKeySource::Column { attnum } = key.source else {
                    // A6 SQL-D: expression keys don't satisfy planner
                    // column-equality matching yet.
                    full_match = false;
                    break;
                };
                let column = attnum as usize;
                match first_constant_eq_for_column(&conjuncts, table, column, bindings) {
                    Some((value, idx)) => {
                        full_key.push(value);
                        consumed_idx.push(idx);
                    }
                    None => {
                        full_match = false;
                        break;
                    }
                }
            }
            if full_match && index.keys.len() == full_key.len() {
                let key = encode_full_key(index, &full_key);
                let predicates = vec![format!(
                    "{} = {}",
                    table.columns[leading].name,
                    sql_value_to_explain(&full_key[0])
                )];
                let residual_conjuncts = residuals_from_consumed(&conjuncts, &consumed_idx);
                return Some(IndexAccessMatch {
                    index: Arc::new(index.clone()),
                    kind: IndexProbeKind::PointLookup,
                    probe: IndexProbe::Point { key },
                    predicates,
                    ordered_limit: None,
                    residual_conjuncts,
                    equality_prefix_len: index.keys.len(),
                });
            }
            // Leading-prefix range scan: encode just the leading value
            // and walk every key that starts with that prefix. Only the
            // leading-column equality was applied to the probe; any
            // partial full-key probes we attempted above did NOT make
            // it into the bytes, so they remain residuals.
            let prefix = encode_prefix_key(index, std::slice::from_ref(&leading_value));
            let (start, end) = prefix_bounds(&prefix);
            let predicates = vec![format!(
                "{} = {}",
                table.columns[leading].name,
                sql_value_to_explain(&leading_value)
            )];
            let residual_conjuncts = residuals_from_consumed(&conjuncts, &[leading_idx]);
            return Some(IndexAccessMatch {
                index: Arc::new(index.clone()),
                kind: IndexProbeKind::RangeScan,
                probe: IndexProbe::Range { start, end },
                predicates,
                ordered_limit: None,
                residual_conjuncts,
                // Only the leading key was equality-pinned (we did NOT
                // bake the partial full_key positions into the probe
                // bytes — only `leading_value` made it into the prefix).
                equality_prefix_len: 1,
            });
        }

        // No leading equality. Try a leading-column range (>=, >, <=, <,
        // BETWEEN) — also produces an `IndexRangeScan`.
        if let Some((bounds, predicates, consumed_idx)) =
            leading_range_bounds(&conjuncts, table, leading, bindings)
        {
            let start = match &bounds.lower {
                Some((value, inclusive)) => {
                    let bytes = encode_prefix_key(index, std::slice::from_ref(value));
                    if *inclusive { bytes } else { next_key(&bytes) }
                }
                None => Vec::new(),
            };
            let end = match &bounds.upper {
                Some((value, inclusive)) => {
                    let bytes = encode_prefix_key(index, std::slice::from_ref(value));
                    if *inclusive { next_key(&bytes) } else { bytes }
                }
                None => max_key_for(index),
            };
            let residual_conjuncts = residuals_from_consumed(&conjuncts, &consumed_idx);
            return Some(IndexAccessMatch {
                index: Arc::new(index.clone()),
                kind: IndexProbeKind::RangeScan,
                probe: IndexProbe::Range { start, end },
                predicates,
                ordered_limit: None,
                residual_conjuncts,
                // Range/BETWEEN on the leading key is not equality.
                equality_prefix_len: 0,
            });
        }
    }

    None
}

/// Phase 5 WS-A1: clone the conjuncts NOT named in `consumed` into
/// owned Exprs. Owned because `IndexAccessMatch` must outlive the
/// borrow on the original `selection` Expr.
fn residuals_from_consumed(conjuncts: &[&Expr], consumed: &[usize]) -> Vec<Expr> {
    conjuncts
        .iter()
        .enumerate()
        .filter_map(|(idx, expr)| {
            if consumed.contains(&idx) {
                None
            } else {
                Some((*expr).clone())
            }
        })
        .collect()
}

/// Run a point lookup through the index MVCC visibility filter and return
/// surviving rowids after the heap visibility check.
pub(crate) fn execute_index_point_lookup(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    key: &[u8],
) -> Result<Vec<RowId>> {
    let Some(handle) = open_handle(engine, index) else {
        // Defensive: if the planner advertised an index that the kernel
        // does not have a physical handle for, we must not crash —
        // instead the caller falls back to a TableScan. The planner is
        // not supposed to advertise this case (Wave 2 wired
        // `meta_page_id` for every new index), but a paranoid early
        // return here keeps the executor safe under outdated catalog
        // snapshots.
        return Ok(Vec::new());
    };
    let counters = engine.phase11_counters();
    let snapshot = tx.snapshot().clone();
    let view = SnapshotView::visible(engine.tx_status(), &snapshot, Some(tx.id()));
    let mut cursor = RawPointCursor::open_with_counters(&handle, key, view, Some(&*counters))?;
    let mut batch: Vec<IndexRowRef> = Vec::with_capacity(MAX_BATCH);
    let mut out = Vec::new();
    loop {
        batch.clear();
        match cursor.next_rowid_batch(&mut batch, MAX_BATCH)? {
            CursorYield::End => break,
            CursorYield::Batch(_) => {
                for entry in &batch {
                    if visible_in_relation(engine, tx, table, entry.row_id)? {
                        out.push(entry.row_id);
                    }
                }
            }
        }
    }
    cursor.close();
    Ok(out)
}

/// Phase 11 W1-C: streaming range scan with batched cursor consumption,
/// per-heap-page grouped recheck, and optional early-stop after `limit`
/// visible rows. Implementation lives in `index_batch.rs`; this is the
/// public re-export.
pub(crate) fn execute_index_range_scan_streaming(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
    limit: Option<usize>,
) -> Result<Vec<RowId>> {
    batch_range_streaming(engine, tx, table, index, start, end, limit)
}

/// Run a visible range scan (half-open `[start, end)`) and return
/// surviving rowids. Thin wrapper around the streaming variant for
/// callers that don't need a limit.
pub(crate) fn execute_index_range_scan(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
) -> Result<Vec<RowId>> {
    batch_range_streaming(engine, tx, table, index, start, end, None)
}

/// Convenience: run the supplied probe and return its rowids. Lets
/// callers stay agnostic about Point vs Range.
pub(crate) fn execute_index_probe(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    probe: &IndexProbe,
) -> Result<Vec<RowId>> {
    match probe {
        IndexProbe::Point { key } => execute_index_point_lookup(engine, tx, table, index, key),
        IndexProbe::Range { start, end } => {
            execute_index_range_scan(engine, tx, table, index, start, end)
        }
    }
}

/// Run the supplied probe with an optional `LIMIT n` early-stop. Used
/// by W1-D's ORDER-BY-LIMIT shortcut: when the caller knows the cursor
/// emits in the index leading-key order and that order matches the
/// `ORDER BY`, the executor can stop the cursor as soon as the desired
/// row count is reached, regardless of what the rest of the range
/// holds. Point lookups ignore the limit (the result is always at most
/// one row anyway).
pub(crate) fn execute_index_probe_with_limit(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    probe: &IndexProbe,
    limit: Option<usize>,
) -> Result<Vec<RowId>> {
    match probe {
        IndexProbe::Point { key } => match limit {
            Some(n) => {
                let end = next_key(key);
                batch_range_ordered(engine, tx, table, index, key, &end, n)
            }
            None => execute_index_point_lookup(engine, tx, table, index, key),
        },
        IndexProbe::Range { start, end } => match limit {
            Some(n) => batch_range_ordered(engine, tx, table, index, start, end, n),
            None => execute_index_range_scan_streaming(engine, tx, table, index, start, end, limit),
        },
    }
}

/// Phase 5 WS-A2c: DESC variant of [`execute_index_probe_with_limit`].
/// Used when the caller knows the cursor's leading key direction aligns
/// with `ORDER BY ... DESC`; the index leaf chain is walked right-to-left
/// so the result is index-ordered descending with the same early-stop
/// guarantee the forward path enjoys. Point probes are treated the same
/// as forward (the result is at most one row).
pub(crate) fn execute_index_probe_with_limit_desc(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    index: &IndexDef,
    probe: &IndexProbe,
    limit: Option<usize>,
) -> Result<Vec<RowId>> {
    match probe {
        IndexProbe::Point { key } => match limit {
            Some(n) => {
                let end = next_key(key);
                batch_range_ordered_desc(engine, tx, table, index, key, &end, n)
            }
            None => execute_index_point_lookup(engine, tx, table, index, key),
        },
        IndexProbe::Range { start, end } => match limit {
            Some(n) => batch_range_ordered_desc(engine, tx, table, index, start, end, n),
            None => execute_index_range_scan_streaming(engine, tx, table, index, start, end, limit),
        },
    }
}

/// Phase 11 W1-E: count visible entries inside the supplied range
/// without any heap loads. Implementation lives in `index_batch.rs`.
pub(crate) fn execute_index_count_range(
    engine: &Engine,
    tx: &Txn,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
) -> Result<i64> {
    batch_count_range(engine, tx, index, start, end)
}

/// Phase 11 W1-E: serve a covering range scan from the index leaf
/// chain. Implementation lives in `index_batch.rs`.
pub(crate) fn execute_index_covering_range(
    engine: &Engine,
    tx: &Txn,
    index: &IndexDef,
    start: &[u8],
    end: &[u8],
    out_columns: &[OutputColumnSource],
    limit: Option<usize>,
) -> Result<Vec<Vec<SqlValue>>> {
    batch_covering_range(engine, tx, index, start, end, out_columns, limit)
}

// ------------------------------ helpers ------------------------------

pub(crate) fn open_handle(
    engine: &Engine,
    index: &IndexDef,
) -> Option<Arc<redlinedb_kernel::index::BtreeIndex>> {
    index.meta_page_id?;
    engine.index_handle(index.index_id)
}

pub(super) fn visible_in_relation(
    engine: &Engine,
    tx: &mut Txn,
    table: &Arc<TableDef>,
    rowid: RowId,
) -> Result<bool> {
    // The visibility check runs through the tx's snapshot. We don't
    // need the tuple bytes here, only "did get_for_relation see a live
    // row" — load_table_row_by_rowid does the right table_id check
    // already, so we reuse it for parity with TableScan reads.
    Ok(load_table_row_by_rowid(engine, tx, table, rowid)?.is_some())
}

fn flatten_top_level_and(expr: &Expr) -> Vec<&Expr> {
    let mut out = Vec::new();
    fn walk<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
        match expr {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                walk(left, out);
                walk(right, out);
            }
            Expr::Nested(inner) => walk(inner, out),
            other => out.push(other),
        }
    }
    walk(expr, &mut out);
    out
}

/// Returns `(value, conjunct_index)` — `conjunct_index` lets the caller
/// mark exactly which conjunct was consumed so the residual set stays
/// honest (Phase 5 WS-A1).
fn first_constant_eq_for_column(
    conjuncts: &[&Expr],
    table: &TableDef,
    column: usize,
    bindings: &[Option<SqlValue>],
) -> Option<(SqlValue, usize)> {
    for (idx, expr) in conjuncts.iter().enumerate() {
        if let Some(value) = constant_eq_for_column(expr, table, column, bindings) {
            // SQLite NULL parity: `col = NULL` is never true; never
            // route a NULL probe through the index — fall back to scan.
            if matches!(value, SqlValue::Null) {
                continue;
            }
            return Some((value, idx));
        }
    }
    None
}

fn constant_eq_for_column(
    expr: &Expr,
    table: &TableDef,
    column: usize,
    bindings: &[Option<SqlValue>],
) -> Option<SqlValue> {
    let Expr::BinaryOp { left, op, right } = strip_nested(expr) else {
        return None;
    };
    if !matches!(op, BinaryOperator::Eq) {
        return None;
    }
    let left_col = expr_column_ordinal(left, table);
    let right_col = expr_column_ordinal(right, table);
    if left_col == Some(column) {
        return eval_constant(right, bindings);
    }
    if right_col == Some(column) {
        return eval_constant(left, bindings);
    }
    None
}

#[derive(Debug, Default)]
struct LeadingRange {
    /// `(value, inclusive)` for the lower bound, when present.
    lower: Option<(SqlValue, bool)>,
    /// `(value, inclusive)` for the upper bound, when present.
    upper: Option<(SqlValue, bool)>,
}

/// Returns `(bounds, predicates, consumed_indices)`. `consumed_indices`
/// lists which conjuncts were folded into the range — anything outside
/// that set becomes a residual predicate (Phase 5 WS-A1).
fn leading_range_bounds(
    conjuncts: &[&Expr],
    table: &TableDef,
    column: usize,
    bindings: &[Option<SqlValue>],
) -> Option<(LeadingRange, Vec<String>, Vec<usize>)> {
    let mut bounds = LeadingRange::default();
    let mut predicates: Vec<String> = Vec::new();
    let mut consumed: Vec<usize> = Vec::new();
    let column_name = table.columns.get(column).map(|c| c.name.to_string())?;
    for (idx, expr) in conjuncts.iter().enumerate() {
        let stripped = strip_nested(expr);
        if let Expr::BinaryOp { left, op, right } = stripped
            && let Some(side) = comparison_constant_for_column(left, right, table, column, bindings)
        {
            let (value, inclusive_lower, inclusive_upper, is_lower) = match op {
                BinaryOperator::Gt if side == ColumnSide::Left => {
                    (side.value(), false, false, true)
                }
                BinaryOperator::GtEq if side == ColumnSide::Left => {
                    (side.value(), true, false, true)
                }
                BinaryOperator::Lt if side == ColumnSide::Left => {
                    (side.value(), false, false, false)
                }
                BinaryOperator::LtEq if side == ColumnSide::Left => {
                    (side.value(), false, true, false)
                }
                BinaryOperator::Gt if side == ColumnSide::Right => {
                    (side.value(), false, false, false)
                }
                BinaryOperator::GtEq if side == ColumnSide::Right => {
                    (side.value(), false, true, false)
                }
                BinaryOperator::Lt if side == ColumnSide::Right => {
                    (side.value(), false, false, true)
                }
                BinaryOperator::LtEq if side == ColumnSide::Right => {
                    (side.value(), true, false, true)
                }
                _ => continue,
            };
            if matches!(value, SqlValue::Null) {
                continue;
            }
            predicates.push(format!(
                "{} {} {}",
                column_name,
                binary_op_to_str(op, side.side),
                sql_value_to_explain(&value)
            ));
            if is_lower {
                bounds.lower = Some((value, inclusive_lower));
            } else {
                bounds.upper = Some((value, inclusive_upper));
            }
            consumed.push(idx);
            continue;
        }
        if let Expr::Between {
            expr: ident,
            negated: false,
            low,
            high,
        } = stripped
            && expr_column_ordinal(ident, table) == Some(column)
        {
            let lo = eval_constant(low, bindings)?;
            let hi = eval_constant(high, bindings)?;
            if matches!(lo, SqlValue::Null) || matches!(hi, SqlValue::Null) {
                continue;
            }
            predicates.push(format!(
                "{} BETWEEN {} AND {}",
                column_name,
                sql_value_to_explain(&lo),
                sql_value_to_explain(&hi)
            ));
            bounds.lower = Some((lo, true));
            bounds.upper = Some((hi, true));
            consumed.push(idx);
        }
    }
    if bounds.lower.is_none() && bounds.upper.is_none() {
        return None;
    }
    Some((bounds, predicates, consumed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnSide {
    Left,
    Right,
}

struct ColumnSideValue {
    side: ColumnSide,
    value: SqlValue,
}

impl ColumnSideValue {
    fn value(&self) -> SqlValue {
        self.value.clone()
    }
}

impl PartialEq<ColumnSide> for ColumnSideValue {
    fn eq(&self, other: &ColumnSide) -> bool {
        self.side == *other
    }
}

fn comparison_constant_for_column(
    left: &Expr,
    right: &Expr,
    table: &TableDef,
    column: usize,
    bindings: &[Option<SqlValue>],
) -> Option<ColumnSideValue> {
    if expr_column_ordinal(left, table) == Some(column) {
        return eval_constant(right, bindings).map(|value| ColumnSideValue {
            side: ColumnSide::Left,
            value,
        });
    }
    if expr_column_ordinal(right, table) == Some(column) {
        return eval_constant(left, bindings).map(|value| ColumnSideValue {
            side: ColumnSide::Right,
            value,
        });
    }
    None
}

fn binary_op_to_str(op: &BinaryOperator, side: ColumnSide) -> &'static str {
    // EXPLAIN renders predicates with the column on the left for
    // readability; flip the operator when the column was on the right.
    match (op, side) {
        (BinaryOperator::Gt, ColumnSide::Left) => ">",
        (BinaryOperator::GtEq, ColumnSide::Left) => ">=",
        (BinaryOperator::Lt, ColumnSide::Left) => "<",
        (BinaryOperator::LtEq, ColumnSide::Left) => "<=",
        (BinaryOperator::Gt, ColumnSide::Right) => "<",
        (BinaryOperator::GtEq, ColumnSide::Right) => "<=",
        (BinaryOperator::Lt, ColumnSide::Right) => ">",
        (BinaryOperator::LtEq, ColumnSide::Right) => ">=",
        _ => "?",
    }
}

fn strip_nested(expr: &Expr) -> &Expr {
    let mut current = expr;
    while let Expr::Nested(inner) = current {
        current = inner;
    }
    current
}

fn expr_column_ordinal(expr: &Expr, table: &TableDef) -> Option<usize> {
    match strip_nested(expr) {
        Expr::Identifier(ident) => column_ordinal_for_table(&ident.value, table),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .and_then(|ident| column_ordinal_for_table(&ident.value, table)),
        _ => None,
    }
}

/// Phase 5 WS-A2g: match a top-level conjunct against an expression
/// index's stored SQL text. Returns `(value, conjunct_idx)` on success.
///
/// The match is conservative: only `expr_text = const` (or the symmetric
/// `const = expr_text`) succeeds, where `expr_text` must render
/// (case-folded, whitespace-collapsed) identical to the index's stored
/// expression SQL. We do not perform SQL semantic-equivalence; if the
/// user wrote `LOWER(name)` and the index stored `lower(name)`, the
/// normalizer folds both to the same canonical form and matches.
fn expression_index_equality_match(
    conjuncts: &[&Expr],
    index_expr_sql: &str,
    bindings: &[Option<SqlValue>],
) -> Option<(SqlValue, usize)> {
    let index_norm = normalize_expr_text(index_expr_sql);
    for (idx, expr) in conjuncts.iter().enumerate() {
        let Expr::BinaryOp { left, op, right } = strip_nested(expr) else {
            continue;
        };
        if !matches!(op, BinaryOperator::Eq) {
            continue;
        }
        let left_matches = expr_text_eq_normalized(left, &index_norm);
        let right_matches = expr_text_eq_normalized(right, &index_norm);
        let value = if left_matches {
            eval_constant(right, bindings)
        } else if right_matches {
            eval_constant(left, bindings)
        } else {
            None
        };
        if let Some(value) = value
            && !matches!(value, SqlValue::Null)
        {
            return Some((value, idx));
        }
    }
    None
}

fn expr_text_eq_normalized(expr: &Expr, index_norm: &str) -> bool {
    normalize_expr_text(&strip_nested(expr).to_string()) == index_norm
}

/// Lower-case + ASCII-whitespace-collapse so `lower(name)` and
/// `LOWER( name )` compare equal. Intentionally light-touch — anything
/// fancier (operator-precedence parsing, full normalization) is out of
/// scope for the conservative match.
fn normalize_expr_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_ws = false;
    for ch in text.chars() {
        if ch.is_ascii_whitespace() {
            if !last_ws && !out.is_empty() {
                out.push(' ');
            }
            last_ws = true;
        } else {
            out.push(ch.to_ascii_lowercase());
            last_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    // Strip a single layer of outer parentheses so `(lower(name))`
    // matches `lower(name)`.
    while out.starts_with('(') && out.ends_with(')') {
        // Only strip when the parens are balanced as a single wrap.
        let mut depth: i32 = 0;
        let mut wraps = true;
        for (i, ch) in out.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 && i != out.len() - 1 {
                        wraps = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if wraps {
            out = out[1..out.len() - 1].trim().to_owned();
        } else {
            break;
        }
    }
    out
}

/// Encode a single SqlValue as an index key. Used by the WS-A2g
/// expression-index point-lookup path (which always has exactly one key
/// part).
fn encode_single_value_key(sort_dir: SortDir, value: &SqlValue) -> Vec<u8> {
    let value_refs = [value.as_ref()];
    let dirs = [sort_dir];
    let mut buf = Vec::new();
    let EncodedIndexKey { bytes, .. } = encode_index_key(&value_refs, &dirs, &mut buf);
    bytes
}

fn encode_full_key(index: &IndexDef, values: &[SqlValue]) -> Vec<u8> {
    let mut dirs: Vec<SortDir> = Vec::with_capacity(index.keys.len());
    let mut owned_refs: Vec<&SqlValue> = Vec::with_capacity(index.keys.len());
    for (key, value) in index.keys.iter().zip(values.iter()) {
        // A6 SQL-D: expression key sources do not reach this encode
        // path today — planner skips expression indexes — so the
        // attnum is informational only.
        if !matches!(key.source, IndexKeySource::Column { .. }) {
            continue;
        }
        owned_refs.push(value);
        dirs.push(key.sort_dir);
    }
    let value_refs: Vec<_> = owned_refs.iter().map(|v| v.as_ref()).collect();
    let mut buf = Vec::new();
    let EncodedIndexKey { bytes, .. } = encode_index_key(&value_refs, &dirs, &mut buf);
    bytes
}

fn encode_prefix_key(index: &IndexDef, leading_values: &[SqlValue]) -> Vec<u8> {
    let mut dirs: Vec<SortDir> = Vec::with_capacity(leading_values.len());
    let mut owned_refs: Vec<&SqlValue> = Vec::with_capacity(leading_values.len());
    for (key, value) in index.keys.iter().zip(leading_values.iter()) {
        if !matches!(key.source, IndexKeySource::Column { .. }) {
            continue;
        }
        owned_refs.push(value);
        dirs.push(key.sort_dir);
    }
    let value_refs: Vec<_> = owned_refs.iter().map(|v| v.as_ref()).collect();
    let mut buf = Vec::new();
    let EncodedIndexKey { bytes, .. } = encode_index_key(&value_refs, &dirs, &mut buf);
    bytes
}

/// Smallest byte string strictly greater than `bytes`. The encoding
/// terminates each key part with `0xff`, but the next part is appended
/// AFTER that terminator, so the strict successor must increment the
/// prefix as a binary number rather than appending. We strip trailing
/// `0xff` bytes (they cannot be incremented in place) and bump the
/// rightmost non-`0xff` byte; if the entire prefix is `0xff`s we fall
/// back to `[0xff; 32]`, which sorts past anything `encode_index_key`
/// produces for a single value.
fn next_key(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    while let Some(&last) = out.last() {
        if last == 0xff {
            out.pop();
        } else {
            break;
        }
    }
    if out.is_empty() {
        return vec![0xff; 32];
    }
    if let Some(last) = out.last_mut() {
        *last = last.saturating_add(1);
    }
    out
}

/// Returns `[start, end)` byte bounds whose half-open range covers
/// every key that begins with `prefix`. The prefix is inclusive on the
/// low end and exclusive on the high end. Composite keys put the
/// next-part type tag IMMEDIATELY after the part separator (`0xff`),
/// so the upper bound has to be the binary successor of the prefix —
/// not `prefix || 0x00`, which sorts BEFORE every full key whose
/// suffix begins with `0x10` (Integer), `0x20` (Real), `0x30` (Text),
/// or `0x40` (Blob).
fn prefix_bounds(prefix: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let start = prefix.to_vec();
    let end = next_key(prefix);
    (start, end)
}

/// An upper bound that is guaranteed to sort after any key whose
/// leading part is bounded — used when there is no upper predicate.
fn max_key_for(_index: &IndexDef) -> Vec<u8> {
    // 32 bytes of 0xff is far past anything `encode_index_key` produces
    // for a single value (signed integers are at most 9 bytes; text and
    // blobs end with 0x00 0x00 separators, never a long 0xff run).
    vec![0xff; 32]
}

fn sql_value_to_explain(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "NULL".to_owned(),
        SqlValue::Integer(v) => v.to_string(),
        SqlValue::Real(v) => format!("{v}"),
        SqlValue::Text(v) => format!("'{}'", v),
        SqlValue::Blob(v) => format!("x'{}'", hex_encode(v)),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

/// Local copy of the planner's column-name lookup. Kept here so the
/// executor module is self-contained; the planner version is private
/// to its module.
fn column_ordinal_for_table(name: &str, table: &TableDef) -> Option<usize> {
    table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
}

/// Local constant-folder mirroring the planner helper. Used only for
/// access-path matching, so we keep it conservative — anything beyond
/// literals, parameter bindings, simple unary +/-, and parenthesized
/// nesting returns `None`, which forces the caller to treat the
/// predicate as non-indexable.
fn eval_constant(expr: &Expr, bindings: &[Option<SqlValue>]) -> Option<SqlValue> {
    use sqlparser::ast::UnaryOperator;
    if let Expr::Value(v) = expr {
        if let Some(name) = crate::parser::bind::as_bind_name(&v.value) {
            return crate::parser::bind::resolve_positional(name, bindings);
        }
    }
    match expr {
        Expr::Value(v) => Some(match &v.value {
            Value::Null => SqlValue::Null,
            Value::Boolean(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
            Value::Number(n, _) => n
                .parse()
                .ok()
                .map(SqlValue::Integer)
                .unwrap_or(SqlValue::Null),
            Value::SingleQuotedString(s) => SqlValue::Text(std::sync::Arc::from(s.as_str())),
            Value::DoubleQuotedString(s) => SqlValue::Text(std::sync::Arc::from(s.as_str())),
            _ => return None,
        }),
        Expr::Nested(inner) => eval_constant(inner, bindings),
        Expr::UnaryOp { op, expr } => {
            let value = eval_constant(expr, bindings)?;
            match op {
                UnaryOperator::Minus => match value {
                    SqlValue::Integer(v) => Some(SqlValue::Integer(-v)),
                    SqlValue::Real(v) => Some(SqlValue::Real(-v)),
                    _ => None,
                },
                UnaryOperator::Plus => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod a7_collate_scan_tests {
    use super::contains_collate_nocase_ci;

    #[test]
    fn matches_lowercase() {
        assert!(contains_collate_nocase_ci("CREATE TABLE t (a TEXT collate nocase)"));
    }

    #[test]
    fn matches_uppercase() {
        assert!(contains_collate_nocase_ci("CREATE TABLE t (a TEXT COLLATE NOCASE)"));
    }

    #[test]
    fn matches_mixed_case() {
        assert!(contains_collate_nocase_ci(
            "CREATE TABLE t (a TEXT Collate NoCase)"
        ));
    }

    #[test]
    fn rejects_unrelated_text() {
        assert!(!contains_collate_nocase_ci(
            "CREATE TABLE t (a INTEGER PRIMARY KEY)"
        ));
    }

    #[test]
    fn rejects_partial_match() {
        // 'collate' alone or 'nocase' alone must not trigger.
        assert!(!contains_collate_nocase_ci("a TEXT COLLATE BINARY"));
        assert!(!contains_collate_nocase_ci("nocase_column TEXT"));
    }

    #[test]
    fn rejects_shorter_than_needle() {
        assert!(!contains_collate_nocase_ci("short"));
        assert!(!contains_collate_nocase_ci(""));
    }
}
