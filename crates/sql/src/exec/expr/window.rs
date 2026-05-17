//! Window function entry point used by the scalar expression dispatcher.
//!
//! `OVER (...)` calls are not evaluated through the scalar evaluator —
//! they are precomputed by `crate::exec::window` against the materialized
//! row set and substituted in during projection. The scalar adapter here
//! returns NULL so that intermediate row evaluation (e.g. selection
//! passes that incidentally call into eval_function on a windowed expr)
//! does not panic.

use super::*;

pub(super) fn try_eval_window(func: &sqlparser::ast::Function) -> Option<Result<SqlValue>> {
    if func.over.is_some() {
        // The real value is computed by the window pipeline. Return NULL
        // as a safe sentinel so callers that route through eval_scalar
        // do not error mid-flight.
        Some(Ok(SqlValue::Null))
    } else {
        None
    }
}
