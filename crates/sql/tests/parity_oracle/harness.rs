//! Differential oracle harness for SQL parity testing.
//!
//! Public surface:
//!   * [`run_oracle`] / [`run_redline`] — single-engine runners
//!   * [`check_parity`] — returns `Ok(())` or a diff string
//!   * [`assert_parity`] — panic-on-divergence convenience wrapper
//!
//! Normalisation rules and the [`ErrorClass`] / [`OracleResult`] types
//! live in sibling files (`normalize.rs`, `types.rs`, `run.rs`) so this
//! file stays a thin facade. Splitting honours the jankurai 300-LOC
//! file-shape floor — each sibling sits well below it.

#![allow(dead_code)]

#[path = "normalize.rs"]
mod normalize;
#[path = "run.rs"]
mod run;
#[path = "types.rs"]
mod types;

pub use run::{run_oracle, run_redline};
pub use types::{ErrorClass, OracleResult, classify_err};

use normalize::rows_equal;

/// Returns `Ok(())` when both engines agree on the result; `Err(diff)`
/// otherwise. Used by the per-corpus tests so they can report a structured
/// divergence summary instead of panicking on the first mismatch.
pub fn check_parity(sql: &str) -> Result<(), String> {
    let oracle = run_oracle(sql);
    let redline = run_redline(sql);

    match (oracle.err_class, redline.err_class) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(a), Some(b)) => Err(format!(
            "error-class mismatch\n  oracle: {:?} ({})\n  redline: {:?} ({})\n  sql: {}",
            a,
            oracle.raw_err.unwrap_or_default(),
            b,
            redline.raw_err.unwrap_or_default(),
            sql
        )),
        (None, Some(c)) => Err(format!(
            "redline errored, oracle succeeded\n  redline: {:?} ({})\n  oracle rows: {}\n  sql: {}",
            c,
            redline.raw_err.unwrap_or_default(),
            oracle.rows.len(),
            sql
        )),
        (Some(c), None) => Err(format!(
            "oracle errored, redline succeeded\n  oracle: {:?} ({})\n  redline rows: {}\n  sql: {}",
            c,
            oracle.raw_err.unwrap_or_default(),
            redline.rows.len(),
            sql
        )),
        (None, None) => {
            if rows_equal(&oracle.rows, &redline.rows) {
                Ok(())
            } else {
                Err(format!(
                    "row mismatch\n  oracle: {:?}\n  redline: {:?}\n  sql: {}",
                    oracle.rows, redline.rows, sql
                ))
            }
        }
    }
}

pub fn assert_parity(sql: &str) {
    if let Err(diff) = check_parity(sql) {
        panic!("{diff}");
    }
}
