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

// Render an `f64` the way SQLite's `printf("%g")` does (17 sig digits with
// trailing-zero trim and `.0` suffix for whole values). The CLI and FFI
// layers call this so REAL column output matches SQLite's reference shell —
// required for sqlite-parity on math-function results like `acos(0.5)`.
pub fn format_real_sqlite(v: f64) -> String {
    exec::expr::scalar::value::format_real_sqlite(v)
}
