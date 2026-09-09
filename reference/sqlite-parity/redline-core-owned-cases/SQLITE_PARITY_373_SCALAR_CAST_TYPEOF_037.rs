// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 373,
        folder: r"SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037",
        name: r"SCALAR_CAST_TYPEOF_037",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_037.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(37), typeof(37.5), typeof('37'), CAST('37' AS INTEGER)+1, CAST(37 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|38|37x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
