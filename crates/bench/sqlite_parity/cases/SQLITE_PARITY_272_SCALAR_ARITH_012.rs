// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_272_SCALAR_ARITH_012

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 272,
        folder: r"SQLITE_PARITY_272_SCALAR_ARITH_012",
        name: r"SCALAR_ARITH_012",
        category: r"GEN_SQL_SCALAR",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for SCALAR_ARITH_012.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 12+24, 36-12, 12*13, (120)/12, (120)%13;",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"36|24|156|10|3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
