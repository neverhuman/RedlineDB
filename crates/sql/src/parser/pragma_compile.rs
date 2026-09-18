use std::sync::Arc;

use crate::value::SqlValue;

/// Static list of RedlineDB compile-time-enabled feature flags exposed
/// through `PRAGMA compile_options` / `pragma_compile_options()`. Order
/// is alphabetical to keep the surface diffable.
pub(crate) fn pragma_compile_options_rows() -> Vec<Vec<SqlValue>> {
    // Honest flags only. Do not advertise FTS/RTree/session/vtab modules
    // or SQLite pager tunables this engine does not implement. ORMs probe
    // this list; lying here is a portability bug.
    const OPTIONS: &[&str] = &[
        "COMPILER=rustc",
        "DEFAULT_PAGE_SIZE=16384",
        "DEFAULT_RECURSIVE_TRIGGERS",
        "ENABLE_JSON1",
        "ENABLE_MATH_FUNCTIONS",
        "MAX_TRIGGER_DEPTH=8",
        "REDLINEDB=1",
        "THREADSAFE=1",
    ];
    OPTIONS
        .iter()
        .map(|opt| vec![SqlValue::Text(Arc::from(*opt))])
        .collect()
}
