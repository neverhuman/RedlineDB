// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 233,
        folder: r"SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002",
        name: r"SCALAR_CAST_TYPEOF_002",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_002.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(2), typeof(2.5), typeof('2'), CAST('2' AS INTEGER)+1, CAST(2 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|3|2x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
