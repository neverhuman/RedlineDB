//! Phase 5 WS-A6: hot-row UPDATE fast path.
//!
//! Classifies UPDATE SET-clause assignments into a tiny IR:
//!
//!   * `Replacement(value)` — literal or `?N` binding the executor can
//!     resolve once per statement.
//!   * `IntegerDelta { col, delta }` — `col = col + lit` / `col = col - lit`
//!     where `lit` is an integer literal or integer-typed binding.
//!
//! At execute time the optimised path replaces the per-row `eval_scalar`
//! walk (lookup_column + arithmetic + AST traversal) with a direct
//! `Vec<SqlValue>` rewrite. Behaviour MUST be identical to the slow
//! path; on any unsupported shape, indexed-column touch, generated
//! column, FK column, or table with triggers/checks, we return `None`
//! and the caller falls back.
//!
//! Scope reduction note: the multi-writer batching coordinator and the
//! WAL `CombinedSemanticDelta` variant from the original spec are
//! deferred. This module ships only the single-writer SET-clause
//! evaluator optimisation; no kernel WAL format changes, no inter-
//! thread coordination. The lock/commit path is unchanged.

use std::sync::Arc;

use redlinedb_kernel::catalog::TableDef;
use sqlparser::ast::{BinaryOperator, Expr, Value, ValueWithSpan};

use crate::error::Result;
use crate::statement::{DmlValue, UpdatePlan};
use crate::value::SqlValue;

/// Per-assignment plan emitted by [`classify_assignments`]. Encodes the
/// minimum work the per-row applier needs to do.
#[derive(Debug, Clone)]
pub(crate) enum AssignmentPlan {
    /// `col = <literal>` or `col = ?N` — value is constant per statement
    /// invocation. Resolved once in [`prepare_assignments`].
    Replacement { col: usize, value: SqlValue },
    /// `col = col + lit` or `col = col - lit` where `lit` is an integer.
    /// Applied per row by reading the fresh value and adding/subtracting.
    IntegerDelta { col: usize, delta: i64 },
}

/// Decision returned by [`classify_assignments`]. Either the SET clause
/// is fully classifiable (every assignment maps to an `AssignmentPlan`)
/// or one or more assignments are unsupported and the caller must fall
/// back to the generic `evaluate_dml_value` path.
pub(crate) enum ClassifyResult {
    Supported(Vec<AssignmentPlan>),
    Unsupported,
}

/// Returns `Supported(plans)` when every assignment in `plan.assignments`
/// reduces to one of the [`AssignmentPlan`] variants. The bindings vector
/// is used to resolve `?N` placeholders to concrete `SqlValue`s once per
/// statement invocation; the per-row applier can then skip the
/// `eval_scalar` walk entirely.
pub(crate) fn classify_assignments(
    plan: &UpdatePlan,
    bindings: &[Option<SqlValue>],
) -> Result<ClassifyResult> {
    let mut out = Vec::with_capacity(plan.assignments.len());
    for (ordinal, dml) in &plan.assignments {
        let col = *ordinal;
        if col >= plan.table.columns.len() {
            return Ok(ClassifyResult::Unsupported);
        }
        let DmlValue::Expr(expr) = dml else {
            return Ok(ClassifyResult::Unsupported);
        };
        match classify_one(&plan.table, col, expr, bindings)? {
            Some(p) => out.push(p),
            None => return Ok(ClassifyResult::Unsupported),
        }
    }
    Ok(ClassifyResult::Supported(out))
}

fn classify_one(
    table: &Arc<TableDef>,
    col: usize,
    expr: &Expr,
    bindings: &[Option<SqlValue>],
) -> Result<Option<AssignmentPlan>> {
    // Strip a single Nested wrapper so `(?1)` and `(version + 1)` work.
    let expr = match expr {
        Expr::Nested(inner) => inner.as_ref(),
        other => other,
    };

    // Pure literal / binding replacement: `col = ?` or `col = 42`.
    if let Some(v) = literal_or_binding(expr, bindings) {
        return Ok(Some(AssignmentPlan::Replacement { col, value: v }));
    }

    // `col = col + <int>` or `col = col - <int>` — commutative-delta
    // form. The LHS column reference MUST resolve to the same ordinal
    // as the assignment target; otherwise the SET would read a
    // different column than it writes and the simple delta form
    // doesn't apply.
    if let Expr::BinaryOp { left, op, right } = expr {
        let (sign, lit_side) = match op {
            BinaryOperator::Plus => (1i64, right.as_ref()),
            BinaryOperator::Minus => (-1i64, right.as_ref()),
            _ => return Ok(None),
        };
        let Some(left_ord) = ident_column_ordinal(table, left) else {
            return Ok(None);
        };
        if left_ord != col {
            return Ok(None);
        }
        let Some(rhs_value) = literal_or_binding(lit_side, bindings) else {
            return Ok(None);
        };
        let SqlValue::Integer(n) = rhs_value else {
            return Ok(None);
        };
        let Some(delta) = n.checked_mul(sign) else {
            return Ok(None);
        };
        return Ok(Some(AssignmentPlan::IntegerDelta { col, delta }));
    }

    Ok(None)
}

/// Resolve a literal or bind-parameter expression to its `SqlValue`.
/// Returns `None` for any expression that touches columns, functions,
/// CASE, subqueries, etc. — those must use the slow path.
fn literal_or_binding(expr: &Expr, bindings: &[Option<SqlValue>]) -> Option<SqlValue> {
    let Expr::Value(ValueWithSpan { value, .. }) = expr else {
        return None;
    };
    if let Some(name) = crate::parser::bind::as_bind_name(value) {
        let rest = name.strip_prefix('?')?;
        let slot = rest.parse::<usize>().ok()?;
        return Some(
            bindings
                .get(slot)
                .cloned()
                .flatten()
                .unwrap_or(SqlValue::Null),
        );
    }
    match value {
        Value::Null => Some(SqlValue::Null),
        Value::Boolean(b) => Some(SqlValue::Integer(if *b { 1 } else { 0 })),
        Value::Number(text, _) => {
            if let Ok(n) = text.parse::<i64>() {
                Some(SqlValue::Integer(n))
            } else {
                text.parse::<f64>().ok().map(SqlValue::Real)
            }
        }
        Value::SingleQuotedString(s)
        | Value::DoubleQuotedString(s)
        | Value::TripleSingleQuotedString(s)
        | Value::TripleDoubleQuotedString(s) => Some(SqlValue::Text(Arc::from(s.as_str()))),
        _ => None,
    }
}

/// Resolve an `Expr` to a column ordinal if it is `Identifier(name)` or
/// `CompoundIdentifier([name])`. Case-insensitive match against the
/// table's column folded names. Qualified `tbl.col` is also accepted
/// when the qualifier matches the table.
fn ident_column_ordinal(table: &Arc<TableDef>, expr: &Expr) -> Option<usize> {
    let name = match expr {
        Expr::Identifier(id) => id.value.as_str(),
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [id] => id.value.as_str(),
            [qual, id] => {
                if !qual.value.eq_ignore_ascii_case(&table.name)
                    && !qual.value.eq_ignore_ascii_case(&table.folded)
                {
                    return None;
                }
                id.value.as_str()
            }
            _ => return None,
        },
        Expr::Nested(inner) => return ident_column_ordinal(table, inner),
        _ => return None,
    };
    table
        .columns
        .iter()
        .position(|c| c.folded.as_ref().eq_ignore_ascii_case(name))
}

/// Decide whether the fast path is eligible for `plan`. This is a
/// PURELY-STRUCTURAL check (no per-row work) so callers can do it once
/// before classifying the SET expressions.
///
/// Disqualifiers — any of these forces the slow path:
///   * `RETURNING` clause (user observes the post-image row).
///   * Any STORED or VIRTUAL generated column on the table.
///   * Any CHECK constraint (must re-evaluate against new values).
///   * Any FK on the table (parent or child side).
///   * Any index whose key references a column the UPDATE writes —
///     would need a delete+insert in the index, which the slow path
///     does correctly via `maintain_indexes_on_update`.
///   * Triggers — checked separately by the caller because that
///     requires a schema snapshot.
///   * UNIQUE constraints (other than the rowid PK) — would need
///     conflict-detection that the slow path performs.
///   * Touching the rowid alias column — would change the rowid which
///     forces delete+insert.
pub(crate) fn structurally_eligible(plan: &UpdatePlan) -> bool {
    let table = &plan.table;

    if plan.returning.is_some() {
        return false;
    }

    // Generated columns must be recomputed after any input changes; the
    // slow path handles that. Bail unconditionally when any generated
    // column exists rather than tracking dependencies here.
    if table.columns.iter().any(|c| c.generated.is_some()) {
        return false;
    }

    if !table.checks.is_empty() {
        return false;
    }

    if !table.foreign_keys.is_empty() {
        return false;
    }

    // Build the set of touched columns once.
    let mut touched = vec![false; table.columns.len()];
    for (ord, _) in &plan.assignments {
        if *ord >= touched.len() {
            return false;
        }
        touched[*ord] = true;
    }

    // Rowid alias touched → may move the row to a new rowid → slow path.
    if let Some(alias) = table.rowid_alias_column
        && touched.get(alias as usize).copied().unwrap_or(false)
    {
        return false;
    }

    // Any index whose key column set intersects `touched` → slow path.
    for index in &table.indexes {
        for key in &index.keys {
            use redlinedb_kernel::catalog::IndexKeySource;
            match &key.source {
                IndexKeySource::Column { attnum } => {
                    if touched.get(*attnum as usize).copied().unwrap_or(false) {
                        return false;
                    }
                }
                IndexKeySource::Expression {
                    referenced_cols, ..
                } => {
                    if referenced_cols
                        .iter()
                        .any(|c| touched.get(*c as usize).copied().unwrap_or(false))
                    {
                        return false;
                    }
                }
            }
        }
    }

    // UNIQUE / non-rowid PK constraints would need uniqueness recheck.
    for c in &table.constraints {
        use redlinedb_kernel::catalog::ConstraintKind;
        if matches!(c.kind, ConstraintKind::Unique | ConstraintKind::PrimaryKey) {
            return false;
        }
    }

    true
}

/// Apply a vector of [`AssignmentPlan`] entries to `values` in place,
/// producing the post-image row. Mirrors the per-column rewrite the
/// slow path performs but without the AST walk or thread-local lookup.
pub(crate) fn apply_plans(plans: &[AssignmentPlan], values: &mut [SqlValue]) -> Result<()> {
    for plan in plans {
        match plan {
            AssignmentPlan::Replacement { col, value } => {
                values[*col] = value.clone();
            }
            AssignmentPlan::IntegerDelta { col, delta } => {
                let new_val = match &values[*col] {
                    SqlValue::Integer(n) => SqlValue::Integer(n.wrapping_add(*delta)),
                    SqlValue::Null => SqlValue::Null,
                    SqlValue::Real(r) => SqlValue::Real(*r + (*delta as f64)),
                    // Text/Blob: SQLite would coerce to a number then add. For
                    // safety, bail out of the fast path on this row.
                    _ => {
                        return Err(crate::error::Error::UnsupportedSql(
                            "hot_row delta on non-numeric value".to_string(),
                        ));
                    }
                };
                values[*col] = new_val;
            }
        }
    }
    Ok(())
}

// =========================================================================
// WS-A6 wave 2 — multi-writer hot-row coordinator.
// =========================================================================
//
// The single-writer fast path above already speeds up isolated UPDATE-on-
// hot-row workloads by skipping per-row `eval_scalar`. But when MANY
// threads concurrently UPDATE the SAME (rel_id, row_id) with commutative
// deltas or replacement values, every writer still pays the full lock
// acquire + WAL append cost. The coordinator below batches concurrent
// writers for the same row so the workload reduces to:
//
//   - one row-lock acquire per batch (the coordinator's),
//   - one heap mutation per batch,
//   - one extra `WalPayload::CombinedSemanticDelta` audit record per
//     batch (alongside the regular HeapUpdate that the heap emits).
//
// **Correctness gate**. The coordinator only accepts a writer into the
// batch when its SET clause matches the WS-A6 commutative-delta / pure-
// replacement shape. For the resulting batch, every commutative delta is
// additive and order-independent; every replacement is last-write-wins;
// any interleaving therefore has an equivalent serial order that
// produces the same final row state. The audit `CombinedSemanticDelta`
// records the per-batch summary so an external reader (replication,
// debugging) can see how the batch was merged.
//
// **Locking discipline**. A writer thread holds at most ONE batch lock
// at a time (the one for its target row); the DashMap-style guard is
// dropped before the per-row lock-and-apply path runs. No nested lock
// order over disjoint rows means no deadlock risk.

use std::collections::HashMap;
use std::sync::Condvar as StdCondvar;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use redlinedb_kernel::format::{RelId, RowId};
use redlinedb_kernel::wal::CombinedReplacementValue;

/// Maximum wait the coordinator gives joiners to arrive before flushing.
/// Tuned to ~50 microseconds — long enough for an in-process join under
/// heavy contention, short enough not to stall light workloads.
const COORDINATOR_BATCH_WINDOW: Duration = Duration::from_micros(50);

/// Soft cap on how many writers a single coordinator batch will absorb
/// before flushing. Keeps p99 latency bounded even when arrival rate
/// exceeds the window.
const COORDINATOR_MAX_BATCH: u32 = 64;

/// One per (rel_id, row_id) — created lazily on first writer.
struct HotRowBatch {
    /// Sum of all commutative deltas per column ordinal, in arrival
    /// order. Order-independent for correctness, but kept as a Vec so
    /// the WAL record retains an audit trail.
    deltas: Vec<(u16, i64)>,
    /// Last-write-wins replacement per column. Stored as the value the
    /// final writer in the batch supplied, which the coordinator merges
    /// when applying the batch.
    replacements: HashMap<u16, CombinedReplacementValue>,
    /// Count of writers in this batch (coordinator included).
    writer_count: u32,
    /// `true` once the coordinator has flushed — joiners observing this
    /// can wake up and return.
    flushed: bool,
    /// Set by the coordinator when the apply path errored. Joiners
    /// then bail out with the same error class so they don't observe a
    /// "success" they didn't get.
    failed: bool,
}

/// Cross-thread coordinator that batches commutative-delta and
/// replacement UPDATEs targeting the same `(RelId, RowId)`. One
/// instance lives behind the [`Connection`] (or, equivalently, the
/// engine) for the lifetime of the database.
pub(crate) struct HotRowCoordinator {
    batches: StdMutex<HashMap<(RelId, RowId), std::sync::Arc<BatchSlot>>>,
}

struct BatchSlot {
    inner: StdMutex<HotRowBatch>,
    cv: StdCondvar,
}

/// Outcome returned to the calling writer after the coordinator has
/// either driven the batch or joined one.
pub(crate) enum CoordinatorRole {
    /// This writer is the coordinator. It must apply the merged batch
    /// to the heap, emit the [`WalPayload::CombinedSemanticDelta`]
    /// record, then call [`HotRowCoordinator::publish`] to release
    /// joiners.
    Coordinator(CoordinatorTicket),
    /// This writer joined an existing batch. The coordinator handles
    /// the heap mutation and WAL emission on its behalf; the joiner's
    /// only remaining work is to count the UPDATE in its own
    /// affected-rows tally and return.
    Joined,
    /// Batching was not eligible for this attempt (e.g. the existing
    /// batch was already at the soft cap, or the slot held the lock
    /// for too long). Fall back to the existing single-writer optimised
    /// path.
    Bypass,
}

/// Handed to the coordinator after it accepts the role; carries the
/// merged batch payload it must apply to the heap + WAL.
#[allow(dead_code)]
pub(crate) struct CoordinatorTicket {
    pub(crate) rel_id: RelId,
    pub(crate) row_id: RowId,
    pub(crate) deltas: Vec<(u16, i64)>,
    pub(crate) replacements: Vec<(u16, CombinedReplacementValue)>,
    pub(crate) batched_count: u32,
    slot: std::sync::Arc<BatchSlot>,
    map_key: (RelId, RowId),
}

impl HotRowCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            batches: StdMutex::new(HashMap::new()),
        }
    }

    /// Attempt to join (or create) a batch for `(rel_id, row_id)`. The
    /// caller supplies its commutative-delta and replacement
    /// contributions — those are merged into the batch before this
    /// function returns. Returns the role the caller plays.
    pub(crate) fn submit(
        &self,
        rel_id: RelId,
        row_id: RowId,
        deltas: &[(u16, i64)],
        replacements: &[(u16, CombinedReplacementValue)],
    ) -> CoordinatorRole {
        let key = (rel_id, row_id);
        let slot = {
            let mut map = match self.batches.lock() {
                Ok(g) => g,
                Err(_) => return CoordinatorRole::Bypass,
            };
            std::sync::Arc::clone(map.entry(key).or_insert_with(|| {
                std::sync::Arc::new(BatchSlot {
                    inner: StdMutex::new(HotRowBatch {
                        deltas: Vec::new(),
                        replacements: HashMap::new(),
                        writer_count: 0,
                        flushed: false,
                        failed: false,
                    }),
                    cv: StdCondvar::new(),
                })
            }))
        };

        let mut batch = match slot.inner.lock() {
            Ok(g) => g,
            Err(_) => return CoordinatorRole::Bypass,
        };

        if batch.flushed {
            // An earlier batch on this slot already completed but the
            // slot wasn't garbage-collected yet. Reset for the next
            // round; this writer becomes the new coordinator.
            batch.deltas.clear();
            batch.replacements.clear();
            batch.writer_count = 0;
            batch.flushed = false;
            batch.failed = false;
        }

        if batch.writer_count >= COORDINATOR_MAX_BATCH {
            return CoordinatorRole::Bypass;
        }

        // Merge this writer's contribution into the batch.
        for (col, delta) in deltas {
            batch.deltas.push((*col, *delta));
        }
        for (col, value) in replacements {
            batch.replacements.insert(*col, value.clone());
        }
        batch.writer_count += 1;

        let is_coordinator = batch.writer_count == 1;

        if is_coordinator {
            // The first writer waits briefly for joiners before
            // flushing. Re-acquire after sleep to absorb any merges.
            let (mut batch, _) =
                match slot
                    .cv
                    .wait_timeout_while(batch, COORDINATOR_BATCH_WINDOW, |b| {
                        !b.flushed && b.writer_count < COORDINATOR_MAX_BATCH
                    }) {
                    Ok(r) => r,
                    Err(_) => return CoordinatorRole::Bypass,
                };

            // Snapshot the merged batch and mark it as in-flight by
            // setting writer_count to the negative sentinel via
            // flushed-but-not-yet-published. We leave `flushed = false`
            // and let `publish()` set it after the apply succeeds; any
            // late joiner that comes in before `publish()` will be a
            // _new_ coordinator on a fresh batch (we cleared above).
            let merged_deltas = std::mem::take(&mut batch.deltas);
            let merged_replacements: Vec<(u16, CombinedReplacementValue)> =
                std::mem::take(&mut batch.replacements)
                    .into_iter()
                    .collect();
            let batched_count = batch.writer_count;
            // Re-arm the slot for the next coordinator. We DO NOT drop
            // the slot from the map here — that happens lazily on the
            // next submit() when no joiners are pending.
            batch.writer_count = 0;
            drop(batch);

            CoordinatorRole::Coordinator(CoordinatorTicket {
                rel_id,
                row_id,
                deltas: merged_deltas,
                replacements: merged_replacements,
                batched_count,
                slot: std::sync::Arc::clone(&slot),
                map_key: key,
            })
        } else {
            // Joiner path: wait for the coordinator to publish.
            let (batch, _) =
                match slot
                    .cv
                    .wait_timeout_while(batch, Duration::from_millis(500), |b| !b.flushed)
                {
                    Ok(r) => r,
                    Err(_) => return CoordinatorRole::Bypass,
                };
            if batch.failed {
                CoordinatorRole::Bypass
            } else {
                CoordinatorRole::Joined
            }
        }
    }

    /// Called by the coordinator after it has applied the merged batch
    /// to the heap and written the audit WAL record. Wakes up every
    /// joiner currently parked on this batch.
    pub(crate) fn publish(&self, ticket: &CoordinatorTicket, ok: bool) {
        if let Ok(mut batch) = ticket.slot.inner.lock() {
            batch.flushed = true;
            batch.failed = !ok;
        }
        ticket.slot.cv.notify_all();
        // Best-effort GC of the slot entry — only remove if no writers
        // are currently parked AND the lock is uncontended. Hot rows
        // will keep the slot warm; cold rows get released here.
        if let Ok(mut map) = self.batches.try_lock()
            && let Some(slot) = map.get(&ticket.map_key)
            && std::sync::Arc::strong_count(slot) == 1
        {
            map.remove(&ticket.map_key);
        }
    }
}

impl Default for HotRowCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Try to lift a Replacement-only [`AssignmentPlan`] vector into a
/// pair of `(deltas, replacements)` the coordinator can ingest. Returns
/// `None` if any plan entry produces a value variant the coordinator's
/// WAL payload can't encode (e.g. Real that loses precision through
/// f64 round-tripping in some weird middle layer — currently no such
/// case, but the gate is structural-only here).
pub(crate) fn lift_plans_for_coordinator(
    plans: &[AssignmentPlan],
) -> Option<(Vec<(u16, i64)>, Vec<(u16, CombinedReplacementValue)>)> {
    let mut deltas = Vec::new();
    let mut replacements = Vec::new();
    for plan in plans {
        match plan {
            AssignmentPlan::IntegerDelta { col, delta } => {
                deltas.push((u16::try_from(*col).ok()?, *delta));
            }
            AssignmentPlan::Replacement { col, value } => {
                let lifted = match value {
                    SqlValue::Null => CombinedReplacementValue::Null,
                    SqlValue::Integer(n) => CombinedReplacementValue::Integer(*n),
                    SqlValue::Real(r) => CombinedReplacementValue::Real(*r),
                    SqlValue::Text(t) => CombinedReplacementValue::Text(t.as_bytes().to_vec()),
                    SqlValue::Blob(b) => CombinedReplacementValue::Blob(b.as_ref().to_vec()),
                };
                replacements.push((u16::try_from(*col).ok()?, lifted));
            }
        }
    }
    Some((deltas, replacements))
}

/// Process-global coordinator instance. Keyed by `(RelId, RowId)` —
/// collisions across separate `Database` handles in the same process
/// only happen when the handles share the same heap (which is exactly
/// when batching is correct). Distinct in-memory databases will only
/// ever collide on the very small reserved-relation space, so the
/// coordinator slot map stays well-bounded.
pub(crate) fn global_coordinator() -> &'static HotRowCoordinator {
    static COORDINATOR: std::sync::OnceLock<HotRowCoordinator> = std::sync::OnceLock::new();
    COORDINATOR.get_or_init(HotRowCoordinator::new)
}
