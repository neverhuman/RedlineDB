// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_344_SCALAR_ARITH_030

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 344,
        folder: r"SQLITE_PARITY_344_SCALAR_ARITH_030",
        name: r"SCALAR_ARITH_030",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_030.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 30+60, 90-30, 30*31, (300)/30, (300)%31;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"90|60|930|10|21
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
