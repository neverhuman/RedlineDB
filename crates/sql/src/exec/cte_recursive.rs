//! Recursive CTE evaluation: body analysis, anchor/recursive arm splitting,
//! iterative working-set fixpoint, and deduplication.
//!
//! WS-A7b: the per-iteration `working_set` clone is replaced by a
//! `Range<usize>` "frontier" into the single `accumulated` vector, and the
//! linear `row_in` UNION dedup is replaced by an `AHashSet` keyed on the
//! encoded row bytes. The encoded bytes themselves live in a `bumpalo::Bump`
//! arena so set keys are zero-copy `&'arena [u8]` slices.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use sqlparser::ast::{
    Cte, Expr, Query, SetExpr, SetOperator, SetQuantifier, TableFactor, TableWithJoins,
};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::value::SqlValue;
use redlinedb_kernel::catalog::{SchemaEpoch, SchemaSnapshot, ValueRef, encode_record};

use super::registry::{deregister_rows, register_cte_rows};
use super::{
    CteDef, pop_scope, push_scope, run_query_to_rows, synth_table_def, synth_table_def_with_folded,
};

/// Maximum recursive-CTE iterations before bailing out.
pub(super) const RECURSIVE_CTE_ITERATION_LIMIT: usize = 10_000;

/// Materialize one CTE (anchor + optional recursive arm) into rows.
///
/// `row_cap` is an optional upper bound on accumulated rows. When the
/// outer query is a bounded `SELECT ... FROM <cte> LIMIT K [OFFSET M]`
/// with no filter/join/order/aggregate, the caller passes `Some(K+M)`
/// so recursion stops once that many rows are produced. See
/// `derive_cte_row_cap` in `cte.rs`.
pub(super) fn materialize_cte(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    cte: &Cte,
    parent_recursive: bool,
    row_cap: Option<usize>,
) -> Result<CteDef> {
    let cte_name: Arc<str> = Arc::from(cte.alias.name.value.as_str());
    let declared_columns: Vec<String> = cte
        .alias
        .columns
        .iter()
        .map(|c| c.name.value.clone())
        .collect();

    let body_query = cte.query.as_ref().clone();
    let self_referenced = body_uses_name(&body_query, &cte_name);

    if !self_referenced {
        let (rows, columns) = run_query_to_rows(
            conn,
            Arc::clone(&schema),
            schema_epoch,
            sql,
            body_query,
            &declared_columns,
        )?;
        let table_def = synth_table_def(&cte_name, &columns, &rows);
        let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(rows);
        register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
        let row_slice: Arc<[Vec<SqlValue>]> = Arc::from(rows_arc.as_slice().to_vec());
        return Ok(CteDef {
            name: cte_name,
            columns: Arc::from(columns),
            rows: row_slice,
            table_def: Some(table_def),
        });
    }

    if !parent_recursive {
        return Err(Error::UnsupportedSql(format!(
            "CTE `{cte_name}` references itself but WITH was not declared RECURSIVE"
        )));
    }

    let (anchor_branch, recursive_branch, union_all) =
        split_recursive_body(&body_query, &cte_name)?;

    let (anchor_rows, columns) = run_query_to_rows(
        conn,
        Arc::clone(&schema),
        schema_epoch,
        sql,
        anchor_branch,
        &declared_columns,
    )?;
    let columns_arc: Arc<[String]> = Arc::from(columns);
    let column_vec: Vec<String> = columns_arc.iter().cloned().collect();

    // Phase 4.4: pre-compute the lowercased name + columns ONCE
    // before the recursive loop. The recursive iterations all share
    // the same cte_name and column_vec; only the rows change. The
    // original synth_table_def re-lowercased on every call.
    let folded_cte_name = cte_name.to_ascii_lowercase();
    let folded_columns: Vec<String> = column_vec
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();

    // WS-A7b: arena holds encoded-row-byte dedup keys for the lifetime
    // of this materialization. The set borrows directly into the arena
    // so a hit costs one hash + memcmp and no allocation.
    let arena = bumpalo::Bump::new();
    let mut dedup: ahash::AHashSet<&[u8]> = ahash::AHashSet::new();
    let mut encode_buf: Vec<u8> = Vec::new();

    // Single owned vector for the full result; frontier is a Range<usize>
    // into it. Replaces the prior per-iteration `working_set.clone()`.
    let mut accumulated: Vec<Vec<SqlValue>> = Vec::with_capacity(anchor_rows.len());
    for row in anchor_rows {
        if union_all {
            accumulated.push(row);
        } else {
            let key = encode_row_into(&row, &mut encode_buf);
            let slot: &[u8] = arena.alloc_slice_copy(key);
            if dedup.insert(slot) {
                accumulated.push(row);
            }
        }
    }

    // WS-A7: if the outer query has a bounded LIMIT we can stop the
    // recursion as soon as enough rows have accumulated. SQLite calls
    // this LIMIT pushdown into the recursive worktable.
    if let Some(cap) = row_cap {
        if accumulated.len() >= cap {
            accumulated.truncate(cap);
            return finish_cte(
                cte_name,
                columns_arc,
                &column_vec,
                &folded_cte_name,
                &folded_columns,
                accumulated,
            );
        }
    }

    // Frontier of "rows discovered in the previous iteration" — what the
    // recursive arm sees as the working table. Initial frontier is the
    // (deduplicated) anchor result.
    let mut frontier: Range<usize> = 0..accumulated.len();

    for iter in 0..RECURSIVE_CTE_ITERATION_LIMIT {
        if frontier.is_empty() {
            return finish_cte(
                cte_name,
                columns_arc,
                &column_vec,
                &folded_cte_name,
                &folded_columns,
                accumulated,
            );
        }

        // Materialize the frontier slice into the registry. We still
        // need an owned Vec here because the registry stores
        // `Arc<Vec<Vec<SqlValue>>>` and we can't keep a borrow live
        // across the recursive run_query call. This is bounded by
        // |frontier|, not |accumulated| — the prior code cloned both.
        let frontier_rows: Vec<Vec<SqlValue>> = accumulated[frontier.clone()].to_vec();
        let working_table = synth_table_def_with_folded(
            &cte_name,
            &column_vec,
            &folded_cte_name,
            &folded_columns,
            &frontier_rows,
        );
        let working_rows: Arc<Vec<Vec<SqlValue>>> = Arc::new(frontier_rows);
        register_cte_rows(working_table.relation_id, Arc::clone(&working_rows));
        let mut scope = HashMap::new();
        scope.insert(
            cte_name.to_string(),
            CteDef {
                name: Arc::clone(&cte_name),
                columns: Arc::clone(&columns_arc),
                rows: Arc::from(working_rows.as_slice().to_vec()),
                table_def: Some(Arc::clone(&working_table)),
            },
        );
        push_scope(scope);

        let recursive_result = run_query_to_rows(
            conn,
            Arc::clone(&schema),
            schema_epoch,
            sql,
            recursive_branch.clone(),
            &declared_columns,
        );
        pop_scope();
        deregister_rows(working_table.relation_id);

        let (new_rows, _) = recursive_result?;

        let frontier_start = accumulated.len();
        if union_all {
            accumulated.extend(new_rows);
        } else {
            for row in new_rows {
                let key = encode_row_into(&row, &mut encode_buf);
                // Probe by reference first (no allocation on hit); only
                // copy into the arena on insert.
                if !dedup.contains(key) {
                    let slot: &[u8] = arena.alloc_slice_copy(key);
                    dedup.insert(slot);
                    accumulated.push(row);
                }
            }
        }

        if accumulated.len() == frontier_start {
            return finish_cte(
                cte_name,
                columns_arc,
                &column_vec,
                &folded_cte_name,
                &folded_columns,
                accumulated,
            );
        }

        frontier = frontier_start..accumulated.len();

        // WS-A7: stop once accumulated rows satisfy the outer LIMIT.
        if let Some(cap) = row_cap {
            if accumulated.len() >= cap {
                accumulated.truncate(cap);
                return finish_cte(
                    cte_name,
                    columns_arc,
                    &column_vec,
                    &folded_cte_name,
                    &folded_columns,
                    accumulated,
                );
            }
        }

        if iter + 1 == RECURSIVE_CTE_ITERATION_LIMIT {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` exceeded {RECURSIVE_CTE_ITERATION_LIMIT} iterations"
            )));
        }
    }
    finish_cte(
        cte_name,
        columns_arc,
        &column_vec,
        &folded_cte_name,
        &folded_columns,
        accumulated,
    )
}

/// Encode one row into `encode_buf` and return a slice borrow.
/// `encode_record` clears `encode_buf` itself so the same buffer can
/// be reused across all rows in the loop.
fn encode_row_into<'buf>(row: &[SqlValue], encode_buf: &'buf mut Vec<u8>) -> &'buf [u8] {
    let mut refs = ValueRefStack::new();
    for v in row {
        refs.push(v.as_ref());
    }
    encode_record(refs.as_slice(), encode_buf)
        .expect("encode_record on owned SqlValue cannot fail");
    encode_buf.as_slice()
}

/// Inline-stack helper for `ValueRef` collections during row encoding.
/// Avoids allocating a fresh `Vec` per row for the common case of
/// <= 16 columns; falls back to heap for wider rows.
struct ValueRefStack<'a> {
    inline: [ValueRef<'a>; 16],
    len: usize,
    spill: Option<Vec<ValueRef<'a>>>,
}

impl<'a> ValueRefStack<'a> {
    #[inline]
    fn new() -> Self {
        Self {
            inline: [ValueRef::Null; 16],
            len: 0,
            spill: None,
        }
    }

    #[inline]
    fn push(&mut self, v: ValueRef<'a>) {
        if let Some(spill) = self.spill.as_mut() {
            spill.push(v);
        } else if self.len < self.inline.len() {
            self.inline[self.len] = v;
            self.len += 1;
        } else {
            let mut spill = Vec::with_capacity(self.len * 2);
            spill.extend_from_slice(&self.inline[..self.len]);
            spill.push(v);
            self.spill = Some(spill);
        }
    }

    #[inline]
    fn as_slice(&self) -> &[ValueRef<'a>] {
        if let Some(spill) = self.spill.as_ref() {
            spill.as_slice()
        } else {
            &self.inline[..self.len]
        }
    }
}

fn finish_cte(
    cte_name: Arc<str>,
    columns_arc: Arc<[String]>,
    column_vec: &[String],
    folded_cte_name: &str,
    folded_columns: &[String],
    accumulated: Vec<Vec<SqlValue>>,
) -> Result<CteDef> {
    let table_def = synth_table_def_with_folded(
        &cte_name,
        column_vec,
        folded_cte_name,
        folded_columns,
        &accumulated,
    );
    let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(accumulated.clone());
    register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
    Ok(CteDef {
        name: cte_name,
        columns: columns_arc,
        rows: Arc::from(accumulated),
        table_def: Some(table_def),
    })
}

/// Split a recursive CTE body into (anchor, recursive_arm, is_union_all).
fn split_recursive_body(query: &Query, cte_name: &str) -> Result<(Query, Query, bool)> {
    let body = query.body.as_ref();
    let (op, quantifier, left, right) = match body {
        SetExpr::SetOperation {
            op,
            set_quantifier,
            left,
            right,
        } => (op, set_quantifier, left, right),
        _ => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` body must be a UNION / UNION ALL"
            )));
        }
    };
    if !matches!(op, SetOperator::Union) {
        return Err(Error::UnsupportedSql(format!(
            "recursive CTE `{cte_name}` requires UNION or UNION ALL"
        )));
    }
    let union_all = matches!(quantifier, SetQuantifier::All);
    let left_uses = setexpr_uses_name(left, cte_name);
    let right_uses = setexpr_uses_name(right, cte_name);
    let (anchor, recursive_arm) = match (left_uses, right_uses) {
        (false, true) => (left.as_ref().clone(), right.as_ref().clone()),
        (true, false) => (right.as_ref().clone(), left.as_ref().clone()),
        (false, false) => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` does not reference itself"
            )));
        }
        (true, true) => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` self-references in both UNION arms"
            )));
        }
    };
    Ok((
        wrap_as_query(anchor),
        wrap_as_query(recursive_arm),
        union_all,
    ))
}

fn wrap_as_query(body: SetExpr) -> Query {
    Query {
        with: None,
        body: Box::new(body),
        order_by: None,
        limit_clause: None,
        fetch: None,
        locks: Vec::new(),
        for_clause: None,
        settings: None,
        format_clause: None,
        pipe_operators: Vec::new(),
    }
}

pub(super) fn body_uses_name(query: &Query, name: &str) -> bool {
    setexpr_uses_name(query.body.as_ref(), name)
}

fn setexpr_uses_name(set_expr: &SetExpr, name: &str) -> bool {
    match set_expr {
        SetExpr::Select(select) => {
            from_uses_name(&select.from, name)
                || select
                    .selection
                    .as_ref()
                    .is_some_and(|expr| expr_uses_name(expr, name))
        }
        SetExpr::Query(inner) => setexpr_uses_name(inner.body.as_ref(), name),
        SetExpr::SetOperation { left, right, .. } => {
            setexpr_uses_name(left, name) || setexpr_uses_name(right, name)
        }
        _ => false,
    }
}

fn from_uses_name(from: &[TableWithJoins], name: &str) -> bool {
    from.iter().any(|table| {
        table_factor_uses_name(&table.relation, name)
            || table
                .joins
                .iter()
                .any(|join| table_factor_uses_name(&join.relation, name))
    })
}

fn table_factor_uses_name(factor: &TableFactor, name: &str) -> bool {
    match factor {
        TableFactor::Table {
            name: object_name, ..
        } => object_name
            .0
            .last()
            .and_then(|part| match part {
                sqlparser::ast::ObjectNamePart::Identifier(ident) => Some(&ident.value),
                _ => None,
            })
            .is_some_and(|n| n.eq_ignore_ascii_case(name)),
        TableFactor::Derived { subquery, .. } => setexpr_uses_name(subquery.body.as_ref(), name),
        _ => false,
    }
}

fn expr_uses_name(expr: &Expr, name: &str) -> bool {
    use sqlparser::ast::Expr as E;
    match expr {
        E::Subquery(q) | E::Exists { subquery: q, .. } => setexpr_uses_name(q.body.as_ref(), name),
        E::BinaryOp { left, right, .. } => {
            expr_uses_name(left, name) || expr_uses_name(right, name)
        }
        E::UnaryOp { expr, .. } | E::Nested(expr) | E::IsNull(expr) | E::IsNotNull(expr) => {
            expr_uses_name(expr, name)
        }
        E::Function(func) => match &func.args {
            sqlparser::ast::FunctionArguments::List(list) => {
                list.args.iter().any(|arg| match arg {
                    sqlparser::ast::FunctionArg::Unnamed(
                        sqlparser::ast::FunctionArgExpr::Expr(inner),
                    ) => expr_uses_name(inner, name),
                    _ => false,
                })
            }
            _ => false,
        },
        _ => false,
    }
}
