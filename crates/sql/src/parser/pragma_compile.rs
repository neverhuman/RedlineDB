use std::sync::Arc;

use crate::value::SqlValue;

/// Static list of RedlineDB compile-time-enabled feature flags exposed
/// through `PRAGMA compile_options` / `pragma_compile_options()`. Order
/// is alphabetical to keep the surface diffable.
pub(crate) fn pragma_compile_options_rows() -> Vec<Vec<SqlValue>> {
    // Mirrors the SQLite v3.53.1 `PRAGMA compile_options` output shape so
    // tooling that probes feature flags sees the same set of names. The
    // RedlineDB engine implements only a subset of these as native code
    // paths; the list is a parity surface for upstream-compatible probing.
    const OPTIONS: &[&str] = &[
        "ATOMIC_INTRINSICS=1",
        "COMPILER=gcc-13.3.0",
        "DEFAULT_AUTOVACUUM",
        "DEFAULT_CACHE_SIZE=-2000",
        "DEFAULT_FILE_FORMAT=4",
        "DEFAULT_JOURNAL_SIZE_LIMIT=-1",
        "DEFAULT_MMAP_SIZE=0",
        "DEFAULT_PAGE_SIZE=4096",
        "DEFAULT_PCACHE_INITSZ=20",
        "DEFAULT_RECURSIVE_TRIGGERS",
        "DEFAULT_SECTOR_SIZE=4096",
        "DEFAULT_SYNCHRONOUS=2",
        "DEFAULT_WAL_AUTOCHECKPOINT=1000",
        "DEFAULT_WAL_SYNCHRONOUS=2",
        "DEFAULT_WORKER_THREADS=0",
        "DIRECT_OVERFLOW_READ",
        "DQS=0",
        "ENABLE_BYTECODE_VTAB",
        "ENABLE_COLUMN_METADATA",
        "ENABLE_DBPAGE_VTAB",
        "ENABLE_DBSTAT_VTAB",
        "ENABLE_EXPLAIN_COMMENTS",
        "ENABLE_FTS3",
        "ENABLE_FTS4",
        "ENABLE_FTS5",
        "ENABLE_MATH_FUNCTIONS",
        "ENABLE_OFFSET_SQL_FUNC",
        "ENABLE_PERCENTILE",
        "ENABLE_PREUPDATE_HOOK",
        "ENABLE_RTREE",
        "ENABLE_SESSION",
        "ENABLE_STMTVTAB",
        "ENABLE_STMT_SCANSTATUS",
        "ENABLE_UNKNOWN_SQL_FUNCTION",
        "ENABLE_UPDATE_DELETE_LIMIT",
        "ENABLE_VFSTRACE",
        "MALLOC_SOFT_LIMIT=1024",
        "MAX_ATTACHED=10",
        "MAX_COLUMN=2000",
        "MAX_COMPOUND_SELECT=500",
        "MAX_DEFAULT_PAGE_SIZE=8192",
        "MAX_EXPR_DEPTH=1000",
        "MAX_FUNCTION_ARG=1000",
        "MAX_LENGTH=1000000000",
        "MAX_LIKE_PATTERN_LENGTH=50000",
        "MAX_MMAP_SIZE=0x7fff0000",
        "MAX_PAGE_COUNT=0xfffffffe",
        "MAX_PAGE_SIZE=65536",
        "MAX_SQL_LENGTH=1000000000",
        "MAX_TRIGGER_DEPTH=1000",
        "MAX_VARIABLE_NUMBER=32766",
        "MAX_VDBE_OP=250000000",
        "MAX_WORKER_THREADS=8",
        "MUTEX_PTHREADS",
        "STRICT_SUBTYPE",
        "SYSTEM_MALLOC",
        "TEMP_STORE=1",
        "THREADSAFE=1",
    ];
    OPTIONS
        .iter()
        .map(|opt| vec![SqlValue::Text(Arc::from(*opt))])
        .collect()
}
