//! Table-valued function dispatch.
//!
//! Table-valued functions (TVFs) materialise into a fixed column list plus
//! a row vector. The result is shaped exactly like a CTE so the rest of the
//! executor can re-use [`crate::statement::SelectSource::Cte`] without
//! changes — column names round-trip through the projection pipeline and
//! `ORDER BY` / `WHERE` / `LIMIT` work for free.
//!
//! Today the only TVFs that ship are the table-valued PRAGMA forms
//! (`pragma_table_info`, `pragma_index_list`, `pragma_index_info`,
//! `pragma_foreign_key_list`, `pragma_database_list`). The trait is kept
//! intentionally small so workstream A8 can land additional functions
//! (`json_each`, `json_tree`, vector search, etc.) without touching the
//! parser-side dispatcher.

use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use sqlparser::ast::{FunctionArg, FunctionArgExpr, TableFunctionArgs};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::value::SqlValue;

/// A materialised TVF result. Column names are exposed via the wrapping
/// `SelectSource::Cte` so they appear in `SELECT *` output and resolve in
/// `ORDER BY` / `WHERE` predicates.
pub(crate) struct TvResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
}

/// Anything callable as `name(arg1, arg2, ...)` in a `FROM` list.
pub(crate) trait TvFunc: Send + Sync {
    fn name(&self) -> &'static str;
    fn eval(
        &self,
        conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult>;
}

/// Normalised single argument value.
#[derive(Debug, Clone)]
pub(crate) enum TvArg {
    Text(Arc<str>),
    Integer(i64),
    Null,
}

impl TvArg {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            TvArg::Text(v) => Some(v.as_ref()),
            _ => None,
        }
    }
}

/// Look up a TVF implementation by name, returning `None` when the name
/// isn't a registered TVF.
pub(crate) fn lookup(name: &str) -> Option<&'static dyn TvFunc> {
    let lower = name.to_ascii_lowercase();
    super::pragma_tv::registry()
        .iter()
        .find(|f| f.name() == lower.as_str())
        .map(|f| *f)
}

/// Parse `TableFunctionArgs` into the simple `TvArg` vector the trait
/// expects. Bind parameters / qualified identifiers are not currently
/// allowed inside TVF arguments because the resolution surface for them
/// would have to thread through param bindings the parser doesn't have
/// at this stage.
pub(crate) fn lower_args(args: &TableFunctionArgs) -> Result<Vec<TvArg>> {
    use sqlparser::ast::{Expr, Value};
    let mut out = Vec::with_capacity(args.args.len());
    for arg in &args.args {
        let expr = match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => expr,
            FunctionArg::ExprNamed { arg: FunctionArgExpr::Expr(expr), .. } => expr,
            _ => {
                return Err(Error::UnsupportedSql(
                    "table-valued functions accept only positional value arguments".to_owned(),
                ));
            }
        };
        out.push(match expr {
            Expr::Value(value) => match &value.value {
                Value::SingleQuotedString(s)
                | Value::DoubleQuotedString(s)
                | Value::EscapedStringLiteral(s)
                | Value::TripleSingleQuotedString(s)
                | Value::TripleDoubleQuotedString(s)
                | Value::UnicodeStringLiteral(s) => TvArg::Text(Arc::from(s.as_str())),
                Value::Number(n, _) => match n.parse::<i64>() {
                    Ok(v) => TvArg::Integer(v),
                    Err(_) => {
                        return Err(Error::UnsupportedSql(format!(
                            "table-valued function expected integer literal, got: {n}"
                        )));
                    }
                },
                Value::Null => TvArg::Null,
                other => {
                    return Err(Error::UnsupportedSql(format!(
                        "unsupported table-valued function argument: {other:?}"
                    )));
                }
            },
            // Allow `pragma_table_info(t)` where `t` is an unquoted
            // identifier. SQLite treats this as the table name string.
            Expr::Identifier(ident) => TvArg::Text(Arc::from(ident.value.as_str())),
            Expr::CompoundIdentifier(parts) => {
                let joined = parts
                    .iter()
                    .map(|p| p.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                TvArg::Text(Arc::from(joined.as_str()))
            }
            other => {
                return Err(Error::UnsupportedSql(format!(
                    "unsupported table-valued function argument: {other:?}"
                )));
            }
        });
    }
    Ok(out)
}
