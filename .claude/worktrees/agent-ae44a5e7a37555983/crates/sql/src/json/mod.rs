//! SQLite JSON1 compatibility surface (Lane J1).
//!
//! Submodules:
//! - [`path`] parses and evaluates SQLite JSON paths (`$.foo[0].bar`).
//! - [`scalar`] implements the scalar JSON functions exposed in
//!   `eval_function`.
//!
//! Aggregates (`json_group_array`, `json_group_object`) and table-valued
//! functions (`json_each`, `json_tree`) require parser/planner changes
//! outside Lane J1's allowed surface and are deferred. See
//! `crates/sql/tests/phase10_j1_compat.rs` for the conformance suite.

pub mod path;
pub mod scalar;
