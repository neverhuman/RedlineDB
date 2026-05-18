//! Window function execution entry points (re-exports from `expr::window_eval`).
//!
//! The bulk of window-function logic lives in `expr::window_eval` so it
//! can share the per-row scalar context and the row-source helpers. This
//! file re-exports the surface used by the SELECT builder.

pub(crate) use super::expr::window_eval::{evaluate_window_functions, projection_has_window};
