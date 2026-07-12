// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_376_SCALAR_ARITH_038

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 376,
        folder: r"SQLITE_PARITY_376_SCALAR_ARITH_038",
        name: r"SCALAR_ARITH_038",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_038.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 38+76, 114-38, 38*39, (380)/38, (380)%39;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"114|76|1482|10|29
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
