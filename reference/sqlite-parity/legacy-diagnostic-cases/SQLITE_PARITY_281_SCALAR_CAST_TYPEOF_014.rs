// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 281,
        folder: r"SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014",
        name: r"SCALAR_CAST_TYPEOF_014",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_014.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(14), typeof(14.5), typeof('14'), CAST('14' AS INTEGER)+1, CAST(14 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|15|14x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
