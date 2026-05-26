//! Scalar-expression building blocks.
//!
//! This module groups the bulk of the leaf evaluation logic that the
//! `Expr` dispatcher in `mod.rs` calls into. The implementation is split
//! across focused sibling files:
//!
//!   * `math`    — arithmetic / numeric helpers (`arithmetic`, `negate`,
//!     `parse_number`, `numeric_value`, `round_function`, `hex_value`,
//!     `hex_string_to_bytes`, `quote_value`, `random_i64`)
//!   * `pattern` — string-pattern matching (`like_*`, `glob_*`,
//!     `match_glob_class`)
//!   * `value`   — vector / date-time helpers, `value_to_string`, and
//!     `resolve_binding` (used everywhere)
//!   * `row`     — row-context plumbing (`SqlRow`, `RowContext`,
//!     `TableRow`, `JoinedRow`, `TableRowSource`, lookup / encode /
//!     decode helpers, `row_width`, `compare_row_ordering`,
//!     `scalar_to_usize`)
//!
//! Helpers used only inside `expr/` keep `pub(super)` visibility; ones
//! reachable from other parts of the SQL crate keep `pub(crate)`. The
//! parent `expr/mod.rs` glob-re-exports `scalar::*`, so this `mod.rs`
//! re-exports each child module's public surface to preserve the
//! pre-split call sites unchanged.

use super::*;

pub(crate) mod math;
pub(crate) mod pattern;
pub(crate) mod row;
pub(crate) mod value;
// Phase 6 R2-A: opt-in ScalarProgram VM hot-path dispatch.
pub(crate) mod vm_dispatch;

pub(crate) use math::*;
pub(crate) use pattern::*;
pub(crate) use row::*;
pub(crate) use value::*;
pub(crate) use vm_dispatch::try_eval_scalar_via_vm;

#[cfg(test)]
mod tests;
