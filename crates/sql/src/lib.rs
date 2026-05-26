mod batch;
mod collation;
mod connection;
mod datetime;
mod error;
mod exec;
mod json;
mod parser;
mod planner;
mod regexp;
mod session;
mod statement;
pub mod udf;
pub mod value;

pub use connection::{
    Connection, Database, DbOptions, OptimizerConfig, QueryMemoryConfig, StatsConfig,
};
pub use error::{Error, Result};
pub use parser::{first_statement_complete, is_blank_sql, split_first_statement, split_statements};
pub use redlinedb_kernel::engine::{Engine, RecoveryTarget};
pub use session::BeginMode;
pub use statement::{
    AnalyzePlan, ExplainFormat, ExplainPlan, PreparedTemplate, SelectPlan, SelectSource, Statement,
    Step,
};
pub use value::{SqlValue, SqlValueRef};

/// WS-C3 R2/R3 test surface. Exposes the parallel covering-scan
/// gate's most recent decision, the worker-context invariant helper,
/// and the per-thread Rayon pool installer so
/// `crates/sql/tests/ws_c3_parallel_scan_safety.rs` and
/// `crates/sql/tests/ws_c3_parallel_scan_dispatch.rs` can assert per-
/// condition branching without reaching into private modules. Not
/// stable — callers outside the test suite should treat the surface
/// as internal.
#[doc(hidden)]
pub mod ws_c3_testing {
    pub use crate::exec::select_top::{
        ParallelCoveringDecision, take_last_parallel_covering_decision,
    };
    pub use crate::exec::{
        WorkerSnapshotCarrier, current_rayon_pool, outer_row_stack_is_empty,
        with_current_rayon_pool, with_executor_context_on_worker,
    };
}

// Render an `f64` the way SQLite's `printf("%g")` does (17 sig digits with
// trailing-zero trim and `.0` suffix for whole values). The CLI and FFI
// layers call this so REAL column output matches SQLite's reference shell —
// required for sqlite-parity on math-function results like `acos(0.5)`.
pub fn format_real_sqlite(v: f64) -> String {
    exec::expr::scalar::value::format_real_sqlite(v)
}
