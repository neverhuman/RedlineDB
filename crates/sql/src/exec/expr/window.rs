//! Window function execution stubs.
//!
//! Lane SQL-D phase 10 Tier-3: window functions parse but no runtime
//! pipeline yet. The dispatcher in `super::json_dispatch::eval_function`
//! delegates here when a function call carries an `OVER (...)` clause so
//! that the planner work can pick it up cleanly later without touching
//! scalar evaluation.

use super::*;

/// Returns `Some(Err(...))` with a structured "not implemented" error when
/// the function call has a window clause. Returns `None` for non-window
/// calls so the scalar dispatcher can continue.
pub(super) fn try_eval_window(func: &sqlparser::ast::Function) -> Option<Result<SqlValue>> {
    if func.over.is_some() {
        Some(Err(Error::UnsupportedSql(format!(
            "window functions are parsed-only; execution not yet implemented: {}",
            func.name
        ))))
    } else {
        None
    }
}
