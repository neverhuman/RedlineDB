// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_268_SCALAR_ARITH_011

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 268,
        folder: r"SQLITE_PARITY_268_SCALAR_ARITH_011",
        name: r"SCALAR_ARITH_011",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_011.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 11+22, 33-11, 11*12, (110)/11, (110)%12;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"33|22|132|10|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
