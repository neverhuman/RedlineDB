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
                IndexKeySource::Expression { referenced_cols, .. } => {
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
                    _ => return Err(crate::error::Error::UnsupportedSql(
                        "hot_row delta on non-numeric value".to_string(),
                    )),
                };
                values[*col] = new_val;
            }
        }
    }
    Ok(())
}
