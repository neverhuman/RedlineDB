// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 357,
        folder: r"SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033",
        name: r"SCALAR_CAST_TYPEOF_033",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_033.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(33), typeof(33.5), typeof('33'), CAST('33' AS INTEGER)+1, CAST(33 AS TEXT)||'x';",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|real|text|34|33x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
