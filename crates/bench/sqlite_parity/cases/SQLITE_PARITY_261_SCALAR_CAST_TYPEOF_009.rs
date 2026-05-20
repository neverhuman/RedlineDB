// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 261,
        folder: r"SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009",
        name: r"SCALAR_CAST_TYPEOF_009",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_009.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(9), typeof(9.5), typeof('9'), CAST('9' AS INTEGER)+1, CAST(9 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|10|9x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
