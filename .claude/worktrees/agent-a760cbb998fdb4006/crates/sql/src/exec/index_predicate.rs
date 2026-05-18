//! Phase-11 SQL-D A6: SQL-side helpers for partial / expression indexes
//! and generated columns.
//!
//! The kernel stores the SQL fragment of a partial-index `WHERE`, an
//! expression-index key, or a generated-column expression as verbatim
//! `sqlparser` text. At evaluation time we parse the fragment back into
//! an `Expr`, build a synthetic `TableRow`, and dispatch through the
//! existing SQL `eval_scalar` machinery. The wrapper here keeps every
//! call site uniform and gives us a single place to add a memoising
//! cache later if profiling proves the parse cost matters.
//!
//! The parser side uses `sqlparser::dialect::SQLiteDialect` to stay in
//! sync with the surface accepted by `crate::parser::bind_query`.
//!
//! Helpers here are scaffolding for future planner / maintenance wiring;
//! the partial- and expression-index parity tests
//! (`crates/sql/tests/parity_partial_index.rs`,
//! `crates/sql/tests/parity_expr_index.rs`) currently exercise the
//! enforcement via the existing index maintenance layer rather than
//! these helpers directly. `#![allow(dead_code)]` keeps the scaffolding
//! warning-free while the wiring lands.

#![allow(dead_code)]

use std::sync::Arc;

use redlinedb_kernel::catalog::TableDef;
use redlinedb_kernel::format::RowId;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use crate::error::{Error, Result};
use crate::exec::expr::scalar::TableRow;
use crate::exec::expr::{RowContext, eval_scalar};
use crate::value::{SqlValue, is_truthy};

/// Parse `text` as a SQL expression using the SQLite dialect. Returns
/// `Error::UnsupportedSql` when the fragment is not a syntactically
/// valid expression — partial-index predicates / expression-index keys
/// / generated-column expressions reach this code only after the parser
/// has already vetted the fragment, so an error here is a corruption
/// signal or a fragment that the executor cannot understand yet.
pub(crate) fn parse_expr_fragment(text: &str) -> Result<sqlparser::ast::Expr> {
    let dialect = SQLiteDialect {};
    let mut parser = Parser::new(&dialect)
        .try_with_sql(text)
        .map_err(|e| Error::UnsupportedSql(format!("invalid index/generated SQL: {e}")))?;
    let expr = parser
        .parse_expr()
        .map_err(|e| Error::UnsupportedSql(format!("invalid index/generated SQL: {e}")))?;
    Ok(expr)
}

fn synthetic_row(table: &TableDef, values: &[SqlValue]) -> TableRow {
    TableRow {
        rowid: RowId::new(0),
        values: values.to_vec(),
        table: Arc::new(table.clone()),
        alias: None,
    }
}

/// Evaluate a partial-index `WHERE` fragment against a row. Returns
/// `Ok(true)` when the predicate is truthy (i.e. SQLite would install a
/// key for this row); returns `Ok(false)` for `NULL` or `0`.
pub(crate) fn eval_index_predicate(
    table: &TableDef,
    predicate_sql: &str,
    values: &[SqlValue],
) -> Result<bool> {
    let expr = parse_expr_fragment(predicate_sql)?;
    let row = synthetic_row(table, values);
    let ctx = RowContext::Table(&row);
    let value = eval_scalar(&expr, &ctx, &[])?;
    Ok(!matches!(value, SqlValue::Null) && is_truthy(&value))
}

/// Evaluate an expression-source index key against a row, returning the
/// resulting `SqlValue` to feed into `encode_index_key`.
pub(crate) fn eval_index_value_expr(
    table: &TableDef,
    expr_sql: &str,
    values: &[SqlValue],
) -> Result<SqlValue> {
    let expr = parse_expr_fragment(expr_sql)?;
    let row = synthetic_row(table, values);
    let ctx = RowContext::Table(&row);
    eval_scalar(&expr, &ctx, &[])
}

/// Evaluate a generated-column expression and return the computed value.
/// Used by both STORED (write-time) and VIRTUAL (read-time) variants.
pub(crate) fn eval_generated_expr(
    table: &TableDef,
    expr_sql: &str,
    values: &[SqlValue],
) -> Result<SqlValue> {
    eval_index_value_expr(table, expr_sql, values)
}

/// Enumerate the column ordinals referenced inside a SQL expression
/// fragment. Used by `index_referenced_columns` so the UPDATE path can
/// detect "did any input touch a key column or predicate column".
pub(crate) fn expr_input_columns(table: &TableDef, expr_sql: &str) -> Result<Vec<u16>> {
    let expr = parse_expr_fragment(expr_sql)?;
    let mut out: Vec<u16> = Vec::new();
    collect_columns(&expr, table, &mut out);
    Ok(out)
}

fn collect_columns(expr: &sqlparser::ast::Expr, table: &TableDef, out: &mut Vec<u16>) {
    use sqlparser::ast::Expr;
    match expr {
        Expr::Identifier(ident) => {
            if let Some(ordinal) = column_ordinal(table, &ident.value) {
                if !out.contains(&ordinal) {
                    out.push(ordinal);
                }
            }
        }
        Expr::CompoundIdentifier(parts) => {
            if let Some(last) = parts.last()
                && let Some(ordinal) = column_ordinal(table, &last.value)
                && !out.contains(&ordinal)
            {
                out.push(ordinal);
            }
        }
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::IsNull(inner)
        | Expr::IsNotNull(inner) | Expr::IsTrue(inner) | Expr::IsNotTrue(inner)
        | Expr::IsFalse(inner) | Expr::IsNotFalse(inner) | Expr::IsUnknown(inner)
        | Expr::IsNotUnknown(inner) | Expr::Cast { expr: inner, .. }
        | Expr::Collate { expr: inner, .. } => {
            collect_columns(inner, table, out);
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_columns(left, table, out);
            collect_columns(right, table, out);
        }
        Expr::Like { expr: e, pattern, .. } | Expr::ILike { expr: e, pattern, .. } => {
            collect_columns(e, table, out);
            collect_columns(pattern, table, out);
        }
        Expr::Between { expr: e, low, high, .. } => {
            collect_columns(e, table, out);
            collect_columns(low, table, out);
            collect_columns(high, table, out);
        }
        Expr::InList { expr: e, list, .. } => {
            collect_columns(e, table, out);
            for item in list {
                collect_columns(item, table, out);
            }
        }
        Expr::Function(func) => {
            // Function arguments via sqlparser's FunctionArguments tree.
            let args = match &func.args {
                sqlparser::ast::FunctionArguments::List(list) => &list.args,
                _ => return,
            };
            for arg in args {
                if let sqlparser::ast::FunctionArg::Unnamed(
                    sqlparser::ast::FunctionArgExpr::Expr(inner),
                ) = arg
                {
                    collect_columns(inner, table, out);
                }
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(op) = operand.as_deref() {
                collect_columns(op, table, out);
            }
            for when in conditions {
                collect_columns(&when.condition, table, out);
                collect_columns(&when.result, table, out);
            }
            if let Some(else_e) = else_result.as_deref() {
                collect_columns(else_e, table, out);
            }
        }
        _ => {}
    }
}

fn column_ordinal(table: &TableDef, name: &str) -> Option<u16> {
    table
        .columns
        .iter()
        .position(|c| c.folded.as_ref().eq_ignore_ascii_case(name))
        .map(|idx| idx as u16)
}

/// Normalise two predicate SQL fragments for equality comparison. The
/// planner uses this to decide whether a query's WHERE provably implies
/// a partial index's WHERE — we accept identical-after-normalisation
/// fragments today; richer implication is a follow-up. The normalisation
/// is whitespace-collapsing + ASCII case folding outside string literals,
/// which is sufficient for `WHERE a > 0`, `where  A > 0`, etc.
pub(crate) fn normalise_predicate(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut last_was_space = false;
    for ch in text.chars() {
        if ch == '\'' {
            in_string = !in_string;
            out.push(ch);
            last_was_space = false;
            continue;
        }
        if in_string {
            out.push(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
            continue;
        }
        out.push(ch.to_ascii_lowercase());
        last_was_space = false;
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}
